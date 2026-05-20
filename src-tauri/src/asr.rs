use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use futures_util::{SinkExt, StreamExt};
use log::debug;
use once_cell::sync::Lazy;
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::Message;

use crate::managers::model::{EngineType, ModelManager};
use crate::managers::transcription::TranscriptionManager;
use crate::settings::{
    get_settings, AsrProviderKind, ALIYUN_QWEN3_ASR_DEFAULT_MODEL, ALIYUN_QWEN3_ASR_PROVIDER_ID,
    ALIYUN_QWEN3_ASR_REALTIME_DEFAULT_MODEL, ALIYUN_QWEN3_ASR_REALTIME_PROVIDER_ID,
    BUILT_IN_LOCAL_ASR_PROVIDER_ID,
};

const ASR_SAMPLE_RATE: usize = 16_000;
const ALIYUN_QWEN3_ASR_MAX_SECONDS: usize = 5 * 60;
const ALIYUN_QWEN3_ASR_MAX_BYTES: usize = 10 * 1024 * 1024;
const DASHSCOPE_API_KEY_ENV: &str = "DASHSCOPE_API_KEY";
const REALTIME_FINISH_TIMEOUT: Duration = Duration::from_secs(25);

pub trait AsrProvider {
    fn provider_id(&self) -> &'static str;
}

pub struct BuiltInLocalProvider;

impl AsrProvider for BuiltInLocalProvider {
    fn provider_id(&self) -> &'static str {
        BUILT_IN_LOCAL_ASR_PROVIDER_ID
    }
}

pub struct AliyunQwen3AsrFlashProvider;

impl AsrProvider for AliyunQwen3AsrFlashProvider {
    fn provider_id(&self) -> &'static str {
        ALIYUN_QWEN3_ASR_PROVIDER_ID
    }
}

pub struct AliyunQwen3AsrRealtimeProvider;

impl AsrProvider for AliyunQwen3AsrRealtimeProvider {
    fn provider_id(&self) -> &'static str {
        ALIYUN_QWEN3_ASR_REALTIME_PROVIDER_ID
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsrTranscriptionResult {
    pub text: String,
    pub provider_id: String,
    pub model: String,
    pub language: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsrProviderMetadata {
    pub provider_id: String,
    pub model: String,
    pub language: String,
}

pub async fn transcribe_with_active_provider(
    app: &AppHandle,
    transcription_manager: &TranscriptionManager,
    samples: Vec<f32>,
) -> Result<AsrTranscriptionResult> {
    let settings = get_settings(app);
    let provider = settings
        .active_asr_provider()
        .ok_or_else(|| anyhow!("Selected ASR provider is not configured"))?;

    match provider.kind {
        AsrProviderKind::BuiltInLocal => {
            let text = transcription_manager.transcribe(samples)?;
            let provider = BuiltInLocalProvider;
            let language = active_local_asr_language(app, &settings);
            Ok(AsrTranscriptionResult {
                text,
                provider_id: provider.provider_id().to_string(),
                model: settings.selected_model,
                language: normalize_metadata_language(&language),
            })
        }
        AsrProviderKind::AliyunQwen3AsrFlash => {
            transcribe_with_aliyun_qwen3_asr_flash(&settings, samples).await
        }
        AsrProviderKind::AliyunQwen3AsrRealtime => Err(anyhow!(
            "Qwen3-ASR realtime requires a live recording stream and cannot transcribe stored audio yet"
        )),
    }
}

async fn transcribe_with_aliyun_qwen3_asr_flash(
    settings: &crate::settings::AppSettings,
    samples: Vec<f32>,
) -> Result<AsrTranscriptionResult> {
    validate_aliyun_audio_limits(&samples)?;

    let provider = settings
        .asr_provider(ALIYUN_QWEN3_ASR_PROVIDER_ID)
        .ok_or_else(|| anyhow!("Aliyun Qwen3-ASR provider is not configured"))?;
    let api_key = resolve_dashscope_api_key(settings)
        .ok_or_else(|| anyhow!("DashScope API key is not configured"))?;
    let model = settings
        .asr_models
        .get(ALIYUN_QWEN3_ASR_PROVIDER_ID)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or(ALIYUN_QWEN3_ASR_DEFAULT_MODEL)
        .to_string();

    let wav_bytes = encode_wav_bytes(&samples)?;
    if wav_bytes.len() > ALIYUN_QWEN3_ASR_MAX_BYTES {
        return Err(anyhow!("Audio is too large for qwen3-asr-flash (max 10MB)"));
    }

    let audio_base64 = STANDARD.encode(&wav_bytes);
    let body = build_dashscope_transcribe_body(
        &model,
        "audio/wav",
        &audio_base64,
        "wav",
        settings.selected_language.as_str(),
    );
    let endpoint = format!(
        "{}/chat/completions",
        provider.base_url.trim_end_matches('/')
    );

    let response = reqwest::Client::new()
        .post(endpoint)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|source| anyhow!("Failed to reach Aliyun Qwen3-ASR: {}", source))?;

    let status = response.status();
    if !status.is_success() {
        return Err(map_dashscope_status(status));
    }

    let payload = response
        .json::<ChatCompletionResponse>()
        .await
        .map_err(|source| anyhow!("Failed to decode Aliyun Qwen3-ASR response: {}", source))?;
    let text = extract_transcript_text(&payload)?;

    Ok(AsrTranscriptionResult {
        text,
        provider_id: AliyunQwen3AsrFlashProvider.provider_id().to_string(),
        model,
        language: normalize_metadata_language(&settings.selected_language),
    })
}

fn resolve_dashscope_api_key(settings: &crate::settings::AppSettings) -> Option<String> {
    resolve_dashscope_api_key_for_provider_from_sources(
        settings,
        ALIYUN_QWEN3_ASR_PROVIDER_ID,
        std::env::var(DASHSCOPE_API_KEY_ENV).ok().as_deref(),
    )
}

#[cfg(test)]
fn resolve_dashscope_api_key_from_sources(
    settings: &crate::settings::AppSettings,
    env_api_key: Option<&str>,
) -> Option<String> {
    resolve_dashscope_api_key_for_provider_from_sources(
        settings,
        ALIYUN_QWEN3_ASR_PROVIDER_ID,
        env_api_key,
    )
}

fn resolve_dashscope_api_key_for_provider_from_sources(
    settings: &crate::settings::AppSettings,
    provider_id: &str,
    env_api_key: Option<&str>,
) -> Option<String> {
    settings
        .asr_api_keys
        .get(provider_id)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            if provider_id == ALIYUN_QWEN3_ASR_REALTIME_PROVIDER_ID {
                settings
                    .asr_api_keys
                    .get(ALIYUN_QWEN3_ASR_PROVIDER_ID)
                    .map(|value| value.trim())
                    .filter(|value| !value.is_empty())
                    .map(ToString::to_string)
            } else {
                None
            }
        })
        .or_else(|| {
            env_api_key
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}

pub fn is_active_asr_provider_local(app: &AppHandle) -> bool {
    get_settings(app)
        .active_asr_provider()
        .map(|provider| provider.kind == AsrProviderKind::BuiltInLocal)
        .unwrap_or(true)
}

pub fn has_configured_cloud_asr(settings: &crate::settings::AppSettings) -> bool {
    matches!(
        settings.active_asr_provider().map(|provider| provider.kind),
        Some(AsrProviderKind::AliyunQwen3AsrFlash | AsrProviderKind::AliyunQwen3AsrRealtime)
    ) && resolve_dashscope_api_key_for_provider_from_sources(
        settings,
        &settings.asr_provider_id,
        std::env::var(DASHSCOPE_API_KEY_ENV).ok().as_deref(),
    )
    .is_some()
}

pub fn is_active_asr_provider_realtime(app: &AppHandle) -> bool {
    get_settings(app)
        .active_asr_provider()
        .map(|provider| provider.kind == AsrProviderKind::AliyunQwen3AsrRealtime)
        .unwrap_or(false)
}

pub fn active_provider_metadata(app: &AppHandle) -> Option<AsrProviderMetadata> {
    let settings = get_settings(app);
    let mut metadata = provider_metadata_from_settings(&settings)?;
    if metadata.provider_id == BUILT_IN_LOCAL_ASR_PROVIDER_ID {
        let language = active_local_asr_language(app, &settings);
        metadata.language = normalize_metadata_language(&language);
    }
    Some(metadata)
}

fn active_local_asr_language(app: &AppHandle, settings: &crate::settings::AppSettings) -> String {
    app.try_state::<Arc<ModelManager>>()
        .and_then(|model_manager| model_manager.get_model_info(&settings.selected_model))
        .map(|model_info| match model_info.engine_type {
            EngineType::Whisper => settings.asr_family_settings.whisper.language.clone(),
            EngineType::Qwen3Asr => settings.asr_family_settings.qwen3_asr.language.clone(),
            _ => settings.selected_language.clone(),
        })
        .unwrap_or_else(|| settings.selected_language.clone())
}

fn provider_metadata_from_settings(
    settings: &crate::settings::AppSettings,
) -> Option<AsrProviderMetadata> {
    let provider = settings.active_asr_provider()?;
    let model = match provider.kind {
        AsrProviderKind::BuiltInLocal => settings.selected_model.clone(),
        AsrProviderKind::AliyunQwen3AsrFlash => settings
            .asr_models
            .get(ALIYUN_QWEN3_ASR_PROVIDER_ID)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .unwrap_or(ALIYUN_QWEN3_ASR_DEFAULT_MODEL)
            .to_string(),
        AsrProviderKind::AliyunQwen3AsrRealtime => settings
            .asr_models
            .get(ALIYUN_QWEN3_ASR_REALTIME_PROVIDER_ID)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .unwrap_or(ALIYUN_QWEN3_ASR_REALTIME_DEFAULT_MODEL)
            .to_string(),
    };

    Some(AsrProviderMetadata {
        provider_id: provider.id.clone(),
        model,
        language: normalize_metadata_language(&settings.selected_language),
    })
}

fn validate_aliyun_audio_limits(samples: &[f32]) -> Result<()> {
    let max_samples = ALIYUN_QWEN3_ASR_MAX_SECONDS * ASR_SAMPLE_RATE;
    if samples.len() > max_samples {
        return Err(anyhow!(
            "Audio is too long for qwen3-asr-flash (max 5 minutes)"
        ));
    }
    Ok(())
}

fn encode_wav_bytes(samples: &[f32]) -> Result<Vec<u8>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: ASR_SAMPLE_RATE as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec)?;

        for sample in samples {
            let clamped = sample.clamp(-1.0, 1.0);
            writer.write_sample((clamped * i16::MAX as f32) as i16)?;
        }

        writer.finalize()?;
    }

    Ok(cursor.into_inner())
}

pub(crate) fn normalize_dashscope_language(language: &str) -> Option<String> {
    let trimmed = language.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("auto") {
        return None;
    }

    match trimmed {
        "zh-Hans" | "zh-Hant" => Some("zh".to_string()),
        other => Some(other.to_string()),
    }
}

fn normalize_metadata_language(language: &str) -> String {
    normalize_dashscope_language(language).unwrap_or_else(|| "auto".to_string())
}

pub(crate) fn build_dashscope_transcribe_body(
    model: &str,
    mime_type: &str,
    audio_base64: &str,
    format: &str,
    language_hint: &str,
) -> Value {
    let data_uri = format!("data:{};base64,{}", mime_type, audio_base64);
    let mut asr_options = json!({
        "enable_lid": true,
        "enable_itn": false,
    });

    if let Some(language) = normalize_dashscope_language(language_hint) {
        asr_options["language"] = json!(language);
    }

    json!({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "input_audio",
                        "input_audio": {
                            "data": data_uri,
                            "format": format,
                        }
                    }
                ]
            }
        ],
        "modalities": ["text"],
        "extra_body": {
            "asr_options": asr_options
        }
    })
}

fn map_dashscope_status(status: StatusCode) -> anyhow::Error {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            anyhow!("Aliyun Qwen3-ASR authentication failed")
        }
        StatusCode::PAYLOAD_TOO_LARGE => anyhow!("Audio is too large for Aliyun Qwen3-ASR"),
        StatusCode::TOO_MANY_REQUESTS => anyhow!("Aliyun Qwen3-ASR rate limit exceeded"),
        _ => anyhow!(
            "Aliyun Qwen3-ASR request failed with HTTP {}",
            status.as_u16()
        ),
    }
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: Value,
}

fn extract_transcript_text(payload: &ChatCompletionResponse) -> Result<String> {
    for choice in &payload.choices {
        if let Some(text) = extract_text_value(&choice.message.content) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }
    }

    Err(anyhow!("Aliyun Qwen3-ASR returned an empty transcription"))
}

fn extract_text_value(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => {
            let joined = items
                .iter()
                .filter_map(extract_text_value)
                .filter(|text| !text.trim().is_empty())
                .collect::<Vec<_>>()
                .join("");
            if joined.is_empty() {
                None
            } else {
                Some(joined)
            }
        }
        Value::Object(map) => {
            if let Some(text) = map.get("text").and_then(Value::as_str) {
                return Some(text.to_string());
            }
            if let Some(text) = map.get("content").and_then(Value::as_str) {
                return Some(text.to_string());
            }
            None
        }
        _ => None,
    }
}

enum RealtimeClientCommand {
    Append(Vec<f32>),
    Finish,
    Cancel,
}

struct ActiveRealtimeSession {
    command_tx: mpsc::UnboundedSender<RealtimeClientCommand>,
    result_rx: oneshot::Receiver<Result<AsrTranscriptionResult, String>>,
}

static ACTIVE_REALTIME_SESSION: Lazy<Mutex<Option<ActiveRealtimeSession>>> =
    Lazy::new(|| Mutex::new(None));

#[derive(Clone, Debug)]
struct AliyunRealtimeConfig {
    base_url: String,
    api_key: String,
    model: String,
    language: String,
}

#[derive(Default)]
struct RealtimeTranscriptState {
    final_segments: Vec<String>,
    partial_text: String,
}

enum RealtimeEventOutcome {
    Continue,
    Finished(String),
}

pub fn has_active_realtime_session() -> bool {
    ACTIVE_REALTIME_SESSION.lock().unwrap().is_some()
}

pub fn start_active_realtime_session(app: &AppHandle) -> Result<()> {
    let settings = get_settings(app);
    let config = build_aliyun_realtime_config(&settings)?;
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let (result_tx, result_rx) = oneshot::channel();
    let app_handle = app.clone();

    {
        let mut active = ACTIVE_REALTIME_SESSION.lock().unwrap();
        if active.is_some() {
            return Err(anyhow!("Realtime ASR session is already active"));
        }
        *active = Some(ActiveRealtimeSession {
            command_tx,
            result_rx,
        });
    }

    crate::utils::emit_realtime_transcription_update(
        app,
        &crate::overlay::RealtimeTranscriptionUpdate {
            status: "connecting".to_string(),
            final_text: String::new(),
            partial_text: String::new(),
        },
    );

    tauri::async_runtime::spawn(async move {
        let result = run_aliyun_realtime_session(app_handle.clone(), config, command_rx)
            .await
            .map_err(|error| error.to_string());
        if let Err(error) = &result {
            crate::utils::emit_realtime_transcription_update(
                &app_handle,
                &crate::overlay::RealtimeTranscriptionUpdate {
                    status: "error".to_string(),
                    final_text: String::new(),
                    partial_text: error.clone(),
                },
            );
        }
        let _ = result_tx.send(result);
    });

    Ok(())
}

pub fn append_active_realtime_audio(samples: Vec<f32>) {
    let command_tx = ACTIVE_REALTIME_SESSION
        .lock()
        .unwrap()
        .as_ref()
        .map(|session| session.command_tx.clone());

    if let Some(command_tx) = command_tx {
        let _ = command_tx.send(RealtimeClientCommand::Append(samples));
    }
}

pub async fn finish_active_realtime_session() -> Result<AsrTranscriptionResult> {
    let session = ACTIVE_REALTIME_SESSION
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| anyhow!("Realtime ASR session is not active"))?;

    let _ = session.command_tx.send(RealtimeClientCommand::Finish);
    match tokio::time::timeout(REALTIME_FINISH_TIMEOUT, session.result_rx).await {
        Ok(Ok(Ok(result))) => Ok(result),
        Ok(Ok(Err(error))) => Err(anyhow!(error)),
        Ok(Err(_)) => Err(anyhow!("Realtime ASR session ended unexpectedly")),
        Err(_) => Err(anyhow!(
            "Timed out waiting for Qwen3-ASR realtime final transcript"
        )),
    }
}

pub fn cancel_active_realtime_session() {
    let session = ACTIVE_REALTIME_SESSION.lock().unwrap().take();
    if let Some(session) = session {
        let _ = session.command_tx.send(RealtimeClientCommand::Cancel);
    }
}

fn build_aliyun_realtime_config(
    settings: &crate::settings::AppSettings,
) -> Result<AliyunRealtimeConfig> {
    let provider = settings
        .asr_provider(ALIYUN_QWEN3_ASR_REALTIME_PROVIDER_ID)
        .ok_or_else(|| anyhow!("Aliyun Qwen3-ASR realtime provider is not configured"))?;
    let api_key = resolve_dashscope_api_key_for_provider_from_sources(
        settings,
        ALIYUN_QWEN3_ASR_REALTIME_PROVIDER_ID,
        std::env::var(DASHSCOPE_API_KEY_ENV).ok().as_deref(),
    )
    .ok_or_else(|| anyhow!("DashScope API key is not configured"))?;
    let model = settings
        .asr_models
        .get(ALIYUN_QWEN3_ASR_REALTIME_PROVIDER_ID)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or(ALIYUN_QWEN3_ASR_REALTIME_DEFAULT_MODEL)
        .to_string();

    Ok(AliyunRealtimeConfig {
        base_url: provider.base_url.clone(),
        api_key,
        model,
        language: normalize_metadata_language(&settings.selected_language),
    })
}

async fn run_aliyun_realtime_session(
    app: AppHandle,
    config: AliyunRealtimeConfig,
    mut command_rx: mpsc::UnboundedReceiver<RealtimeClientCommand>,
) -> Result<AsrTranscriptionResult> {
    let url = build_aliyun_realtime_url(&config.base_url, &config.model);
    let request = build_aliyun_realtime_request(&url, &config.api_key)?;
    let (ws_stream, _) = connect_async(request)
        .await
        .map_err(|source| anyhow!("Failed to connect Qwen3-ASR realtime: {}", source))?;
    let (mut write, mut read) = ws_stream.split();
    let mut state = RealtimeTranscriptState::default();

    write
        .send(Message::Text(
            build_aliyun_realtime_session_update(&config.language)
                .to_string()
                .into(),
        ))
        .await
        .map_err(|source| {
            anyhow!(
                "Failed to initialize Qwen3-ASR realtime session: {}",
                source
            )
        })?;

    emit_realtime_update(&app, "listening", &state);

    loop {
        tokio::select! {
            command = command_rx.recv() => {
                match command {
                    Some(RealtimeClientCommand::Append(samples)) => {
                        if samples.is_empty() {
                            continue;
                        }
                        write
                            .send(Message::Text(
                                build_aliyun_realtime_audio_append_event(&samples)
                                    .to_string()
                                    .into(),
                            ))
                            .await
                            .map_err(|source| anyhow!("Failed to send realtime audio chunk: {}", source))?;
                    }
                    Some(RealtimeClientCommand::Finish) => {
                        emit_realtime_update(&app, "finalizing", &state);
                        write
                            .send(Message::Text(
                                json!({"type": "session.finish"}).to_string().into(),
                            ))
                            .await
                            .map_err(|source| anyhow!("Failed to finish Qwen3-ASR realtime session: {}", source))?;
                    }
                    Some(RealtimeClientCommand::Cancel) | None => {
                        let _ = write.close().await;
                        return Err(anyhow!("Realtime ASR session was cancelled"));
                    }
                }
            }
            message = read.next() => {
                match message {
                    Some(Ok(Message::Text(text))) => {
                        match handle_aliyun_realtime_message(&app, &mut state, &text)? {
                            RealtimeEventOutcome::Continue => {}
                            RealtimeEventOutcome::Finished(text) => {
                                let _ = write.close().await;
                                return Ok(AsrTranscriptionResult {
                                    text: text.trim().to_string(),
                                    provider_id: AliyunQwen3AsrRealtimeProvider.provider_id().to_string(),
                                    model: config.model,
                                    language: config.language,
                                });
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        return Err(anyhow!("Qwen3-ASR realtime connection closed before final transcript"));
                    }
                    Some(Ok(_)) => {}
                    Some(Err(source)) => {
                        return Err(anyhow!("Qwen3-ASR realtime WebSocket error: {}", source));
                    }
                    None => {
                        return Err(anyhow!("Qwen3-ASR realtime connection ended before final transcript"));
                    }
                }
            }
        }
    }
}

fn build_aliyun_realtime_url(base_url: &str, model: &str) -> String {
    let separator = if base_url.contains('?') { '&' } else { '?' };
    format!(
        "{}{}model={}",
        base_url.trim_end_matches('/'),
        separator,
        model
    )
}

fn build_aliyun_realtime_request(
    url: &str,
    api_key: &str,
) -> Result<tokio_tungstenite::tungstenite::http::Request<()>> {
    let mut request = url
        .into_client_request()
        .map_err(|source| anyhow!("Invalid Qwen3-ASR realtime URL: {}", source))?;
    request.headers_mut().insert(
        "Authorization",
        HeaderValue::from_str(&format!("Bearer {}", api_key.trim()))
            .map_err(|source| anyhow!("Invalid DashScope API key header: {}", source))?,
    );
    request
        .headers_mut()
        .insert("OpenAI-Beta", HeaderValue::from_static("realtime=v1"));
    Ok(request)
}

pub(crate) fn build_aliyun_realtime_session_update(language_hint: &str) -> Value {
    let mut input_audio_transcription = json!({});
    if let Some(language) = normalize_dashscope_language(language_hint) {
        input_audio_transcription["language"] = json!(language);
    }

    json!({
        "type": "session.update",
        "session": {
            "modalities": ["text"],
            "input_audio_format": "pcm",
            "sample_rate": 16000,
            "input_audio_transcription": input_audio_transcription,
            "turn_detection": {
                "type": "server_vad",
                "threshold": 0.0,
                "silence_duration_ms": 400
            }
        }
    })
}

pub(crate) fn build_aliyun_realtime_audio_append_event(samples: &[f32]) -> Value {
    json!({
        "type": "input_audio_buffer.append",
        "audio": STANDARD.encode(samples_to_pcm16_bytes(samples))
    })
}

fn samples_to_pcm16_bytes(samples: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let sample_i16 = (clamped * i16::MAX as f32) as i16;
        bytes.extend_from_slice(&sample_i16.to_le_bytes());
    }
    bytes
}

fn handle_aliyun_realtime_message(
    app: &AppHandle,
    state: &mut RealtimeTranscriptState,
    text: &str,
) -> Result<RealtimeEventOutcome> {
    let value: Value = serde_json::from_str(text)
        .map_err(|source| anyhow!("Failed to decode Qwen3-ASR realtime event: {}", source))?;
    let outcome = apply_aliyun_realtime_event(state, &value)?;
    emit_realtime_update(app, realtime_status_for_outcome(&outcome), state);
    Ok(outcome)
}

fn apply_aliyun_realtime_event(
    state: &mut RealtimeTranscriptState,
    value: &Value,
) -> Result<RealtimeEventOutcome> {
    let event_type = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    log_realtime_event_shape(event_type, value);

    match event_type {
        event_type if is_realtime_partial_event(event_type, value) => {
            apply_realtime_partial_text(state, value);
        }
        event_type if is_realtime_completed_event(event_type, value) => {
            if let Some(transcript) = value.get("transcript").and_then(Value::as_str) {
                push_final_segment(state, transcript);
            }
        }
        "session.finished" => {
            let transcript = value
                .get("transcript")
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .unwrap_or_else(|| state.final_segments.join(""));
            state.partial_text.clear();
            if !transcript.trim().is_empty() {
                state.final_segments = vec![transcript.clone()];
            }
            return Ok(RealtimeEventOutcome::Finished(transcript));
        }
        event_type if event_type.contains("error") => {
            let error = value
                .get("error")
                .and_then(extract_text_value)
                .or_else(|| {
                    value
                        .get("message")
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                })
                .unwrap_or_else(|| "Qwen3-ASR realtime returned an error".to_string());
            return Err(anyhow!(error));
        }
        _ => {}
    }

    Ok(RealtimeEventOutcome::Continue)
}

fn realtime_partial_text(value: &Value) -> Option<&str> {
    value
        .get("text")
        .and_then(Value::as_str)
        .or_else(|| value.get("stash").and_then(Value::as_str))
        .or_else(|| value.get("delta").and_then(Value::as_str))
}

fn apply_realtime_partial_text(state: &mut RealtimeTranscriptState, value: &Value) {
    if let Some(partial) = value
        .get("text")
        .and_then(Value::as_str)
        .or_else(|| value.get("stash").and_then(Value::as_str))
    {
        state.partial_text = partial.to_string();
        return;
    }

    if let Some(delta) = value.get("delta").and_then(Value::as_str) {
        state.partial_text.push_str(delta);
    }
}

fn is_realtime_partial_event(event_type: &str, value: &Value) -> bool {
    event_type.contains("input_audio_transcription") && realtime_partial_text(value).is_some()
}

fn is_realtime_completed_event(event_type: &str, value: &Value) -> bool {
    event_type.contains("input_audio_transcription")
        && value.get("transcript").and_then(Value::as_str).is_some()
}

fn log_realtime_event_shape(event_type: &str, value: &Value) {
    let text_len = value
        .get("text")
        .and_then(Value::as_str)
        .map(str::len)
        .unwrap_or(0);
    let stash_len = value
        .get("stash")
        .and_then(Value::as_str)
        .map(str::len)
        .unwrap_or(0);
    let transcript_len = value
        .get("transcript")
        .and_then(Value::as_str)
        .map(str::len)
        .unwrap_or(0);
    let delta_len = value
        .get("delta")
        .and_then(Value::as_str)
        .map(str::len)
        .unwrap_or(0);

    debug!(
        "Qwen3-ASR realtime event: type={}, text_len={}, stash_len={}, delta_len={}, transcript_len={}",
        event_type, text_len, stash_len, delta_len, transcript_len
    );
}

fn realtime_status_for_outcome(outcome: &RealtimeEventOutcome) -> &'static str {
    match outcome {
        RealtimeEventOutcome::Continue => "transcribing",
        RealtimeEventOutcome::Finished(_) => "finalizing",
    }
}

fn push_final_segment(state: &mut RealtimeTranscriptState, segment: &str) {
    let trimmed = segment.trim();
    if trimmed.is_empty() {
        return;
    }
    state.final_segments.push(trimmed.to_string());
    state.partial_text.clear();
}

fn emit_realtime_update(app: &AppHandle, status: &str, state: &RealtimeTranscriptState) {
    crate::utils::emit_realtime_transcription_update(
        app,
        &crate::overlay::RealtimeTranscriptionUpdate {
            status: status.to_string(),
            final_text: state.final_segments.join(""),
            partial_text: state.partial_text.clone(),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::get_default_settings;

    #[test]
    fn dashscope_body_omits_auto_language() {
        let body =
            build_dashscope_transcribe_body("qwen3-asr-flash", "audio/wav", "abc", "wav", "auto");

        assert_eq!(body["model"], "qwen3-asr-flash");
        assert_eq!(body["modalities"][0], "text");
        assert_eq!(
            body["messages"][0]["content"][0]["input_audio"]["data"],
            "data:audio/wav;base64,abc"
        );
        assert!(body["extra_body"]["asr_options"]["language"].is_null());
    }

    #[test]
    fn dashscope_body_normalizes_chinese_language() {
        let body = build_dashscope_transcribe_body(
            "qwen3-asr-flash",
            "audio/wav",
            "abc",
            "wav",
            "zh-Hans",
        );

        assert_eq!(body["extra_body"]["asr_options"]["language"], "zh");
    }

    #[test]
    fn realtime_url_and_headers_are_constructed() {
        let url = build_aliyun_realtime_url(
            "wss://dashscope.aliyuncs.com/api-ws/v1/realtime",
            "qwen3-asr-flash-realtime-2026-02-10",
        );
        let request = build_aliyun_realtime_request(&url, "test-key").unwrap();

        assert_eq!(
            request.uri().to_string(),
            "wss://dashscope.aliyuncs.com/api-ws/v1/realtime?model=qwen3-asr-flash-realtime-2026-02-10"
        );
        assert_eq!(
            request.headers().get("Authorization").unwrap(),
            "Bearer test-key"
        );
        assert_eq!(request.headers().get("OpenAI-Beta").unwrap(), "realtime=v1");
    }

    #[test]
    fn realtime_session_update_normalizes_language() {
        let body = build_aliyun_realtime_session_update("zh-Hans");

        assert_eq!(body["type"], "session.update");
        assert_eq!(body["session"]["input_audio_format"], "pcm");
        assert_eq!(body["session"]["sample_rate"], 16000);
        assert_eq!(
            body["session"]["input_audio_transcription"]["language"],
            "zh"
        );
        assert_eq!(
            body["session"]["turn_detection"]["silence_duration_ms"],
            400
        );
    }

    #[test]
    fn realtime_session_update_omits_auto_language() {
        let body = build_aliyun_realtime_session_update("auto");

        assert!(body["session"]["input_audio_transcription"]["language"].is_null());
    }

    #[test]
    fn realtime_audio_append_encodes_pcm16_base64() {
        let body = build_aliyun_realtime_audio_append_event(&[0.0, 1.0, -1.0]);
        let encoded = body["audio"].as_str().unwrap();
        let decoded = STANDARD.decode(encoded).unwrap();

        assert_eq!(body["type"], "input_audio_buffer.append");
        assert_eq!(decoded.len(), 6);
        assert_eq!(&decoded[0..2], &0i16.to_le_bytes());
        assert_eq!(&decoded[2..4], &i16::MAX.to_le_bytes());
        assert_eq!(&decoded[4..6], &i16::MIN.wrapping_add(1).to_le_bytes());
    }

    #[test]
    fn realtime_event_parser_tracks_partial_and_final_text() {
        let mut state = RealtimeTranscriptState::default();

        let partial = json!({
            "type": "conversation.item.input_audio_transcription.text",
            "text": "临时"
        });
        let completed = json!({
            "type": "conversation.item.input_audio_transcription.completed",
            "transcript": "最终"
        });

        assert!(matches!(
            apply_aliyun_realtime_event(&mut state, &partial).unwrap(),
            RealtimeEventOutcome::Continue
        ));
        assert_eq!(state.partial_text, "临时");

        assert!(matches!(
            apply_aliyun_realtime_event(&mut state, &completed).unwrap(),
            RealtimeEventOutcome::Continue
        ));
        assert_eq!(state.final_segments, vec!["最终".to_string()]);
        assert!(state.partial_text.is_empty());
    }

    #[test]
    fn realtime_event_parser_accepts_stash_partial_text() {
        let mut state = RealtimeTranscriptState::default();
        let partial = json!({
            "type": "conversation.item.input_audio_transcription.text",
            "stash": "边说边出的中间结果"
        });

        assert!(matches!(
            apply_aliyun_realtime_event(&mut state, &partial).unwrap(),
            RealtimeEventOutcome::Continue
        ));
        assert_eq!(state.partial_text, "边说边出的中间结果");
    }

    #[test]
    fn realtime_event_parser_accepts_delta_partial_text() {
        let mut state = RealtimeTranscriptState::default();
        let first = json!({
            "type": "conversation.item.input_audio_transcription.delta",
            "delta": "边"
        });
        let second = json!({
            "type": "conversation.item.input_audio_transcription.delta",
            "delta": "说"
        });

        assert!(matches!(
            apply_aliyun_realtime_event(&mut state, &first).unwrap(),
            RealtimeEventOutcome::Continue
        ));
        assert!(matches!(
            apply_aliyun_realtime_event(&mut state, &second).unwrap(),
            RealtimeEventOutcome::Continue
        ));
        assert_eq!(state.partial_text, "边说");
    }

    #[test]
    fn realtime_event_parser_accepts_variant_partial_event_name() {
        let mut state = RealtimeTranscriptState::default();
        let partial = json!({
            "type": "response.input_audio_transcription.delta",
            "text": "变体事件名"
        });

        assert!(matches!(
            apply_aliyun_realtime_event(&mut state, &partial).unwrap(),
            RealtimeEventOutcome::Continue
        ));
        assert_eq!(state.partial_text, "变体事件名");
    }

    #[test]
    fn realtime_event_parser_prefers_session_finished_transcript() {
        let mut state = RealtimeTranscriptState {
            final_segments: vec!["旧".to_string()],
            partial_text: "临时".to_string(),
        };
        let finished = json!({
            "type": "session.finished",
            "transcript": "完整最终"
        });

        match apply_aliyun_realtime_event(&mut state, &finished).unwrap() {
            RealtimeEventOutcome::Finished(text) => assert_eq!(text, "完整最终"),
            RealtimeEventOutcome::Continue => panic!("expected finished event"),
        }
        assert_eq!(state.final_segments, vec!["完整最终".to_string()]);
        assert!(state.partial_text.is_empty());
    }

    #[test]
    fn extract_transcript_from_string_response() {
        let payload = ChatCompletionResponse {
            choices: vec![ChatChoice {
                message: ChatMessage {
                    content: Value::String("  你好 SpeakMore  ".to_string()),
                },
            }],
        };

        assert_eq!(
            extract_transcript_text(&payload).unwrap(),
            "你好 SpeakMore".to_string()
        );
    }

    #[test]
    fn extract_transcript_rejects_empty_response() {
        let payload = ChatCompletionResponse { choices: vec![] };

        assert!(extract_transcript_text(&payload).is_err());
    }

    #[test]
    fn dashscope_key_uses_settings_before_environment() {
        let mut settings = get_default_settings();
        settings.asr_api_keys.insert(
            ALIYUN_QWEN3_ASR_PROVIDER_ID.to_string(),
            " settings-secret ".to_string(),
        );

        assert_eq!(
            resolve_dashscope_api_key_from_sources(&settings, Some("env-secret")).as_deref(),
            Some("settings-secret")
        );
    }

    #[test]
    fn dashscope_key_missing_when_sources_empty() {
        let settings = get_default_settings();

        assert!(resolve_dashscope_api_key_from_sources(&settings, None).is_none());
        assert!(resolve_dashscope_api_key_from_sources(&settings, Some("  ")).is_none());
    }

    #[test]
    fn realtime_key_reuses_batch_provider_key_before_environment() {
        let mut settings = get_default_settings();
        settings.asr_api_keys.insert(
            ALIYUN_QWEN3_ASR_PROVIDER_ID.to_string(),
            "batch-secret".to_string(),
        );

        assert_eq!(
            resolve_dashscope_api_key_for_provider_from_sources(
                &settings,
                ALIYUN_QWEN3_ASR_REALTIME_PROVIDER_ID,
                Some("env-secret"),
            )
            .as_deref(),
            Some("batch-secret")
        );
    }

    #[test]
    fn dashscope_authentication_error_is_redacted() {
        let error = map_dashscope_status(StatusCode::UNAUTHORIZED).to_string();

        assert_eq!(error, "Aliyun Qwen3-ASR authentication failed");
        assert!(!error.contains("sk-"));
    }

    #[test]
    fn wav_encoding_has_expected_header() {
        let bytes = encode_wav_bytes(&[0.0, 0.5, -0.5]).unwrap();

        assert!(bytes.starts_with(b"RIFF"));
        assert!(bytes.windows(4).any(|window| window == b"WAVE"));
    }
}

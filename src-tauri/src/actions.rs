#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
use crate::apple_intelligence;
use crate::audio_feedback::{play_feedback_sound, play_feedback_sound_blocking, SoundType};
use crate::audio_toolkit::{is_microphone_access_denied, is_no_input_device_error};
use crate::managers::audio::AudioRecordingManager;
use crate::managers::history::{
    AsrHistoryMetadata, HistoryManager, NewHistoryEvent, NewPostProcessRun, NewTranscriptionRun,
    HISTORY_EVENT_PASTE_FAILED, HISTORY_EVENT_PASTE_SUCCEEDED, HISTORY_EVENT_POST_PROCESS_FALLBACK,
    HISTORY_EVENT_SOURCE_BACKEND, HISTORY_RUN_TYPE_POST_PROCESS, HISTORY_RUN_TYPE_TRANSCRIPTION,
    HISTORY_STATUS_COMPLETED, HISTORY_STATUS_EMPTY, HISTORY_STATUS_FAILED,
    TRANSCRIPTION_RUN_STATUS_EMPTY, TRANSCRIPTION_RUN_STATUS_FAILED,
    TRANSCRIPTION_RUN_STATUS_SUCCESS,
};
use crate::managers::model::{EngineType, ModelManager};
use crate::managers::transcription::TranscriptionManager;
use crate::settings::{
    get_settings, AppSettings, PostProcessPreset, PostProcessProvider, PostProcessReasoningControl,
    PostProcessStructuredOutputMode, APPLE_INTELLIGENCE_PROVIDER_ID,
    BUILT_IN_LOCAL_ASR_PROVIDER_ID,
};
use crate::shortcut;
use crate::tray::{change_tray_icon, TrayIconState};
use crate::utils::{
    self, show_processing_overlay, show_realtime_overlay, show_recording_overlay,
    show_transcribing_overlay,
};
use crate::TranscriptionCoordinator;
use ferrous_opencc::{config::BuiltinConfig, OpenCC};
use log::{debug, error, warn};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tauri::Manager;
use tauri::{AppHandle, Emitter};

#[derive(Clone, serde::Serialize)]
struct RecordingErrorEvent {
    error_type: String,
    detail: Option<String>,
}

#[derive(Clone, serde::Serialize)]
struct PostProcessFallbackEvent {
    error_summary: Option<String>,
}

/// Drop guard that notifies the [`TranscriptionCoordinator`] when the
/// transcription pipeline finishes — whether it completes normally or panics.
struct FinishGuard(AppHandle);
impl Drop for FinishGuard {
    fn drop(&mut self) {
        if let Some(c) = self.0.try_state::<TranscriptionCoordinator>() {
            c.notify_processing_finished();
        }
    }
}

// Shortcut Action Trait
pub trait ShortcutAction: Send + Sync {
    fn start(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str);
    fn stop(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str);
}

// Transcribe Action
struct TranscribeAction {
    post_process: bool,
}

/// Field name for structured output JSON schema
const TRANSCRIPTION_FIELD: &str = "transcription";

/// Strip invisible Unicode characters that some LLMs may insert
fn strip_invisible_chars(s: &str) -> String {
    s.replace(['\u{200B}', '\u{200C}', '\u{200D}', '\u{FEFF}'], "")
}

fn render_template(template: &str, transcription: &str) -> String {
    template.replace("${output}", transcription)
}

fn compose_chat_messages_for_legacy(system_prompt: &str, user_content: &str) -> String {
    let system = system_prompt.trim();
    let user = user_content.trim();

    match (system.is_empty(), user.is_empty()) {
        (true, true) => String::new(),
        (true, false) => user.to_string(),
        (false, true) => system.to_string(),
        (false, false) => format!("{system}\n\n{user}"),
    }
}

fn build_json_object_system_prompt(system_prompt: &str) -> String {
    let base = system_prompt.trim();
    let json_instruction =
        "Return a valid JSON object with exactly one string field named \"transcription\".";
    if base.is_empty() {
        json_instruction.to_string()
    } else {
        format!("{base}\n\n{json_instruction}")
    }
}

const POST_PROCESS_STATUS_SUCCESS: &str = "success";
const POST_PROCESS_STATUS_FAILED: &str = "failed";
const ERROR_SUMMARY_MAX_CHARS: usize = 240;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct PostProcessPreviewResult {
    pub preset_id: String,
    pub preset_version: i64,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub status: String,
    pub output_text: Option<String>,
    pub latency_ms: i64,
    pub error_summary: Option<String>,
}

#[derive(Clone, Debug)]
struct PostProcessAttempt {
    processed_text: Option<String>,
    prompt_template: Option<String>,
    preset_id: Option<String>,
    preset_version: Option<i64>,
    provider_id: Option<String>,
    model: Option<String>,
    status: String,
    latency_ms: i64,
    error_summary: Option<String>,
}

impl PostProcessAttempt {
    fn failed(
        started_at: Instant,
        preset: Option<&PostProcessPreset>,
        provider_id: Option<String>,
        model: Option<String>,
        error_summary: String,
    ) -> Self {
        Self {
            processed_text: None,
            prompt_template: preset.map(|preset| preset.prompt_template_snapshot()),
            preset_id: preset.map(|preset| preset.id.clone()),
            preset_version: preset.map(|preset| i64::from(preset.version)),
            provider_id,
            model,
            status: POST_PROCESS_STATUS_FAILED.to_string(),
            latency_ms: elapsed_ms(started_at),
            error_summary: Some(error_summary),
        }
    }

    fn success(
        started_at: Instant,
        preset: &PostProcessPreset,
        provider_id: String,
        model: String,
        processed_text: String,
    ) -> Self {
        Self {
            processed_text: Some(processed_text),
            prompt_template: Some(preset.prompt_template_snapshot()),
            preset_id: Some(preset.id.clone()),
            preset_version: Some(i64::from(preset.version)),
            provider_id: Some(provider_id),
            model: Some(model),
            status: POST_PROCESS_STATUS_SUCCESS.to_string(),
            latency_ms: elapsed_ms(started_at),
            error_summary: None,
        }
    }

    fn to_run(&self, input_text: &str) -> NewPostProcessRun {
        NewPostProcessRun {
            preset_id: self.preset_id.clone(),
            preset_version: self.preset_version,
            provider_id: self.provider_id.clone(),
            model: self.model.clone(),
            status: self.status.clone(),
            input_text: Some(input_text.to_string()),
            output_text: self.processed_text.clone(),
            prompt_template_snapshot: self.prompt_template.clone(),
            latency_ms: self.latency_ms,
            error_summary: self.error_summary.clone(),
        }
    }

    fn to_preview_result(&self, fallback_preset: &PostProcessPreset) -> PostProcessPreviewResult {
        PostProcessPreviewResult {
            preset_id: self
                .preset_id
                .clone()
                .unwrap_or_else(|| fallback_preset.id.clone()),
            preset_version: self
                .preset_version
                .unwrap_or_else(|| i64::from(fallback_preset.version)),
            provider_id: self.provider_id.clone(),
            model: self.model.clone(),
            status: self.status.clone(),
            output_text: self.processed_text.clone(),
            latency_ms: self.latency_ms,
            error_summary: self.error_summary.clone(),
        }
    }
}

fn elapsed_ms(started_at: Instant) -> i64 {
    started_at.elapsed().as_millis().min(i64::MAX as u128) as i64
}

fn transcription_run_from_result(
    metadata: &AsrHistoryMetadata,
    status: &str,
    transcript_text: String,
    latency_ms: i64,
    error_summary: Option<String>,
) -> NewTranscriptionRun {
    NewTranscriptionRun {
        provider_id: Some(metadata.provider_id.clone()),
        model: Some(metadata.model.clone()),
        language: Some(metadata.language.clone()),
        status: status.to_string(),
        transcript_text,
        latency_ms,
        error_summary,
    }
}

fn transcription_run_from_metadata(
    metadata: Option<&AsrHistoryMetadata>,
    status: &str,
    transcript_text: String,
    latency_ms: i64,
    error_summary: Option<String>,
) -> NewTranscriptionRun {
    NewTranscriptionRun {
        provider_id: metadata.map(|metadata| metadata.provider_id.clone()),
        model: metadata.map(|metadata| metadata.model.clone()),
        language: metadata.map(|metadata| metadata.language.clone()),
        status: status.to_string(),
        transcript_text,
        latency_ms,
        error_summary,
    }
}

fn sanitize_error_summary(
    error: &str,
    settings: &AppSettings,
    prompt_template: Option<&str>,
) -> String {
    let mut summary = error.replace(['\n', '\r'], " ");

    for secret in settings
        .post_process_api_keys
        .values()
        .chain(settings.asr_api_keys.values())
    {
        if secret.trim().len() >= 4 {
            summary = summary.replace(secret, "[REDACTED]");
        }
    }

    if let Some(prompt) = prompt_template {
        if prompt.trim().len() >= 12 {
            summary = summary.replace(prompt, "[PROMPT]");
        }
    }

    for marker in ["Bearer ", "sk-", "dashscope-"] {
        if let Some(index) = summary.find(marker) {
            let end = summary[index..]
                .find(char::is_whitespace)
                .map(|offset| index + offset)
                .unwrap_or(summary.len());
            summary.replace_range(index..end, "[REDACTED]");
        }
    }

    let summary = summary.trim();
    if summary.chars().count() <= ERROR_SUMMARY_MAX_CHARS {
        return summary.to_string();
    }

    let mut truncated: String = summary.chars().take(ERROR_SUMMARY_MAX_CHARS).collect();
    truncated.push_str("...");
    truncated
}

fn selected_post_process_preset(settings: &AppSettings) -> Result<PostProcessPreset, String> {
    if let Some(preset) = settings.active_post_process_preset() {
        return Ok(preset.clone());
    }

    if let Some(prompt_id) = settings.post_process_selected_prompt_id.as_deref() {
        if let Some(prompt) = settings
            .post_process_prompts
            .iter()
            .find(|prompt| prompt.id == prompt_id)
        {
            return Ok(PostProcessPreset {
                id: prompt.id.clone(),
                name: prompt.name.clone(),
                description: String::new(),
                prompt_template: prompt.prompt.clone(),
                system_template: None,
                user_template: None,
                output_kind: "plain_text".to_string(),
                version: 1,
                is_builtin: false,
                enabled: true,
                examples: Vec::new(),
            });
        }
    }

    Err("No post-processing preset is selected".to_string())
}

fn post_process_chat_options(
    settings: &AppSettings,
    provider: &PostProcessProvider,
    system_prompt: Option<String>,
    structured_output_mode: PostProcessStructuredOutputMode,
    json_schema: Option<serde_json::Value>,
) -> Result<crate::llm_client::ChatCompletionOptions, String> {
    let mut options = crate::llm_client::ChatCompletionOptions {
        system_prompt,
        structured_output_mode,
        json_schema,
        ..Default::default()
    };
    let configured_effort = settings
        .post_process_reasoning_efforts
        .get(&provider.id)
        .map(|value| value.trim().to_lowercase())
        .unwrap_or_default();

    match provider.reasoning_control {
        PostProcessReasoningControl::None => {}
        PostProcessReasoningControl::OpenAiReasoningEffort => match configured_effort.as_str() {
            "" | "disabled" | "none" => {}
            "low" | "medium" | "high" | "max" | "xhigh" => {
                options.reasoning_effort = Some(configured_effort);
            }
            _ => {
                return Err(
                    "Configured reasoning effort is not supported by this provider".to_string(),
                )
            }
        },
        PostProcessReasoningControl::OpenRouterReasoning => {
            let effort = if configured_effort.is_empty() {
                "none".to_string()
            } else {
                configured_effort
            };
            match effort.as_str() {
                "none" | "minimal" | "low" | "medium" | "high" | "xhigh" => {
                    options.reasoning = Some(crate::llm_client::ReasoningConfig {
                        effort: Some(effort),
                        exclude: Some(true),
                    });
                }
                _ => {
                    return Err(
                        "Configured OpenRouter reasoning effort is not supported".to_string()
                    )
                }
            }
        }
        PostProcessReasoningControl::DeepSeekThinking => match configured_effort.as_str() {
            "" | "disabled" | "none" => {
                options.thinking = Some(crate::llm_client::ThinkingConfig {
                    thinking_type: "disabled".to_string(),
                });
            }
            "high" | "max" => {
                options.thinking = Some(crate::llm_client::ThinkingConfig {
                    thinking_type: "enabled".to_string(),
                });
                options.reasoning_effort = Some(configured_effort);
            }
            _ => return Err("DeepSeek thinking only supports disabled, high, or max".to_string()),
        },
    }

    Ok(options)
}

async fn run_post_process_preset(
    settings: &AppSettings,
    preset: &PostProcessPreset,
    transcription: &str,
) -> PostProcessAttempt {
    let started_at = Instant::now();
    let provider = match settings.active_post_process_provider().cloned() {
        Some(provider) => provider,
        None => {
            debug!("Post-processing enabled but no provider is selected");
            return PostProcessAttempt::failed(
                started_at,
                Some(preset),
                None,
                None,
                "No post-processing provider is selected".to_string(),
            );
        }
    };

    let model = settings
        .post_process_models
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();

    if model.trim().is_empty() {
        debug!(
            "Post-processing skipped because provider '{}' has no model configured",
            provider.id
        );
        return PostProcessAttempt::failed(
            started_at,
            Some(preset),
            Some(provider.id.clone()),
            None,
            "No model is configured for the selected post-processing provider".to_string(),
        );
    }

    let prompt_snapshot = preset.prompt_template_snapshot();
    let system_template = preset.effective_system_template();
    let user_template = preset.effective_user_template();

    if system_template.trim().is_empty() && user_template.trim().is_empty() {
        debug!("Post-processing skipped because the selected prompt is empty");
        return PostProcessAttempt::failed(
            started_at,
            Some(preset),
            Some(provider.id.clone()),
            Some(model),
            "Selected post-processing preset has no system or user template".to_string(),
        );
    }

    debug!(
        "Starting LLM post-processing with provider '{}' (model: {})",
        provider.id, model
    );

    let api_key = settings
        .post_process_api_keys
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();

    if provider.id != "custom"
        && provider.id != APPLE_INTELLIGENCE_PROVIDER_ID
        && api_key.trim().is_empty()
    {
        return PostProcessAttempt::failed(
            started_at,
            Some(preset),
            Some(provider.id.clone()),
            Some(model),
            "API key is required for the selected post-processing provider".to_string(),
        );
    }

    if provider.structured_output_mode != PostProcessStructuredOutputMode::None {
        debug!("Using structured outputs for provider '{}'", provider.id);

        let rendered_system_prompt = render_template(&system_template, transcription);
        let system_prompt = match provider.structured_output_mode {
            PostProcessStructuredOutputMode::JsonObject => {
                build_json_object_system_prompt(&rendered_system_prompt)
            }
            _ => rendered_system_prompt.trim().to_string(),
        };
        let user_content = render_template(&user_template, transcription);

        // Handle Apple Intelligence separately since it uses native Swift APIs
        if provider.id == APPLE_INTELLIGENCE_PROVIDER_ID {
            #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
            {
                if !apple_intelligence::check_apple_intelligence_availability() {
                    debug!(
                        "Apple Intelligence selected but not currently available on this device"
                    );
                    return PostProcessAttempt::failed(
                        started_at,
                        Some(preset),
                        Some(provider.id.clone()),
                        Some(model),
                        "Apple Intelligence is not available on this device".to_string(),
                    );
                }

                let token_limit = model.trim().parse::<i32>().unwrap_or(0);
                return match apple_intelligence::process_text_with_system_prompt(
                    &system_prompt,
                    &user_content,
                    token_limit,
                ) {
                    Ok(result) => {
                        if result.trim().is_empty() {
                            debug!("Apple Intelligence returned an empty response");
                            PostProcessAttempt::failed(
                                started_at,
                                Some(preset),
                                Some(provider.id.clone()),
                                Some(model),
                                "Post-processing returned an empty response".to_string(),
                            )
                        } else {
                            let result = strip_invisible_chars(&result);
                            debug!(
                                "Apple Intelligence post-processing succeeded. Output length: {} chars",
                                result.len()
                            );
                            PostProcessAttempt::success(
                                started_at,
                                preset,
                                provider.id.clone(),
                                model,
                                result,
                            )
                        }
                    }
                    Err(err) => {
                        let summary = sanitize_error_summary(
                            &err.to_string(),
                            settings,
                            Some(&prompt_snapshot),
                        );
                        error!("Apple Intelligence post-processing failed: {}", summary);
                        PostProcessAttempt::failed(
                            started_at,
                            Some(preset),
                            Some(provider.id.clone()),
                            Some(model),
                            summary,
                        )
                    }
                };
            }

            #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
            {
                debug!("Apple Intelligence provider selected on unsupported platform");
                return PostProcessAttempt::failed(
                    started_at,
                    Some(preset),
                    Some(provider.id.clone()),
                    Some(model),
                    "Apple Intelligence is not supported on this platform".to_string(),
                );
            }
        }

        // Define JSON schema for transcription output
        let json_schema = serde_json::json!({
            "type": "object",
            "properties": {
                (TRANSCRIPTION_FIELD): {
                    "type": "string",
                    "description": "The cleaned and processed transcription text"
                }
            },
            "required": [TRANSCRIPTION_FIELD],
            "additionalProperties": false
        });

        let json_schema = match provider.structured_output_mode {
            PostProcessStructuredOutputMode::OpenAiJsonSchema => Some(json_schema),
            PostProcessStructuredOutputMode::JsonObject | PostProcessStructuredOutputMode::None => {
                None
            }
        };
        let options = match post_process_chat_options(
            settings,
            &provider,
            Some(system_prompt),
            provider.structured_output_mode,
            json_schema,
        ) {
            Ok(options) => options,
            Err(error) => {
                return PostProcessAttempt::failed(
                    started_at,
                    Some(preset),
                    Some(provider.id.clone()),
                    Some(model),
                    error,
                );
            }
        };

        match crate::llm_client::send_chat_completion(
            &provider,
            api_key.clone(),
            &model,
            user_content,
            options,
        )
        .await
        {
            Ok(Some(content)) => {
                // Parse the JSON response to extract the transcription field
                match serde_json::from_str::<serde_json::Value>(&content) {
                    Ok(json) => {
                        if let Some(transcription_value) =
                            json.get(TRANSCRIPTION_FIELD).and_then(|t| t.as_str())
                        {
                            let result = strip_invisible_chars(transcription_value);
                            debug!(
                                "Structured output post-processing succeeded for provider '{}'. Output length: {} chars",
                                provider.id,
                                result.len()
                            );
                            if result.trim().is_empty() {
                                return PostProcessAttempt::failed(
                                    started_at,
                                    Some(preset),
                                    Some(provider.id.clone()),
                                    Some(model),
                                    "Post-processing returned an empty response".to_string(),
                                );
                            }
                            return PostProcessAttempt::success(
                                started_at,
                                preset,
                                provider.id.clone(),
                                model,
                                result,
                            );
                        } else {
                            error!("Structured output response missing 'transcription' field");
                            let result = strip_invisible_chars(&content);
                            if result.trim().is_empty() {
                                return PostProcessAttempt::failed(
                                    started_at,
                                    Some(preset),
                                    Some(provider.id.clone()),
                                    Some(model),
                                    "Post-processing returned an empty response".to_string(),
                                );
                            }
                            return PostProcessAttempt::success(
                                started_at,
                                preset,
                                provider.id.clone(),
                                model,
                                result,
                            );
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to parse structured output JSON: {}. Returning raw content.",
                            e
                        );
                        let result = strip_invisible_chars(&content);
                        if result.trim().is_empty() {
                            return PostProcessAttempt::failed(
                                started_at,
                                Some(preset),
                                Some(provider.id.clone()),
                                Some(model),
                                "Post-processing returned an empty response".to_string(),
                            );
                        }
                        return PostProcessAttempt::success(
                            started_at,
                            preset,
                            provider.id.clone(),
                            model,
                            result,
                        );
                    }
                }
            }
            Ok(None) => {
                error!("LLM API response has no content");
                return PostProcessAttempt::failed(
                    started_at,
                    Some(preset),
                    Some(provider.id.clone()),
                    Some(model),
                    "Post-processing returned no content".to_string(),
                );
            }
            Err(e) => {
                let summary = sanitize_error_summary(&e, settings, Some(&prompt_snapshot));
                warn!(
                    "Structured output failed for provider '{}': {}. Falling back to legacy mode.",
                    provider.id, summary
                );
                // Fall through to legacy mode below
            }
        }
    }

    // Legacy mode: send a single user message. Old presets keep the exact
    // historical `${output}` replacement behavior; explicit chat-template
    // presets flatten System + User into one message for providers that do
    // not support structured output.
    let processed_prompt = if preset.has_explicit_chat_templates() {
        compose_chat_messages_for_legacy(
            &render_template(&system_template, transcription),
            &render_template(&user_template, transcription),
        )
    } else {
        preset.prompt_template.replace("${output}", transcription)
    };
    debug!("Processed prompt length: {} chars", processed_prompt.len());

    let options = match post_process_chat_options(
        settings,
        &provider,
        None,
        PostProcessStructuredOutputMode::None,
        None,
    ) {
        Ok(options) => options,
        Err(error) => {
            return PostProcessAttempt::failed(
                started_at,
                Some(preset),
                Some(provider.id.clone()),
                Some(model),
                error,
            );
        }
    };

    match crate::llm_client::send_chat_completion(
        &provider,
        api_key,
        &model,
        processed_prompt,
        options,
    )
    .await
    {
        Ok(Some(content)) => {
            let content = strip_invisible_chars(&content);
            debug!(
                "LLM post-processing succeeded for provider '{}'. Output length: {} chars",
                provider.id,
                content.len()
            );
            if content.trim().is_empty() {
                PostProcessAttempt::failed(
                    started_at,
                    Some(preset),
                    Some(provider.id.clone()),
                    Some(model),
                    "Post-processing returned an empty response".to_string(),
                )
            } else {
                PostProcessAttempt::success(started_at, preset, provider.id.clone(), model, content)
            }
        }
        Ok(None) => {
            error!("LLM API response has no content");
            PostProcessAttempt::failed(
                started_at,
                Some(preset),
                Some(provider.id.clone()),
                Some(model),
                "Post-processing returned no content".to_string(),
            )
        }
        Err(e) => {
            let summary = sanitize_error_summary(&e, settings, Some(&prompt_snapshot));
            error!(
                "LLM post-processing failed for provider '{}': {}. Falling back to original transcription.",
                provider.id,
                summary
            );
            PostProcessAttempt::failed(
                started_at,
                Some(preset),
                Some(provider.id.clone()),
                Some(model),
                summary,
            )
        }
    }
}

async fn post_process_transcription(
    settings: &AppSettings,
    transcription: &str,
) -> PostProcessAttempt {
    let selected_at = Instant::now();
    let preset = match selected_post_process_preset(settings) {
        Ok(preset) => preset,
        Err(error) => {
            return PostProcessAttempt::failed(selected_at, None, None, None, error);
        }
    };

    run_post_process_preset(settings, &preset, transcription).await
}

pub(crate) async fn run_post_process_preset_preview(
    app: &AppHandle,
    preset_id: String,
    input_text: String,
) -> Result<PostProcessPreviewResult, String> {
    if input_text.trim().is_empty() {
        return Err("Sample text is required".to_string());
    }

    let settings = get_settings(app);
    let preset = settings
        .post_process_preset(&preset_id)
        .cloned()
        .ok_or_else(|| format!("Post-processing preset '{}' not found", preset_id))?;

    if !preset.enabled {
        return Err(format!(
            "Post-processing preset '{}' is disabled",
            preset.name
        ));
    }

    let result = run_post_process_preset(&settings, &preset, &input_text).await;
    Ok(result.to_preview_result(&preset))
}

fn active_local_asr_language(app: &AppHandle, settings: &AppSettings) -> String {
    if settings.asr_provider_id != BUILT_IN_LOCAL_ASR_PROVIDER_ID {
        return settings.selected_language.clone();
    }

    app.try_state::<Arc<ModelManager>>()
        .and_then(|model_manager| model_manager.get_model_info(&settings.selected_model))
        .map(|model_info| match model_info.engine_type {
            EngineType::Whisper => settings.asr_family_settings.whisper.language.clone(),
            EngineType::Qwen3Asr => settings.asr_family_settings.qwen3_asr.language.clone(),
            _ => settings.selected_language.clone(),
        })
        .unwrap_or_else(|| settings.selected_language.clone())
}

async fn maybe_convert_chinese_variant(language: &str, transcription: &str) -> Option<String> {
    // Check if language is set to Simplified or Traditional Chinese
    let is_simplified = language == "zh-Hans";
    let is_traditional = language == "zh-Hant";

    if !is_simplified && !is_traditional {
        debug!("selected_language is not Simplified or Traditional Chinese; skipping translation");
        return None;
    }

    debug!(
        "Starting Chinese translation using OpenCC for language: {}",
        language
    );

    // Use OpenCC to convert based on selected language
    let config = if is_simplified {
        // Convert Traditional Chinese to Simplified Chinese
        BuiltinConfig::Tw2sp
    } else {
        // Convert Simplified Chinese to Traditional Chinese
        BuiltinConfig::S2tw
    };

    match OpenCC::from_config(config) {
        Ok(converter) => {
            let converted = converter.convert(transcription);
            debug!(
                "OpenCC translation completed. Input length: {}, Output length: {}",
                transcription.len(),
                converted.len()
            );
            Some(converted)
        }
        Err(e) => {
            error!("Failed to initialize OpenCC converter: {}. Falling back to original transcription.", e);
            None
        }
    }
}

pub(crate) struct ProcessedTranscription {
    pub final_text: String,
    pub post_processed_text: Option<String>,
    pub post_process_prompt: Option<String>,
    pub post_process_preset_id: Option<String>,
    pub post_process_preset_version: Option<i64>,
    pub post_process_run: Option<NewPostProcessRun>,
}

pub(crate) async fn process_transcription_output(
    app: &AppHandle,
    transcription: &str,
    post_process: bool,
) -> ProcessedTranscription {
    let settings = get_settings(app);
    let mut final_text = transcription.to_string();
    let mut post_processed_text: Option<String> = None;
    let mut post_process_prompt: Option<String> = None;
    let mut post_process_preset_id: Option<String> = None;
    let mut post_process_preset_version: Option<i64> = None;
    let mut post_process_run: Option<NewPostProcessRun> = None;
    let transcription_language = active_local_asr_language(app, &settings);

    if let Some(converted_text) =
        maybe_convert_chinese_variant(&transcription_language, transcription).await
    {
        final_text = converted_text;
    }

    if post_process {
        let post_process_input = final_text.clone();
        let attempt = post_process_transcription(&settings, &final_text).await;
        post_process_prompt = attempt.prompt_template.clone();
        post_process_preset_id = attempt.preset_id.clone();
        post_process_preset_version = attempt.preset_version;
        post_process_run = Some(attempt.to_run(&post_process_input));

        if let Some(processed_text) = attempt.processed_text {
            post_processed_text = Some(processed_text.clone());
            final_text = processed_text;
        } else {
            let _ = app.emit(
                "post-process-fallback",
                PostProcessFallbackEvent {
                    error_summary: attempt.error_summary,
                },
            );
        }
    } else if final_text != transcription {
        post_processed_text = Some(final_text.clone());
    }

    ProcessedTranscription {
        final_text,
        post_processed_text,
        post_process_prompt,
        post_process_preset_id,
        post_process_preset_version,
        post_process_run,
    }
}

impl ShortcutAction for TranscribeAction {
    fn start(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        let start_time = Instant::now();
        debug!("TranscribeAction::start called for binding: {}", binding_id);

        // Load model in the background
        let tm = app.state::<Arc<TranscriptionManager>>();
        let rm = app.state::<Arc<AudioRecordingManager>>();
        let is_realtime_asr = crate::asr::is_active_asr_provider_realtime(app);
        let mut recording_error: Option<String> = None;

        // Load ASR model and VAD model in parallel when the active provider is local.
        if crate::asr::is_active_asr_provider_local(app) {
            tm.initiate_model_load();
        }
        let rm_clone = Arc::clone(&rm);
        std::thread::spawn(move || {
            if let Err(e) = rm_clone.preload_vad() {
                debug!("VAD pre-load failed: {}", e);
            }
        });

        let binding_id = binding_id.to_string();
        change_tray_icon(app, TrayIconState::Recording);
        if is_realtime_asr {
            match crate::asr::start_active_realtime_session(app) {
                Ok(()) => {
                    rm.set_recording_chunk_callback(Some(Arc::new(|samples| {
                        crate::asr::append_active_realtime_audio(samples);
                    })));
                    show_realtime_overlay(app);
                }
                Err(err) => {
                    recording_error = Some(err.to_string());
                    rm.set_recording_chunk_callback(None);
                    show_recording_overlay(app);
                }
            }
        } else {
            rm.set_recording_chunk_callback(None);
            show_recording_overlay(app);
        }

        // Get the microphone mode to determine audio feedback timing
        let settings = get_settings(app);
        let is_always_on = settings.always_on_microphone;
        debug!("Microphone mode - always_on: {}", is_always_on);

        if recording_error.is_none() && is_always_on {
            // Always-on mode: Play audio feedback immediately, then apply mute after sound finishes
            debug!("Always-on mode: Playing audio feedback immediately");
            let rm_clone = Arc::clone(&rm);
            let app_clone = app.clone();
            // The blocking helper exits immediately if audio feedback is disabled,
            // so we can always reuse this thread to ensure mute happens right after playback.
            std::thread::spawn(move || {
                play_feedback_sound_blocking(&app_clone, SoundType::Start);
                rm_clone.apply_mute();
            });

            if let Err(e) = rm.try_start_recording(&binding_id) {
                debug!("Recording failed: {}", e);
                recording_error = Some(e);
            }
        } else if recording_error.is_none() {
            // On-demand mode: Start recording first, then play audio feedback, then apply mute
            // This allows the microphone to be activated before playing the sound
            debug!("On-demand mode: Starting recording first, then audio feedback");
            let recording_start_time = Instant::now();
            match rm.try_start_recording(&binding_id) {
                Ok(()) => {
                    debug!("Recording started in {:?}", recording_start_time.elapsed());
                    // Small delay to ensure microphone stream is active
                    let app_clone = app.clone();
                    let rm_clone = Arc::clone(&rm);
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        debug!("Handling delayed audio feedback/mute sequence");
                        // Helper handles disabled audio feedback by returning early, so we reuse it
                        // to keep mute sequencing consistent in every mode.
                        play_feedback_sound_blocking(&app_clone, SoundType::Start);
                        rm_clone.apply_mute();
                    });
                }
                Err(e) => {
                    debug!("Failed to start recording: {}", e);
                    recording_error = Some(e);
                }
            }
        }

        if recording_error.is_none() {
            // Dynamically register the cancel shortcut in a separate task to avoid deadlock
            shortcut::register_cancel_shortcut(app);
        } else {
            // Starting failed (for example due to blocked microphone permissions).
            // Revert UI state so we don't stay stuck in the recording overlay.
            rm.set_recording_chunk_callback(None);
            crate::asr::cancel_active_realtime_session();
            utils::hide_recording_overlay(app);
            change_tray_icon(app, TrayIconState::Idle);
            if let Some(err) = recording_error {
                let error_type = if is_microphone_access_denied(&err) {
                    "microphone_permission_denied"
                } else if is_no_input_device_error(&err) {
                    "no_input_device"
                } else {
                    "unknown"
                };
                let _ = app.emit(
                    "recording-error",
                    RecordingErrorEvent {
                        error_type: error_type.to_string(),
                        detail: Some(err),
                    },
                );
            }
        }

        debug!(
            "TranscribeAction::start completed in {:?}",
            start_time.elapsed()
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        // Unregister the cancel shortcut when transcription stops
        shortcut::unregister_cancel_shortcut(app);

        let stop_time = Instant::now();
        debug!("TranscribeAction::stop called for binding: {}", binding_id);

        let ah = app.clone();
        let rm = Arc::clone(&app.state::<Arc<AudioRecordingManager>>());
        let tm = Arc::clone(&app.state::<Arc<TranscriptionManager>>());
        let hm = Arc::clone(&app.state::<Arc<HistoryManager>>());

        change_tray_icon(app, TrayIconState::Transcribing);
        show_transcribing_overlay(app);

        // Unmute before playing audio feedback so the stop sound is audible
        rm.remove_mute();

        // Play audio feedback for recording stop
        play_feedback_sound(app, SoundType::Stop);

        let binding_id = binding_id.to_string(); // Clone binding_id for the async task
        let post_process = self.post_process;

        tauri::async_runtime::spawn(async move {
            let _guard = FinishGuard(ah.clone());
            debug!(
                "Starting async transcription task for binding: {}",
                binding_id
            );

            let stop_recording_time = Instant::now();
            let realtime_asr = crate::asr::has_active_realtime_session();
            let samples_result: Option<Vec<f32>> = if realtime_asr {
                rm.stop_recording_with_raw(&binding_id)
                    .map(|recording| recording.raw_samples)
            } else {
                rm.stop_recording(&binding_id)
            };
            rm.set_recording_chunk_callback(None);

            if let Some(samples) = samples_result {
                debug!(
                    "Recording stopped and samples retrieved in {:?}, sample count: {}",
                    stop_recording_time.elapsed(),
                    samples.len()
                );

                if samples.is_empty() {
                    debug!("Recording produced no audio samples; skipping persistence");
                    if realtime_asr {
                        crate::asr::cancel_active_realtime_session();
                    }
                    utils::hide_recording_overlay(&ah);
                    change_tray_icon(&ah, TrayIconState::Idle);
                } else {
                    // Save WAV concurrently with transcription
                    let sample_count = samples.len();
                    let file_name = format!("speakmore-{}.wav", chrono::Utc::now().timestamp());
                    let wav_path = hm.recordings_dir().join(&file_name);
                    let wav_path_for_verify = wav_path.clone();
                    let samples_for_wav = samples.clone();
                    let wav_handle = tauri::async_runtime::spawn_blocking(move || {
                        crate::audio_toolkit::save_wav_file(&wav_path, &samples_for_wav)
                    });

                    // Transcribe concurrently with WAV save
                    let transcription_time = Instant::now();
                    let transcription_result = if realtime_asr {
                        crate::asr::finish_active_realtime_session().await
                    } else {
                        crate::asr::transcribe_with_active_provider(&ah, &tm, samples).await
                    };

                    // Await WAV save and verify
                    let wav_saved = match wav_handle.await {
                        Ok(Ok(())) => {
                            match crate::audio_toolkit::verify_wav_file(
                                &wav_path_for_verify,
                                sample_count,
                            ) {
                                Ok(()) => true,
                                Err(e) => {
                                    error!("WAV verification failed: {}", e);
                                    false
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            error!("Failed to save WAV file: {}", e);
                            false
                        }
                        Err(e) => {
                            error!("WAV save task panicked: {}", e);
                            false
                        }
                    };

                    match transcription_result {
                        Ok(asr_result) => {
                            let transcription_latency_ms = elapsed_ms(transcription_time);
                            let transcription = asr_result.text;
                            let asr_metadata = AsrHistoryMetadata {
                                provider_id: asr_result.provider_id,
                                model: asr_result.model,
                                language: asr_result.language,
                            };
                            debug!(
                                "Transcription completed in {:?}: '{}'",
                                transcription_time.elapsed(),
                                transcription
                            );

                            if post_process {
                                show_processing_overlay(&ah);
                            }
                            let processed =
                                process_transcription_output(&ah, &transcription, post_process)
                                    .await;

                            let transcription_run_status = if transcription.trim().is_empty() {
                                TRANSCRIPTION_RUN_STATUS_EMPTY
                            } else {
                                TRANSCRIPTION_RUN_STATUS_SUCCESS
                            };
                            let history_status = if transcription.trim().is_empty() {
                                HISTORY_STATUS_EMPTY
                            } else {
                                HISTORY_STATUS_COMPLETED
                            };
                            let mut history_entry_id: Option<i64> = None;
                            let mut transcription_run_id: Option<i64> = None;
                            let mut post_process_run_id: Option<i64> = None;
                            let post_process_produced_output =
                                processed.post_processed_text.is_some();

                            // Save to history if WAV was saved
                            if wav_saved {
                                match hm.save_entry(
                                    file_name,
                                    transcription.clone(),
                                    post_process,
                                    processed.post_processed_text.clone(),
                                    processed.post_process_prompt.clone(),
                                    processed.post_process_preset_id.clone(),
                                    processed.post_process_preset_version,
                                    Some(asr_metadata.clone()),
                                    history_status.to_string(),
                                ) {
                                    Ok(entry) => {
                                        history_entry_id = Some(entry.id);
                                        match hm.save_transcription_run(
                                            entry.id,
                                            transcription_run_from_result(
                                                &asr_metadata,
                                                transcription_run_status,
                                                transcription.clone(),
                                                transcription_latency_ms,
                                                None,
                                            ),
                                            history_status.to_string(),
                                        ) {
                                            Ok(run) => transcription_run_id = Some(run.id),
                                            Err(err) => {
                                                error!("Failed to save transcription run: {}", err)
                                            }
                                        }
                                        if let Some(run) = processed.post_process_run.clone() {
                                            match hm.save_post_process_run(entry.id, run) {
                                                Ok(saved_run) => {
                                                    post_process_run_id = Some(saved_run.id);
                                                    if saved_run.status
                                                        == POST_PROCESS_STATUS_FAILED
                                                    {
                                                        if let Err(err) = hm.record_history_event(
                                                            entry.id,
                                                            NewHistoryEvent {
                                                                run_type: Some(
                                                                    HISTORY_RUN_TYPE_POST_PROCESS
                                                                        .to_string(),
                                                                ),
                                                                run_id: Some(saved_run.id),
                                                                event_type:
                                                                    HISTORY_EVENT_POST_PROCESS_FALLBACK
                                                                        .to_string(),
                                                                source:
                                                                    HISTORY_EVENT_SOURCE_BACKEND
                                                                        .to_string(),
                                                                payload_json: saved_run
                                                                    .error_summary
                                                                    .as_ref()
                                                                    .map(|error| {
                                                                        serde_json::json!({
                                                                            "error_summary": error
                                                                        })
                                                                        .to_string()
                                                                    }),
                                                            },
                                                        ) {
                                                            error!(
                                                                "Failed to save post-process fallback event: {}",
                                                                err
                                                            );
                                                        }
                                                    }
                                                }
                                                Err(err) => {
                                                    error!(
                                                        "Failed to save post-process run: {}",
                                                        err
                                                    )
                                                }
                                            }
                                        }
                                    }
                                    Err(err) => error!("Failed to save history entry: {}", err),
                                }
                            }

                            if processed.final_text.is_empty() {
                                utils::hide_recording_overlay(&ah);
                                change_tray_icon(&ah, TrayIconState::Idle);
                            } else {
                                let ah_clone = ah.clone();
                                let hm_for_paste = Arc::clone(&hm);
                                let paste_time = Instant::now();
                                let final_text = processed.final_text;
                                let paste_history_entry_id = history_entry_id;
                                let paste_run_type = if post_process_produced_output {
                                    Some(HISTORY_RUN_TYPE_POST_PROCESS.to_string())
                                } else {
                                    Some(HISTORY_RUN_TYPE_TRANSCRIPTION.to_string())
                                };
                                let paste_run_id = if post_process_produced_output {
                                    post_process_run_id
                                } else {
                                    transcription_run_id
                                };
                                ah.run_on_main_thread(move || {
                                    match utils::paste(final_text, ah_clone.clone()) {
                                        Ok(()) => {
                                            debug!(
                                                "Text pasted successfully in {:?}",
                                                paste_time.elapsed()
                                            );
                                            if let Some(entry_id) = paste_history_entry_id {
                                                if let Err(err) = hm_for_paste.record_history_event(
                                                    entry_id,
                                                    NewHistoryEvent {
                                                        run_type: paste_run_type.clone(),
                                                        run_id: paste_run_id,
                                                        event_type: HISTORY_EVENT_PASTE_SUCCEEDED
                                                            .to_string(),
                                                        source: HISTORY_EVENT_SOURCE_BACKEND
                                                            .to_string(),
                                                        payload_json: None,
                                                    },
                                                ) {
                                                    error!(
                                                        "Failed to save paste success event: {}",
                                                        err
                                                    );
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            error!("Failed to paste transcription: {}", e);
                                            if let Some(entry_id) = paste_history_entry_id {
                                                if let Err(err) = hm_for_paste.record_history_event(
                                                    entry_id,
                                                    NewHistoryEvent {
                                                        run_type: paste_run_type,
                                                        run_id: paste_run_id,
                                                        event_type: HISTORY_EVENT_PASTE_FAILED
                                                            .to_string(),
                                                        source: HISTORY_EVENT_SOURCE_BACKEND
                                                            .to_string(),
                                                        payload_json: Some(
                                                            serde_json::json!({
                                                                "error": e.to_string()
                                                            })
                                                            .to_string(),
                                                        ),
                                                    },
                                                ) {
                                                    error!(
                                                        "Failed to save paste failure event: {}",
                                                        err
                                                    );
                                                }
                                            }
                                            let _ = ah_clone.emit("paste-error", ());
                                        }
                                    }
                                    utils::hide_recording_overlay(&ah_clone);
                                    change_tray_icon(&ah_clone, TrayIconState::Idle);
                                })
                                .unwrap_or_else(|e| {
                                    error!("Failed to run paste on main thread: {:?}", e);
                                    utils::hide_recording_overlay(&ah);
                                    change_tray_icon(&ah, TrayIconState::Idle);
                                });
                            }
                        }
                        Err(err) => {
                            let transcription_latency_ms = elapsed_ms(transcription_time);
                            debug!("Global Shortcut Transcription error: {}", err);
                            let failed_asr_metadata = crate::asr::active_provider_metadata(&ah)
                                .map(|metadata| AsrHistoryMetadata {
                                    provider_id: metadata.provider_id,
                                    model: metadata.model,
                                    language: metadata.language,
                                });
                            let _ = ah.emit(
                                "recording-error",
                                RecordingErrorEvent {
                                    error_type: "unknown".to_string(),
                                    detail: Some(err.to_string()),
                                },
                            );
                            // Save entry with empty text so user can retry
                            if wav_saved {
                                match hm.save_entry(
                                    file_name,
                                    String::new(),
                                    post_process,
                                    None,
                                    None,
                                    None,
                                    None,
                                    failed_asr_metadata.clone(),
                                    HISTORY_STATUS_FAILED.to_string(),
                                ) {
                                    Ok(entry) => {
                                        let settings = get_settings(&ah);
                                        let error_summary = sanitize_error_summary(
                                            &err.to_string(),
                                            &settings,
                                            None,
                                        );
                                        if let Err(run_err) = hm.save_transcription_run(
                                            entry.id,
                                            transcription_run_from_metadata(
                                                failed_asr_metadata.as_ref(),
                                                TRANSCRIPTION_RUN_STATUS_FAILED,
                                                String::new(),
                                                transcription_latency_ms,
                                                Some(error_summary),
                                            ),
                                            HISTORY_STATUS_FAILED.to_string(),
                                        ) {
                                            error!(
                                                "Failed to save failed transcription run: {}",
                                                run_err
                                            );
                                        }
                                    }
                                    Err(save_err) => {
                                        error!("Failed to save failed history entry: {}", save_err)
                                    }
                                }
                            }
                            utils::hide_recording_overlay(&ah);
                            change_tray_icon(&ah, TrayIconState::Idle);
                        }
                    }
                }
            } else {
                debug!("No samples retrieved from recording stop");
                if realtime_asr {
                    crate::asr::cancel_active_realtime_session();
                }
                utils::hide_recording_overlay(&ah);
                change_tray_icon(&ah, TrayIconState::Idle);
            }
        });

        debug!(
            "TranscribeAction::stop completed in {:?}",
            stop_time.elapsed()
        );
    }
}

// Cancel Action
struct CancelAction;

impl ShortcutAction for CancelAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        utils::cancel_current_operation(app);
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        // Nothing to do on stop for cancel
    }
}

// Test Action
struct TestAction;

impl ShortcutAction for TestAction {
    fn start(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str) {
        log::info!(
            "Shortcut ID '{}': Started - {} (App: {})", // Changed "Pressed" to "Started" for consistency
            binding_id,
            shortcut_str,
            app.package_info().name
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str) {
        log::info!(
            "Shortcut ID '{}': Stopped - {} (App: {})", // Changed "Released" to "Stopped" for consistency
            binding_id,
            shortcut_str,
            app.package_info().name
        );
    }
}

// Static Action Map
pub static ACTION_MAP: Lazy<HashMap<String, Arc<dyn ShortcutAction>>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert(
        "transcribe".to_string(),
        Arc::new(TranscribeAction {
            post_process: false,
        }) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "transcribe_with_post_process".to_string(),
        Arc::new(TranscribeAction { post_process: true }) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "cancel".to_string(),
        Arc::new(CancelAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "test".to_string(),
        Arc::new(TestAction) as Arc<dyn ShortcutAction>,
    );
    map
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{get_default_settings, CLEAN_DICTATION_PRESET_ID, DEEPSEEK_PROVIDER_ID};

    #[test]
    fn sanitize_error_summary_redacts_keys_and_prompt() {
        let mut settings = get_default_settings();
        settings
            .post_process_api_keys
            .insert("openai".to_string(), "redaction-test-key".to_string());
        let prompt = "Clean this transcript: ${output}";
        let error = "API error for redaction-test-key with prompt Clean this transcript: ${output}";

        let summary = sanitize_error_summary(error, &settings, Some(prompt));

        assert!(!summary.contains("redaction-test-key"));
        assert!(!summary.contains(prompt));
        assert!(summary.contains("[REDACTED]"));
        assert!(summary.contains("[PROMPT]"));
    }

    #[tokio::test]
    async fn post_process_missing_key_returns_failed_attempt() {
        let mut settings = get_default_settings();
        settings.post_process_provider_id = "openai".to_string();
        settings
            .post_process_models
            .insert("openai".to_string(), "gpt-test".to_string());
        settings
            .post_process_api_keys
            .insert("openai".to_string(), String::new());
        let preset = settings
            .post_process_preset(CLEAN_DICTATION_PRESET_ID)
            .cloned()
            .expect("clean preset exists");

        let attempt = run_post_process_preset(&settings, &preset, "hello").await;

        assert_eq!(attempt.status, POST_PROCESS_STATUS_FAILED);
        assert_eq!(
            attempt.preset_id.as_deref(),
            Some(CLEAN_DICTATION_PRESET_ID)
        );
        assert_eq!(attempt.provider_id.as_deref(), Some("openai"));
        assert_eq!(attempt.model.as_deref(), Some("gpt-test"));
        assert!(attempt
            .error_summary
            .as_deref()
            .unwrap_or_default()
            .contains("API key is required"));
    }

    #[tokio::test]
    async fn post_process_missing_model_returns_failed_attempt() {
        let mut settings = get_default_settings();
        settings.post_process_provider_id = "openai".to_string();
        settings
            .post_process_models
            .insert("openai".to_string(), String::new());
        let preset = settings
            .post_process_preset(CLEAN_DICTATION_PRESET_ID)
            .cloned()
            .expect("clean preset exists");

        let attempt = run_post_process_preset(&settings, &preset, "hello").await;

        assert_eq!(attempt.status, POST_PROCESS_STATUS_FAILED);
        assert_eq!(attempt.provider_id.as_deref(), Some("openai"));
        assert!(attempt
            .error_summary
            .as_deref()
            .unwrap_or_default()
            .contains("No model is configured"));
    }

    #[test]
    fn custom_provider_options_do_not_send_reasoning_fields() {
        let settings = get_default_settings();
        let provider = settings
            .post_process_provider("custom")
            .expect("custom provider exists");
        let options = post_process_chat_options(
            &settings,
            provider,
            None,
            PostProcessStructuredOutputMode::None,
            None,
        )
        .expect("custom provider options build");
        let body = crate::llm_client::build_chat_completion_request_json(
            "model",
            "hello".to_string(),
            options,
        );

        assert!(body.get("reasoning_effort").is_none());
        assert!(body.get("reasoning").is_none());
        assert!(body.get("thinking").is_none());
    }

    #[test]
    fn deepseek_disabled_thinking_omits_reasoning_effort() {
        let settings = get_default_settings();
        let provider = settings
            .post_process_provider(DEEPSEEK_PROVIDER_ID)
            .expect("deepseek provider exists");
        let options = post_process_chat_options(
            &settings,
            provider,
            None,
            PostProcessStructuredOutputMode::None,
            None,
        )
        .expect("deepseek disabled options build");
        let body = crate::llm_client::build_chat_completion_request_json(
            "model",
            "hello".to_string(),
            options,
        );

        assert_eq!(body["thinking"]["type"], "disabled");
        assert!(body.get("reasoning_effort").is_none());
    }

    #[test]
    fn deepseek_high_thinking_sends_supported_reasoning_effort() {
        let mut settings = get_default_settings();
        settings
            .post_process_reasoning_efforts
            .insert(DEEPSEEK_PROVIDER_ID.to_string(), "high".to_string());
        let provider = settings
            .post_process_provider(DEEPSEEK_PROVIDER_ID)
            .expect("deepseek provider exists");
        let options = post_process_chat_options(
            &settings,
            provider,
            None,
            PostProcessStructuredOutputMode::None,
            None,
        )
        .expect("deepseek high options build");
        let body = crate::llm_client::build_chat_completion_request_json(
            "model",
            "hello".to_string(),
            options,
        );

        assert_eq!(body["thinking"]["type"], "enabled");
        assert_eq!(body["reasoning_effort"], "high");
    }

    #[test]
    fn deepseek_json_object_output_does_not_send_json_schema() {
        let settings = get_default_settings();
        let provider = settings
            .post_process_provider(DEEPSEEK_PROVIDER_ID)
            .expect("deepseek provider exists");
        let options = post_process_chat_options(
            &settings,
            provider,
            Some("Return JSON".to_string()),
            PostProcessStructuredOutputMode::JsonObject,
            Some(serde_json::json!({"type": "object"})),
        )
        .expect("deepseek json object options build");
        let body = crate::llm_client::build_chat_completion_request_json(
            "model",
            "hello".to_string(),
            options,
        );

        assert_eq!(body["response_format"]["type"], "json_object");
        assert!(body["response_format"].get("json_schema").is_none());
    }
}

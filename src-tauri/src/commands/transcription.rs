use crate::managers::transcription::TranscriptionManager;
use crate::settings::{
    get_settings, write_settings, AppSettings, AsrProviderKind, ModelUnloadTimeout,
    ALIYUN_QWEN3_ASR_PROVIDER_ID, ALIYUN_QWEN3_ASR_REALTIME_PROVIDER_ID,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, State};

const DASHSCOPE_API_KEY_ENV: &str = "DASHSCOPE_API_KEY";

#[derive(Serialize, Type)]
pub struct ModelLoadStatus {
    is_loaded: bool,
    current_model: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum AsrApiKeySource {
    NotRequired,
    Settings,
    Environment,
    Missing,
}

#[derive(Serialize, Debug, Clone, Type)]
pub struct AsrProviderStatus {
    pub provider_id: String,
    pub configured: bool,
    pub api_key_source: AsrApiKeySource,
    pub model: String,
    pub error: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub fn set_model_unload_timeout(app: AppHandle, timeout: ModelUnloadTimeout) {
    let mut settings = get_settings(&app);
    settings.model_unload_timeout = timeout;
    write_settings(&app, settings);
}

#[tauri::command]
#[specta::specta]
pub fn get_model_load_status(
    transcription_manager: State<TranscriptionManager>,
) -> Result<ModelLoadStatus, String> {
    Ok(ModelLoadStatus {
        is_loaded: transcription_manager.is_model_loaded(),
        current_model: transcription_manager.get_current_model(),
    })
}

#[tauri::command]
#[specta::specta]
pub fn unload_model_manually(
    transcription_manager: State<TranscriptionManager>,
) -> Result<(), String> {
    transcription_manager
        .unload_model()
        .map_err(|e| format!("Failed to unload model: {}", e))
}

#[tauri::command]
#[specta::specta]
pub fn set_asr_provider(app: AppHandle, provider_id: String) -> Result<(), String> {
    let mut settings = get_settings(&app);
    if settings.asr_provider(&provider_id).is_none() {
        return Err(format!("Unknown ASR provider: {}", provider_id));
    }

    settings.asr_provider_id = provider_id;
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn apply_transcription_profile(app: AppHandle, profile_id: String) -> Result<(), String> {
    let mut settings = get_settings(&app);
    let profile = settings
        .transcription_profile(&profile_id)
        .cloned()
        .ok_or_else(|| format!("Unknown transcription profile: {}", profile_id))?;

    if settings.asr_provider(&profile.asr_provider_id).is_none() {
        return Err(format!(
            "Profile '{}' references an unknown ASR provider",
            profile_id
        ));
    }

    if profile.post_process_enabled {
        let preset_id = profile.post_process_preset_id.as_deref().ok_or_else(|| {
            format!(
                "Profile '{}' does not specify a post-process preset",
                profile_id
            )
        })?;
        if settings.post_process_preset(preset_id).is_none() {
            return Err(format!(
                "Profile '{}' references an unknown post-process preset",
                profile_id
            ));
        }
    }

    settings.asr_provider_id = profile.asr_provider_id.clone();
    if let Some(model) = profile
        .asr_model
        .as_deref()
        .map(str::trim)
        .filter(|model| !model.is_empty())
    {
        settings
            .asr_models
            .insert(profile.asr_provider_id.clone(), model.to_string());
    }
    settings.selected_language = profile.language.clone();
    settings.translate_to_english = profile.translate_to_english;
    settings.asr_family_settings.whisper.language = profile.language.clone();
    settings.asr_family_settings.whisper.translate_to_english = profile.translate_to_english;
    settings.post_process_enabled = profile.post_process_enabled;
    if profile.post_process_enabled {
        settings.post_process_selected_preset_id = profile.post_process_preset_id.clone();
        settings.post_process_selected_prompt_id = profile.post_process_preset_id.clone();
    }
    settings.selected_transcription_profile_id = Some(profile.id.clone());

    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_asr_api_key(
    app: AppHandle,
    provider_id: String,
    api_key: String,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    if settings.asr_provider(&provider_id).is_none() {
        return Err(format!("Unknown ASR provider: {}", provider_id));
    }

    settings
        .asr_api_keys
        .insert(provider_id, api_key.trim().to_string());
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_asr_model(app: AppHandle, provider_id: String, model: String) -> Result<(), String> {
    let mut settings = get_settings(&app);
    if settings.asr_provider(&provider_id).is_none() {
        return Err(format!("Unknown ASR provider: {}", provider_id));
    }

    settings
        .asr_models
        .insert(provider_id, model.trim().to_string());
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn get_asr_provider_status(
    app: AppHandle,
    provider_id: String,
) -> Result<AsrProviderStatus, String> {
    let settings = get_settings(&app);
    asr_provider_status_from_settings(
        &settings,
        &provider_id,
        std::env::var(DASHSCOPE_API_KEY_ENV).ok().as_deref(),
    )
}

fn asr_provider_status_from_settings(
    settings: &AppSettings,
    provider_id: &str,
    env_api_key: Option<&str>,
) -> Result<AsrProviderStatus, String> {
    let provider = settings
        .asr_provider(provider_id)
        .ok_or_else(|| format!("Unknown ASR provider: {}", provider_id))?;
    let model = settings
        .asr_models
        .get(provider_id)
        .cloned()
        .unwrap_or_default();

    match provider.kind {
        AsrProviderKind::BuiltInLocal => Ok(AsrProviderStatus {
            provider_id: provider_id.to_string(),
            configured: true,
            api_key_source: AsrApiKeySource::NotRequired,
            model,
            error: None,
        }),
        AsrProviderKind::AliyunQwen3AsrFlash | AsrProviderKind::AliyunQwen3AsrRealtime => {
            let api_key_source =
                resolve_dashscope_api_key_source(settings, provider_id, env_api_key);
            let configured = api_key_source != AsrApiKeySource::Missing;

            Ok(AsrProviderStatus {
                provider_id: provider_id.to_string(),
                configured,
                api_key_source,
                model,
                error: if configured {
                    None
                } else {
                    Some("DashScope API key is not configured".to_string())
                },
            })
        }
    }
}

fn resolve_dashscope_api_key_source(
    settings: &AppSettings,
    provider_id: &str,
    env_api_key: Option<&str>,
) -> AsrApiKeySource {
    let has_settings_key = |id: &str| {
        settings
            .asr_api_keys
            .get(id)
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
    };

    if has_settings_key(provider_id)
        || (provider_id == ALIYUN_QWEN3_ASR_REALTIME_PROVIDER_ID
            && has_settings_key(ALIYUN_QWEN3_ASR_PROVIDER_ID))
    {
        return AsrApiKeySource::Settings;
    }

    if env_api_key
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
    {
        return AsrApiKeySource::Environment;
    }

    AsrApiKeySource::Missing
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{
        get_default_settings, ALIYUN_QWEN3_ASR_DEFAULT_MODEL, ALIYUN_QWEN3_ASR_PROVIDER_ID,
        ALIYUN_QWEN3_ASR_REALTIME_DEFAULT_MODEL, ALIYUN_QWEN3_ASR_REALTIME_PROVIDER_ID,
        BUILT_IN_LOCAL_ASR_PROVIDER_ID,
    };

    #[test]
    fn built_in_asr_status_is_configured_without_key() {
        let settings = get_default_settings();

        let status =
            asr_provider_status_from_settings(&settings, BUILT_IN_LOCAL_ASR_PROVIDER_ID, None)
                .expect("local provider status");

        assert!(status.configured);
        assert_eq!(status.api_key_source, AsrApiKeySource::NotRequired);
        assert!(status.error.is_none());
    }

    #[test]
    fn aliyun_asr_status_reports_missing_key() {
        let settings = get_default_settings();

        let status =
            asr_provider_status_from_settings(&settings, ALIYUN_QWEN3_ASR_PROVIDER_ID, None)
                .expect("aliyun provider status");

        assert!(!status.configured);
        assert_eq!(status.api_key_source, AsrApiKeySource::Missing);
        assert_eq!(status.model, ALIYUN_QWEN3_ASR_DEFAULT_MODEL);
        assert!(status.error.is_some());
    }

    #[test]
    fn aliyun_asr_status_prefers_settings_key() {
        let mut settings = get_default_settings();
        settings.asr_api_keys.insert(
            ALIYUN_QWEN3_ASR_PROVIDER_ID.to_string(),
            "settings-secret".to_string(),
        );

        let status = asr_provider_status_from_settings(
            &settings,
            ALIYUN_QWEN3_ASR_PROVIDER_ID,
            Some("env-secret"),
        )
        .expect("aliyun provider status");

        assert!(status.configured);
        assert_eq!(status.api_key_source, AsrApiKeySource::Settings);
        assert!(!format!("{:?}", status).contains("settings-secret"));
        assert!(!format!("{:?}", status).contains("env-secret"));
    }

    #[test]
    fn aliyun_asr_status_uses_environment_key() {
        let settings = get_default_settings();

        let status = asr_provider_status_from_settings(
            &settings,
            ALIYUN_QWEN3_ASR_PROVIDER_ID,
            Some("env-secret"),
        )
        .expect("aliyun provider status");

        assert!(status.configured);
        assert_eq!(status.api_key_source, AsrApiKeySource::Environment);
        assert!(status.error.is_none());
    }

    #[test]
    fn aliyun_realtime_asr_status_reuses_batch_key() {
        let mut settings = get_default_settings();
        settings.asr_api_keys.insert(
            ALIYUN_QWEN3_ASR_PROVIDER_ID.to_string(),
            "batch-secret".to_string(),
        );

        let status = asr_provider_status_from_settings(
            &settings,
            ALIYUN_QWEN3_ASR_REALTIME_PROVIDER_ID,
            Some("env-secret"),
        )
        .expect("aliyun realtime provider status");

        assert!(status.configured);
        assert_eq!(status.api_key_source, AsrApiKeySource::Settings);
        assert_eq!(status.model, ALIYUN_QWEN3_ASR_REALTIME_DEFAULT_MODEL);
        assert!(status.error.is_none());
        assert!(!format!("{:?}", status).contains("batch-secret"));
    }
}

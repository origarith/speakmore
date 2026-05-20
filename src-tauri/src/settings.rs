use log::{debug, warn};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use specta::Type;
use std::collections::{hash_map::Entry, HashMap};
use std::fmt;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

pub const APPLE_INTELLIGENCE_PROVIDER_ID: &str = "apple_intelligence";
pub const APPLE_INTELLIGENCE_DEFAULT_MODEL_ID: &str = "Apple Intelligence";
pub const DEEPSEEK_PROVIDER_ID: &str = "deepseek";
pub const BUILT_IN_LOCAL_ASR_PROVIDER_ID: &str = "built_in_local";
pub const ALIYUN_QWEN3_ASR_PROVIDER_ID: &str = "aliyun_qwen3_asr_flash";
pub const ALIYUN_QWEN3_ASR_DEFAULT_MODEL: &str = "qwen3-asr-flash";
pub const ALIYUN_QWEN3_ASR_REALTIME_PROVIDER_ID: &str = "aliyun_qwen3_asr_realtime";
pub const ALIYUN_QWEN3_ASR_REALTIME_DEFAULT_MODEL: &str = "qwen3-asr-flash-realtime-2026-02-10";
pub const ALIYUN_QWEN3_ASR_REALTIME_DEFAULT_BASE_URL: &str =
    "wss://dashscope.aliyuncs.com/api-ws/v1/realtime";
pub const CLEAN_DICTATION_PRESET_ID: &str = "clean_dictation";
pub const DEFAULT_TRANSCRIPTION_PROFILE_ID: &str = "quick_input_local";
const LEGACY_QWEN3_ASR_MAX_NEW_TOKENS_DEFAULT: u32 = 128;
pub const DEFAULT_QWEN3_ASR_MAX_NEW_TOKENS: u32 = 384;
pub const QWEN3_ASR_MAX_NEW_TOKENS_LIMIT: u32 = 512;
pub const DEFAULT_QWEN3_ASR_MAX_TOTAL_LEN: u32 = 1024;
pub const QWEN3_ASR_MAX_TOTAL_LEN_LIMIT: u32 = 2048;

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

// Custom deserializer to handle both old numeric format (1-5) and new string format ("trace", "debug", etc.)
impl<'de> Deserialize<'de> for LogLevel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LogLevelVisitor;

        impl<'de> Visitor<'de> for LogLevelVisitor {
            type Value = LogLevel;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or integer representing log level")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<LogLevel, E> {
                match value.to_lowercase().as_str() {
                    "trace" => Ok(LogLevel::Trace),
                    "debug" => Ok(LogLevel::Debug),
                    "info" => Ok(LogLevel::Info),
                    "warn" => Ok(LogLevel::Warn),
                    "error" => Ok(LogLevel::Error),
                    _ => Err(E::unknown_variant(
                        value,
                        &["trace", "debug", "info", "warn", "error"],
                    )),
                }
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<LogLevel, E> {
                match value {
                    1 => Ok(LogLevel::Trace),
                    2 => Ok(LogLevel::Debug),
                    3 => Ok(LogLevel::Info),
                    4 => Ok(LogLevel::Warn),
                    5 => Ok(LogLevel::Error),
                    _ => Err(E::invalid_value(de::Unexpected::Unsigned(value), &"1-5")),
                }
            }
        }

        deserializer.deserialize_any(LogLevelVisitor)
    }
}

impl From<LogLevel> for tauri_plugin_log::LogLevel {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => tauri_plugin_log::LogLevel::Trace,
            LogLevel::Debug => tauri_plugin_log::LogLevel::Debug,
            LogLevel::Info => tauri_plugin_log::LogLevel::Info,
            LogLevel::Warn => tauri_plugin_log::LogLevel::Warn,
            LogLevel::Error => tauri_plugin_log::LogLevel::Error,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct ShortcutBinding {
    pub id: String,
    pub name: String,
    pub description: String,
    pub default_binding: String,
    pub current_binding: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct LLMPrompt {
    pub id: String,
    pub name: String,
    pub prompt: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Type)]
pub struct PostProcessPresetExample {
    pub input: String,
    pub output: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Type)]
pub struct PostProcessPreset {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub prompt_template: String,
    #[serde(default)]
    pub system_template: Option<String>,
    #[serde(default)]
    pub user_template: Option<String>,
    #[serde(default = "default_post_process_preset_output_kind")]
    pub output_kind: String,
    #[serde(default = "default_post_process_preset_version")]
    pub version: u32,
    #[serde(default)]
    pub is_builtin: bool,
    #[serde(default = "default_post_process_preset_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub examples: Vec<PostProcessPresetExample>,
}

impl PostProcessPreset {
    pub fn has_explicit_chat_templates(&self) -> bool {
        self.system_template.is_some() || self.user_template.is_some()
    }

    pub fn effective_system_template(&self) -> String {
        self.system_template
            .as_deref()
            .map(str::trim)
            .map(ToString::to_string)
            .unwrap_or_else(|| prompt_template_to_system_template(&self.prompt_template))
    }

    pub fn effective_user_template(&self) -> String {
        self.user_template
            .as_deref()
            .map(str::trim)
            .filter(|template| !template.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| "${output}".to_string())
    }

    pub fn prompt_template_snapshot(&self) -> String {
        if !self.has_explicit_chat_templates() {
            return self.prompt_template.clone();
        }

        compose_chat_prompt_template(
            &self.effective_system_template(),
            &self.effective_user_template(),
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct PostProcessProvider {
    pub id: String,
    pub label: String,
    pub base_url: String,
    #[serde(default)]
    pub allow_base_url_edit: bool,
    #[serde(default)]
    pub models_endpoint: Option<String>,
    #[serde(default)]
    pub supports_structured_output: bool,
    #[serde(default)]
    pub api_kind: PostProcessApiKind,
    #[serde(default)]
    pub structured_output_mode: PostProcessStructuredOutputMode,
    #[serde(default)]
    pub reasoning_control: PostProcessReasoningControl,
    #[serde(default)]
    pub model_suggestions: Vec<String>,
    #[serde(default)]
    pub deprecated_model_suggestions: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum PostProcessApiKind {
    #[default]
    OpenAiCompatible,
    AppleIntelligence,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum PostProcessStructuredOutputMode {
    #[default]
    None,
    OpenAiJsonSchema,
    JsonObject,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum PostProcessReasoningControl {
    #[default]
    None,
    OpenAiReasoningEffort,
    OpenRouterReasoning,
    DeepSeekThinking,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum AsrProviderKind {
    BuiltInLocal,
    AliyunQwen3AsrFlash,
    AliyunQwen3AsrRealtime,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct AsrProvider {
    pub id: String,
    pub label: String,
    pub base_url: String,
    pub kind: AsrProviderKind,
    #[serde(default)]
    pub allow_model_edit: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Type)]
pub struct TranscriptionProfile {
    pub id: String,
    pub name: String,
    pub description: String,
    pub asr_provider_id: String,
    pub asr_model: Option<String>,
    pub language: String,
    pub translate_to_english: bool,
    pub post_process_enabled: bool,
    pub post_process_preset_id: Option<String>,
    #[serde(default)]
    pub is_builtin: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "lowercase")]
pub enum OverlayPosition {
    None,
    Top,
    Bottom,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum ModelUnloadTimeout {
    Never,
    Immediately,
    Min2,
    #[default]
    Min5,
    Min10,
    Min15,
    Hour1,
    Sec15, // Debug mode only
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum PasteMethod {
    CtrlV,
    Direct,
    None,
    ShiftInsert,
    CtrlShiftV,
    ExternalScript,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardHandling {
    #[default]
    DontModify,
    CopyToClipboard,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum AutoSubmitKey {
    #[default]
    Enter,
    CtrlEnter,
    CmdEnter,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum RecordingRetentionPeriod {
    Never,
    PreserveLimit,
    Days3,
    Weeks2,
    Months3,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum KeyboardImplementation {
    Tauri,
    #[serde(rename = "speakmore_keys")]
    SpeakMoreKeys,
}

impl Default for KeyboardImplementation {
    fn default() -> Self {
        #[cfg(target_os = "linux")]
        return KeyboardImplementation::Tauri;
        #[cfg(not(target_os = "linux"))]
        return KeyboardImplementation::SpeakMoreKeys;
    }
}

impl Default for PasteMethod {
    fn default() -> Self {
        // Default to CtrlV for macOS and Windows, Direct for Linux
        #[cfg(target_os = "linux")]
        return PasteMethod::Direct;
        #[cfg(not(target_os = "linux"))]
        return PasteMethod::CtrlV;
    }
}

impl ModelUnloadTimeout {
    pub fn to_minutes(self) -> Option<u64> {
        match self {
            ModelUnloadTimeout::Never => None,
            ModelUnloadTimeout::Immediately => Some(0), // Special case for immediate unloading
            ModelUnloadTimeout::Min2 => Some(2),
            ModelUnloadTimeout::Min5 => Some(5),
            ModelUnloadTimeout::Min10 => Some(10),
            ModelUnloadTimeout::Min15 => Some(15),
            ModelUnloadTimeout::Hour1 => Some(60),
            ModelUnloadTimeout::Sec15 => Some(0), // Special case for debug - handled separately
        }
    }

    pub fn to_seconds(self) -> Option<u64> {
        match self {
            ModelUnloadTimeout::Never => None,
            ModelUnloadTimeout::Immediately => Some(0), // Special case for immediate unloading
            ModelUnloadTimeout::Sec15 => Some(15),
            _ => self.to_minutes().map(|m| m * 60),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum SoundTheme {
    Marimba,
    Pop,
    Custom,
}

impl SoundTheme {
    fn as_str(self) -> &'static str {
        match self {
            SoundTheme::Marimba => "marimba",
            SoundTheme::Pop => "pop",
            SoundTheme::Custom => "custom",
        }
    }

    pub fn to_start_path(self) -> String {
        format!("resources/{}_start.wav", self.as_str())
    }

    pub fn to_stop_path(self) -> String {
        format!("resources/{}_stop.wav", self.as_str())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum TypingTool {
    #[default]
    Auto,
    Wtype,
    Kwtype,
    Dotool,
    Ydotool,
    Xdotool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum WhisperAcceleratorSetting {
    #[default]
    Auto,
    Cpu,
    Gpu,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum OrtAcceleratorSetting {
    #[default]
    Auto,
    Cpu,
    Cuda,
    #[serde(rename = "directml")]
    DirectMl,
    Rocm,
}

#[derive(Clone, Serialize, Deserialize, Type)]
#[serde(transparent)]
pub(crate) struct SecretMap(HashMap<String, String>);

impl fmt::Debug for SecretMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let redacted: HashMap<&String, &str> = self
            .0
            .iter()
            .map(|(k, v)| (k, if v.is_empty() { "" } else { "[REDACTED]" }))
            .collect();
        redacted.fmt(f)
    }
}

impl std::ops::Deref for SecretMap {
    type Target = HashMap<String, String>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for SecretMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/* helper wrapper for composing the initial JSON in the store ------------- */
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Type)]
pub struct WhisperFamilySettings {
    pub language: String,
    pub translate_to_english: bool,
    pub custom_vocabulary: Vec<String>,
}

impl Default for WhisperFamilySettings {
    fn default() -> Self {
        Self {
            language: default_selected_language(),
            translate_to_english: default_translate_to_english(),
            custom_vocabulary: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Type)]
pub struct Qwen3AsrFamilySettings {
    pub language: String,
    pub custom_vocabulary: Vec<String>,
    pub max_new_tokens: u32,
    pub max_total_len: u32,
}

impl Default for Qwen3AsrFamilySettings {
    fn default() -> Self {
        Self {
            language: default_selected_language(),
            custom_vocabulary: Vec::new(),
            max_new_tokens: DEFAULT_QWEN3_ASR_MAX_NEW_TOKENS,
            max_total_len: DEFAULT_QWEN3_ASR_MAX_TOTAL_LEN,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Type)]
pub struct AsrFamilySettings {
    pub whisper: WhisperFamilySettings,
    pub qwen3_asr: Qwen3AsrFamilySettings,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct AppSettings {
    pub bindings: HashMap<String, ShortcutBinding>,
    pub push_to_talk: bool,
    pub audio_feedback: bool,
    #[serde(default = "default_audio_feedback_volume")]
    pub audio_feedback_volume: f32,
    #[serde(default = "default_sound_theme")]
    pub sound_theme: SoundTheme,
    #[serde(default = "default_start_hidden")]
    pub start_hidden: bool,
    #[serde(default = "default_autostart_enabled")]
    pub autostart_enabled: bool,
    #[serde(default = "default_update_checks_enabled")]
    pub update_checks_enabled: bool,
    #[serde(default = "default_model")]
    pub selected_model: String,
    #[serde(default = "default_asr_provider_id")]
    pub asr_provider_id: String,
    #[serde(default = "default_asr_providers")]
    pub asr_providers: Vec<AsrProvider>,
    #[serde(default = "default_asr_api_keys")]
    pub asr_api_keys: SecretMap,
    #[serde(default = "default_asr_models")]
    pub asr_models: HashMap<String, String>,
    #[serde(default = "default_asr_family_settings")]
    pub asr_family_settings: AsrFamilySettings,
    #[serde(default = "default_transcription_profiles")]
    pub transcription_profiles: Vec<TranscriptionProfile>,
    #[serde(default = "default_selected_transcription_profile_id")]
    pub selected_transcription_profile_id: Option<String>,
    #[serde(default = "default_always_on_microphone")]
    pub always_on_microphone: bool,
    #[serde(default)]
    pub selected_microphone: Option<String>,
    #[serde(default)]
    pub clamshell_microphone: Option<String>,
    #[serde(default)]
    pub selected_output_device: Option<String>,
    #[serde(default = "default_translate_to_english")]
    pub translate_to_english: bool,
    #[serde(default = "default_selected_language")]
    pub selected_language: String,
    #[serde(default = "default_overlay_position")]
    pub overlay_position: OverlayPosition,
    #[serde(default = "default_debug_mode")]
    pub debug_mode: bool,
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
    #[serde(default)]
    pub custom_words: Vec<String>,
    #[serde(default)]
    pub model_unload_timeout: ModelUnloadTimeout,
    #[serde(default = "default_word_correction_threshold")]
    pub word_correction_threshold: f64,
    #[serde(default = "default_history_limit")]
    pub history_limit: usize,
    #[serde(default = "default_recording_retention_period")]
    pub recording_retention_period: RecordingRetentionPeriod,
    #[serde(default)]
    pub paste_method: PasteMethod,
    #[serde(default)]
    pub clipboard_handling: ClipboardHandling,
    #[serde(default = "default_auto_submit")]
    pub auto_submit: bool,
    #[serde(default)]
    pub auto_submit_key: AutoSubmitKey,
    #[serde(default = "default_post_process_enabled")]
    pub post_process_enabled: bool,
    #[serde(default = "default_post_process_provider_id")]
    pub post_process_provider_id: String,
    #[serde(default = "default_post_process_providers")]
    pub post_process_providers: Vec<PostProcessProvider>,
    #[serde(default = "default_post_process_api_keys")]
    pub post_process_api_keys: SecretMap,
    #[serde(default = "default_post_process_models")]
    pub post_process_models: HashMap<String, String>,
    #[serde(default = "default_post_process_reasoning_efforts")]
    pub post_process_reasoning_efforts: HashMap<String, String>,
    #[serde(default = "default_post_process_prompts")]
    pub post_process_prompts: Vec<LLMPrompt>,
    #[serde(default)]
    pub post_process_selected_prompt_id: Option<String>,
    #[serde(default = "default_post_process_presets")]
    pub post_process_presets: Vec<PostProcessPreset>,
    #[serde(default)]
    pub post_process_selected_preset_id: Option<String>,
    #[serde(default)]
    pub mute_while_recording: bool,
    #[serde(default)]
    pub append_trailing_space: bool,
    #[serde(default = "default_app_language")]
    pub app_language: String,
    #[serde(default)]
    pub experimental_enabled: bool,
    #[serde(default)]
    pub lazy_stream_close: bool,
    #[serde(default)]
    pub keyboard_implementation: KeyboardImplementation,
    #[serde(default = "default_show_tray_icon")]
    pub show_tray_icon: bool,
    #[serde(default = "default_paste_delay_ms")]
    pub paste_delay_ms: u64,
    #[serde(default = "default_typing_tool")]
    pub typing_tool: TypingTool,
    pub external_script_path: Option<String>,
    #[serde(default)]
    pub custom_filler_words: Option<Vec<String>>,
    #[serde(default)]
    pub whisper_accelerator: WhisperAcceleratorSetting,
    #[serde(default)]
    pub ort_accelerator: OrtAcceleratorSetting,
    #[serde(default = "default_whisper_gpu_device")]
    pub whisper_gpu_device: i32,
    #[serde(default)]
    pub extra_recording_buffer_ms: u64,
}

fn default_model() -> String {
    "".to_string()
}

fn default_always_on_microphone() -> bool {
    false
}

fn default_translate_to_english() -> bool {
    false
}

fn default_start_hidden() -> bool {
    false
}

fn default_autostart_enabled() -> bool {
    false
}

fn default_update_checks_enabled() -> bool {
    false
}

fn ensure_update_checks_disabled(settings: &mut AppSettings) -> bool {
    if settings.update_checks_enabled {
        settings.update_checks_enabled = false;
        return true;
    }
    false
}

fn default_selected_language() -> String {
    "auto".to_string()
}

fn default_asr_family_settings() -> AsrFamilySettings {
    AsrFamilySettings::default()
}

fn default_overlay_position() -> OverlayPosition {
    #[cfg(target_os = "linux")]
    return OverlayPosition::None;
    #[cfg(not(target_os = "linux"))]
    return OverlayPosition::Bottom;
}

fn default_debug_mode() -> bool {
    false
}

fn default_log_level() -> LogLevel {
    LogLevel::Debug
}

fn default_word_correction_threshold() -> f64 {
    0.18
}

fn default_paste_delay_ms() -> u64 {
    60
}

fn default_auto_submit() -> bool {
    false
}

fn default_history_limit() -> usize {
    5
}

fn default_recording_retention_period() -> RecordingRetentionPeriod {
    RecordingRetentionPeriod::PreserveLimit
}

fn default_audio_feedback_volume() -> f32 {
    1.0
}

fn default_sound_theme() -> SoundTheme {
    SoundTheme::Marimba
}

fn default_post_process_enabled() -> bool {
    false
}

fn default_app_language() -> String {
    tauri_plugin_os::locale()
        .map(|l| l.replace('_', "-"))
        .unwrap_or_else(|| "en".to_string())
}

fn default_show_tray_icon() -> bool {
    true
}

fn default_post_process_provider_id() -> String {
    "openai".to_string()
}

fn default_asr_provider_id() -> String {
    BUILT_IN_LOCAL_ASR_PROVIDER_ID.to_string()
}

fn default_asr_providers() -> Vec<AsrProvider> {
    vec![
        AsrProvider {
            id: BUILT_IN_LOCAL_ASR_PROVIDER_ID.to_string(),
            label: "Built-in local models".to_string(),
            base_url: "local://built-in".to_string(),
            kind: AsrProviderKind::BuiltInLocal,
            allow_model_edit: false,
        },
        AsrProvider {
            id: ALIYUN_QWEN3_ASR_PROVIDER_ID.to_string(),
            label: "Alibaba Cloud Qwen3-ASR".to_string(),
            base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
            kind: AsrProviderKind::AliyunQwen3AsrFlash,
            allow_model_edit: true,
        },
        AsrProvider {
            id: ALIYUN_QWEN3_ASR_REALTIME_PROVIDER_ID.to_string(),
            label: "Alibaba Cloud Qwen3-ASR Realtime".to_string(),
            base_url: ALIYUN_QWEN3_ASR_REALTIME_DEFAULT_BASE_URL.to_string(),
            kind: AsrProviderKind::AliyunQwen3AsrRealtime,
            allow_model_edit: true,
        },
    ]
}

fn default_asr_api_keys() -> SecretMap {
    let mut map = HashMap::new();
    for provider in default_asr_providers() {
        map.insert(provider.id, String::new());
    }
    SecretMap(map)
}

fn default_asr_model_for_provider(provider_id: &str) -> String {
    match provider_id {
        ALIYUN_QWEN3_ASR_PROVIDER_ID => ALIYUN_QWEN3_ASR_DEFAULT_MODEL.to_string(),
        ALIYUN_QWEN3_ASR_REALTIME_PROVIDER_ID => {
            ALIYUN_QWEN3_ASR_REALTIME_DEFAULT_MODEL.to_string()
        }
        _ => String::new(),
    }
}

fn default_asr_models() -> HashMap<String, String> {
    let mut map = HashMap::new();
    for provider in default_asr_providers() {
        map.insert(
            provider.id.clone(),
            default_asr_model_for_provider(&provider.id),
        );
    }
    map
}

fn default_selected_transcription_profile_id() -> Option<String> {
    Some(DEFAULT_TRANSCRIPTION_PROFILE_ID.to_string())
}

fn default_transcription_profiles() -> Vec<TranscriptionProfile> {
    vec![
        TranscriptionProfile {
            id: DEFAULT_TRANSCRIPTION_PROFILE_ID.to_string(),
            name: "Quick input (local)".to_string(),
            description: "Fast local dictation without post-processing.".to_string(),
            asr_provider_id: BUILT_IN_LOCAL_ASR_PROVIDER_ID.to_string(),
            asr_model: None,
            language: "auto".to_string(),
            translate_to_english: false,
            post_process_enabled: false,
            post_process_preset_id: None,
            is_builtin: true,
        },
        TranscriptionProfile {
            id: "qwen_realtime_dictation".to_string(),
            name: "Realtime dictation".to_string(),
            description: "Cloud realtime transcription with Qwen3-ASR.".to_string(),
            asr_provider_id: ALIYUN_QWEN3_ASR_REALTIME_PROVIDER_ID.to_string(),
            asr_model: Some(ALIYUN_QWEN3_ASR_REALTIME_DEFAULT_MODEL.to_string()),
            language: "auto".to_string(),
            translate_to_english: false,
            post_process_enabled: false,
            post_process_preset_id: None,
            is_builtin: true,
        },
        TranscriptionProfile {
            id: "clean_message".to_string(),
            name: "Clean message".to_string(),
            description: "Transcribe, then clean the result for messages.".to_string(),
            asr_provider_id: ALIYUN_QWEN3_ASR_REALTIME_PROVIDER_ID.to_string(),
            asr_model: Some(ALIYUN_QWEN3_ASR_REALTIME_DEFAULT_MODEL.to_string()),
            language: "auto".to_string(),
            translate_to_english: false,
            post_process_enabled: true,
            post_process_preset_id: Some(CLEAN_DICTATION_PRESET_ID.to_string()),
            is_builtin: true,
        },
    ]
}

#[allow(clippy::too_many_arguments)]
fn build_post_process_provider(
    id: &str,
    label: &str,
    base_url: &str,
    allow_base_url_edit: bool,
    models_endpoint: Option<&str>,
    api_kind: PostProcessApiKind,
    structured_output_mode: PostProcessStructuredOutputMode,
    reasoning_control: PostProcessReasoningControl,
    model_suggestions: &[&str],
    deprecated_model_suggestions: &[&str],
) -> PostProcessProvider {
    PostProcessProvider {
        id: id.to_string(),
        label: label.to_string(),
        base_url: base_url.to_string(),
        allow_base_url_edit,
        models_endpoint: models_endpoint.map(str::to_string),
        supports_structured_output: structured_output_mode != PostProcessStructuredOutputMode::None,
        api_kind,
        structured_output_mode,
        reasoning_control,
        model_suggestions: model_suggestions
            .iter()
            .map(|model| (*model).to_string())
            .collect(),
        deprecated_model_suggestions: deprecated_model_suggestions
            .iter()
            .map(|model| (*model).to_string())
            .collect(),
    }
}

fn default_post_process_providers() -> Vec<PostProcessProvider> {
    let mut providers = vec![
        build_post_process_provider(
            "openai",
            "OpenAI",
            "https://api.openai.com/v1",
            false,
            Some("/models"),
            PostProcessApiKind::OpenAiCompatible,
            PostProcessStructuredOutputMode::OpenAiJsonSchema,
            PostProcessReasoningControl::None,
            &[],
            &[],
        ),
        build_post_process_provider(
            "zai",
            "Z.AI",
            "https://api.z.ai/api/paas/v4",
            false,
            Some("/models"),
            PostProcessApiKind::OpenAiCompatible,
            PostProcessStructuredOutputMode::OpenAiJsonSchema,
            PostProcessReasoningControl::None,
            &[],
            &[],
        ),
        build_post_process_provider(
            "openrouter",
            "OpenRouter",
            "https://openrouter.ai/api/v1",
            false,
            Some("/models"),
            PostProcessApiKind::OpenAiCompatible,
            PostProcessStructuredOutputMode::OpenAiJsonSchema,
            PostProcessReasoningControl::OpenRouterReasoning,
            &[],
            &[],
        ),
        build_post_process_provider(
            "anthropic",
            "Anthropic",
            "https://api.anthropic.com/v1",
            false,
            Some("/models"),
            PostProcessApiKind::OpenAiCompatible,
            PostProcessStructuredOutputMode::None,
            PostProcessReasoningControl::None,
            &[],
            &[],
        ),
        build_post_process_provider(
            "groq",
            "Groq",
            "https://api.groq.com/openai/v1",
            false,
            Some("/models"),
            PostProcessApiKind::OpenAiCompatible,
            PostProcessStructuredOutputMode::None,
            PostProcessReasoningControl::None,
            &[],
            &[],
        ),
        build_post_process_provider(
            "cerebras",
            "Cerebras",
            "https://api.cerebras.ai/v1",
            false,
            Some("/models"),
            PostProcessApiKind::OpenAiCompatible,
            PostProcessStructuredOutputMode::OpenAiJsonSchema,
            PostProcessReasoningControl::None,
            &[],
            &[],
        ),
        build_post_process_provider(
            DEEPSEEK_PROVIDER_ID,
            "DeepSeek",
            "https://api.deepseek.com",
            false,
            None,
            PostProcessApiKind::OpenAiCompatible,
            PostProcessStructuredOutputMode::JsonObject,
            PostProcessReasoningControl::DeepSeekThinking,
            &["deepseek-v4-flash", "deepseek-v4-pro"],
            &["deepseek-chat", "deepseek-reasoner"],
        ),
    ];

    // Note: We always include Apple Intelligence on macOS ARM64 without checking availability
    // at startup. The availability check is deferred to when the user actually tries to use it
    // (in actions.rs). This prevents crashes on macOS 26.x beta where accessing
    // SystemLanguageModel.default during early app initialization causes SIGABRT.
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        providers.push(build_post_process_provider(
            APPLE_INTELLIGENCE_PROVIDER_ID,
            "Apple Intelligence",
            "apple-intelligence://local",
            false,
            None,
            PostProcessApiKind::AppleIntelligence,
            PostProcessStructuredOutputMode::OpenAiJsonSchema,
            PostProcessReasoningControl::None,
            &[APPLE_INTELLIGENCE_DEFAULT_MODEL_ID],
            &[],
        ));
    }

    // AWS Bedrock via Mantle (OpenAI-compatible endpoint)
    providers.push(build_post_process_provider(
        "bedrock_mantle",
        "AWS Bedrock (Mantle)",
        "https://bedrock-mantle.us-east-1.api.aws/v1",
        false,
        Some("/models"),
        PostProcessApiKind::OpenAiCompatible,
        PostProcessStructuredOutputMode::OpenAiJsonSchema,
        PostProcessReasoningControl::None,
        &[],
        &[],
    ));

    // Custom provider always comes last
    providers.push(build_post_process_provider(
        "custom",
        "Custom",
        "http://localhost:11434/v1",
        true,
        Some("/models"),
        PostProcessApiKind::OpenAiCompatible,
        PostProcessStructuredOutputMode::None,
        PostProcessReasoningControl::None,
        &[],
        &[],
    ));

    providers
}

fn default_post_process_api_keys() -> SecretMap {
    let mut map = HashMap::new();
    for provider in default_post_process_providers() {
        map.insert(provider.id, String::new());
    }
    SecretMap(map)
}

fn default_model_for_provider(provider_id: &str) -> String {
    if provider_id == APPLE_INTELLIGENCE_PROVIDER_ID {
        return APPLE_INTELLIGENCE_DEFAULT_MODEL_ID.to_string();
    }
    String::new()
}

fn default_post_process_models() -> HashMap<String, String> {
    let mut map = HashMap::new();
    for provider in default_post_process_providers() {
        map.insert(
            provider.id.clone(),
            default_model_for_provider(&provider.id),
        );
    }
    map
}

fn default_reasoning_effort_for_provider(provider_id: &str) -> String {
    match provider_id {
        DEEPSEEK_PROVIDER_ID => "disabled".to_string(),
        "openrouter" => "none".to_string(),
        _ => String::new(),
    }
}

fn default_post_process_reasoning_efforts() -> HashMap<String, String> {
    let mut map = HashMap::new();
    for provider in default_post_process_providers() {
        map.insert(
            provider.id.clone(),
            default_reasoning_effort_for_provider(&provider.id),
        );
    }
    map
}

fn default_post_process_prompts() -> Vec<LLMPrompt> {
    vec![LLMPrompt {
        id: "default_improve_transcriptions".to_string(),
        name: "Improve Transcriptions".to_string(),
        prompt: "Clean this transcript:\n1. Fix spelling, capitalization, and punctuation errors\n2. Convert number words to digits (twenty-five → 25, ten percent → 10%, five dollars → $5)\n3. Replace spoken punctuation with symbols (period → ., comma → ,, question mark → ?)\n4. Remove filler words (um, uh, like as filler)\n5. Keep the language in the original version (if it was french, keep it in french for example)\n\nPreserve exact meaning and word order. Do not paraphrase or reorder content.\n\nReturn only the cleaned transcript.\n\nTranscript:\n${output}".to_string(),
    }]
}

fn default_post_process_preset_output_kind() -> String {
    "plain_text".to_string()
}

fn default_post_process_preset_version() -> u32 {
    1
}

fn default_post_process_preset_enabled() -> bool {
    true
}

fn prompt_template_to_system_template(prompt_template: &str) -> String {
    prompt_template.replace("${output}", "").trim().to_string()
}

fn compose_chat_prompt_template(system_template: &str, user_template: &str) -> String {
    let system = system_template.trim();
    let user = user_template.trim();

    match (system.is_empty(), user.is_empty()) {
        (true, true) => String::new(),
        (true, false) => user.to_string(),
        (false, true) => system.to_string(),
        (false, false) => format!("{system}\n\n{user}"),
    }
}

fn build_builtin_post_process_preset(
    id: &str,
    name: &str,
    description: &str,
    prompt_template: &str,
    output_kind: String,
    examples: Vec<PostProcessPresetExample>,
) -> PostProcessPreset {
    PostProcessPreset {
        id: id.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        prompt_template: prompt_template.to_string(),
        system_template: Some(prompt_template_to_system_template(prompt_template)),
        user_template: Some("${output}".to_string()),
        output_kind,
        version: 1,
        is_builtin: true,
        enabled: true,
        examples,
    }
}

fn default_post_process_presets() -> Vec<PostProcessPreset> {
    vec![
        build_builtin_post_process_preset(
            CLEAN_DICTATION_PRESET_ID,
            "Clean Dictation",
            "Clean punctuation, casing, filler words, and spoken punctuation while preserving the original meaning.",
            "Clean this transcript:\n1. Fix spelling, capitalization, and punctuation errors.\n2. Convert spoken numbers and common spoken punctuation to text symbols where appropriate.\n3. Remove filler words only when they are clearly filler.\n4. Preserve the original language, meaning, and order.\n5. Do not add facts or commentary.\n\nReturn only the cleaned transcript.\n\nTranscript:\n${output}",
            default_post_process_preset_output_kind(),
            vec![PostProcessPresetExample {
                input: "um please open the MCP service comma then run cargo test".to_string(),
                output: "Please open the MCP service, then run cargo test.".to_string(),
            }],
        ),
        build_builtin_post_process_preset(
            "format_as_markdown",
            "Format as Markdown",
            "Turn dictated notes into clean Markdown without changing the substance.",
            "Format this transcript as concise Markdown:\n1. Keep the original meaning and language.\n2. Use headings, bullets, or numbered lists only when they fit the content.\n3. Preserve commands, file paths, code identifiers, and project names exactly when possible.\n4. Do not add facts or commentary.\n\nReturn only the Markdown.\n\nTranscript:\n${output}",
            "markdown".to_string(),
            vec![PostProcessPresetExample {
                input: "next steps first update the provider abstraction second run bun run build".to_string(),
                output: "## Next Steps\n\n1. Update the provider abstraction.\n2. Run `bun run build`.".to_string(),
            }],
        ),
        build_builtin_post_process_preset(
            "concise_message",
            "Concise Message",
            "Rewrite the dictation as a short message while keeping the intent intact.",
            "Rewrite this transcript as a concise message:\n1. Keep the original intent and language.\n2. Remove rambling and filler.\n3. Keep names, technical terms, commands, and paths intact.\n4. Do not make the tone overly formal.\n5. Do not add facts.\n\nReturn only the message.\n\nTranscript:\n${output}",
            default_post_process_preset_output_kind(),
            vec![PostProcessPresetExample {
                input: "can you help me check why the qwen asr provider is failing and then run the lint again".to_string(),
                output: "Can you check why the Qwen ASR provider is failing and run lint again?".to_string(),
            }],
        ),
        build_builtin_post_process_preset(
            "translate_to_chinese",
            "自动翻译为中文",
            "将口述文本翻译为自然的简体中文，同时保留技术词、命令、路径和名称。",
            "Translate this transcript into natural Simplified Chinese:\n1. Preserve the original meaning and intent.\n2. Keep code identifiers, commands, file paths, URLs, product names, project names, and proper nouns unchanged unless a conventional Chinese translation is clearly expected.\n3. Keep mixed Chinese/English technical terms readable; do not force-translate acronyms or code terms.\n4. If the transcript is already Chinese, only clean punctuation and wording lightly.\n5. Do not add facts, commentary, explanations, or quotation marks around the result.\n\nReturn only the translated text.\n\nTranscript:\n${output}",
            default_post_process_preset_output_kind(),
            vec![PostProcessPresetExample {
                input: "please check the qwen asr provider and run cargo test again".to_string(),
                output: "请检查 Qwen ASR provider，然后再运行 cargo test。".to_string(),
            }],
        ),
        build_builtin_post_process_preset(
            "translate_to_english",
            "自动翻译为英文",
            "将口述文本翻译为自然的英文，同时保留技术词、命令、路径和名称。",
            "Translate this transcript into natural English:\n1. Preserve the original meaning and intent.\n2. Keep code identifiers, commands, file paths, URLs, product names, project names, and proper nouns unchanged unless a conventional English translation is clearly expected.\n3. Keep mixed Chinese/English technical terms readable; do not over-explain acronyms or code terms.\n4. If the transcript is already English, only clean punctuation and wording lightly.\n5. Do not add facts, commentary, explanations, or quotation marks around the result.\n\nReturn only the translated text.\n\nTranscript:\n${output}",
            default_post_process_preset_output_kind(),
            vec![PostProcessPresetExample {
                input: "帮我检查 Qwen ASR provider，然后再跑一次 cargo test".to_string(),
                output: "Help me check the Qwen ASR provider, then run cargo test again.".to_string(),
            }],
        ),
    ]
}

fn default_whisper_gpu_device() -> i32 {
    -1 // auto
}

fn default_typing_tool() -> TypingTool {
    TypingTool::Auto
}

fn ensure_post_process_defaults(settings: &mut AppSettings) -> bool {
    let mut changed = false;
    for provider in default_post_process_providers() {
        // Use match to do a single lookup - either sync existing or add new
        match settings
            .post_process_providers
            .iter_mut()
            .find(|p| p.id == provider.id)
        {
            Some(existing) => {
                if existing.label != provider.label {
                    existing.label = provider.label.clone();
                    changed = true;
                }
                if existing.allow_base_url_edit != provider.allow_base_url_edit {
                    existing.allow_base_url_edit = provider.allow_base_url_edit;
                    changed = true;
                }
                if existing.models_endpoint != provider.models_endpoint {
                    existing.models_endpoint = provider.models_endpoint.clone();
                    changed = true;
                }
                if existing.supports_structured_output != provider.supports_structured_output
                    || existing.api_kind != provider.api_kind
                    || existing.structured_output_mode != provider.structured_output_mode
                    || existing.reasoning_control != provider.reasoning_control
                    || existing.model_suggestions != provider.model_suggestions
                    || existing.deprecated_model_suggestions
                        != provider.deprecated_model_suggestions
                {
                    debug!(
                        "Updating post-processing capabilities for provider '{}'",
                        provider.id
                    );
                    existing.supports_structured_output = provider.supports_structured_output;
                    existing.api_kind = provider.api_kind;
                    existing.structured_output_mode = provider.structured_output_mode;
                    existing.reasoning_control = provider.reasoning_control;
                    existing.model_suggestions = provider.model_suggestions.clone();
                    existing.deprecated_model_suggestions =
                        provider.deprecated_model_suggestions.clone();
                    changed = true;
                }
            }
            None => {
                // Provider doesn't exist, add it
                settings.post_process_providers.push(provider.clone());
                changed = true;
            }
        }

        if !settings.post_process_api_keys.contains_key(&provider.id) {
            settings
                .post_process_api_keys
                .insert(provider.id.clone(), String::new());
            changed = true;
        }

        let default_model = default_model_for_provider(&provider.id);
        match settings.post_process_models.get_mut(&provider.id) {
            Some(existing) => {
                if existing.is_empty() && !default_model.is_empty() {
                    *existing = default_model.clone();
                    changed = true;
                }
            }
            None => {
                settings
                    .post_process_models
                    .insert(provider.id.clone(), default_model);
                changed = true;
            }
        }

        let default_reasoning_effort = default_reasoning_effort_for_provider(&provider.id);
        if !settings
            .post_process_reasoning_efforts
            .contains_key(&provider.id)
        {
            settings
                .post_process_reasoning_efforts
                .insert(provider.id.clone(), default_reasoning_effort);
            changed = true;
        }
    }

    for preset in default_post_process_presets() {
        match settings
            .post_process_presets
            .iter_mut()
            .find(|existing| existing.id == preset.id)
        {
            Some(existing) => {
                if !existing.is_builtin || *existing != preset {
                    *existing = preset;
                    changed = true;
                }
            }
            None => {
                settings.post_process_presets.push(preset);
                changed = true;
            }
        }
    }

    let legacy_prompts = settings.post_process_prompts.clone();
    for prompt in legacy_prompts {
        if settings
            .post_process_presets
            .iter()
            .any(|preset| preset.id == prompt.id)
        {
            continue;
        }

        settings.post_process_presets.push(PostProcessPreset {
            id: prompt.id,
            name: prompt.name,
            description: String::new(),
            prompt_template: prompt.prompt,
            system_template: None,
            user_template: None,
            output_kind: default_post_process_preset_output_kind(),
            version: default_post_process_preset_version(),
            is_builtin: false,
            enabled: true,
            examples: Vec::new(),
        });
        changed = true;
    }

    let selected_preset_is_valid = settings
        .post_process_selected_preset_id
        .as_deref()
        .is_some_and(|id| {
            settings
                .post_process_presets
                .iter()
                .any(|preset| preset.id == id && preset.enabled)
        });

    if !selected_preset_is_valid {
        let migrated_prompt_selection = settings
            .post_process_selected_prompt_id
            .as_deref()
            .and_then(|id| {
                settings
                    .post_process_presets
                    .iter()
                    .find(|preset| preset.id == id && preset.enabled)
                    .map(|preset| preset.id.clone())
            });
        let clean_dictation_selection = settings
            .post_process_presets
            .iter()
            .find(|preset| preset.id == CLEAN_DICTATION_PRESET_ID && preset.enabled)
            .map(|preset| preset.id.clone());
        let first_enabled_selection = settings
            .post_process_presets
            .iter()
            .find(|preset| preset.enabled)
            .map(|preset| preset.id.clone());

        settings.post_process_selected_preset_id = migrated_prompt_selection
            .or(clean_dictation_selection)
            .or(first_enabled_selection);
        changed = true;
    }

    changed
}

fn ensure_asr_defaults(settings: &mut AppSettings) -> bool {
    let mut changed = false;
    let defaults = default_asr_providers();

    if settings.asr_provider_id.trim().is_empty()
        || !defaults
            .iter()
            .any(|provider| provider.id == settings.asr_provider_id)
    {
        settings.asr_provider_id = default_asr_provider_id();
        changed = true;
    }

    for provider in defaults {
        match settings
            .asr_providers
            .iter_mut()
            .find(|existing| existing.id == provider.id)
        {
            Some(existing) => {
                if existing.label != provider.label
                    || existing.base_url != provider.base_url
                    || existing.kind != provider.kind
                    || existing.allow_model_edit != provider.allow_model_edit
                {
                    *existing = provider.clone();
                    changed = true;
                }
            }
            None => {
                settings.asr_providers.push(provider.clone());
                changed = true;
            }
        }

        if !settings.asr_api_keys.contains_key(&provider.id) {
            settings
                .asr_api_keys
                .insert(provider.id.clone(), String::new());
            changed = true;
        }

        let default_model = default_asr_model_for_provider(&provider.id);
        match settings.asr_models.get_mut(&provider.id) {
            Some(existing) => {
                if existing.trim().is_empty() && !default_model.is_empty() {
                    *existing = default_model;
                    changed = true;
                }
            }
            None => {
                settings
                    .asr_models
                    .insert(provider.id.clone(), default_model);
                changed = true;
            }
        }
    }

    changed
}

fn ensure_asr_family_settings_defaults(settings: &mut AppSettings) -> bool {
    let mut changed = false;

    if settings.asr_family_settings == AsrFamilySettings::default()
        && (settings.selected_language != default_selected_language()
            || settings.translate_to_english != default_translate_to_english()
            || !settings.custom_words.is_empty())
    {
        settings.asr_family_settings.whisper = WhisperFamilySettings {
            language: settings.selected_language.clone(),
            translate_to_english: settings.translate_to_english,
            custom_vocabulary: settings.custom_words.clone(),
        };
        changed = true;
    }

    if settings
        .asr_family_settings
        .whisper
        .language
        .trim()
        .is_empty()
    {
        settings.asr_family_settings.whisper.language = default_selected_language();
        changed = true;
    }

    if settings
        .asr_family_settings
        .qwen3_asr
        .language
        .trim()
        .is_empty()
    {
        settings.asr_family_settings.qwen3_asr.language = default_selected_language();
        changed = true;
    }

    if settings.asr_family_settings.qwen3_asr.max_new_tokens == 0 {
        settings.asr_family_settings.qwen3_asr.max_new_tokens =
            Qwen3AsrFamilySettings::default().max_new_tokens;
        changed = true;
    }
    if settings.asr_family_settings.qwen3_asr.max_new_tokens
        == LEGACY_QWEN3_ASR_MAX_NEW_TOKENS_DEFAULT
        && settings.asr_family_settings.qwen3_asr.max_total_len == DEFAULT_QWEN3_ASR_MAX_TOTAL_LEN
        && settings
            .asr_family_settings
            .qwen3_asr
            .custom_vocabulary
            .is_empty()
    {
        settings.asr_family_settings.qwen3_asr.max_new_tokens = DEFAULT_QWEN3_ASR_MAX_NEW_TOKENS;
        changed = true;
    }
    if settings.asr_family_settings.qwen3_asr.max_new_tokens > QWEN3_ASR_MAX_NEW_TOKENS_LIMIT {
        settings.asr_family_settings.qwen3_asr.max_new_tokens = QWEN3_ASR_MAX_NEW_TOKENS_LIMIT;
        changed = true;
    }

    if settings.asr_family_settings.qwen3_asr.max_total_len == 0 {
        settings.asr_family_settings.qwen3_asr.max_total_len =
            Qwen3AsrFamilySettings::default().max_total_len;
        changed = true;
    }
    if settings.asr_family_settings.qwen3_asr.max_total_len > QWEN3_ASR_MAX_TOTAL_LEN_LIMIT {
        settings.asr_family_settings.qwen3_asr.max_total_len = QWEN3_ASR_MAX_TOTAL_LEN_LIMIT;
        changed = true;
    }

    if settings.asr_family_settings.qwen3_asr.max_total_len
        < settings.asr_family_settings.qwen3_asr.max_new_tokens
    {
        settings.asr_family_settings.qwen3_asr.max_total_len =
            settings.asr_family_settings.qwen3_asr.max_new_tokens;
        changed = true;
    }

    changed
}

fn ensure_transcription_profile_defaults(settings: &mut AppSettings) -> bool {
    let mut changed = false;
    let defaults = default_transcription_profiles();

    for profile in defaults {
        match settings
            .transcription_profiles
            .iter_mut()
            .find(|existing| existing.id == profile.id)
        {
            Some(existing) if existing.is_builtin && existing != &profile => {
                *existing = profile;
                changed = true;
            }
            Some(_) => {}
            None => {
                settings.transcription_profiles.push(profile);
                changed = true;
            }
        }
    }

    let selected_is_valid = settings
        .selected_transcription_profile_id
        .as_deref()
        .map(|id| {
            settings
                .transcription_profiles
                .iter()
                .any(|profile| profile.id == id)
        })
        .unwrap_or(false);

    if !selected_is_valid {
        settings.selected_transcription_profile_id = default_selected_transcription_profile_id();
        changed = true;
    }

    changed
}

pub const SETTINGS_STORE_PATH: &str = "settings_store.json";

pub fn get_default_settings() -> AppSettings {
    #[cfg(target_os = "windows")]
    let default_shortcut = "ctrl+space";
    #[cfg(target_os = "macos")]
    let default_shortcut = "option+space";
    #[cfg(target_os = "linux")]
    let default_shortcut = "ctrl+space";
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let default_shortcut = "alt+space";

    let mut bindings = HashMap::new();
    bindings.insert(
        "transcribe".to_string(),
        ShortcutBinding {
            id: "transcribe".to_string(),
            name: "Transcribe".to_string(),
            description: "Converts your speech into text.".to_string(),
            default_binding: default_shortcut.to_string(),
            current_binding: default_shortcut.to_string(),
        },
    );
    #[cfg(target_os = "windows")]
    let default_post_process_shortcut = "ctrl+shift+space";
    #[cfg(target_os = "macos")]
    let default_post_process_shortcut = "option+shift+space";
    #[cfg(target_os = "linux")]
    let default_post_process_shortcut = "ctrl+shift+space";
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let default_post_process_shortcut = "alt+shift+space";

    bindings.insert(
        "transcribe_with_post_process".to_string(),
        ShortcutBinding {
            id: "transcribe_with_post_process".to_string(),
            name: "Transcribe with Post-Processing".to_string(),
            description: "Converts your speech into text and applies AI post-processing."
                .to_string(),
            default_binding: default_post_process_shortcut.to_string(),
            current_binding: default_post_process_shortcut.to_string(),
        },
    );
    bindings.insert(
        "cancel".to_string(),
        ShortcutBinding {
            id: "cancel".to_string(),
            name: "Cancel".to_string(),
            description: "Cancels the current recording.".to_string(),
            default_binding: "escape".to_string(),
            current_binding: "escape".to_string(),
        },
    );

    AppSettings {
        bindings,
        push_to_talk: true,
        audio_feedback: false,
        audio_feedback_volume: default_audio_feedback_volume(),
        sound_theme: default_sound_theme(),
        start_hidden: default_start_hidden(),
        autostart_enabled: default_autostart_enabled(),
        update_checks_enabled: default_update_checks_enabled(),
        selected_model: "".to_string(),
        asr_provider_id: default_asr_provider_id(),
        asr_providers: default_asr_providers(),
        asr_api_keys: default_asr_api_keys(),
        asr_models: default_asr_models(),
        asr_family_settings: default_asr_family_settings(),
        transcription_profiles: default_transcription_profiles(),
        selected_transcription_profile_id: default_selected_transcription_profile_id(),
        always_on_microphone: false,
        selected_microphone: None,
        clamshell_microphone: None,
        selected_output_device: None,
        translate_to_english: false,
        selected_language: "auto".to_string(),
        overlay_position: default_overlay_position(),
        debug_mode: false,
        log_level: default_log_level(),
        custom_words: Vec::new(),
        model_unload_timeout: ModelUnloadTimeout::default(),
        word_correction_threshold: default_word_correction_threshold(),
        history_limit: default_history_limit(),
        recording_retention_period: default_recording_retention_period(),
        paste_method: PasteMethod::default(),
        clipboard_handling: ClipboardHandling::default(),
        auto_submit: default_auto_submit(),
        auto_submit_key: AutoSubmitKey::default(),
        post_process_enabled: default_post_process_enabled(),
        post_process_provider_id: default_post_process_provider_id(),
        post_process_providers: default_post_process_providers(),
        post_process_api_keys: default_post_process_api_keys(),
        post_process_models: default_post_process_models(),
        post_process_reasoning_efforts: default_post_process_reasoning_efforts(),
        post_process_prompts: default_post_process_prompts(),
        post_process_selected_prompt_id: None,
        post_process_presets: default_post_process_presets(),
        post_process_selected_preset_id: Some(CLEAN_DICTATION_PRESET_ID.to_string()),
        mute_while_recording: false,
        append_trailing_space: false,
        app_language: default_app_language(),
        experimental_enabled: false,
        lazy_stream_close: false,
        keyboard_implementation: KeyboardImplementation::default(),
        show_tray_icon: default_show_tray_icon(),
        paste_delay_ms: default_paste_delay_ms(),
        typing_tool: default_typing_tool(),
        external_script_path: None,
        custom_filler_words: None,
        whisper_accelerator: WhisperAcceleratorSetting::default(),
        ort_accelerator: OrtAcceleratorSetting::default(),
        whisper_gpu_device: default_whisper_gpu_device(),
        extra_recording_buffer_ms: 0,
    }
}

impl AppSettings {
    pub fn active_asr_provider(&self) -> Option<&AsrProvider> {
        self.asr_providers
            .iter()
            .find(|provider| provider.id == self.asr_provider_id)
    }

    pub fn asr_provider(&self, provider_id: &str) -> Option<&AsrProvider> {
        self.asr_providers
            .iter()
            .find(|provider| provider.id == provider_id)
    }

    pub fn transcription_profile(&self, profile_id: &str) -> Option<&TranscriptionProfile> {
        self.transcription_profiles
            .iter()
            .find(|profile| profile.id == profile_id)
    }

    pub fn active_post_process_provider(&self) -> Option<&PostProcessProvider> {
        self.post_process_providers
            .iter()
            .find(|provider| provider.id == self.post_process_provider_id)
    }

    pub fn post_process_provider(&self, provider_id: &str) -> Option<&PostProcessProvider> {
        self.post_process_providers
            .iter()
            .find(|provider| provider.id == provider_id)
    }

    pub fn post_process_provider_mut(
        &mut self,
        provider_id: &str,
    ) -> Option<&mut PostProcessProvider> {
        self.post_process_providers
            .iter_mut()
            .find(|provider| provider.id == provider_id)
    }

    pub fn active_post_process_preset(&self) -> Option<&PostProcessPreset> {
        self.post_process_selected_preset_id
            .as_deref()
            .and_then(|preset_id| self.post_process_preset(preset_id))
            .filter(|preset| preset.enabled)
    }

    pub fn post_process_preset(&self, preset_id: &str) -> Option<&PostProcessPreset> {
        self.post_process_presets
            .iter()
            .find(|preset| preset.id == preset_id)
    }

    pub fn post_process_preset_mut(&mut self, preset_id: &str) -> Option<&mut PostProcessPreset> {
        self.post_process_presets
            .iter_mut()
            .find(|preset| preset.id == preset_id)
    }
}

pub fn load_or_create_app_settings(app: &AppHandle) -> AppSettings {
    // Initialize store
    let store = app
        .store(crate::portable::store_path(SETTINGS_STORE_PATH))
        .expect("Failed to initialize store");

    let mut settings = if let Some(settings_value) = store.get("settings") {
        // Parse the entire settings object
        match serde_json::from_value::<AppSettings>(settings_value) {
            Ok(mut settings) => {
                debug!("Found existing settings: {:?}", settings);
                let default_settings = get_default_settings();
                let mut updated = false;

                // Merge default bindings into existing settings
                for (key, value) in default_settings.bindings {
                    if let Entry::Vacant(entry) = settings.bindings.entry(key) {
                        debug!("Adding missing binding: {}", entry.key());
                        entry.insert(value);
                        updated = true;
                    }
                }

                if updated {
                    debug!("Settings updated with new bindings");
                    store.set("settings", serde_json::to_value(&settings).unwrap());
                }

                settings
            }
            Err(e) => {
                warn!("Failed to parse settings: {}", e);
                // Fall back to default settings if parsing fails
                let default_settings = get_default_settings();
                store.set("settings", serde_json::to_value(&default_settings).unwrap());
                default_settings
            }
        }
    } else {
        let default_settings = get_default_settings();
        store.set("settings", serde_json::to_value(&default_settings).unwrap());
        default_settings
    };

    let post_process_changed = ensure_post_process_defaults(&mut settings);
    let asr_changed = ensure_asr_defaults(&mut settings);
    let asr_family_changed = ensure_asr_family_settings_defaults(&mut settings);
    let transcription_profile_changed = ensure_transcription_profile_defaults(&mut settings);
    let update_checks_changed = ensure_update_checks_disabled(&mut settings);
    if post_process_changed
        || asr_changed
        || asr_family_changed
        || transcription_profile_changed
        || update_checks_changed
    {
        store.set("settings", serde_json::to_value(&settings).unwrap());
    }

    settings
}

pub fn get_settings(app: &AppHandle) -> AppSettings {
    let store = app
        .store(crate::portable::store_path(SETTINGS_STORE_PATH))
        .expect("Failed to initialize store");

    let mut settings = if let Some(settings_value) = store.get("settings") {
        serde_json::from_value::<AppSettings>(settings_value).unwrap_or_else(|_| {
            let default_settings = get_default_settings();
            store.set("settings", serde_json::to_value(&default_settings).unwrap());
            default_settings
        })
    } else {
        let default_settings = get_default_settings();
        store.set("settings", serde_json::to_value(&default_settings).unwrap());
        default_settings
    };

    let post_process_changed = ensure_post_process_defaults(&mut settings);
    let asr_changed = ensure_asr_defaults(&mut settings);
    let asr_family_changed = ensure_asr_family_settings_defaults(&mut settings);
    let transcription_profile_changed = ensure_transcription_profile_defaults(&mut settings);
    let update_checks_changed = ensure_update_checks_disabled(&mut settings);
    if post_process_changed
        || asr_changed
        || asr_family_changed
        || transcription_profile_changed
        || update_checks_changed
    {
        store.set("settings", serde_json::to_value(&settings).unwrap());
    }

    settings
}

pub fn write_settings(app: &AppHandle, settings: AppSettings) {
    let store = app
        .store(crate::portable::store_path(SETTINGS_STORE_PATH))
        .expect("Failed to initialize store");

    store.set("settings", serde_json::to_value(&settings).unwrap());
}

pub fn get_bindings(app: &AppHandle) -> HashMap<String, ShortcutBinding> {
    let settings = get_settings(app);

    settings.bindings
}

pub fn get_stored_binding(app: &AppHandle, id: &str) -> ShortcutBinding {
    let bindings = get_bindings(app);

    let binding = bindings.get(id).unwrap().clone();

    binding
}

pub fn get_history_limit(app: &AppHandle) -> usize {
    let settings = get_settings(app);
    settings.history_limit
}

pub fn get_recording_retention_period(app: &AppHandle) -> RecordingRetentionPeriod {
    let settings = get_settings(app);
    settings.recording_retention_period
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_disable_auto_submit() {
        let settings = get_default_settings();
        assert!(!settings.auto_submit);
        assert_eq!(settings.auto_submit_key, AutoSubmitKey::Enter);
    }

    #[test]
    fn default_settings_include_asr_family_settings() {
        let settings = get_default_settings();

        assert_eq!(settings.asr_family_settings.whisper.language, "auto");
        assert!(!settings.asr_family_settings.whisper.translate_to_english);
        assert!(settings
            .asr_family_settings
            .whisper
            .custom_vocabulary
            .is_empty());
        assert_eq!(settings.asr_family_settings.qwen3_asr.language, "auto");
        assert_eq!(
            settings.asr_family_settings.qwen3_asr.max_new_tokens,
            DEFAULT_QWEN3_ASR_MAX_NEW_TOKENS
        );
        assert_eq!(
            settings.asr_family_settings.qwen3_asr.max_total_len,
            DEFAULT_QWEN3_ASR_MAX_TOTAL_LEN
        );
    }

    #[test]
    fn legacy_transcription_fields_migrate_to_whisper_family_settings() {
        let mut settings = get_default_settings();
        settings.selected_language = "zh-Hans".to_string();
        settings.translate_to_english = true;
        settings.custom_words = vec!["CustomTerm".to_string(), "Qwen".to_string()];
        settings.asr_family_settings = AsrFamilySettings::default();

        assert!(ensure_asr_family_settings_defaults(&mut settings));

        assert_eq!(settings.asr_family_settings.whisper.language, "zh-Hans");
        assert!(settings.asr_family_settings.whisper.translate_to_english);
        assert_eq!(
            settings.asr_family_settings.whisper.custom_vocabulary,
            vec!["CustomTerm".to_string(), "Qwen".to_string()]
        );
        assert_eq!(
            settings.asr_family_settings.qwen3_asr,
            Qwen3AsrFamilySettings::default()
        );
    }

    #[test]
    fn qwen_family_token_defaults_are_repaired() {
        let mut settings = get_default_settings();
        settings.asr_family_settings.qwen3_asr.language = String::new();
        settings.asr_family_settings.qwen3_asr.max_new_tokens = 256;
        settings.asr_family_settings.qwen3_asr.max_total_len = 128;

        assert!(ensure_asr_family_settings_defaults(&mut settings));

        assert_eq!(settings.asr_family_settings.qwen3_asr.language, "auto");
        assert_eq!(settings.asr_family_settings.qwen3_asr.max_new_tokens, 256);
        assert_eq!(settings.asr_family_settings.qwen3_asr.max_total_len, 256);
    }

    #[test]
    fn qwen_family_legacy_max_new_tokens_default_is_migrated() {
        let mut settings = get_default_settings();
        settings.asr_family_settings.qwen3_asr.max_new_tokens =
            LEGACY_QWEN3_ASR_MAX_NEW_TOKENS_DEFAULT;
        settings.asr_family_settings.qwen3_asr.max_total_len = DEFAULT_QWEN3_ASR_MAX_TOTAL_LEN;
        settings
            .asr_family_settings
            .qwen3_asr
            .custom_vocabulary
            .clear();

        assert!(ensure_asr_family_settings_defaults(&mut settings));

        assert_eq!(
            settings.asr_family_settings.qwen3_asr.max_new_tokens,
            DEFAULT_QWEN3_ASR_MAX_NEW_TOKENS
        );
    }

    #[test]
    fn default_settings_include_asr_providers() {
        let settings = get_default_settings();

        assert_eq!(settings.asr_provider_id, BUILT_IN_LOCAL_ASR_PROVIDER_ID);
        assert!(settings
            .asr_provider(BUILT_IN_LOCAL_ASR_PROVIDER_ID)
            .is_some());
        assert!(settings
            .asr_provider(ALIYUN_QWEN3_ASR_PROVIDER_ID)
            .is_some());
        assert!(settings
            .asr_provider(ALIYUN_QWEN3_ASR_REALTIME_PROVIDER_ID)
            .is_some());
        assert_eq!(
            settings
                .asr_models
                .get(ALIYUN_QWEN3_ASR_PROVIDER_ID)
                .map(String::as_str),
            Some(ALIYUN_QWEN3_ASR_DEFAULT_MODEL)
        );
        assert_eq!(
            settings
                .asr_models
                .get(ALIYUN_QWEN3_ASR_REALTIME_PROVIDER_ID)
                .map(String::as_str),
            Some(ALIYUN_QWEN3_ASR_REALTIME_DEFAULT_MODEL)
        );
    }

    #[test]
    fn default_settings_include_transcription_profiles() {
        let settings = get_default_settings();

        assert_eq!(
            settings.selected_transcription_profile_id.as_deref(),
            Some(DEFAULT_TRANSCRIPTION_PROFILE_ID)
        );

        let quick = settings
            .transcription_profile(DEFAULT_TRANSCRIPTION_PROFILE_ID)
            .expect("quick local profile exists");
        assert!(quick.is_builtin);
        assert_eq!(quick.asr_provider_id, BUILT_IN_LOCAL_ASR_PROVIDER_ID);
        assert_eq!(quick.asr_model, None);
        assert!(!quick.post_process_enabled);

        let realtime = settings
            .transcription_profile("qwen_realtime_dictation")
            .expect("realtime profile exists");
        assert_eq!(
            realtime.asr_provider_id,
            ALIYUN_QWEN3_ASR_REALTIME_PROVIDER_ID
        );
        assert_eq!(
            realtime.asr_model.as_deref(),
            Some(ALIYUN_QWEN3_ASR_REALTIME_DEFAULT_MODEL)
        );

        let clean = settings
            .transcription_profile("clean_message")
            .expect("clean message profile exists");
        assert!(clean.post_process_enabled);
        assert_eq!(
            clean.post_process_preset_id.as_deref(),
            Some(CLEAN_DICTATION_PRESET_ID)
        );
    }

    #[test]
    fn ensure_transcription_profile_defaults_repairs_builtins_and_preserves_custom_profiles() {
        let mut settings = get_default_settings();
        let custom_profile = TranscriptionProfile {
            id: "custom_profile".to_string(),
            name: "Custom profile".to_string(),
            description: "User owned profile".to_string(),
            asr_provider_id: BUILT_IN_LOCAL_ASR_PROVIDER_ID.to_string(),
            asr_model: None,
            language: "en".to_string(),
            translate_to_english: false,
            post_process_enabled: false,
            post_process_preset_id: None,
            is_builtin: false,
        };

        settings
            .transcription_profiles
            .retain(|profile| profile.id != "clean_message");
        settings
            .transcription_profiles
            .iter_mut()
            .find(|profile| profile.id == DEFAULT_TRANSCRIPTION_PROFILE_ID)
            .expect("quick local profile exists before mutation")
            .name = "Stale name".to_string();
        settings.transcription_profiles.push(custom_profile.clone());
        settings.selected_transcription_profile_id = Some("missing_profile".to_string());

        assert!(ensure_transcription_profile_defaults(&mut settings));

        assert_eq!(
            settings
                .transcription_profile(DEFAULT_TRANSCRIPTION_PROFILE_ID)
                .expect("quick local profile repaired")
                .name,
            "Quick input (local)"
        );
        assert!(settings.transcription_profile("clean_message").is_some());
        assert_eq!(
            settings
                .transcription_profile("custom_profile")
                .expect("custom profile preserved"),
            &custom_profile
        );
        assert_eq!(
            settings.selected_transcription_profile_id.as_deref(),
            Some(DEFAULT_TRANSCRIPTION_PROFILE_ID)
        );
    }

    #[test]
    fn default_settings_include_post_process_presets() {
        let settings = get_default_settings();

        assert_eq!(
            settings.post_process_selected_preset_id.as_deref(),
            Some(CLEAN_DICTATION_PRESET_ID)
        );
        let clean = settings
            .post_process_preset(CLEAN_DICTATION_PRESET_ID)
            .expect("clean dictation preset exists");
        assert!(clean.is_builtin);
        assert!(clean.enabled);
        assert!(settings.post_process_preset("format_as_markdown").is_some());
        assert!(settings.post_process_preset("concise_message").is_some());
        assert!(settings
            .post_process_preset("translate_to_chinese")
            .is_some());
        assert!(settings
            .post_process_preset("translate_to_english")
            .is_some());
    }

    #[test]
    fn default_settings_include_deepseek_provider_capabilities() {
        let settings = get_default_settings();
        let provider = settings
            .post_process_provider(DEEPSEEK_PROVIDER_ID)
            .expect("deepseek provider exists");

        assert_eq!(provider.base_url, "https://api.deepseek.com");
        assert_eq!(
            provider.structured_output_mode,
            PostProcessStructuredOutputMode::JsonObject
        );
        assert_eq!(
            provider.reasoning_control,
            PostProcessReasoningControl::DeepSeekThinking
        );
        assert_eq!(
            settings
                .post_process_reasoning_efforts
                .get(DEEPSEEK_PROVIDER_ID)
                .map(String::as_str),
            Some("disabled")
        );
        assert!(provider
            .model_suggestions
            .contains(&"deepseek-v4-flash".to_string()));
        assert!(provider
            .deprecated_model_suggestions
            .contains(&"deepseek-chat".to_string()));
    }

    #[test]
    fn legacy_prompts_are_migrated_to_presets() {
        let mut settings = get_default_settings();
        settings.post_process_presets.clear();
        settings.post_process_prompts = vec![LLMPrompt {
            id: "legacy_prompt".to_string(),
            name: "Legacy Prompt".to_string(),
            prompt: "Fix this: ${output}".to_string(),
        }];
        settings.post_process_selected_prompt_id = Some("legacy_prompt".to_string());
        settings.post_process_selected_preset_id = None;

        let changed = ensure_post_process_defaults(&mut settings);

        assert!(changed);
        let migrated = settings
            .post_process_preset("legacy_prompt")
            .expect("legacy prompt migrated");
        assert!(!migrated.is_builtin);
        assert_eq!(migrated.prompt_template, "Fix this: ${output}");
        assert_eq!(
            settings.post_process_selected_preset_id.as_deref(),
            Some("legacy_prompt")
        );
    }

    #[test]
    fn missing_selected_preset_uses_clean_dictation() {
        let mut settings = get_default_settings();
        settings.post_process_selected_preset_id = Some("missing".to_string());

        let changed = ensure_post_process_defaults(&mut settings);

        assert!(changed);
        assert_eq!(
            settings.post_process_selected_preset_id.as_deref(),
            Some(CLEAN_DICTATION_PRESET_ID)
        );
    }

    #[test]
    fn debug_output_redacts_api_keys() {
        let mut settings = get_default_settings();
        settings.post_process_api_keys.insert(
            "openai".to_string(),
            "redaction-test-key-openai".to_string(),
        );
        settings.post_process_api_keys.insert(
            "anthropic".to_string(),
            "redaction-test-key-anthropic".to_string(),
        );
        settings
            .post_process_api_keys
            .insert("empty_provider".to_string(), "".to_string());
        settings.asr_api_keys.insert(
            ALIYUN_QWEN3_ASR_PROVIDER_ID.to_string(),
            "dashscope-secret-key".to_string(),
        );

        let debug_output = format!("{:?}", settings);

        assert!(!debug_output.contains("redaction-test-key-openai"));
        assert!(!debug_output.contains("redaction-test-key-anthropic"));
        assert!(!debug_output.contains("dashscope-secret-key"));
        assert!(debug_output.contains("[REDACTED]"));
    }

    #[test]
    fn secret_map_debug_redacts_values() {
        let map = SecretMap(HashMap::from([("key".into(), "secret".into())]));
        let out = format!("{:?}", map);
        assert!(!out.contains("secret"));
        assert!(out.contains("[REDACTED]"));
    }
}

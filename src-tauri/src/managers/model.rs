use crate::settings::{get_settings, write_settings};
use anyhow::Result;
use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use futures_util::StreamExt;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tar::Archive;
use tauri::{AppHandle, Emitter, Manager};

const MODEL_DOWNLOAD_USER_AGENT: &str = concat!("SpeakMore/", env!("CARGO_PKG_VERSION"));
const HANDY_MODEL_DOWNLOAD_BASE_URL: &str = "https://blob.handy.computer";
const MODEL_CATALOG_JSON: &str = include_str!("../../resources/models/catalog.json");

fn model_download_url(filename: &str) -> String {
    let base_url = std::env::var("SPEAKMORE_MODEL_DOWNLOAD_BASE_URL")
        .unwrap_or_else(|_| HANDY_MODEL_DOWNLOAD_BASE_URL.to_string());
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        filename.trim_start_matches('/')
    )
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum EngineType {
    Whisper,
    Parakeet,
    Moonshine,
    MoonshineStreaming,
    SenseVoice,
    GigaAM,
    Canary,
    Cohere,
    Qwen3Asr,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub enum ModelSource {
    Handy,
    SpeakMore,
    Custom,
}

#[derive(Clone)]
struct ModelDownloadPart {
    path: &'static str,
    url: &'static str,
    sha256: &'static str,
    size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub filename: String,
    pub url: Option<String>,
    pub sha256: Option<String>,
    pub size_mb: u64,
    pub is_downloaded: bool,
    pub is_downloading: bool,
    pub partial_size: u64,
    pub is_directory: bool,
    pub engine_type: EngineType,
    pub source: ModelSource,
    pub accuracy_score: f32,        // 0.0 to 1.0, higher is more accurate
    pub speed_score: f32,           // 0.0 to 1.0, higher is faster
    pub supports_translation: bool, // Whether the model supports translating to English
    pub is_recommended: bool,       // Whether this is the recommended model for new users
    pub supported_languages: Vec<String>, // Languages this model can transcribe
    pub supports_language_selection: bool, // Whether the user can explicitly pick a language
    pub is_custom: bool,            // Whether this is a user-provided custom model
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogModelInfo {
    id: String,
    name: String,
    description: String,
    filename: String,
    download_path: Option<String>,
    url: Option<String>,
    sha256: Option<String>,
    size_mb: u64,
    is_directory: bool,
    engine_type: EngineType,
    source: ModelSource,
    accuracy_score: f32,
    speed_score: f32,
    supports_translation: bool,
    is_recommended: bool,
    supported_languages: CatalogSupportedLanguages,
    supports_language_selection: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum CatalogSupportedLanguages {
    Named(String),
    Codes(Vec<String>),
}

fn language_codes(codes: &[&str]) -> Vec<String> {
    codes.iter().map(|code| (*code).to_string()).collect()
}

fn catalog_language_set(name: &str) -> Result<Vec<String>> {
    let languages = match name {
        "whisper" => language_codes(&[
            "en", "zh", "zh-Hans", "zh-Hant", "de", "es", "ru", "ko", "fr", "ja", "pt", "tr", "pl",
            "ca", "nl", "ar", "sv", "it", "id", "hi", "fi", "vi", "he", "uk", "el", "ms", "cs",
            "ro", "da", "hu", "ta", "no", "th", "ur", "hr", "bg", "lt", "la", "mi", "ml", "cy",
            "sk", "te", "fa", "lv", "bn", "sr", "az", "sl", "kn", "et", "mk", "br", "eu", "is",
            "hy", "ne", "mn", "bs", "kk", "sq", "sw", "gl", "mr", "pa", "si", "km", "sn", "yo",
            "so", "af", "oc", "ka", "be", "tg", "sd", "gu", "am", "yi", "lo", "uz", "fo", "ht",
            "ps", "tk", "nn", "mt", "sa", "lb", "my", "bo", "tl", "mg", "as", "tt", "haw", "ln",
            "ha", "ba", "jw", "su", "yue",
        ]),
        "parakeet_v3" | "canary_1b" => language_codes(&[
            "bg", "hr", "cs", "da", "nl", "en", "et", "fi", "fr", "de", "el", "hu", "it", "lv",
            "lt", "mt", "pl", "pt", "ro", "sk", "sl", "es", "sv", "ru", "uk",
        ]),
        "sense_voice" => language_codes(&["zh", "zh-Hans", "zh-Hant", "en", "yue", "ja", "ko"]),
        "canary_flash" => language_codes(&["en", "de", "es", "fr"]),
        "cohere" => language_codes(&[
            "en", "fr", "de", "it", "es", "pt", "el", "nl", "pl", "zh", "zh-Hans", "zh-Hant", "ja",
            "ko", "vi", "ar",
        ]),
        "qwen3_asr" => language_codes(&[
            "zh", "zh-Hans", "zh-Hant", "en", "yue", "ar", "de", "fr", "es", "pt", "id", "it",
            "ko", "ru", "th", "vi", "ja", "tr", "hi", "ms", "nl", "sv", "da", "fi", "pl", "cs",
            "tl", "fa", "el", "hu", "mk", "ro",
        ]),
        _ => {
            return Err(anyhow::anyhow!(
                "Unknown model catalog language set: {}",
                name
            ))
        }
    };

    Ok(languages)
}

impl CatalogSupportedLanguages {
    fn into_language_codes(self) -> Result<Vec<String>> {
        match self {
            CatalogSupportedLanguages::Named(name) => catalog_language_set(&name),
            CatalogSupportedLanguages::Codes(codes) => Ok(codes),
        }
    }
}

fn model_display_rank(model_id: &str) -> usize {
    match model_id {
        "small" => 10,
        "medium" => 20,
        "turbo" => 30,
        "large" => 40,
        "sense-voice-int8" => 50,
        "qwen3-asr-0.6b-int8" => 60,
        "qwen3-asr-1.7b-int8" => 61,
        "moonshine-tiny-streaming-en" => 70,
        "moonshine-small-streaming-en" => 71,
        "moonshine-medium-streaming-en" => 72,
        "moonshine-base" => 73,
        "parakeet-tdt-0.6b-v2" => 80,
        "parakeet-tdt-0.6b-v3" => 81,
        "canary-180m-flash" => 90,
        "canary-1b-v2" => 91,
        "gigaam-v3-e2e-ctc" => 100,
        "cohere-int8" => 110,
        "breeze-asr" => 120,
        _ => 10_000,
    }
}

fn model_download_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(MODEL_DOWNLOAD_USER_AGENT)
        .build()
        .map_err(Into::into)
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DownloadProgress {
    pub model_id: String,
    pub downloaded: u64,
    pub total: u64,
    pub percentage: f64,
}

/// RAII guard that cleans up download state (`is_downloading` flag and cancel flag)
/// when dropped, unless explicitly disarmed. This ensures consistent cleanup on
/// every error path without requiring manual cleanup at each `?` or `return Err`.
struct DownloadCleanup<'a> {
    available_models: &'a Mutex<HashMap<String, ModelInfo>>,
    cancel_flags: &'a Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    model_id: String,
    disarmed: bool,
}

impl<'a> Drop for DownloadCleanup<'a> {
    fn drop(&mut self) {
        if self.disarmed {
            return;
        }
        {
            let mut models = self.available_models.lock().unwrap();
            if let Some(model) = models.get_mut(self.model_id.as_str()) {
                model.is_downloading = false;
            }
        }
        self.cancel_flags.lock().unwrap().remove(&self.model_id);
    }
}

pub struct ModelManager {
    app_handle: AppHandle,
    models_dir: PathBuf,
    available_models: Mutex<HashMap<String, ModelInfo>>,
    cancel_flags: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    extracting_models: Arc<Mutex<HashSet<String>>>,
}

impl ModelManager {
    fn load_builtin_model_catalog() -> Result<HashMap<String, ModelInfo>> {
        let catalog_models: Vec<CatalogModelInfo> = serde_json::from_str(MODEL_CATALOG_JSON)?;
        let mut available_models = HashMap::new();

        for catalog_model in catalog_models {
            let id = catalog_model.id;
            if available_models.contains_key(&id) {
                return Err(anyhow::anyhow!("Duplicate model catalog id: {}", id));
            }

            let url = match (catalog_model.download_path, catalog_model.url) {
                (Some(download_path), None) => Some(model_download_url(&download_path)),
                (None, Some(url)) => Some(url),
                (None, None) => None,
                (Some(_), Some(_)) => {
                    return Err(anyhow::anyhow!(
                        "Model catalog entry '{}' cannot define both download_path and url",
                        id
                    ))
                }
            };

            available_models.insert(
                id.clone(),
                ModelInfo {
                    id,
                    name: catalog_model.name,
                    description: catalog_model.description,
                    filename: catalog_model.filename,
                    url,
                    sha256: catalog_model.sha256,
                    size_mb: catalog_model.size_mb,
                    is_downloaded: false,
                    is_downloading: false,
                    partial_size: 0,
                    is_directory: catalog_model.is_directory,
                    engine_type: catalog_model.engine_type,
                    source: catalog_model.source,
                    accuracy_score: catalog_model.accuracy_score,
                    speed_score: catalog_model.speed_score,
                    supports_translation: catalog_model.supports_translation,
                    is_recommended: catalog_model.is_recommended,
                    supported_languages: catalog_model.supported_languages.into_language_codes()?,
                    supports_language_selection: catalog_model.supports_language_selection,
                    is_custom: false,
                },
            );
        }

        Ok(available_models)
    }

    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        // Create models directory in app data
        let models_dir = crate::portable::app_data_dir(app_handle)
            .map_err(|e| anyhow::anyhow!("Failed to get app data dir: {}", e))?
            .join("models");

        if !models_dir.exists() {
            fs::create_dir_all(&models_dir)?;
        }

        let mut available_models = Self::load_builtin_model_catalog()?;

        // Auto-discover custom Whisper models (.bin files) in the models directory
        if let Err(e) = Self::discover_custom_whisper_models(&models_dir, &mut available_models) {
            warn!("Failed to discover custom models: {}", e);
        }

        let manager = Self {
            app_handle: app_handle.clone(),
            models_dir,
            available_models: Mutex::new(available_models),
            cancel_flags: Arc::new(Mutex::new(HashMap::new())),
            extracting_models: Arc::new(Mutex::new(HashSet::new())),
        };

        // Migrate any bundled models to user directory
        manager.migrate_bundled_models()?;

        // Migrate GigaAM from single-file to directory format
        manager.migrate_gigaam_to_directory()?;

        // Check which models are already downloaded
        manager.update_download_status()?;

        // Auto-select a model if none is currently selected
        manager.auto_select_model_if_needed()?;

        Ok(manager)
    }

    pub fn get_available_models(&self) -> Vec<ModelInfo> {
        let models = self.available_models.lock().unwrap();
        let mut models = models.values().cloned().collect::<Vec<_>>();
        models.sort_by(|a, b| {
            model_display_rank(&a.id)
                .cmp(&model_display_rank(&b.id))
                .then_with(|| a.name.cmp(&b.name))
        });
        models
    }

    pub fn get_model_info(&self, model_id: &str) -> Option<ModelInfo> {
        let models = self.available_models.lock().unwrap();
        models.get(model_id).cloned()
    }

    fn migrate_bundled_models(&self) -> Result<()> {
        // Check for bundled models and copy them to user directory
        let bundled_models = ["ggml-small.bin"]; // Add other bundled models here if any

        for filename in &bundled_models {
            let bundled_path = self.app_handle.path().resolve(
                format!("resources/models/{}", filename),
                tauri::path::BaseDirectory::Resource,
            );

            if let Ok(bundled_path) = bundled_path {
                if bundled_path.exists() {
                    let user_path = self.models_dir.join(filename);

                    // Only copy if user doesn't already have the model
                    if !user_path.exists() {
                        info!("Migrating bundled model {} to user directory", filename);
                        fs::copy(&bundled_path, &user_path)?;
                        info!("Successfully migrated {}", filename);
                    }
                }
            }
        }

        Ok(())
    }

    /// Migrate GigaAM from the old single-file format (giga-am-v3.int8.onnx)
    /// to the new directory format (giga-am-v3-int8/model.int8.onnx + vocab.txt).
    /// This was required by the transcribe-rs 0.3.x upgrade.
    fn migrate_gigaam_to_directory(&self) -> Result<()> {
        let old_file = self.models_dir.join("giga-am-v3.int8.onnx");
        let new_dir = self.models_dir.join("giga-am-v3-int8");

        if !old_file.exists() || new_dir.exists() {
            return Ok(());
        }

        info!("Migrating GigaAM from single-file to directory format");

        let vocab_path = self
            .app_handle
            .path()
            .resolve(
                "resources/models/gigaam_vocab.txt",
                tauri::path::BaseDirectory::Resource,
            )
            .map_err(|e| anyhow::anyhow!("Failed to resolve GigaAM vocab path: {}", e))?;

        info!(
            "Resolved vocab path: {:?} (exists: {})",
            vocab_path,
            vocab_path.exists()
        );
        info!("Old file: {:?} (exists: {})", old_file, old_file.exists());
        info!("New dir: {:?} (exists: {})", new_dir, new_dir.exists());

        fs::create_dir_all(&new_dir)?;
        fs::rename(&old_file, new_dir.join("model.int8.onnx"))?;
        fs::copy(&vocab_path, new_dir.join("vocab.txt"))?;

        // Clean up old partial file if it exists
        let old_partial = self.models_dir.join("giga-am-v3.int8.onnx.partial");
        if old_partial.exists() {
            let _ = fs::remove_file(&old_partial);
        }

        info!("GigaAM migration complete");
        Ok(())
    }

    fn update_download_status(&self) -> Result<()> {
        let mut models = self.available_models.lock().unwrap();

        for model in models.values_mut() {
            if model.is_directory {
                // For directory-based models, check if the directory exists
                let model_path = self.models_dir.join(&model.filename);
                let partial_path = self.models_dir.join(format!("{}.partial", &model.filename));
                let extracting_path = self
                    .models_dir
                    .join(format!("{}.extracting", &model.filename));

                // Clean up any leftover .extracting directories from interrupted extractions
                // But only if this model is NOT currently being extracted
                let is_currently_extracting = {
                    let extracting = self.extracting_models.lock().unwrap();
                    extracting.contains(&model.id)
                };
                if extracting_path.exists() && !is_currently_extracting {
                    warn!("Cleaning up interrupted extraction for model: {}", model.id);
                    let _ = fs::remove_dir_all(&extracting_path);
                }

                model.is_downloaded = if model_path.exists() && model_path.is_dir() {
                    !matches!(model.engine_type, EngineType::Qwen3Asr)
                        || Self::validate_qwen3_asr_model_dir(&model_path).is_ok()
                } else {
                    false
                };
                model.is_downloading = false;

                // Get partial file size if it exists (for the .tar.gz being downloaded)
                if partial_path.exists() {
                    model.partial_size = partial_path.metadata().map(|m| m.len()).unwrap_or(0);
                } else {
                    model.partial_size = 0;
                }
            } else {
                // For file-based models (existing logic)
                let model_path = self.models_dir.join(&model.filename);
                let partial_path = self.models_dir.join(format!("{}.partial", &model.filename));

                model.is_downloaded = model_path.exists();
                model.is_downloading = false;

                // Get partial file size if it exists
                if partial_path.exists() {
                    model.partial_size = partial_path.metadata().map(|m| m.len()).unwrap_or(0);
                } else {
                    model.partial_size = 0;
                }
            }
        }

        Ok(())
    }

    fn auto_select_model_if_needed(&self) -> Result<()> {
        let mut settings = get_settings(&self.app_handle);

        // Clear stale selection: selected model is set but doesn't exist
        // in available_models (e.g. deleted custom model file)
        if !settings.selected_model.is_empty() {
            let models = self.available_models.lock().unwrap();
            let exists = models.contains_key(&settings.selected_model);
            drop(models);

            if !exists {
                info!(
                    "Selected model '{}' not found in available models, clearing selection",
                    settings.selected_model
                );
                settings.selected_model = String::new();
                write_settings(&self.app_handle, settings.clone());
            }
        }

        // If no model is selected, pick the first downloaded one
        if settings.selected_model.is_empty() {
            // Find the first available (downloaded) model
            let models = self.available_models.lock().unwrap();
            if let Some(available_model) = models.values().find(|model| model.is_downloaded) {
                info!(
                    "Auto-selecting model: {} ({})",
                    available_model.id, available_model.name
                );

                // Update settings with the selected model
                let mut updated_settings = settings;
                updated_settings.selected_model = available_model.id.clone();
                write_settings(&self.app_handle, updated_settings);

                info!("Successfully auto-selected model: {}", available_model.id);
            }
        }

        Ok(())
    }

    /// Discover custom Whisper models (.bin files) in the models directory.
    /// Skips files that match predefined model filenames.
    fn discover_custom_whisper_models(
        models_dir: &Path,
        available_models: &mut HashMap<String, ModelInfo>,
    ) -> Result<()> {
        if !models_dir.exists() {
            return Ok(());
        }

        // Collect filenames of predefined Whisper file-based models to skip
        let predefined_filenames: HashSet<String> = available_models
            .values()
            .filter(|m| matches!(m.engine_type, EngineType::Whisper) && !m.is_directory)
            .map(|m| m.filename.clone())
            .collect();

        // Scan models directory for .bin files
        for entry in fs::read_dir(models_dir)? {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    warn!("Failed to read directory entry: {}", e);
                    continue;
                }
            };

            let path = entry.path();

            // Only process .bin files (not directories)
            if !path.is_file() {
                continue;
            }

            let filename = match path.file_name().and_then(|s| s.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            // Skip hidden files
            if filename.starts_with('.') {
                continue;
            }

            // Only process .bin files (Whisper GGML format).
            // This also excludes .partial downloads (e.g., "model.bin.partial").
            // If we add discovery for other formats, add a .partial check before this filter.
            if !filename.ends_with(".bin") {
                continue;
            }

            // Skip predefined model files
            if predefined_filenames.contains(&filename) {
                continue;
            }

            // Generate model ID from filename (remove .bin extension)
            let model_id = filename.trim_end_matches(".bin").to_string();

            // Skip if model ID already exists (shouldn't happen, but be safe)
            if available_models.contains_key(&model_id) {
                continue;
            }

            // Generate display name: replace - and _ with space, capitalize words
            let display_name = model_id
                .replace(['-', '_'], " ")
                .split_whitespace()
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            // Get file size in MB
            let size_mb = match path.metadata() {
                Ok(meta) => meta.len() / (1024 * 1024),
                Err(e) => {
                    warn!("Failed to get metadata for {}: {}", filename, e);
                    0
                }
            };

            info!(
                "Discovered custom Whisper model: {} ({}, {} MB)",
                model_id, filename, size_mb
            );

            available_models.insert(
                model_id.clone(),
                ModelInfo {
                    id: model_id,
                    name: display_name,
                    description: "Not officially supported".to_string(),
                    filename,
                    url: None,    // Custom models have no download URL
                    sha256: None, // Custom models skip verification
                    size_mb,
                    is_downloaded: true, // Already present on disk
                    is_downloading: false,
                    partial_size: 0,
                    is_directory: false,
                    engine_type: EngineType::Whisper,
                    source: ModelSource::Custom,
                    accuracy_score: 0.0, // Sentinel: UI hides score bars when both are 0
                    speed_score: 0.0,
                    supports_translation: false,
                    is_recommended: false,
                    supported_languages: vec![],
                    supports_language_selection: true,
                    is_custom: true,
                },
            );
        }

        Ok(())
    }

    /// Verifies the SHA256 of `path` against `expected_sha256` (if provided).
    /// On mismatch or read error the partial file is deleted and an error is returned,
    /// so the next download attempt always starts from a clean state.
    /// When `expected_sha256` is `None` (custom user models) verification is skipped.
    fn verify_sha256(path: &Path, expected_sha256: Option<&str>, model_id: &str) -> Result<()> {
        let Some(expected) = expected_sha256 else {
            return Ok(());
        };
        match Self::compute_sha256(path) {
            Ok(actual) if actual == expected => {
                info!("SHA256 verified for model {}", model_id);
                Ok(())
            }
            Ok(actual) => {
                warn!(
                    "SHA256 mismatch for model {}: expected {}, got {}",
                    model_id, expected, actual
                );
                let _ = fs::remove_file(path);
                Err(anyhow::anyhow!(
                    "Download verification failed for model {}: file is corrupt. Please retry.",
                    model_id
                ))
            }
            Err(e) => {
                let _ = fs::remove_file(path);
                Err(anyhow::anyhow!(
                    "Failed to verify download for model {}: {}. Please retry.",
                    model_id,
                    e
                ))
            }
        }
    }

    /// Computes the SHA256 hex digest of a file, reading in 64KB chunks to handle large models.
    fn compute_sha256(path: &Path) -> Result<String> {
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 65536];
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
        Ok(format!("{:x}", hasher.finalize()))
    }

    fn qwen3_asr_download_parts(model_id: &str) -> Option<Vec<ModelDownloadPart>> {
        let tokenizer = [
            ModelDownloadPart {
                path: "tokenizer/merges.txt",
                url: concat!(
                    "https://modelscope.cn/models/zengshuishui/Qwen3-ASR-onnx/resolve/master",
                    "/tokenizer/merges.txt"
                ),
                sha256: "8831e4f1a044471340f7c0a83d7bd71306a5b867e95fd870f74d0c5308a904d5",
                size: 1_671_853,
            },
            ModelDownloadPart {
                path: "tokenizer/tokenizer_config.json",
                url: concat!(
                    "https://modelscope.cn/models/zengshuishui/Qwen3-ASR-onnx/resolve/master",
                    "/tokenizer/tokenizer_config.json"
                ),
                sha256: "4942d005604266809309cabc9f4e9cb89ce855d59b14681fdc0e1cc62ea26c4c",
                size: 12_487,
            },
            ModelDownloadPart {
                path: "tokenizer/vocab.json",
                url: concat!(
                    "https://modelscope.cn/models/zengshuishui/Qwen3-ASR-onnx/resolve/master",
                    "/tokenizer/vocab.json"
                ),
                sha256: "ca10d7e9fb3ed18575dd1e277a2579c16d108e32f27439684afa0e10b1440910",
                size: 2_776_833,
            },
        ];

        let mut parts = match model_id {
            "qwen3-asr-0.6b-int8" => vec![
                ModelDownloadPart {
                    path: "conv_frontend.onnx",
                    url: concat!(
                        "https://modelscope.cn/models/zengshuishui/Qwen3-ASR-onnx/resolve/master",
                        "/model_0.6B/conv_frontend.onnx"
                    ),
                    sha256: "d22dc4423e0940e49884e903d2ea2f7e5567c14fc1aed97e4e26d6b8f208ef9e",
                    size: 44_148_281,
                },
                ModelDownloadPart {
                    path: "encoder.int8.onnx",
                    url: concat!(
                        "https://modelscope.cn/models/zengshuishui/Qwen3-ASR-onnx/resolve/master",
                        "/model_0.6B/encoder.int8.onnx"
                    ),
                    sha256: "60748d3e6744a57c9c91e1b17424a6c2990567e8adceb0783940c03ed98fa9d9",
                    size: 182_491_662,
                },
                ModelDownloadPart {
                    path: "decoder.int8.onnx",
                    url: concat!(
                        "https://modelscope.cn/models/zengshuishui/Qwen3-ASR-onnx/resolve/master",
                        "/model_0.6B/decoder.int8.onnx"
                    ),
                    sha256: "4f6885be5959ae26af3089d38ee7972c5fafbeeb1cf8d5e76eab6d8b61ca5771",
                    size: 755_914_231,
                },
            ],
            "qwen3-asr-1.7b-int8" => vec![
                ModelDownloadPart {
                    path: "conv_frontend.onnx",
                    url: concat!(
                        "https://modelscope.cn/models/zengshuishui/Qwen3-ASR-onnx/resolve/master",
                        "/model_1.7B/conv_frontend.onnx"
                    ),
                    sha256: "fa894a4ba53da6a4238f2a6ca0b09362e505d39cecbd646051b033e2e8d7e2fb",
                    size: 48_080_441,
                },
                ModelDownloadPart {
                    path: "encoder.int8.onnx",
                    url: concat!(
                        "https://modelscope.cn/models/zengshuishui/Qwen3-ASR-onnx/resolve/master",
                        "/model_1.7B/encoder.int8.onnx"
                    ),
                    sha256: "436fbd910a0c8914851e5ac1354e807be9f283d08a5da728adaa609731c41469",
                    size: 314_222_162,
                },
                ModelDownloadPart {
                    path: "decoder.int8.onnx",
                    url: concat!(
                        "https://modelscope.cn/models/zengshuishui/Qwen3-ASR-onnx/resolve/master",
                        "/model_1.7B/decoder.int8.onnx"
                    ),
                    sha256: "c43c853fa6e97d08365cb8a5502b360b595cd43c00dc60e4d8ca7cc18cad460b",
                    size: 2_037_458_645,
                },
            ],
            _ => return None,
        };

        parts.extend(tokenizer);
        Some(parts)
    }

    pub fn validate_qwen3_asr_model_dir(model_dir: &Path) -> Result<()> {
        let required_files = [
            "conv_frontend.onnx",
            "encoder.int8.onnx",
            "decoder.int8.onnx",
            "tokenizer/vocab.json",
            "tokenizer/merges.txt",
            "tokenizer/tokenizer_config.json",
        ];

        for file in required_files {
            let path = model_dir.join(file);
            if !path.is_file() {
                return Err(anyhow::anyhow!(
                    "Qwen3-ASR model is incomplete: missing {}",
                    file
                ));
            }
        }

        Ok(())
    }

    async fn download_model_parts(
        &self,
        model_id: &str,
        model_info: &ModelInfo,
        parts: Vec<ModelDownloadPart>,
    ) -> Result<()> {
        let final_model_dir = self.models_dir.join(&model_info.filename);
        let partial_dir = self
            .models_dir
            .join(format!("{}.partial", &model_info.filename));

        if final_model_dir.exists() {
            Self::validate_qwen3_asr_model_dir(&final_model_dir)?;
            if partial_dir.exists() {
                let _ = fs::remove_dir_all(&partial_dir);
            }
            self.update_download_status()?;
            return Ok(());
        }

        {
            let mut models = self.available_models.lock().unwrap();
            if let Some(model) = models.get_mut(model_id) {
                model.is_downloading = true;
            }
        }

        let cancel_flag = Arc::new(AtomicBool::new(false));
        {
            let mut flags = self.cancel_flags.lock().unwrap();
            flags.insert(model_id.to_string(), cancel_flag.clone());
        }

        let mut cleanup = DownloadCleanup {
            available_models: &self.available_models,
            cancel_flags: &self.cancel_flags,
            model_id: model_id.to_string(),
            disarmed: false,
        };

        fs::create_dir_all(&partial_dir)?;

        let total_size: u64 = parts.iter().map(|part| part.size).sum();
        let client = model_download_client()?;
        let mut completed_size = 0u64;
        let mut last_emit = Instant::now();
        let throttle_duration = Duration::from_millis(100);

        for part in parts {
            let target_path = partial_dir.join(part.path);
            let part_partial_path = partial_dir.join(format!("{}.partial", part.path));

            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            if let Some(parent) = part_partial_path.parent() {
                fs::create_dir_all(parent)?;
            }

            if target_path.exists() {
                match Self::verify_sha256(&target_path, Some(part.sha256), model_id) {
                    Ok(()) => {
                        completed_size += target_path.metadata()?.len();
                        continue;
                    }
                    Err(_) => {
                        let _ = fs::remove_file(&target_path);
                    }
                }
            }

            let mut resume_from = if part_partial_path.exists() {
                part_partial_path.metadata()?.len()
            } else {
                0
            };

            let mut request = client.get(part.url);
            if resume_from > 0 {
                request = request.header("Range", format!("bytes={}-", resume_from));
            }

            let mut response = request.send().await?;
            if resume_from > 0 && response.status() == reqwest::StatusCode::OK {
                warn!(
                    "Server doesn't support range requests for model part {}, restarting",
                    part.path
                );
                drop(response);
                let _ = fs::remove_file(&part_partial_path);
                resume_from = 0;
                response = client.get(part.url).send().await?;
            }

            if !response.status().is_success()
                && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
            {
                return Err(anyhow::anyhow!(
                    "Failed to download model part {}: HTTP {}",
                    part.path,
                    response.status()
                ));
            }

            let mut file = if resume_from > 0 {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&part_partial_path)?
            } else {
                std::fs::File::create(&part_partial_path)?
            };

            let mut downloaded_for_part = resume_from;
            let mut stream = response.bytes_stream();

            while let Some(chunk) = stream.next().await {
                if cancel_flag.load(Ordering::Relaxed) {
                    drop(file);
                    info!("Download cancelled for: {}", model_id);
                    return Ok(());
                }

                let chunk = chunk?;
                file.write_all(&chunk)?;
                downloaded_for_part += chunk.len() as u64;

                if last_emit.elapsed() >= throttle_duration {
                    let downloaded = completed_size + downloaded_for_part;
                    let progress = DownloadProgress {
                        model_id: model_id.to_string(),
                        downloaded,
                        total: total_size,
                        percentage: if total_size > 0 {
                            (downloaded as f64 / total_size as f64) * 100.0
                        } else {
                            0.0
                        },
                    };
                    let _ = self.app_handle.emit("model-download-progress", &progress);
                    last_emit = Instant::now();
                }
            }

            file.flush()?;
            drop(file);

            if downloaded_for_part != part.size {
                let _ = fs::remove_file(&part_partial_path);
                return Err(anyhow::anyhow!(
                    "Download incomplete for {}: expected {} bytes, got {} bytes",
                    part.path,
                    part.size,
                    downloaded_for_part
                ));
            }

            let _ = self.app_handle.emit("model-verification-started", model_id);
            Self::verify_sha256(&part_partial_path, Some(part.sha256), model_id)?;
            let _ = self
                .app_handle
                .emit("model-verification-completed", model_id);

            fs::rename(&part_partial_path, &target_path)?;
            completed_size += part.size;
        }

        Self::validate_qwen3_asr_model_dir(&partial_dir)?;

        if final_model_dir.exists() {
            fs::remove_dir_all(&final_model_dir)?;
        }
        fs::rename(&partial_dir, &final_model_dir)?;

        cleanup.disarmed = true;
        {
            let mut models = self.available_models.lock().unwrap();
            if let Some(model) = models.get_mut(model_id) {
                model.is_downloading = false;
                model.is_downloaded = true;
                model.partial_size = 0;
            }
        }
        self.cancel_flags.lock().unwrap().remove(model_id);

        let final_progress = DownloadProgress {
            model_id: model_id.to_string(),
            downloaded: total_size,
            total: total_size,
            percentage: 100.0,
        };
        let _ = self
            .app_handle
            .emit("model-download-progress", &final_progress);
        let _ = self.app_handle.emit("model-download-complete", model_id);

        info!(
            "Successfully downloaded model {} to {:?}",
            model_id, final_model_dir
        );

        Ok(())
    }

    pub async fn download_model(&self, model_id: &str) -> Result<()> {
        let model_info = {
            let models = self.available_models.lock().unwrap();
            models.get(model_id).cloned()
        };

        let model_info =
            model_info.ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_id))?;

        if let Some(parts) = Self::qwen3_asr_download_parts(model_id) {
            return self
                .download_model_parts(model_id, &model_info, parts)
                .await;
        }

        let url = model_info
            .url
            .ok_or_else(|| anyhow::anyhow!("No download URL for model"))?;
        let model_path = self.models_dir.join(&model_info.filename);
        let partial_path = self
            .models_dir
            .join(format!("{}.partial", &model_info.filename));

        // Don't download if complete version already exists
        if model_path.exists() {
            // Clean up any partial file that might exist
            if partial_path.exists() {
                let _ = fs::remove_file(&partial_path);
            }
            self.update_download_status()?;
            return Ok(());
        }

        // Check if we have a partial download to resume
        let mut resume_from = if partial_path.exists() {
            let size = partial_path.metadata()?.len();
            info!("Resuming download of model {} from byte {}", model_id, size);
            size
        } else {
            info!("Starting fresh download of model {} from {}", model_id, url);
            0
        };

        // Mark as downloading
        {
            let mut models = self.available_models.lock().unwrap();
            if let Some(model) = models.get_mut(model_id) {
                model.is_downloading = true;
            }
        }

        // Create cancellation flag for this download
        let cancel_flag = Arc::new(AtomicBool::new(false));
        {
            let mut flags = self.cancel_flags.lock().unwrap();
            flags.insert(model_id.to_string(), cancel_flag.clone());
        }

        // Guard ensures is_downloading and cancel_flags are cleaned up on every
        // error path. Disarmed only on success (which sets is_downloaded = true).
        let mut cleanup = DownloadCleanup {
            available_models: &self.available_models,
            cancel_flags: &self.cancel_flags,
            model_id: model_id.to_string(),
            disarmed: false,
        };

        // Create HTTP client with range request for resuming
        let client = model_download_client()?;
        let mut request = client.get(&url);

        if resume_from > 0 {
            request = request.header("Range", format!("bytes={}-", resume_from));
        }

        let mut response = request.send().await?;

        // If we tried to resume but server returned 200 (not 206 Partial Content),
        // the server doesn't support range requests. Delete partial file and restart
        // fresh to avoid file corruption (appending full file to partial).
        if resume_from > 0 && response.status() == reqwest::StatusCode::OK {
            warn!(
                "Server doesn't support range requests for model {}, restarting download",
                model_id
            );
            drop(response);
            let _ = fs::remove_file(&partial_path);

            // Reset resume_from since we're starting fresh
            resume_from = 0;

            // Restart download without range header
            response = client.get(&url).send().await?;
        }

        // Check for success or partial content status
        if !response.status().is_success()
            && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
        {
            return Err(anyhow::anyhow!(
                "Failed to download model: HTTP {}",
                response.status()
            ));
        }

        let total_size = if resume_from > 0 {
            // For resumed downloads, add the resume point to content length
            resume_from + response.content_length().unwrap_or(0)
        } else {
            response.content_length().unwrap_or(0)
        };

        let mut downloaded = resume_from;
        let mut stream = response.bytes_stream();

        // Open file for appending if resuming, or create new if starting fresh
        let mut file = if resume_from > 0 {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&partial_path)?
        } else {
            std::fs::File::create(&partial_path)?
        };

        // Emit initial progress
        let initial_progress = DownloadProgress {
            model_id: model_id.to_string(),
            downloaded,
            total: total_size,
            percentage: if total_size > 0 {
                (downloaded as f64 / total_size as f64) * 100.0
            } else {
                0.0
            },
        };
        let _ = self
            .app_handle
            .emit("model-download-progress", &initial_progress);

        // Throttle progress events to max 10/sec (100ms intervals)
        let mut last_emit = Instant::now();
        let throttle_duration = Duration::from_millis(100);

        // Download with progress
        while let Some(chunk) = stream.next().await {
            // Check if download was cancelled
            if cancel_flag.load(Ordering::Relaxed) {
                drop(file);
                info!("Download cancelled for: {}", model_id);
                // Keep partial file for resume functionality.
                // Guard handles is_downloading + cancel_flags cleanup on drop.
                return Ok(());
            }

            let chunk = chunk?;

            file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;

            let percentage = if total_size > 0 {
                (downloaded as f64 / total_size as f64) * 100.0
            } else {
                0.0
            };

            // Emit progress event (throttled to avoid UI freeze)
            if last_emit.elapsed() >= throttle_duration {
                let progress = DownloadProgress {
                    model_id: model_id.to_string(),
                    downloaded,
                    total: total_size,
                    percentage,
                };
                let _ = self.app_handle.emit("model-download-progress", &progress);
                last_emit = Instant::now();
            }
        }

        // Emit final progress to ensure 100% is shown
        let final_progress = DownloadProgress {
            model_id: model_id.to_string(),
            downloaded,
            total: total_size,
            percentage: if total_size > 0 {
                (downloaded as f64 / total_size as f64) * 100.0
            } else {
                100.0
            },
        };
        let _ = self
            .app_handle
            .emit("model-download-progress", &final_progress);

        file.flush()?;
        drop(file); // Ensure file is closed before moving

        // Verify downloaded file size matches expected size
        if total_size > 0 {
            let actual_size = partial_path.metadata()?.len();
            if actual_size != total_size {
                // Download is incomplete/corrupted - delete partial and return error
                let _ = fs::remove_file(&partial_path);
                return Err(anyhow::anyhow!(
                    "Download incomplete: expected {} bytes, got {} bytes",
                    total_size,
                    actual_size
                ));
            }
        }

        // Verify SHA256 checksum. Runs in a blocking thread so the async executor is not
        // stalled while hashing large model files (up to 1.6 GB). On failure the partial
        // is deleted inside verify_sha256 so the next attempt always starts fresh.
        let _ = self.app_handle.emit("model-verification-started", model_id);
        info!("Verifying SHA256 for model {}...", model_id);
        let verify_path = partial_path.clone();
        let verify_expected = model_info.sha256.clone();
        let verify_model_id = model_id.to_string();
        let verify_result = tokio::task::spawn_blocking(move || {
            Self::verify_sha256(&verify_path, verify_expected.as_deref(), &verify_model_id)
        })
        .await
        .map_err(|e| anyhow::anyhow!("SHA256 task panicked: {}", e))?;
        verify_result?;
        let _ = self
            .app_handle
            .emit("model-verification-completed", model_id);

        // Handle directory-based models (extract tar.gz) vs file-based models
        if model_info.is_directory {
            // Track that this model is being extracted
            {
                let mut extracting = self.extracting_models.lock().unwrap();
                extracting.insert(model_id.to_string());
            }

            // Emit extraction started event
            let _ = self.app_handle.emit("model-extraction-started", model_id);
            info!("Extracting archive for directory-based model: {}", model_id);

            // Use a temporary extraction directory to ensure atomic operations
            let temp_extract_dir = self
                .models_dir
                .join(format!("{}.extracting", &model_info.filename));
            let final_model_dir = self.models_dir.join(&model_info.filename);

            // Clean up any previous incomplete extraction
            if temp_extract_dir.exists() {
                let _ = fs::remove_dir_all(&temp_extract_dir);
            }

            // Create temporary extraction directory
            fs::create_dir_all(&temp_extract_dir)?;

            // Open the downloaded tar archive. Most existing models are .tar.gz;
            // some sherpa-onnx upstream models are published as .tar.bz2.
            let archive_file = File::open(&partial_path)?;
            let archive_reader: Box<dyn Read> = if url.ends_with(".tar.bz2") {
                Box::new(BzDecoder::new(archive_file))
            } else {
                Box::new(GzDecoder::new(archive_file))
            };
            let mut archive = Archive::new(archive_reader);

            // Extract to the temporary directory first
            archive.unpack(&temp_extract_dir).map_err(|e| {
                let error_msg = format!("Failed to extract archive: {}", e);
                // Clean up failed extraction
                let _ = fs::remove_dir_all(&temp_extract_dir);
                // Delete the corrupt partial file so the next download attempt starts fresh
                // instead of resuming from a broken archive (issue #858).
                let _ = fs::remove_file(&partial_path);
                // Remove from extracting set
                {
                    let mut extracting = self.extracting_models.lock().unwrap();
                    extracting.remove(model_id);
                }
                let _ = self.app_handle.emit(
                    "model-extraction-failed",
                    &serde_json::json!({
                        "model_id": model_id,
                        "error": error_msg
                    }),
                );
                anyhow::anyhow!(error_msg)
            })?;

            // Find the actual extracted directory (archive might have a nested structure)
            let extracted_dirs: Vec<_> = fs::read_dir(&temp_extract_dir)?
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
                .collect();

            if extracted_dirs.len() == 1 {
                // Single directory extracted, move it to the final location
                let source_dir = extracted_dirs[0].path();
                if final_model_dir.exists() {
                    fs::remove_dir_all(&final_model_dir)?;
                }
                fs::rename(&source_dir, &final_model_dir)?;
                // Clean up temp directory
                let _ = fs::remove_dir_all(&temp_extract_dir);
            } else {
                // Multiple items or no directories, rename the temp directory itself
                if final_model_dir.exists() {
                    fs::remove_dir_all(&final_model_dir)?;
                }
                fs::rename(&temp_extract_dir, &final_model_dir)?;
            }

            info!("Successfully extracted archive for model: {}", model_id);
            // Remove from extracting set
            {
                let mut extracting = self.extracting_models.lock().unwrap();
                extracting.remove(model_id);
            }
            // Emit extraction completed event
            let _ = self.app_handle.emit("model-extraction-completed", model_id);

            // Remove the downloaded tar.gz file
            let _ = fs::remove_file(&partial_path);
        } else {
            // Move partial file to final location for file-based models
            fs::rename(&partial_path, &model_path)?;
        }

        // Disarm the guard — success path does its own cleanup because it
        // additionally sets is_downloaded = true.
        cleanup.disarmed = true;
        {
            let mut models = self.available_models.lock().unwrap();
            if let Some(model) = models.get_mut(model_id) {
                model.is_downloading = false;
                model.is_downloaded = true;
                model.partial_size = 0;
            }
        }
        self.cancel_flags.lock().unwrap().remove(model_id);

        // Emit completion event
        let _ = self.app_handle.emit("model-download-complete", model_id);

        info!(
            "Successfully downloaded model {} to {:?}",
            model_id, model_path
        );

        Ok(())
    }

    pub fn delete_model(&self, model_id: &str) -> Result<()> {
        debug!("ModelManager: delete_model called for: {}", model_id);

        let model_info = {
            let models = self.available_models.lock().unwrap();
            models.get(model_id).cloned()
        };

        let model_info =
            model_info.ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_id))?;

        debug!("ModelManager: Found model info: {:?}", model_info);

        let model_path = self.models_dir.join(&model_info.filename);
        let partial_path = self
            .models_dir
            .join(format!("{}.partial", &model_info.filename));
        debug!("ModelManager: Model path: {:?}", model_path);
        debug!("ModelManager: Partial path: {:?}", partial_path);

        let mut deleted_something = false;

        if model_info.is_directory {
            // Delete complete model directory if it exists
            if model_path.exists() && model_path.is_dir() {
                info!("Deleting model directory at: {:?}", model_path);
                fs::remove_dir_all(&model_path)?;
                info!("Model directory deleted successfully");
                deleted_something = true;
            }
        } else {
            // Delete complete model file if it exists
            if model_path.exists() {
                info!("Deleting model file at: {:?}", model_path);
                fs::remove_file(&model_path)?;
                info!("Model file deleted successfully");
                deleted_something = true;
            }
        }

        // Delete partial file or directory if it exists.
        if partial_path.exists() {
            info!("Deleting partial download at: {:?}", partial_path);
            if partial_path.is_dir() {
                fs::remove_dir_all(&partial_path)?;
            } else {
                fs::remove_file(&partial_path)?;
            }
            info!("Partial download deleted successfully");
            deleted_something = true;
        }

        if !deleted_something {
            return Err(anyhow::anyhow!("No model files found to delete"));
        }

        // Custom models should be removed from the list entirely since they
        // have no download URL and can't be re-downloaded
        if model_info.is_custom {
            let mut models = self.available_models.lock().unwrap();
            models.remove(model_id);
            debug!("ModelManager: removed custom model from available models");
        } else {
            // Update download status (marks predefined models as not downloaded)
            self.update_download_status()?;
            debug!("ModelManager: download status updated");
        }

        // Emit event to notify UI
        let _ = self.app_handle.emit("model-deleted", model_id);

        Ok(())
    }

    pub fn get_model_path(&self, model_id: &str) -> Result<PathBuf> {
        let model_info = self
            .get_model_info(model_id)
            .ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_id))?;

        if !model_info.is_downloaded {
            return Err(anyhow::anyhow!("Model not available: {}", model_id));
        }

        // Ensure we don't return partial files/directories
        if model_info.is_downloading {
            return Err(anyhow::anyhow!(
                "Model is currently downloading: {}",
                model_id
            ));
        }

        let model_path = self.models_dir.join(&model_info.filename);
        let partial_path = self
            .models_dir
            .join(format!("{}.partial", &model_info.filename));

        if model_info.is_directory {
            // For directory-based models, ensure the directory exists and is complete
            if model_path.exists() && model_path.is_dir() && !partial_path.exists() {
                if matches!(model_info.engine_type, EngineType::Qwen3Asr) {
                    Self::validate_qwen3_asr_model_dir(&model_path)?;
                }
                Ok(model_path)
            } else {
                Err(anyhow::anyhow!(
                    "Complete model directory not found: {}",
                    model_id
                ))
            }
        } else {
            // For file-based models (existing logic)
            if model_path.exists() && !partial_path.exists() {
                Ok(model_path)
            } else {
                Err(anyhow::anyhow!(
                    "Complete model file not found: {}",
                    model_id
                ))
            }
        }
    }

    pub fn cancel_download(&self, model_id: &str) -> Result<()> {
        debug!("ModelManager: cancel_download called for: {}", model_id);

        // Set the cancellation flag to stop the download loop
        {
            let flags = self.cancel_flags.lock().unwrap();
            if let Some(flag) = flags.get(model_id) {
                flag.store(true, Ordering::Relaxed);
                info!("Cancellation flag set for: {}", model_id);
            } else {
                warn!("No active download found for: {}", model_id);
            }
        }

        // Update state immediately for UI responsiveness
        {
            let mut models = self.available_models.lock().unwrap();
            if let Some(model) = models.get_mut(model_id) {
                model.is_downloading = false;
            }
        }

        // Update download status to reflect current state
        self.update_download_status()?;

        // Emit cancellation event so all UI components can clear their state
        let _ = self.app_handle.emit("model-download-cancelled", model_id);

        info!("Download cancellation initiated for: {}", model_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_builtin_model_catalog_loads() {
        let models = ModelManager::load_builtin_model_catalog().unwrap();

        let small = models.get("small").unwrap();
        let expected_small_url = model_download_url("ggml-small.bin");
        assert_eq!(small.name, "Whisper Small");
        assert_eq!(small.url.as_deref(), Some(expected_small_url.as_str()));
        assert!(matches!(small.engine_type, EngineType::Whisper));
        assert_eq!(small.source, ModelSource::Handy);
        assert!(!small.is_custom);
        assert!(small.supported_languages.contains(&"zh-Hans".to_string()));

        let qwen = models.get("qwen3-asr-0.6b-int8").unwrap();
        assert_eq!(
            qwen.url.as_deref(),
            Some("https://modelscope.cn/models/zengshuishui/Qwen3-ASR-onnx/files")
        );
        assert!(matches!(qwen.engine_type, EngineType::Qwen3Asr));
        assert_eq!(qwen.source, ModelSource::SpeakMore);
        assert!(qwen.sha256.is_none());
        assert!(qwen.supported_languages.contains(&"zh-Hans".to_string()));
    }

    #[test]
    fn test_discover_custom_whisper_models() {
        let temp_dir = TempDir::new().unwrap();
        let models_dir = temp_dir.path().to_path_buf();

        // Create test .bin files
        let mut custom_file = File::create(models_dir.join("my-custom-model.bin")).unwrap();
        custom_file.write_all(b"fake model data").unwrap();

        let mut another_file = File::create(models_dir.join("whisper_medical_v2.bin")).unwrap();
        another_file.write_all(b"another fake model").unwrap();

        // Create files that should be ignored
        File::create(models_dir.join(".hidden-model.bin")).unwrap(); // Hidden file
        File::create(models_dir.join("readme.txt")).unwrap(); // Non-.bin file
        File::create(models_dir.join("ggml-small.bin")).unwrap(); // Predefined filename
        fs::create_dir(models_dir.join("some-directory.bin")).unwrap(); // Directory

        // Set up available_models with a predefined Whisper model
        let mut models = HashMap::new();
        models.insert(
            "small".to_string(),
            ModelInfo {
                id: "small".to_string(),
                name: "Whisper Small".to_string(),
                description: "Test".to_string(),
                filename: "ggml-small.bin".to_string(),
                url: Some("https://example.invalid/ggml-small.bin".to_string()),
                sha256: None,
                size_mb: 100,
                is_downloaded: false,
                is_downloading: false,
                partial_size: 0,
                is_directory: false,
                engine_type: EngineType::Whisper,
                source: ModelSource::Handy,
                accuracy_score: 0.5,
                speed_score: 0.5,
                supports_translation: true,
                is_recommended: false,
                supported_languages: vec!["en".to_string()],
                supports_language_selection: true,
                is_custom: false,
            },
        );

        // Discover custom models
        ModelManager::discover_custom_whisper_models(&models_dir, &mut models).unwrap();

        // Should have discovered 2 custom models (my-custom-model and whisper_medical_v2)
        assert!(models.contains_key("my-custom-model"));
        assert!(models.contains_key("whisper_medical_v2"));

        // Verify custom model properties
        let custom = models.get("my-custom-model").unwrap();
        assert_eq!(custom.name, "My Custom Model");
        assert_eq!(custom.filename, "my-custom-model.bin");
        assert!(custom.url.is_none()); // Custom models have no URL
        assert!(custom.is_downloaded);
        assert!(custom.is_custom);
        assert_eq!(custom.accuracy_score, 0.0);
        assert_eq!(custom.speed_score, 0.0);
        assert!(custom.supported_languages.is_empty());

        // Verify underscore handling
        let medical = models.get("whisper_medical_v2").unwrap();
        assert_eq!(medical.name, "Whisper Medical V2");

        // Should NOT have discovered hidden, non-.bin, predefined, or directories
        assert!(!models.contains_key(".hidden-model"));
        assert!(!models.contains_key("readme"));
        assert!(!models.contains_key("some-directory"));
    }

    #[test]
    fn test_discover_custom_models_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let models_dir = temp_dir.path().to_path_buf();

        let mut models = HashMap::new();
        let count_before = models.len();

        ModelManager::discover_custom_whisper_models(&models_dir, &mut models).unwrap();

        // No new models should be added
        assert_eq!(models.len(), count_before);
    }

    #[test]
    fn test_discover_custom_models_nonexistent_dir() {
        let models_dir = PathBuf::from("/nonexistent/path/that/does/not/exist");

        let mut models = HashMap::new();
        let count_before = models.len();

        // Should not error, just return Ok
        let result = ModelManager::discover_custom_whisper_models(&models_dir, &mut models);
        assert!(result.is_ok());
        assert_eq!(models.len(), count_before);
    }

    #[test]
    fn test_validate_qwen3_asr_model_dir_reports_missing_files() {
        let temp_dir = TempDir::new().unwrap();
        let model_dir = temp_dir.path();

        let result = ModelManager::validate_qwen3_asr_model_dir(model_dir);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("conv_frontend.onnx"));

        fs::create_dir_all(model_dir.join("tokenizer")).unwrap();
        for file in [
            "conv_frontend.onnx",
            "encoder.int8.onnx",
            "decoder.int8.onnx",
            "tokenizer/vocab.json",
            "tokenizer/merges.txt",
            "tokenizer/tokenizer_config.json",
        ] {
            File::create(model_dir.join(file)).unwrap();
        }

        assert!(ModelManager::validate_qwen3_asr_model_dir(model_dir).is_ok());
    }

    #[test]
    fn test_qwen3_asr_models_sort_together() {
        assert!(
            model_display_rank("qwen3-asr-0.6b-int8") < model_display_rank("qwen3-asr-1.7b-int8")
        );
        assert!(
            model_display_rank("qwen3-asr-1.7b-int8")
                < model_display_rank("moonshine-tiny-streaming-en")
        );
    }

    // ── SHA256 verification tests ─────────────────────────────────────────────

    /// Helper: write `data` to a temp file and return (TempDir, path).
    /// TempDir must be kept alive for the duration of the test.
    fn write_temp_file(data: &[u8]) -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("model.partial");
        let mut f = File::create(&path).unwrap();
        f.write_all(data).unwrap();
        (dir, path)
    }

    #[test]
    fn test_verify_sha256_skipped_when_none() {
        // Custom models have no expected hash — verification must be a no-op.
        let (_dir, path) = write_temp_file(b"anything");
        assert!(ModelManager::verify_sha256(&path, None, "custom").is_ok());
        assert!(
            path.exists(),
            "file must be untouched when verification is skipped"
        );
    }

    #[test]
    fn test_verify_sha256_passes_on_correct_hash() {
        // Compute the real hash so the test is self-consistent.
        let (_dir, path) = write_temp_file(b"hello world");
        let actual = ModelManager::compute_sha256(&path).unwrap();
        assert!(
            ModelManager::verify_sha256(&path, Some(&actual), "test_model").is_ok(),
            "should pass when hash matches"
        );
        assert!(
            path.exists(),
            "file must be kept on successful verification"
        );
    }

    #[test]
    fn test_verify_sha256_fails_and_deletes_partial_on_mismatch() {
        let (_dir, path) = write_temp_file(b"this is not the real model");
        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";

        let result = ModelManager::verify_sha256(&path, Some(wrong_hash), "bad_model");

        assert!(result.is_err(), "mismatch must return an error");
        assert!(
            result.unwrap_err().to_string().contains("corrupt"),
            "error message should mention corruption"
        );
        assert!(
            !path.exists(),
            "partial file must be deleted after hash mismatch"
        );
    }

    #[test]
    fn test_verify_sha256_fails_and_deletes_partial_when_file_missing() {
        // Simulate a partial file that was already removed (e.g. disk full mid-download).
        let dir = TempDir::new().unwrap();
        let missing_path = dir.path().join("gone.partial");
        // Don't create the file — it should not exist.

        let result =
            ModelManager::verify_sha256(&missing_path, Some("anyexpectedhash"), "missing_model");

        assert!(result.is_err(), "missing file must return an error");
    }
}

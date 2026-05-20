use crate::context_awareness::{
    ContextProbeConfidence, ContextProbeRun, ContextProbeStatus, NewContextProbeRun,
};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Local, Utc};
use log::{debug, error, info};
use rusqlite::{params, Connection, OptionalExtension};
use rusqlite_migration::{Migrations, M};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri_specta::Event;

pub const HISTORY_STATUS_COMPLETED: &str = "completed";
pub const HISTORY_STATUS_FAILED: &str = "failed";
pub const HISTORY_STATUS_EMPTY: &str = "empty";
pub const TRANSCRIPTION_RUN_STATUS_SUCCESS: &str = "success";
pub const TRANSCRIPTION_RUN_STATUS_FAILED: &str = "failed";
pub const TRANSCRIPTION_RUN_STATUS_EMPTY: &str = "empty";
pub const HISTORY_EVENT_SOURCE_BACKEND: &str = "backend";
pub const HISTORY_EVENT_SOURCE_FRONTEND: &str = "frontend";
pub const HISTORY_RUN_TYPE_TRANSCRIPTION: &str = "transcription";
pub const HISTORY_RUN_TYPE_POST_PROCESS: &str = "post_process";
pub const HISTORY_EVENT_PASTE_SUCCEEDED: &str = "paste_succeeded";
pub const HISTORY_EVENT_PASTE_FAILED: &str = "paste_failed";
pub const HISTORY_EVENT_SAVED: &str = "saved";
pub const HISTORY_EVENT_UNSAVED: &str = "unsaved";
pub const HISTORY_EVENT_RETRY_REQUESTED: &str = "retry_requested";
pub const HISTORY_EVENT_POST_PROCESS_FALLBACK: &str = "post_process_fallback";
pub const HISTORY_EVENT_USER_EDIT_SAVED: &str = "user_edit_saved";
pub const HISTORY_EVENT_USER_EDIT_CLEARED: &str = "user_edit_cleared";
pub const HISTORY_EVENT_USER_EDIT_CLEARED_BY_RETRY: &str = "user_edit_cleared_by_retry";

/// Database migrations for transcription history.
/// Each migration is applied in order. The library tracks which migrations
/// have been applied using SQLite's user_version pragma.
///
/// Note: For users upgrading from tauri-plugin-sql, migrate_from_tauri_plugin_sql()
/// converts the old _sqlx_migrations table tracking to the user_version pragma,
/// ensuring migrations don't re-run on existing databases.
static MIGRATIONS: &[M] = &[
    M::up(
        "CREATE TABLE IF NOT EXISTS transcription_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_name TEXT NOT NULL,
            timestamp INTEGER NOT NULL,
            saved BOOLEAN NOT NULL DEFAULT 0,
            title TEXT NOT NULL,
            transcription_text TEXT NOT NULL
        );",
    ),
    M::up("ALTER TABLE transcription_history ADD COLUMN post_processed_text TEXT;"),
    M::up("ALTER TABLE transcription_history ADD COLUMN post_process_prompt TEXT;"),
    M::up("ALTER TABLE transcription_history ADD COLUMN post_process_requested BOOLEAN NOT NULL DEFAULT 0;"),
    M::up("ALTER TABLE transcription_history ADD COLUMN asr_provider_id TEXT;"),
    M::up("ALTER TABLE transcription_history ADD COLUMN asr_model TEXT;"),
    M::up("ALTER TABLE transcription_history ADD COLUMN asr_language TEXT;"),
    M::up("ALTER TABLE transcription_history ADD COLUMN post_process_preset_id TEXT;"),
    M::up("ALTER TABLE transcription_history ADD COLUMN post_process_preset_version INTEGER;"),
    M::up(
        "CREATE TABLE IF NOT EXISTS post_process_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            history_entry_id INTEGER NOT NULL,
            preset_id TEXT,
            preset_version INTEGER,
            provider_id TEXT,
            model TEXT,
            status TEXT NOT NULL,
            latency_ms INTEGER NOT NULL DEFAULT 0,
            error_summary TEXT,
            created_at INTEGER NOT NULL
        );",
    ),
    M::up("ALTER TABLE transcription_history ADD COLUMN latest_transcription_run_id INTEGER;"),
    M::up("ALTER TABLE transcription_history ADD COLUMN latest_post_process_run_id INTEGER;"),
    M::up(
        "ALTER TABLE transcription_history ADD COLUMN status TEXT NOT NULL DEFAULT 'completed';",
    ),
    M::up(
        "UPDATE transcription_history
         SET status = CASE
             WHEN transcription_text = '' THEN 'failed'
             ELSE 'completed'
         END;",
    ),
    M::up(
        "CREATE TABLE IF NOT EXISTS transcription_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            history_entry_id INTEGER NOT NULL,
            provider_id TEXT,
            model TEXT,
            language TEXT,
            status TEXT NOT NULL,
            transcript_text TEXT NOT NULL DEFAULT '',
            latency_ms INTEGER NOT NULL DEFAULT 0,
            error_summary TEXT,
            created_at INTEGER NOT NULL
        );",
    ),
    M::up("ALTER TABLE post_process_runs ADD COLUMN input_text TEXT;"),
    M::up("ALTER TABLE post_process_runs ADD COLUMN output_text TEXT;"),
    M::up("ALTER TABLE post_process_runs ADD COLUMN prompt_template_snapshot TEXT;"),
    M::up(
        "CREATE TABLE IF NOT EXISTS history_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            history_entry_id INTEGER NOT NULL,
            run_type TEXT,
            run_id INTEGER,
            event_type TEXT NOT NULL,
            source TEXT NOT NULL,
            payload_json TEXT,
            created_at INTEGER NOT NULL
        );",
    ),
    M::up(
        "CREATE INDEX IF NOT EXISTS idx_transcription_runs_history_created
            ON transcription_runs(history_entry_id, created_at);
         CREATE INDEX IF NOT EXISTS idx_post_process_runs_history_created
            ON post_process_runs(history_entry_id, created_at);
         CREATE INDEX IF NOT EXISTS idx_history_events_history_created
            ON history_events(history_entry_id, created_at);",
    ),
    M::up("ALTER TABLE transcription_history ADD COLUMN user_edited_text TEXT;"),
    M::up("ALTER TABLE transcription_history ADD COLUMN user_edited_at INTEGER;"),
    M::up(
        "CREATE TABLE IF NOT EXISTS context_probe_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            captured_at INTEGER NOT NULL,
            source TEXT NOT NULL,
            status TEXT NOT NULL,
            confidence TEXT NOT NULL,
            latency_ms INTEGER NOT NULL DEFAULT 0,
            app_name TEXT,
            bundle_id TEXT,
            pid INTEGER,
            window_title TEXT,
            element_role TEXT,
            element_subrole TEXT,
            is_secure BOOLEAN NOT NULL DEFAULT 0,
            value_text TEXT,
            before_text TEXT,
            selected_text TEXT,
            after_text TEXT,
            selected_location_utf16 INTEGER,
            selected_length_utf16 INTEGER,
            number_of_characters INTEGER,
            available_attributes_json TEXT,
            failure_reason TEXT,
            truncated BOOLEAN NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_context_probe_runs_captured
            ON context_probe_runs(captured_at DESC, id DESC);",
    ),
];

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct PaginatedHistory {
    pub entries: Vec<HistoryEntry>,
    pub has_more: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
#[serde(tag = "action")]
pub enum HistoryUpdatePayload {
    #[serde(rename = "added")]
    Added { entry: HistoryEntry },
    #[serde(rename = "updated")]
    Updated { entry: HistoryEntry },
    #[serde(rename = "deleted")]
    Deleted { id: i64 },
    #[serde(rename = "toggled")]
    Toggled { id: i64 },
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct HistoryEntry {
    pub id: i64,
    pub file_name: String,
    pub timestamp: i64,
    pub saved: bool,
    pub title: String,
    pub transcription_text: String,
    pub post_processed_text: Option<String>,
    pub post_process_prompt: Option<String>,
    pub post_process_requested: bool,
    pub post_process_preset_id: Option<String>,
    pub post_process_preset_version: Option<i64>,
    pub asr_provider_id: Option<String>,
    pub asr_model: Option<String>,
    pub asr_language: Option<String>,
    pub user_edited_text: Option<String>,
    pub user_edited_at: Option<i64>,
    pub final_text: String,
    pub latest_transcription_run_id: Option<i64>,
    pub latest_post_process_run_id: Option<i64>,
    pub status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct AsrHistoryMetadata {
    pub provider_id: String,
    pub model: String,
    pub language: String,
}

#[derive(Clone, Debug)]
pub struct NewPostProcessRun {
    pub preset_id: Option<String>,
    pub preset_version: Option<i64>,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub status: String,
    pub input_text: Option<String>,
    pub output_text: Option<String>,
    pub prompt_template_snapshot: Option<String>,
    pub latency_ms: i64,
    pub error_summary: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct PostProcessRun {
    pub id: i64,
    pub history_entry_id: i64,
    pub preset_id: Option<String>,
    pub preset_version: Option<i64>,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub status: String,
    pub input_text: Option<String>,
    pub output_text: Option<String>,
    pub prompt_template_snapshot: Option<String>,
    pub latency_ms: i64,
    pub error_summary: Option<String>,
    pub created_at: i64,
}

#[derive(Clone, Debug)]
pub struct NewTranscriptionRun {
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub language: Option<String>,
    pub status: String,
    pub transcript_text: String,
    pub latency_ms: i64,
    pub error_summary: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct TranscriptionRun {
    pub id: i64,
    pub history_entry_id: i64,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub language: Option<String>,
    pub status: String,
    pub transcript_text: String,
    pub latency_ms: i64,
    pub error_summary: Option<String>,
    pub created_at: i64,
}

#[derive(Clone, Debug)]
pub struct NewHistoryEvent {
    pub run_type: Option<String>,
    pub run_id: Option<i64>,
    pub event_type: String,
    pub source: String,
    pub payload_json: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct HistoryEvent {
    pub id: i64,
    pub history_entry_id: i64,
    pub run_type: Option<String>,
    pub run_id: Option<i64>,
    pub event_type: String,
    pub source: String,
    pub payload_json: Option<String>,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct HistoryEntryDetail {
    pub entry: HistoryEntry,
    pub transcription_runs: Vec<TranscriptionRun>,
    pub post_process_runs: Vec<PostProcessRun>,
    pub events: Vec<HistoryEvent>,
}

pub struct HistoryManager {
    app_handle: AppHandle,
    recordings_dir: PathBuf,
    db_path: PathBuf,
}

impl HistoryManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        // Create recordings directory in app data dir
        let app_data_dir = crate::portable::app_data_dir(app_handle)?;
        let recordings_dir = app_data_dir.join("recordings");
        let db_path = app_data_dir.join("history.db");

        // Ensure recordings directory exists
        if !recordings_dir.exists() {
            fs::create_dir_all(&recordings_dir)?;
            debug!("Created recordings directory: {:?}", recordings_dir);
        }

        let manager = Self {
            app_handle: app_handle.clone(),
            recordings_dir,
            db_path,
        };

        // Initialize database and run migrations synchronously
        manager.init_database()?;

        Ok(manager)
    }

    fn init_database(&self) -> Result<()> {
        info!("Initializing database at {:?}", self.db_path);

        let mut conn = Connection::open(&self.db_path)?;

        // Handle migration from tauri-plugin-sql to rusqlite_migration
        // tauri-plugin-sql used _sqlx_migrations table, rusqlite_migration uses user_version pragma
        self.migrate_from_tauri_plugin_sql(&conn)?;

        // Create migrations object and run to latest version
        let migrations = Migrations::new(MIGRATIONS.to_vec());

        // Validate migrations in debug builds
        #[cfg(debug_assertions)]
        migrations.validate().expect("Invalid migrations");

        // Get current version before migration
        let version_before: i32 =
            conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        debug!("Database version before migration: {}", version_before);

        // Apply any pending migrations
        migrations.to_latest(&mut conn)?;

        // Get version after migration
        let version_after: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

        if version_after > version_before {
            info!(
                "Database migrated from version {} to {}",
                version_before, version_after
            );
        } else {
            debug!("Database already at latest version {}", version_after);
        }

        Ok(())
    }

    /// Migrate from tauri-plugin-sql's migration tracking to rusqlite_migration's.
    /// tauri-plugin-sql used a _sqlx_migrations table, while rusqlite_migration uses
    /// SQLite's user_version pragma. This function checks if the old system was in use
    /// and sets the user_version accordingly so migrations don't re-run.
    fn migrate_from_tauri_plugin_sql(&self, conn: &Connection) -> Result<()> {
        // Check if the old _sqlx_migrations table exists
        let has_sqlx_migrations: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='_sqlx_migrations'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !has_sqlx_migrations {
            return Ok(());
        }

        // Check current user_version
        let current_version: i32 =
            conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

        if current_version > 0 {
            // Already migrated to rusqlite_migration system
            return Ok(());
        }

        // Get the highest version from the old migrations table
        let old_version: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations WHERE success = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if old_version > 0 {
            info!(
                "Migrating from tauri-plugin-sql (version {}) to rusqlite_migration",
                old_version
            );

            // Set user_version to match the old migration state
            conn.pragma_update(None, "user_version", old_version)?;

            // Optionally drop the old migrations table (keeping it doesn't hurt)
            // conn.execute("DROP TABLE IF EXISTS _sqlx_migrations", [])?;

            info!(
                "Migration tracking converted: user_version set to {}",
                old_version
            );
        }

        Ok(())
    }

    fn get_connection(&self) -> Result<Connection> {
        Ok(Connection::open(&self.db_path)?)
    }

    fn history_entry_select_columns() -> &'static str {
        "id, file_name, timestamp, saved, title, transcription_text, post_processed_text, post_process_prompt, post_process_requested, post_process_preset_id, post_process_preset_version, asr_provider_id, asr_model, asr_language, user_edited_text, user_edited_at, latest_transcription_run_id, latest_post_process_run_id, status"
    }

    fn map_history_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryEntry> {
        let transcription_text: String = row.get("transcription_text")?;
        let post_processed_text: Option<String> = row.get("post_processed_text")?;
        let user_edited_text: Option<String> = row.get("user_edited_text")?;
        let final_text = user_edited_text
            .clone()
            .or_else(|| post_processed_text.clone())
            .unwrap_or_else(|| transcription_text.clone());

        Ok(HistoryEntry {
            id: row.get("id")?,
            file_name: row.get("file_name")?,
            timestamp: row.get("timestamp")?,
            saved: row.get("saved")?,
            title: row.get("title")?,
            transcription_text,
            post_processed_text,
            post_process_prompt: row.get("post_process_prompt")?,
            post_process_requested: row.get("post_process_requested")?,
            post_process_preset_id: row.get("post_process_preset_id")?,
            post_process_preset_version: row.get("post_process_preset_version")?,
            asr_provider_id: row.get("asr_provider_id")?,
            asr_model: row.get("asr_model")?,
            asr_language: row.get("asr_language")?,
            user_edited_text,
            user_edited_at: row.get("user_edited_at")?,
            final_text,
            latest_transcription_run_id: row.get("latest_transcription_run_id")?,
            latest_post_process_run_id: row.get("latest_post_process_run_id")?,
            status: row.get("status")?,
        })
    }

    fn final_text_for_entry(
        transcription_text: &str,
        post_processed_text: Option<&String>,
        user_edited_text: Option<&String>,
    ) -> String {
        user_edited_text
            .cloned()
            .or_else(|| post_processed_text.cloned())
            .unwrap_or_else(|| transcription_text.to_string())
    }

    fn user_edit_payload_json(text_len: usize) -> String {
        serde_json::json!({ "text_len": text_len }).to_string()
    }

    fn map_post_process_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<PostProcessRun> {
        Ok(PostProcessRun {
            id: row.get("id")?,
            history_entry_id: row.get("history_entry_id")?,
            preset_id: row.get("preset_id")?,
            preset_version: row.get("preset_version")?,
            provider_id: row.get("provider_id")?,
            model: row.get("model")?,
            status: row.get("status")?,
            input_text: row.get("input_text")?,
            output_text: row.get("output_text")?,
            prompt_template_snapshot: row.get("prompt_template_snapshot")?,
            latency_ms: row.get("latency_ms")?,
            error_summary: row.get("error_summary")?,
            created_at: row.get("created_at")?,
        })
    }

    fn map_transcription_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<TranscriptionRun> {
        Ok(TranscriptionRun {
            id: row.get("id")?,
            history_entry_id: row.get("history_entry_id")?,
            provider_id: row.get("provider_id")?,
            model: row.get("model")?,
            language: row.get("language")?,
            status: row.get("status")?,
            transcript_text: row.get("transcript_text")?,
            latency_ms: row.get("latency_ms")?,
            error_summary: row.get("error_summary")?,
            created_at: row.get("created_at")?,
        })
    }

    fn map_history_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryEvent> {
        Ok(HistoryEvent {
            id: row.get("id")?,
            history_entry_id: row.get("history_entry_id")?,
            run_type: row.get("run_type")?,
            run_id: row.get("run_id")?,
            event_type: row.get("event_type")?,
            source: row.get("source")?,
            payload_json: row.get("payload_json")?,
            created_at: row.get("created_at")?,
        })
    }

    fn context_probe_select_columns() -> &'static str {
        "id, captured_at, source, status, confidence, latency_ms, app_name, bundle_id, pid, window_title, element_role, element_subrole, is_secure, value_text, before_text, selected_text, after_text, selected_location_utf16, selected_length_utf16, number_of_characters, available_attributes_json, failure_reason, truncated"
    }

    fn map_context_probe_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<ContextProbeRun> {
        let status: String = row.get("status")?;
        let confidence: String = row.get("confidence")?;

        Ok(ContextProbeRun {
            id: row.get("id")?,
            captured_at: row.get("captured_at")?,
            source: row.get("source")?,
            status: ContextProbeStatus::from_db(&status),
            confidence: ContextProbeConfidence::from_db(&confidence),
            latency_ms: row.get("latency_ms")?,
            app_name: row.get("app_name")?,
            bundle_id: row.get("bundle_id")?,
            pid: row.get("pid")?,
            window_title: row.get("window_title")?,
            element_role: row.get("element_role")?,
            element_subrole: row.get("element_subrole")?,
            is_secure: row.get("is_secure")?,
            value_text: row.get("value_text")?,
            before_text: row.get("before_text")?,
            selected_text: row.get("selected_text")?,
            after_text: row.get("after_text")?,
            selected_location_utf16: row.get("selected_location_utf16")?,
            selected_length_utf16: row.get("selected_length_utf16")?,
            number_of_characters: row.get("number_of_characters")?,
            available_attributes_json: row.get("available_attributes_json")?,
            failure_reason: row.get("failure_reason")?,
            truncated: row.get("truncated")?,
        })
    }

    fn ensure_entry_exists_with_conn(conn: &Connection, history_entry_id: i64) -> Result<()> {
        let exists = conn
            .query_row(
                "SELECT 1 FROM transcription_history WHERE id = ?1",
                params![history_entry_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some();

        if exists {
            Ok(())
        } else {
            Err(anyhow!("History entry {} not found", history_entry_id))
        }
    }

    pub fn recordings_dir(&self) -> &std::path::Path {
        &self.recordings_dir
    }

    /// Save a new history entry to the database.
    /// The WAV file should already have been written to the recordings directory.
    #[allow(clippy::too_many_arguments)]
    pub fn save_entry(
        &self,
        file_name: String,
        transcription_text: String,
        post_process_requested: bool,
        post_processed_text: Option<String>,
        post_process_prompt: Option<String>,
        post_process_preset_id: Option<String>,
        post_process_preset_version: Option<i64>,
        asr_metadata: Option<AsrHistoryMetadata>,
        status: String,
    ) -> Result<HistoryEntry> {
        let timestamp = Utc::now().timestamp();
        let title = self.format_timestamp_title(timestamp);

        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO transcription_history (
                file_name,
                timestamp,
                saved,
                title,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_requested,
                post_process_preset_id,
                post_process_preset_version,
                asr_provider_id,
                asr_model,
                asr_language,
                latest_transcription_run_id,
                latest_post_process_run_id,
                status
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                &file_name,
                timestamp,
                false,
                &title,
                &transcription_text,
                &post_processed_text,
                &post_process_prompt,
                post_process_requested,
                &post_process_preset_id,
                post_process_preset_version,
                asr_metadata.as_ref().map(|metadata| &metadata.provider_id),
                asr_metadata.as_ref().map(|metadata| &metadata.model),
                asr_metadata.as_ref().map(|metadata| &metadata.language),
                Option::<i64>::None,
                Option::<i64>::None,
                &status,
            ],
        )?;

        let asr_provider_id = asr_metadata
            .as_ref()
            .map(|metadata| metadata.provider_id.clone());
        let asr_model = asr_metadata.as_ref().map(|metadata| metadata.model.clone());
        let asr_language = asr_metadata
            .as_ref()
            .map(|metadata| metadata.language.clone());
        let final_text =
            Self::final_text_for_entry(&transcription_text, post_processed_text.as_ref(), None);
        let entry = HistoryEntry {
            id: conn.last_insert_rowid(),
            file_name,
            timestamp,
            saved: false,
            title,
            transcription_text,
            post_processed_text,
            post_process_prompt,
            post_process_requested,
            post_process_preset_id,
            post_process_preset_version,
            asr_provider_id,
            asr_model,
            asr_language,
            user_edited_text: None,
            user_edited_at: None,
            final_text,
            latest_transcription_run_id: None,
            latest_post_process_run_id: None,
            status,
        };

        debug!("Saved history entry with id {}", entry.id);

        self.cleanup_old_entries()?;

        // Emit typed event for real-time frontend updates
        if let Err(e) = (HistoryUpdatePayload::Added {
            entry: entry.clone(),
        })
        .emit(&self.app_handle)
        {
            error!("Failed to emit history-updated event: {}", e);
        }

        Ok(entry)
    }

    /// Update an existing history entry with new transcription results (used by retry).
    #[allow(clippy::too_many_arguments)]
    pub fn update_transcription(
        &self,
        id: i64,
        transcription_text: String,
        post_processed_text: Option<String>,
        post_process_prompt: Option<String>,
        post_process_preset_id: Option<String>,
        post_process_preset_version: Option<i64>,
        asr_metadata: Option<AsrHistoryMetadata>,
        status: String,
        latest_transcription_run_id: Option<i64>,
    ) -> Result<HistoryEntry> {
        let conn = self.get_connection()?;
        let previous_user_edited_text = conn
            .query_row(
                "SELECT user_edited_text FROM transcription_history WHERE id = ?1",
                params![id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        let updated = conn.execute(
            "UPDATE transcription_history
             SET transcription_text = ?1,
                 post_processed_text = ?2,
                 post_process_prompt = ?3,
                 post_process_preset_id = ?4,
                 post_process_preset_version = ?5,
                 asr_provider_id = ?6,
                 asr_model = ?7,
                 asr_language = ?8,
                 latest_transcription_run_id = ?9,
                 latest_post_process_run_id = NULL,
                 user_edited_text = NULL,
                 user_edited_at = NULL,
                 status = ?10
             WHERE id = ?11",
            params![
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_preset_id,
                post_process_preset_version,
                asr_metadata.as_ref().map(|metadata| &metadata.provider_id),
                asr_metadata.as_ref().map(|metadata| &metadata.model),
                asr_metadata.as_ref().map(|metadata| &metadata.language),
                latest_transcription_run_id,
                status,
                id
            ],
        )?;

        if updated == 0 {
            return Err(anyhow!("History entry {} not found", id));
        }

        let entry = conn.query_row(
            &format!(
                "SELECT {} FROM transcription_history WHERE id = ?1",
                Self::history_entry_select_columns()
            ),
            params![id],
            Self::map_history_entry,
        )?;

        debug!("Updated transcription for history entry {}", id);

        if let Some(previous_text) = previous_user_edited_text
            .as_ref()
            .filter(|text| !text.trim().is_empty())
        {
            Self::insert_history_event_with_conn(
                &conn,
                id,
                NewHistoryEvent {
                    run_type: None,
                    run_id: None,
                    event_type: HISTORY_EVENT_USER_EDIT_CLEARED_BY_RETRY.to_string(),
                    source: HISTORY_EVENT_SOURCE_BACKEND.to_string(),
                    payload_json: Some(Self::user_edit_payload_json(previous_text.chars().count())),
                },
            )?;
        }

        if let Err(e) = (HistoryUpdatePayload::Updated {
            entry: entry.clone(),
        })
        .emit(&self.app_handle)
        {
            error!("Failed to emit history-updated event: {}", e);
        }

        Ok(entry)
    }

    pub fn update_user_edit(&self, id: i64, text: Option<String>) -> Result<HistoryEntry> {
        let conn = self.get_connection()?;
        let entry = Self::update_user_edit_with_conn(&conn, id, text)?;

        if let Err(e) = (HistoryUpdatePayload::Updated {
            entry: entry.clone(),
        })
        .emit(&self.app_handle)
        {
            error!("Failed to emit history-updated event: {}", e);
        }

        Ok(entry)
    }

    fn update_user_edit_with_conn(
        conn: &Connection,
        id: i64,
        text: Option<String>,
    ) -> Result<HistoryEntry> {
        let previous_user_edited_text = conn
            .query_row(
                "SELECT user_edited_text FROM transcription_history WHERE id = ?1",
                params![id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();

        let (user_edited_text, user_edited_at, event_type, text_len) = match text {
            Some(text) => {
                if text.trim().is_empty() {
                    return Err(anyhow!("Edited text cannot be empty"));
                }
                let text_len = text.chars().count();
                (
                    Some(text),
                    Some(Utc::now().timestamp()),
                    HISTORY_EVENT_USER_EDIT_SAVED,
                    text_len,
                )
            }
            None => {
                let text_len = previous_user_edited_text
                    .as_ref()
                    .map(|text| text.chars().count())
                    .unwrap_or(0);
                (None, None, HISTORY_EVENT_USER_EDIT_CLEARED, text_len)
            }
        };

        let updated = conn.execute(
            "UPDATE transcription_history
             SET user_edited_text = ?1,
                 user_edited_at = ?2
             WHERE id = ?3",
            params![user_edited_text, user_edited_at, id],
        )?;

        if updated == 0 {
            return Err(anyhow!("History entry {} not found", id));
        }

        let entry = conn.query_row(
            &format!(
                "SELECT {} FROM transcription_history WHERE id = ?1",
                Self::history_entry_select_columns()
            ),
            params![id],
            Self::map_history_entry,
        )?;

        Self::insert_history_event_with_conn(
            conn,
            id,
            NewHistoryEvent {
                run_type: None,
                run_id: None,
                event_type: event_type.to_string(),
                source: HISTORY_EVENT_SOURCE_BACKEND.to_string(),
                payload_json: Some(Self::user_edit_payload_json(text_len)),
            },
        )?;

        Ok(entry)
    }

    fn insert_post_process_run_with_conn(
        conn: &Connection,
        history_entry_id: i64,
        run: NewPostProcessRun,
    ) -> Result<PostProcessRun> {
        let created_at = Utc::now().timestamp();
        conn.execute(
            "INSERT INTO post_process_runs (
                history_entry_id,
                preset_id,
                preset_version,
                provider_id,
                model,
                status,
                input_text,
                output_text,
                prompt_template_snapshot,
                latency_ms,
                error_summary,
                created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                history_entry_id,
                run.preset_id,
                run.preset_version,
                run.provider_id,
                run.model,
                run.status,
                run.input_text,
                run.output_text,
                run.prompt_template_snapshot,
                run.latency_ms,
                run.error_summary,
                created_at,
            ],
        )?;

        let id = conn.last_insert_rowid();
        let saved = conn.query_row(
            "SELECT id, history_entry_id, preset_id, preset_version, provider_id, model, status, input_text, output_text, prompt_template_snapshot, latency_ms, error_summary, created_at
             FROM post_process_runs WHERE id = ?1",
            params![id],
            Self::map_post_process_run,
        )?;
        Ok(saved)
    }

    pub fn save_post_process_run(
        &self,
        history_entry_id: i64,
        run: NewPostProcessRun,
    ) -> Result<PostProcessRun> {
        let conn = self.get_connection()?;
        Self::ensure_entry_exists_with_conn(&conn, history_entry_id)?;
        let saved = Self::insert_post_process_run_with_conn(&conn, history_entry_id, run)?;
        conn.execute(
            "UPDATE transcription_history
             SET latest_post_process_run_id = ?1
             WHERE id = ?2",
            params![saved.id, history_entry_id],
        )?;
        debug!(
            "Saved post-process run {} for history entry {} with status {}",
            saved.id, history_entry_id, saved.status
        );
        self.emit_entry_updated(history_entry_id)?;
        Ok(saved)
    }

    fn insert_context_probe_run_with_conn(
        conn: &Connection,
        run: NewContextProbeRun,
    ) -> Result<ContextProbeRun> {
        conn.execute(
            "INSERT INTO context_probe_runs (
                captured_at,
                source,
                status,
                confidence,
                latency_ms,
                app_name,
                bundle_id,
                pid,
                window_title,
                element_role,
                element_subrole,
                is_secure,
                value_text,
                before_text,
                selected_text,
                after_text,
                selected_location_utf16,
                selected_length_utf16,
                number_of_characters,
                available_attributes_json,
                failure_reason,
                truncated
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
            params![
                run.captured_at,
                run.source,
                run.status.as_str(),
                run.confidence.as_str(),
                run.latency_ms,
                run.app_name,
                run.bundle_id,
                run.pid,
                run.window_title,
                run.element_role,
                run.element_subrole,
                run.is_secure,
                run.value_text,
                run.before_text,
                run.selected_text,
                run.after_text,
                run.selected_location_utf16,
                run.selected_length_utf16,
                run.number_of_characters,
                run.available_attributes_json,
                run.failure_reason,
                run.truncated,
            ],
        )?;

        let id = conn.last_insert_rowid();
        let saved = conn.query_row(
            &format!(
                "SELECT {} FROM context_probe_runs WHERE id = ?1",
                Self::context_probe_select_columns()
            ),
            params![id],
            Self::map_context_probe_run,
        )?;
        Ok(saved)
    }

    pub fn save_context_probe_run(&self, run: NewContextProbeRun) -> Result<ContextProbeRun> {
        let conn = self.get_connection()?;
        let saved = Self::insert_context_probe_run_with_conn(&conn, run)?;
        debug!(
            "Saved context probe run {} with status {}",
            saved.id,
            saved.status.as_str()
        );
        Ok(saved)
    }

    pub fn get_context_probe_runs(&self, limit: u32) -> Result<Vec<ContextProbeRun>> {
        let limit = limit.clamp(1, 100);
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM context_probe_runs ORDER BY captured_at DESC, id DESC LIMIT ?1",
            Self::context_probe_select_columns()
        ))?;
        let rows = stmt.query_map(params![i64::from(limit)], Self::map_context_probe_run)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn clear_context_probe_runs(&self) -> Result<usize> {
        let conn = self.get_connection()?;
        let deleted = conn.execute("DELETE FROM context_probe_runs", [])?;
        debug!("Cleared {} context probe runs", deleted);
        Ok(deleted)
    }

    fn insert_transcription_run_with_conn(
        conn: &Connection,
        history_entry_id: i64,
        run: NewTranscriptionRun,
    ) -> Result<TranscriptionRun> {
        let created_at = Utc::now().timestamp();
        conn.execute(
            "INSERT INTO transcription_runs (
                history_entry_id,
                provider_id,
                model,
                language,
                status,
                transcript_text,
                latency_ms,
                error_summary,
                created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                history_entry_id,
                run.provider_id,
                run.model,
                run.language,
                run.status,
                run.transcript_text,
                run.latency_ms,
                run.error_summary,
                created_at,
            ],
        )?;

        let id = conn.last_insert_rowid();
        let saved = conn.query_row(
            "SELECT id, history_entry_id, provider_id, model, language, status, transcript_text, latency_ms, error_summary, created_at
             FROM transcription_runs WHERE id = ?1",
            params![id],
            Self::map_transcription_run,
        )?;
        Ok(saved)
    }

    pub fn save_transcription_run(
        &self,
        history_entry_id: i64,
        run: NewTranscriptionRun,
        history_status: String,
    ) -> Result<TranscriptionRun> {
        let conn = self.get_connection()?;
        Self::ensure_entry_exists_with_conn(&conn, history_entry_id)?;
        let saved = Self::insert_transcription_run_with_conn(&conn, history_entry_id, run)?;
        conn.execute(
            "UPDATE transcription_history
             SET latest_transcription_run_id = ?1,
                 status = ?2
             WHERE id = ?3",
            params![saved.id, history_status, history_entry_id],
        )?;
        debug!(
            "Saved transcription run {} for history entry {} with status {}",
            saved.id, history_entry_id, saved.status
        );
        self.emit_entry_updated(history_entry_id)?;
        Ok(saved)
    }

    pub fn record_history_event(
        &self,
        history_entry_id: i64,
        event: NewHistoryEvent,
    ) -> Result<HistoryEvent> {
        let conn = self.get_connection()?;
        Self::insert_history_event_with_conn(&conn, history_entry_id, event)
    }

    fn insert_history_event_with_conn(
        conn: &Connection,
        history_entry_id: i64,
        event: NewHistoryEvent,
    ) -> Result<HistoryEvent> {
        Self::ensure_entry_exists_with_conn(conn, history_entry_id)?;
        let created_at = Utc::now().timestamp();
        conn.execute(
            "INSERT INTO history_events (
                history_entry_id,
                run_type,
                run_id,
                event_type,
                source,
                payload_json,
                created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                history_entry_id,
                event.run_type,
                event.run_id,
                event.event_type,
                event.source,
                event.payload_json,
                created_at,
            ],
        )?;

        let id = conn.last_insert_rowid();
        let saved = conn.query_row(
            "SELECT id, history_entry_id, run_type, run_id, event_type, source, payload_json, created_at
             FROM history_events WHERE id = ?1",
            params![id],
            Self::map_history_event,
        )?;
        Ok(saved)
    }

    fn emit_entry_updated(&self, history_entry_id: i64) -> Result<()> {
        let Some(entry) = self.get_entry_by_id_sync(history_entry_id)? else {
            return Ok(());
        };

        if let Err(e) = (HistoryUpdatePayload::Updated {
            entry: entry.clone(),
        })
        .emit(&self.app_handle)
        {
            error!("Failed to emit history-updated event: {}", e);
        }

        Ok(())
    }

    pub fn cleanup_old_entries(&self) -> Result<()> {
        let retention_period = crate::settings::get_recording_retention_period(&self.app_handle);

        match retention_period {
            crate::settings::RecordingRetentionPeriod::Never => {
                // Don't delete anything
                Ok(())
            }
            crate::settings::RecordingRetentionPeriod::PreserveLimit => {
                // Use the old count-based logic with history_limit
                let limit = crate::settings::get_history_limit(&self.app_handle);
                self.cleanup_by_count(limit)
            }
            _ => {
                // Use time-based logic
                self.cleanup_by_time(retention_period)
            }
        }
    }

    fn delete_entries_and_files(&self, entries: &[(i64, String)]) -> Result<usize> {
        if entries.is_empty() {
            return Ok(0);
        }

        let conn = self.get_connection()?;
        let mut deleted_count = 0;

        for (id, file_name) in entries {
            Self::delete_entry_rows_with_conn(&conn, *id)?;

            // Delete WAV file
            let file_path = self.recordings_dir.join(file_name);
            if file_path.exists() {
                if let Err(e) = fs::remove_file(&file_path) {
                    error!("Failed to delete WAV file {}: {}", file_name, e);
                } else {
                    debug!("Deleted old WAV file: {}", file_name);
                    deleted_count += 1;
                }
            }
        }

        Ok(deleted_count)
    }

    fn delete_entry_rows_with_conn(conn: &Connection, id: i64) -> Result<()> {
        conn.execute(
            "DELETE FROM history_events WHERE history_entry_id = ?1",
            params![id],
        )?;
        conn.execute(
            "DELETE FROM post_process_runs WHERE history_entry_id = ?1",
            params![id],
        )?;
        conn.execute(
            "DELETE FROM transcription_runs WHERE history_entry_id = ?1",
            params![id],
        )?;
        conn.execute(
            "DELETE FROM transcription_history WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    fn cleanup_by_count(&self, limit: usize) -> Result<()> {
        let conn = self.get_connection()?;

        // Get all entries that are not saved, ordered by timestamp desc
        let mut stmt = conn.prepare(
            "SELECT id, file_name FROM transcription_history WHERE saved = 0 ORDER BY timestamp DESC"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>("id")?, row.get::<_, String>("file_name")?))
        })?;

        let mut entries: Vec<(i64, String)> = Vec::new();
        for row in rows {
            entries.push(row?);
        }

        if entries.len() > limit {
            let entries_to_delete = &entries[limit..];
            let deleted_count = self.delete_entries_and_files(entries_to_delete)?;

            if deleted_count > 0 {
                debug!("Cleaned up {} old history entries by count", deleted_count);
            }
        }

        Ok(())
    }

    fn cleanup_by_time(
        &self,
        retention_period: crate::settings::RecordingRetentionPeriod,
    ) -> Result<()> {
        let conn = self.get_connection()?;

        // Calculate cutoff timestamp (current time minus retention period)
        let now = Utc::now().timestamp();
        let cutoff_timestamp = match retention_period {
            crate::settings::RecordingRetentionPeriod::Days3 => now - (3 * 24 * 60 * 60), // 3 days in seconds
            crate::settings::RecordingRetentionPeriod::Weeks2 => now - (2 * 7 * 24 * 60 * 60), // 2 weeks in seconds
            crate::settings::RecordingRetentionPeriod::Months3 => now - (3 * 30 * 24 * 60 * 60), // 3 months in seconds (approximate)
            _ => unreachable!("Should not reach here"),
        };

        // Get all unsaved entries older than the cutoff timestamp
        let mut stmt = conn.prepare(
            "SELECT id, file_name FROM transcription_history WHERE saved = 0 AND timestamp < ?1",
        )?;

        let rows = stmt.query_map(params![cutoff_timestamp], |row| {
            Ok((row.get::<_, i64>("id")?, row.get::<_, String>("file_name")?))
        })?;

        let mut entries_to_delete: Vec<(i64, String)> = Vec::new();
        for row in rows {
            entries_to_delete.push(row?);
        }

        let deleted_count = self.delete_entries_and_files(&entries_to_delete)?;

        if deleted_count > 0 {
            debug!(
                "Cleaned up {} old history entries based on retention period",
                deleted_count
            );
        }

        Ok(())
    }

    pub async fn get_history_entries(
        &self,
        cursor: Option<i64>,
        limit: Option<usize>,
    ) -> Result<PaginatedHistory> {
        let conn = self.get_connection()?;
        let limit = limit.map(|l| l.min(100));

        let mut entries: Vec<HistoryEntry> = match (cursor, limit) {
            (Some(cursor_id), Some(lim)) => {
                let fetch_count = (lim + 1) as i64;
                let mut stmt = conn.prepare(&format!(
                    "SELECT {}
                     FROM transcription_history
                     WHERE id < ?1
                     ORDER BY id DESC
                     LIMIT ?2",
                    Self::history_entry_select_columns()
                ))?;
                let result = stmt
                    .query_map(params![cursor_id, fetch_count], Self::map_history_entry)?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                result
            }
            (None, Some(lim)) => {
                let fetch_count = (lim + 1) as i64;
                let mut stmt = conn.prepare(&format!(
                    "SELECT {}
                     FROM transcription_history
                     ORDER BY id DESC
                     LIMIT ?1",
                    Self::history_entry_select_columns()
                ))?;
                let result = stmt
                    .query_map(params![fetch_count], Self::map_history_entry)?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                result
            }
            (_, None) => {
                let mut stmt = conn.prepare(&format!(
                    "SELECT {}
                     FROM transcription_history
                     ORDER BY id DESC",
                    Self::history_entry_select_columns()
                ))?;
                let result = stmt
                    .query_map([], Self::map_history_entry)?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                result
            }
        };

        let has_more = limit.is_some_and(|lim| entries.len() > lim);
        if has_more {
            entries.pop();
        }

        Ok(PaginatedHistory { entries, has_more })
    }

    #[cfg(test)]
    fn get_latest_entry_with_conn(conn: &Connection) -> Result<Option<HistoryEntry>> {
        let mut stmt = conn.prepare(&format!(
            "SELECT {}
             FROM transcription_history
             ORDER BY timestamp DESC
             LIMIT 1",
            Self::history_entry_select_columns()
        ))?;

        let entry = stmt.query_row([], Self::map_history_entry).optional()?;
        Ok(entry)
    }

    /// Get the latest entry with non-empty transcription text.
    pub fn get_latest_completed_entry(&self) -> Result<Option<HistoryEntry>> {
        let conn = self.get_connection()?;
        Self::get_latest_completed_entry_with_conn(&conn)
    }

    fn get_latest_completed_entry_with_conn(conn: &Connection) -> Result<Option<HistoryEntry>> {
        let mut stmt = conn.prepare(&format!(
            "SELECT {}
             FROM transcription_history
             WHERE transcription_text != ''
             ORDER BY timestamp DESC
             LIMIT 1",
            Self::history_entry_select_columns()
        ))?;

        let entry = stmt.query_row([], Self::map_history_entry).optional()?;
        Ok(entry)
    }

    pub async fn toggle_saved_status(&self, id: i64) -> Result<()> {
        let conn = self.get_connection()?;

        // Get current saved status
        let current_saved: bool = conn.query_row(
            "SELECT saved FROM transcription_history WHERE id = ?1",
            params![id],
            |row| row.get("saved"),
        )?;

        let new_saved = !current_saved;

        conn.execute(
            "UPDATE transcription_history SET saved = ?1 WHERE id = ?2",
            params![new_saved, id],
        )?;

        debug!("Toggled saved status for entry {}: {}", id, new_saved);
        let event_type = if new_saved {
            HISTORY_EVENT_SAVED
        } else {
            HISTORY_EVENT_UNSAVED
        };
        self.record_history_event(
            id,
            NewHistoryEvent {
                run_type: None,
                run_id: None,
                event_type: event_type.to_string(),
                source: HISTORY_EVENT_SOURCE_BACKEND.to_string(),
                payload_json: None,
            },
        )?;

        // Emit history updated event
        if let Err(e) = (HistoryUpdatePayload::Toggled { id }).emit(&self.app_handle) {
            error!("Failed to emit history-updated event: {}", e);
        }

        Ok(())
    }

    pub fn get_audio_file_path(&self, file_name: &str) -> PathBuf {
        self.recordings_dir.join(file_name)
    }

    pub async fn get_entry_by_id(&self, id: i64) -> Result<Option<HistoryEntry>> {
        self.get_entry_by_id_sync(id)
    }

    fn get_entry_by_id_sync(&self, id: i64) -> Result<Option<HistoryEntry>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {}
             FROM transcription_history
             WHERE id = ?1",
            Self::history_entry_select_columns()
        ))?;

        let entry = stmt.query_row([id], Self::map_history_entry).optional()?;

        Ok(entry)
    }

    pub async fn get_entry_detail(&self, id: i64) -> Result<Option<HistoryEntryDetail>> {
        let conn = self.get_connection()?;
        let Some(entry) = self.get_entry_by_id_sync(id)? else {
            return Ok(None);
        };

        let transcription_runs = {
            let mut stmt = conn.prepare(
                "SELECT id, history_entry_id, provider_id, model, language, status, transcript_text, latency_ms, error_summary, created_at
                 FROM transcription_runs
                 WHERE history_entry_id = ?1
                 ORDER BY created_at ASC, id ASC",
            )?;
            let rows = stmt.query_map(params![id], Self::map_transcription_run)?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };

        let post_process_runs = {
            let mut stmt = conn.prepare(
                "SELECT id, history_entry_id, preset_id, preset_version, provider_id, model, status, input_text, output_text, prompt_template_snapshot, latency_ms, error_summary, created_at
                 FROM post_process_runs
                 WHERE history_entry_id = ?1
                 ORDER BY created_at ASC, id ASC",
            )?;
            let rows = stmt.query_map(params![id], Self::map_post_process_run)?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };

        let events = {
            let mut stmt = conn.prepare(
                "SELECT id, history_entry_id, run_type, run_id, event_type, source, payload_json, created_at
                 FROM history_events
                 WHERE history_entry_id = ?1
                 ORDER BY created_at ASC, id ASC",
            )?;
            let rows = stmt.query_map(params![id], Self::map_history_event)?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };

        Ok(Some(HistoryEntryDetail {
            entry,
            transcription_runs,
            post_process_runs,
            events,
        }))
    }

    pub async fn delete_entry(&self, id: i64) -> Result<()> {
        let conn = self.get_connection()?;

        // Get the entry to find the file name
        if let Some(entry) = self.get_entry_by_id(id).await? {
            // Delete the audio file first
            let file_path = self.get_audio_file_path(&entry.file_name);
            if file_path.exists() {
                if let Err(e) = fs::remove_file(&file_path) {
                    error!("Failed to delete audio file {}: {}", entry.file_name, e);
                    // Continue with database deletion even if file deletion fails
                }
            }
        }

        Self::delete_entry_rows_with_conn(&conn, id)?;

        debug!("Deleted history entry with id: {}", id);

        // Emit history updated event
        if let Err(e) = (HistoryUpdatePayload::Deleted { id }).emit(&self.app_handle) {
            error!("Failed to emit history-updated event: {}", e);
        }

        Ok(())
    }

    fn format_timestamp_title(&self, timestamp: i64) -> String {
        if let Some(utc_datetime) = DateTime::from_timestamp(timestamp, 0) {
            // Convert UTC to local timezone
            let local_datetime = utc_datetime.with_timezone(&Local);
            local_datetime.format("%B %e, %Y - %l:%M%p").to_string()
        } else {
            format!("Recording {}", timestamp)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            "CREATE TABLE transcription_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_name TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                saved BOOLEAN NOT NULL DEFAULT 0,
                title TEXT NOT NULL,
                transcription_text TEXT NOT NULL,
                post_processed_text TEXT,
                post_process_prompt TEXT,
                post_process_requested BOOLEAN NOT NULL DEFAULT 0,
                post_process_preset_id TEXT,
                post_process_preset_version INTEGER,
                asr_provider_id TEXT,
                asr_model TEXT,
                asr_language TEXT,
                user_edited_text TEXT,
                user_edited_at INTEGER,
                latest_transcription_run_id INTEGER,
                latest_post_process_run_id INTEGER,
                status TEXT NOT NULL DEFAULT 'completed'
            );
            CREATE TABLE post_process_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                history_entry_id INTEGER NOT NULL,
                preset_id TEXT,
                preset_version INTEGER,
                provider_id TEXT,
                model TEXT,
                status TEXT NOT NULL,
                input_text TEXT,
                output_text TEXT,
                prompt_template_snapshot TEXT,
                latency_ms INTEGER NOT NULL DEFAULT 0,
                error_summary TEXT,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE transcription_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                history_entry_id INTEGER NOT NULL,
                provider_id TEXT,
                model TEXT,
                language TEXT,
                status TEXT NOT NULL,
                transcript_text TEXT NOT NULL DEFAULT '',
                latency_ms INTEGER NOT NULL DEFAULT 0,
                error_summary TEXT,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE history_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                history_entry_id INTEGER NOT NULL,
                run_type TEXT,
                run_id INTEGER,
                event_type TEXT NOT NULL,
                source TEXT NOT NULL,
                payload_json TEXT,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE context_probe_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                captured_at INTEGER NOT NULL,
                source TEXT NOT NULL,
                status TEXT NOT NULL,
                confidence TEXT NOT NULL,
                latency_ms INTEGER NOT NULL DEFAULT 0,
                app_name TEXT,
                bundle_id TEXT,
                pid INTEGER,
                window_title TEXT,
                element_role TEXT,
                element_subrole TEXT,
                is_secure BOOLEAN NOT NULL DEFAULT 0,
                value_text TEXT,
                before_text TEXT,
                selected_text TEXT,
                after_text TEXT,
                selected_location_utf16 INTEGER,
                selected_length_utf16 INTEGER,
                number_of_characters INTEGER,
                available_attributes_json TEXT,
                failure_reason TEXT,
                truncated BOOLEAN NOT NULL DEFAULT 0
            );",
        )
        .expect("create transcription_history table");
        conn
    }

    fn insert_entry(conn: &Connection, timestamp: i64, text: &str, post_processed: Option<&str>) {
        conn.execute(
            "INSERT INTO transcription_history (
                file_name,
                timestamp,
                saved,
                title,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_requested,
                post_process_preset_id,
                post_process_preset_version,
                asr_provider_id,
                asr_model,
                asr_language,
                latest_transcription_run_id,
                latest_post_process_run_id,
                status
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                format!("speakmore-{}.wav", timestamp),
                timestamp,
                false,
                format!("Recording {}", timestamp),
                text,
                post_processed,
                Option::<String>::None,
                false,
                Option::<String>::None,
                Option::<i64>::None,
                Option::<String>::None,
                Option::<String>::None,
                Option::<String>::None,
                Option::<i64>::None,
                Option::<i64>::None,
                if text.is_empty() {
                    HISTORY_STATUS_FAILED
                } else {
                    HISTORY_STATUS_COMPLETED
                },
            ],
        )
        .expect("insert history entry");
    }

    #[test]
    fn get_latest_entry_returns_none_when_empty() {
        let conn = setup_conn();
        let entry = HistoryManager::get_latest_entry_with_conn(&conn).expect("fetch latest entry");
        assert!(entry.is_none());
    }

    #[test]
    fn get_latest_entry_returns_newest_entry() {
        let conn = setup_conn();
        insert_entry(&conn, 100, "first", None);
        insert_entry(&conn, 200, "second", Some("processed"));

        let entry = HistoryManager::get_latest_entry_with_conn(&conn)
            .expect("fetch latest entry")
            .expect("entry exists");

        assert_eq!(entry.timestamp, 200);
        assert_eq!(entry.transcription_text, "second");
        assert_eq!(entry.post_processed_text.as_deref(), Some("processed"));
        assert_eq!(entry.final_text, "processed");
    }

    #[test]
    fn map_history_entry_prefers_user_edited_text_for_final_text() {
        let conn = setup_conn();
        insert_entry(&conn, 250, "raw", Some("processed"));
        let history_entry_id = conn.last_insert_rowid();

        conn.execute(
            "UPDATE transcription_history
             SET user_edited_text = ?1,
                 user_edited_at = ?2
             WHERE id = ?3",
            params!["human approved", 251_i64, history_entry_id],
        )
        .expect("set user edit");

        let entry = HistoryManager::get_latest_entry_with_conn(&conn)
            .expect("fetch latest entry")
            .expect("entry exists");

        assert_eq!(entry.user_edited_text.as_deref(), Some("human approved"));
        assert_eq!(entry.user_edited_at, Some(251));
        assert_eq!(entry.final_text, "human approved");
    }

    #[test]
    fn update_user_edit_saves_and_clears_without_storing_text_in_events() {
        let conn = setup_conn();
        insert_entry(&conn, 260, "raw", Some("processed"));
        let history_entry_id = conn.last_insert_rowid();

        let edited = HistoryManager::update_user_edit_with_conn(
            &conn,
            history_entry_id,
            Some("human approved".to_string()),
        )
        .expect("save user edit");

        assert_eq!(edited.user_edited_text.as_deref(), Some("human approved"));
        assert!(edited.user_edited_at.is_some());
        assert_eq!(edited.final_text, "human approved");

        let saved_event = conn
            .query_row(
                "SELECT event_type, payload_json FROM history_events WHERE history_entry_id = ?1 ORDER BY id DESC LIMIT 1",
                params![history_entry_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .expect("fetch save event");
        assert_eq!(saved_event.0, HISTORY_EVENT_USER_EDIT_SAVED);
        assert_eq!(saved_event.1.as_deref(), Some(r#"{"text_len":14}"#));
        assert!(!saved_event.1.unwrap().contains("human approved"));

        let cleared = HistoryManager::update_user_edit_with_conn(&conn, history_entry_id, None)
            .expect("clear user edit");

        assert!(cleared.user_edited_text.is_none());
        assert!(cleared.user_edited_at.is_none());
        assert_eq!(cleared.final_text, "processed");

        let cleared_event = conn
            .query_row(
                "SELECT event_type, payload_json FROM history_events WHERE history_entry_id = ?1 ORDER BY id DESC LIMIT 1",
                params![history_entry_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .expect("fetch clear event");
        assert_eq!(cleared_event.0, HISTORY_EVENT_USER_EDIT_CLEARED);
        assert_eq!(cleared_event.1.as_deref(), Some(r#"{"text_len":14}"#));
    }

    #[test]
    fn get_latest_completed_entry_skips_empty_entries() {
        let conn = setup_conn();
        insert_entry(&conn, 100, "completed", None);
        insert_entry(&conn, 200, "", None);

        let entry = HistoryManager::get_latest_completed_entry_with_conn(&conn)
            .expect("fetch latest completed entry")
            .expect("completed entry exists");

        assert_eq!(entry.timestamp, 100);
        assert_eq!(entry.transcription_text, "completed");
    }

    #[test]
    fn map_history_entry_reads_asr_metadata() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO transcription_history (
                file_name,
                timestamp,
                saved,
                title,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_requested,
                post_process_preset_id,
                post_process_preset_version,
                asr_provider_id,
                asr_model,
                asr_language,
                latest_transcription_run_id,
                latest_post_process_run_id,
                status
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                "speakmore-300.wav",
                300,
                false,
                "Recording 300",
                "hello",
                Option::<String>::None,
                Option::<String>::None,
                false,
                "clean_dictation",
                1_i64,
                "aliyun_qwen3_asr_flash",
                "qwen3-asr-flash",
                "zh",
                Option::<i64>::None,
                Option::<i64>::None,
                HISTORY_STATUS_COMPLETED,
            ],
        )
        .expect("insert history entry");

        let entry = HistoryManager::get_latest_entry_with_conn(&conn)
            .expect("fetch latest entry")
            .expect("entry exists");

        assert_eq!(
            entry.asr_provider_id.as_deref(),
            Some("aliyun_qwen3_asr_flash")
        );
        assert_eq!(entry.asr_model.as_deref(), Some("qwen3-asr-flash"));
        assert_eq!(entry.asr_language.as_deref(), Some("zh"));
        assert_eq!(
            entry.post_process_preset_id.as_deref(),
            Some("clean_dictation")
        );
        assert_eq!(entry.post_process_preset_version, Some(1));
    }

    #[test]
    fn insert_post_process_run_records_success_and_failure_metadata() {
        let conn = setup_conn();
        insert_entry(&conn, 500, "hello", Some("Hello."));
        let history_entry_id = conn.last_insert_rowid();

        let success = HistoryManager::insert_post_process_run_with_conn(
            &conn,
            history_entry_id,
            NewPostProcessRun {
                preset_id: Some("clean_dictation".to_string()),
                preset_version: Some(1),
                provider_id: Some("openai".to_string()),
                model: Some("gpt-test".to_string()),
                status: "success".to_string(),
                input_text: Some("hello".to_string()),
                output_text: Some("Hello.".to_string()),
                prompt_template_snapshot: Some("Clean ${output}".to_string()),
                latency_ms: 120,
                error_summary: None,
            },
        )
        .expect("insert success run");

        assert_eq!(success.history_entry_id, history_entry_id);
        assert_eq!(success.status, "success");
        assert_eq!(success.preset_id.as_deref(), Some("clean_dictation"));
        assert_eq!(success.model.as_deref(), Some("gpt-test"));
        assert_eq!(success.input_text.as_deref(), Some("hello"));
        assert_eq!(success.output_text.as_deref(), Some("Hello."));

        let failed = HistoryManager::insert_post_process_run_with_conn(
            &conn,
            history_entry_id,
            NewPostProcessRun {
                preset_id: Some("clean_dictation".to_string()),
                preset_version: Some(1),
                provider_id: Some("openai".to_string()),
                model: Some("bad-model".to_string()),
                status: "failed".to_string(),
                input_text: Some("hello".to_string()),
                output_text: None,
                prompt_template_snapshot: Some("Clean ${output}".to_string()),
                latency_ms: 20,
                error_summary: Some("model not found".to_string()),
            },
        )
        .expect("insert failed run");

        assert_eq!(failed.status, "failed");
        assert_eq!(failed.error_summary.as_deref(), Some("model not found"));
    }

    #[test]
    fn context_probe_runs_insert_query_and_clear() {
        let conn = setup_conn();
        let run = HistoryManager::insert_context_probe_run_with_conn(
            &conn,
            NewContextProbeRun {
                captured_at: 800,
                source: "manual".to_string(),
                status: ContextProbeStatus::Success,
                confidence: ContextProbeConfidence::High,
                latency_ms: 12,
                app_name: Some("TextEdit".to_string()),
                bundle_id: Some("com.apple.TextEdit".to_string()),
                pid: Some(123),
                window_title: Some("Untitled".to_string()),
                element_role: Some("AXTextArea".to_string()),
                element_subrole: None,
                is_secure: false,
                value_text: Some("hello world".to_string()),
                before_text: Some("hello ".to_string()),
                selected_text: Some("world".to_string()),
                after_text: Some(String::new()),
                selected_location_utf16: Some(6),
                selected_length_utf16: Some(5),
                number_of_characters: Some(11),
                available_attributes_json: Some(r#"["AXValue"]"#.to_string()),
                failure_reason: None,
                truncated: false,
            },
        )
        .expect("insert context probe");

        assert_eq!(run.id, 1);
        assert_eq!(run.status, ContextProbeStatus::Success);
        assert_eq!(run.confidence, ContextProbeConfidence::High);
        assert_eq!(run.value_text.as_deref(), Some("hello world"));
        assert_eq!(run.selected_text.as_deref(), Some("world"));

        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} FROM context_probe_runs ORDER BY captured_at DESC, id DESC LIMIT 10",
                HistoryManager::context_probe_select_columns()
            ))
            .expect("prepare context query");
        let rows = stmt
            .query_map([], HistoryManager::map_context_probe_run)
            .expect("query context rows")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("collect context rows");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].app_name.as_deref(), Some("TextEdit"));

        let deleted = conn
            .execute("DELETE FROM context_probe_runs", [])
            .expect("clear context runs");
        assert_eq!(deleted, 1);
    }

    #[test]
    fn insert_transcription_run_records_attempt_metadata() {
        let conn = setup_conn();
        insert_entry(&conn, 600, "hello", None);
        let history_entry_id = conn.last_insert_rowid();

        let run = HistoryManager::insert_transcription_run_with_conn(
            &conn,
            history_entry_id,
            NewTranscriptionRun {
                provider_id: Some("built_in_local".to_string()),
                model: Some("ggml-small.bin".to_string()),
                language: Some("en".to_string()),
                status: TRANSCRIPTION_RUN_STATUS_SUCCESS.to_string(),
                transcript_text: "hello".to_string(),
                latency_ms: 42,
                error_summary: None,
            },
        )
        .expect("insert transcription run");

        assert_eq!(run.history_entry_id, history_entry_id);
        assert_eq!(run.provider_id.as_deref(), Some("built_in_local"));
        assert_eq!(run.transcript_text, "hello");
        assert_eq!(run.latency_ms, 42);
    }

    #[test]
    fn deleting_entry_rows_removes_child_evidence() {
        let conn = setup_conn();
        insert_entry(&conn, 700, "hello", Some("Hello."));
        let history_entry_id = conn.last_insert_rowid();

        HistoryManager::insert_transcription_run_with_conn(
            &conn,
            history_entry_id,
            NewTranscriptionRun {
                provider_id: Some("built_in_local".to_string()),
                model: Some("ggml-small.bin".to_string()),
                language: Some("en".to_string()),
                status: TRANSCRIPTION_RUN_STATUS_SUCCESS.to_string(),
                transcript_text: "hello".to_string(),
                latency_ms: 42,
                error_summary: None,
            },
        )
        .expect("insert transcription run");
        HistoryManager::insert_post_process_run_with_conn(
            &conn,
            history_entry_id,
            NewPostProcessRun {
                preset_id: Some("clean_dictation".to_string()),
                preset_version: Some(1),
                provider_id: Some("openai".to_string()),
                model: Some("gpt-test".to_string()),
                status: "success".to_string(),
                input_text: Some("hello".to_string()),
                output_text: Some("Hello.".to_string()),
                prompt_template_snapshot: Some("Clean ${output}".to_string()),
                latency_ms: 120,
                error_summary: None,
            },
        )
        .expect("insert post-process run");
        conn.execute(
            "INSERT INTO history_events (
                history_entry_id, run_type, run_id, event_type, source, payload_json, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                history_entry_id,
                HISTORY_RUN_TYPE_TRANSCRIPTION,
                1_i64,
                HISTORY_EVENT_PASTE_SUCCEEDED,
                HISTORY_EVENT_SOURCE_BACKEND,
                Option::<String>::None,
                700_i64,
            ],
        )
        .expect("insert history event");

        HistoryManager::delete_entry_rows_with_conn(&conn, history_entry_id)
            .expect("delete entry rows");

        let history_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM transcription_history WHERE id = ?1",
                params![history_entry_id],
                |row| row.get(0),
            )
            .expect("count history rows");
        assert_eq!(history_count, 0);

        for table in ["transcription_runs", "post_process_runs", "history_events"] {
            let count: i64 = conn
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE history_entry_id = ?1"),
                    params![history_entry_id],
                    |row| row.get(0),
                )
                .expect("count child rows");
            assert_eq!(count, 0, "{table} should be empty for entry");
        }
    }

    #[test]
    fn migrations_add_asr_metadata_columns_to_existing_history() {
        let mut conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            "CREATE TABLE transcription_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_name TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                saved BOOLEAN NOT NULL DEFAULT 0,
                title TEXT NOT NULL,
                transcription_text TEXT NOT NULL,
                post_processed_text TEXT,
                post_process_prompt TEXT,
                post_process_requested BOOLEAN NOT NULL DEFAULT 0
            );
            PRAGMA user_version = 4;",
        )
        .expect("create pre-ASR history table");
        conn.execute(
            "INSERT INTO transcription_history (
                file_name,
                timestamp,
                saved,
                title,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_requested
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                "speakmore-400.wav",
                400,
                false,
                "Recording 400",
                "old entry",
                Option::<String>::None,
                Option::<String>::None,
                false,
            ],
        )
        .expect("insert pre-ASR history entry");

        let migrations = Migrations::new(MIGRATIONS.to_vec());
        migrations.to_latest(&mut conn).expect("run migrations");

        let entry = HistoryManager::get_latest_entry_with_conn(&conn)
            .expect("fetch latest entry")
            .expect("entry exists");

        assert_eq!(entry.transcription_text, "old entry");
        assert!(entry.asr_provider_id.is_none());
        assert!(entry.asr_model.is_none());
        assert!(entry.asr_language.is_none());
        assert!(entry.post_process_preset_id.is_none());
        assert!(entry.post_process_preset_version.is_none());
        assert_eq!(entry.status, HISTORY_STATUS_COMPLETED);
        assert!(entry.latest_transcription_run_id.is_none());
        assert!(entry.latest_post_process_run_id.is_none());
        assert!(entry.user_edited_text.is_none());
        assert!(entry.user_edited_at.is_none());

        let has_runs_table: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='post_process_runs'",
                [],
                |row| row.get(0),
            )
            .expect("check post_process_runs table");
        assert!(has_runs_table);

        let has_transcription_runs_table: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='transcription_runs'",
                [],
                |row| row.get(0),
            )
            .expect("check transcription_runs table");
        assert!(has_transcription_runs_table);

        let has_history_events_table: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='history_events'",
                [],
                |row| row.get(0),
            )
            .expect("check history_events table");
        assert!(has_history_events_table);

        let has_post_process_input_text: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('post_process_runs') WHERE name = 'input_text'",
                [],
                |row| row.get(0),
            )
            .expect("check post_process_runs input_text column");
        assert!(has_post_process_input_text);

        let has_user_edited_text: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('transcription_history') WHERE name = 'user_edited_text'",
                [],
                |row| row.get(0),
            )
            .expect("check history user_edited_text column");
        assert!(has_user_edited_text);

        let has_user_edited_at: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('transcription_history') WHERE name = 'user_edited_at'",
                [],
                |row| row.get(0),
            )
            .expect("check history user_edited_at column");
        assert!(has_user_edited_at);

        let has_context_probe_runs_table: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='context_probe_runs'",
                [],
                |row| row.get(0),
            )
            .expect("check context_probe_runs table");
        assert!(has_context_probe_runs_table);

        let has_context_probe_value_text: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('context_probe_runs') WHERE name = 'value_text'",
                [],
                |row| row.get(0),
            )
            .expect("check context_probe_runs value_text column");
        assert!(has_context_probe_value_text);
    }
}

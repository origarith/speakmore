use crate::cli::{CliDataCommand, ConfigSubcommand, HistorySubcommand};
use crate::context_awareness::{ContextProbeConfidence, ContextProbeRun, ContextProbeStatus};
use crate::managers::history::{
    HistoryEntry, HistoryEvent, PostProcessRun, TranscriptionRun, HISTORY_EVENT_SAVED,
    HISTORY_EVENT_UNSAVED, HISTORY_EVENT_USER_EDIT_CLEARED, HISTORY_EVENT_USER_EDIT_SAVED,
};
use crate::settings::{AppSettings, SETTINGS_STORE_PATH};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

const APP_IDENTIFIER: &str = "app.speakmore.desktop";
const HISTORY_DB_FILE: &str = "history.db";
const RECORDINGS_DIR: &str = "recordings";
const CLI_EVENT_SOURCE: &str = "cli";

type CliResult<T> = Result<T, DataCliError>;

#[derive(Debug)]
struct DataCliError {
    code: String,
    message: String,
}

impl DataCliError {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    fn from_error(code: impl Into<String>, error: impl std::fmt::Display) -> Self {
        Self::new(code, error.to_string())
    }
}

#[derive(Serialize)]
struct OkEnvelope<T: Serialize> {
    ok: bool,
    data: T,
}

#[derive(Serialize)]
struct ErrorEnvelope {
    ok: bool,
    error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
    code: String,
    message: String,
}

pub fn run(command: &CliDataCommand) -> ! {
    let result = execute(command);
    match result {
        Ok(data) => {
            print_json(&OkEnvelope { ok: true, data });
            std::process::exit(0);
        }
        Err(error) => {
            print_json(&ErrorEnvelope {
                ok: false,
                error: ErrorBody {
                    code: error.code,
                    message: error.message,
                },
            });
            std::process::exit(1);
        }
    }
}

fn print_json<T: Serialize>(value: &T) {
    match serde_json::to_string(value) {
        Ok(text) => println!("{text}"),
        Err(error) => {
            println!(
                "{}",
                json!({
                    "ok": false,
                    "error": {
                        "code": "serialization_error",
                        "message": error.to_string()
                    }
                })
            );
        }
    }
}

fn execute(command: &CliDataCommand) -> CliResult<Value> {
    let data_dir = app_data_dir()?;
    match command {
        CliDataCommand::History(command) => execute_history(&data_dir, &command.command),
        CliDataCommand::Config(command) => execute_config(&data_dir, &command.command),
    }
}

fn app_data_dir() -> CliResult<PathBuf> {
    if let Some(dir) = crate::portable::data_dir() {
        return Ok(dir.clone());
    }

    platform_app_data_dir().map(|dir| dir.join(APP_IDENTIFIER))
}

#[cfg(target_os = "macos")]
fn platform_app_data_dir() -> CliResult<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join("Library").join("Application Support"))
        .ok_or_else(|| DataCliError::new("app_data_dir_unavailable", "HOME is not set"))
}

#[cfg(target_os = "windows")]
fn platform_app_data_dir() -> CliResult<PathBuf> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| DataCliError::new("app_data_dir_unavailable", "APPDATA is not set"))
}

#[cfg(target_os = "linux")]
fn platform_app_data_dir() -> CliResult<PathBuf> {
    if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(config_home));
    }

    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".config"))
        .ok_or_else(|| DataCliError::new("app_data_dir_unavailable", "HOME is not set"))
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn platform_app_data_dir() -> CliResult<PathBuf> {
    Err(DataCliError::new(
        "unsupported_platform",
        "data CLI is only supported on macOS, Windows, and Linux",
    ))
}

fn execute_config(data_dir: &Path, command: &ConfigSubcommand) -> CliResult<Value> {
    match command {
        ConfigSubcommand::Get { path } => {
            let (_, settings_value, _) = load_settings(data_dir)?;
            if let Some(path) = path {
                let value = get_dotted_path(&settings_value, path).ok_or_else(|| {
                    DataCliError::new(
                        "config_path_not_found",
                        format!("Unknown settings path: {path}"),
                    )
                })?;
                Ok(value.clone())
            } else {
                Ok(settings_value)
            }
        }
        ConfigSubcommand::Patch { file, stdin } => {
            let patch = read_json_input(file.as_deref(), *stdin)?;
            let settings = patch_settings(data_dir, patch, true)?;
            serde_json::to_value(settings)
                .map_err(|error| DataCliError::from_error("serialization_error", error))
        }
        ConfigSubcommand::Validate { file, stdin } => {
            let patch = read_json_input(file.as_deref(), *stdin)?;
            let settings = patch_settings(data_dir, patch, false)?;
            serde_json::to_value(settings)
                .map_err(|error| DataCliError::from_error("serialization_error", error))
        }
    }
}

fn settings_path(data_dir: &Path) -> PathBuf {
    data_dir.join(SETTINGS_STORE_PATH)
}

fn load_settings(data_dir: &Path) -> CliResult<(Value, Value, AppSettings)> {
    load_settings_from_path(&settings_path(data_dir))
}

fn load_settings_from_path(path: &Path) -> CliResult<(Value, Value, AppSettings)> {
    if !path.exists() {
        return Err(DataCliError::new(
            "settings_not_found",
            format!("Settings store not found: {}", path.display()),
        ));
    }

    let text = fs::read_to_string(path)
        .map_err(|error| DataCliError::from_error("settings_read_failed", error))?;
    let root: Value = serde_json::from_str(&text)
        .map_err(|error| DataCliError::from_error("settings_json_invalid", error))?;
    let settings_value = root.get("settings").cloned().ok_or_else(|| {
        DataCliError::new(
            "settings_missing",
            "Settings store does not contain a 'settings' object",
        )
    })?;
    let settings = serde_json::from_value::<AppSettings>(settings_value.clone())
        .map_err(|error| DataCliError::from_error("settings_invalid", error))?;

    Ok((root, settings_value, settings))
}

fn patch_settings(data_dir: &Path, patch: Value, write: bool) -> CliResult<AppSettings> {
    let path = settings_path(data_dir);
    let (mut root, mut settings_value, _) = load_settings_from_path(&path)?;
    merge_json(&mut settings_value, patch);
    let settings = serde_json::from_value::<AppSettings>(settings_value.clone())
        .map_err(|error| DataCliError::from_error("settings_invalid", error))?;

    if write {
        let settings_value = serde_json::to_value(&settings)
            .map_err(|error| DataCliError::from_error("serialization_error", error))?;
        let object = root.as_object_mut().ok_or_else(|| {
            DataCliError::new(
                "settings_json_invalid",
                "Settings store root is not an object",
            )
        })?;
        object.insert("settings".to_string(), settings_value);
        let text = serde_json::to_string_pretty(&root)
            .map_err(|error| DataCliError::from_error("serialization_error", error))?;
        fs::write(path, text)
            .map_err(|error| DataCliError::from_error("settings_write_failed", error))?;
    }

    Ok(settings)
}

fn merge_json(target: &mut Value, patch: Value) {
    match (target, patch) {
        (Value::Object(target_map), Value::Object(patch_map)) => {
            for (key, patch_value) in patch_map {
                match target_map.get_mut(&key) {
                    Some(target_value) => merge_json(target_value, patch_value),
                    None => {
                        target_map.insert(key, patch_value);
                    }
                }
            }
        }
        (target_value, patch_value) => {
            *target_value = patch_value;
        }
    }
}

fn get_dotted_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for segment in path.split('.') {
        if segment.is_empty() {
            return None;
        }
        current = match current {
            Value::Object(map) => map.get(segment)?,
            Value::Array(items) => items.get(segment.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(current)
}

fn read_json_input(file: Option<&Path>, stdin: bool) -> CliResult<Value> {
    let input = read_text_input(file, stdin)?;
    serde_json::from_str(&input).map_err(|error| DataCliError::from_error("json_invalid", error))
}

fn read_text_input(file: Option<&Path>, stdin: bool) -> CliResult<String> {
    match (file, stdin) {
        (Some(path), false) => fs::read_to_string(path)
            .map_err(|error| DataCliError::from_error("input_read_failed", error)),
        (None, true) => {
            let mut input = String::new();
            io::stdin()
                .read_to_string(&mut input)
                .map_err(|error| DataCliError::from_error("input_read_failed", error))?;
            Ok(input)
        }
        (None, false) => Err(DataCliError::new(
            "input_required",
            "Provide --file or --stdin",
        )),
        (Some(_), true) => Err(DataCliError::new(
            "input_conflict",
            "Use only one of --file or --stdin",
        )),
    }
}

fn execute_history(data_dir: &Path, command: &HistorySubcommand) -> CliResult<Value> {
    let mut conn = open_history_connection(data_dir)?;
    match command {
        HistorySubcommand::List { limit, cursor } => {
            let data = list_history(&conn, *limit, *cursor)?;
            serde_json::to_value(data)
                .map_err(|error| DataCliError::from_error("serialization_error", error))
        }
        HistorySubcommand::Get { id } => {
            let data = get_history_detail(&conn, data_dir, *id)?;
            serde_json::to_value(data)
                .map_err(|error| DataCliError::from_error("serialization_error", error))
        }
        HistorySubcommand::Edit { id, text, stdin } => {
            let text = read_history_edit_text(text.as_deref(), *stdin)?;
            let entry = update_user_edit(&mut conn, *id, Some(text))?;
            serde_json::to_value(entry)
                .map_err(|error| DataCliError::from_error("serialization_error", error))
        }
        HistorySubcommand::ClearEdit { id } => {
            let entry = update_user_edit(&mut conn, *id, None)?;
            serde_json::to_value(entry)
                .map_err(|error| DataCliError::from_error("serialization_error", error))
        }
        HistorySubcommand::Save { id } => {
            let entry = set_saved(&mut conn, *id, true)?;
            serde_json::to_value(entry)
                .map_err(|error| DataCliError::from_error("serialization_error", error))
        }
        HistorySubcommand::Unsave { id } => {
            let entry = set_saved(&mut conn, *id, false)?;
            serde_json::to_value(entry)
                .map_err(|error| DataCliError::from_error("serialization_error", error))
        }
        HistorySubcommand::Delete { id } => delete_history_entry(&mut conn, data_dir, *id),
    }
}

fn open_history_connection(data_dir: &Path) -> CliResult<Connection> {
    let path = data_dir.join(HISTORY_DB_FILE);
    if !path.exists() {
        return Err(DataCliError::new(
            "history_db_not_found",
            format!("History database not found: {}", path.display()),
        ));
    }
    Connection::open(path)
        .map_err(|error| DataCliError::from_error("history_db_open_failed", error))
}

#[derive(Serialize)]
struct PaginatedHistorySummary {
    entries: Vec<HistorySummaryEntry>,
    has_more: bool,
}

#[derive(Serialize)]
struct HistorySummaryEntry {
    id: i64,
    timestamp: i64,
    title: String,
    saved: bool,
    status: String,
    final_text: String,
    text_layer: String,
    asr_provider_id: Option<String>,
    asr_model: Option<String>,
    asr_language: Option<String>,
    post_process_requested: bool,
}

#[derive(Serialize)]
struct HistoryDetailData {
    entry: HistoryEntry,
    transcription_runs: Vec<TranscriptionRun>,
    post_process_runs: Vec<PostProcessRun>,
    events: Vec<HistoryEvent>,
    focused_context: Option<ContextProbeRun>,
    audio_path: String,
}

fn list_history(
    conn: &Connection,
    limit: usize,
    cursor: Option<i64>,
) -> CliResult<PaginatedHistorySummary> {
    if limit == 0 {
        return Err(DataCliError::new(
            "invalid_limit",
            "History list limit must be greater than 0",
        ));
    }

    let limit = limit.min(100);
    let fetch_count = (limit + 1) as i64;
    let entries = if let Some(cursor) = cursor {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} FROM transcription_history WHERE id < ?1 ORDER BY id DESC LIMIT ?2",
                history_entry_columns()
            ))
            .map_err(|error| DataCliError::from_error("history_query_failed", error))?;
        let rows = stmt
            .query_map(params![cursor, fetch_count], map_history_entry)
            .map_err(|error| DataCliError::from_error("history_query_failed", error))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| DataCliError::from_error("history_query_failed", error))?
    } else {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} FROM transcription_history ORDER BY id DESC LIMIT ?1",
                history_entry_columns()
            ))
            .map_err(|error| DataCliError::from_error("history_query_failed", error))?;
        let rows = stmt
            .query_map(params![fetch_count], map_history_entry)
            .map_err(|error| DataCliError::from_error("history_query_failed", error))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| DataCliError::from_error("history_query_failed", error))?
    };

    let has_more = entries.len() > limit;
    let entries = entries
        .into_iter()
        .take(limit)
        .map(|entry| {
            let (final_text, text_layer) = final_text_and_layer(&entry);
            HistorySummaryEntry {
                id: entry.id,
                timestamp: entry.timestamp,
                title: entry.title,
                saved: entry.saved,
                status: entry.status,
                final_text,
                text_layer,
                asr_provider_id: entry.asr_provider_id,
                asr_model: entry.asr_model,
                asr_language: entry.asr_language,
                post_process_requested: entry.post_process_requested,
            }
        })
        .collect();

    Ok(PaginatedHistorySummary { entries, has_more })
}

fn get_history_detail(conn: &Connection, data_dir: &Path, id: i64) -> CliResult<HistoryDetailData> {
    let entry = get_history_entry(conn, id)?;
    let transcription_runs = {
        let mut stmt = conn
            .prepare(
                "SELECT id, history_entry_id, provider_id, model, language, status, transcript_text, latency_ms, error_summary, created_at
                 FROM transcription_runs
                 WHERE history_entry_id = ?1
                 ORDER BY created_at ASC, id ASC",
            )
            .map_err(|error| DataCliError::from_error("history_query_failed", error))?;
        let rows = stmt
            .query_map(params![id], map_transcription_run)
            .map_err(|error| DataCliError::from_error("history_query_failed", error))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| DataCliError::from_error("history_query_failed", error))?
    };
    let post_process_runs = {
        let mut stmt = conn
            .prepare(
                "SELECT id, history_entry_id, preset_id, preset_version, provider_id, model, status, input_text, output_text, prompt_template_snapshot, latency_ms, error_summary, created_at
                 FROM post_process_runs
                 WHERE history_entry_id = ?1
                 ORDER BY created_at ASC, id ASC",
            )
            .map_err(|error| DataCliError::from_error("history_query_failed", error))?;
        let rows = stmt
            .query_map(params![id], map_post_process_run)
            .map_err(|error| DataCliError::from_error("history_query_failed", error))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| DataCliError::from_error("history_query_failed", error))?
    };
    let events = {
        let mut stmt = conn
            .prepare(
                "SELECT id, history_entry_id, run_type, run_id, event_type, source, payload_json, created_at
                 FROM history_events
                 WHERE history_entry_id = ?1
                 ORDER BY created_at ASC, id ASC",
            )
            .map_err(|error| DataCliError::from_error("history_query_failed", error))?;
        let rows = stmt
            .query_map(params![id], map_history_event)
            .map_err(|error| DataCliError::from_error("history_query_failed", error))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| DataCliError::from_error("history_query_failed", error))?
    };
    let focused_context = get_focused_context_for_entry(conn, id)?;

    let audio_path = data_dir
        .join(RECORDINGS_DIR)
        .join(&entry.file_name)
        .to_string_lossy()
        .to_string();

    Ok(HistoryDetailData {
        entry,
        transcription_runs,
        post_process_runs,
        events,
        focused_context,
        audio_path,
    })
}

fn read_history_edit_text(text: Option<&str>, stdin: bool) -> CliResult<String> {
    let text = match (text, stdin) {
        (Some(text), false) => text.to_string(),
        (None, true) => {
            let mut input = String::new();
            io::stdin()
                .read_to_string(&mut input)
                .map_err(|error| DataCliError::from_error("input_read_failed", error))?;
            input
        }
        (None, false) => {
            return Err(DataCliError::new(
                "input_required",
                "Provide --text or --stdin",
            ))
        }
        (Some(_), true) => {
            return Err(DataCliError::new(
                "input_conflict",
                "Use only one of --text or --stdin",
            ))
        }
    };

    if text.trim().is_empty() {
        return Err(DataCliError::new(
            "empty_edit",
            "Edited text cannot be empty",
        ));
    }

    Ok(text)
}

fn update_user_edit(
    conn: &mut Connection,
    id: i64,
    text: Option<String>,
) -> CliResult<HistoryEntry> {
    ensure_history_entry_exists(conn, id)?;
    let previous_user_edited_text: Option<String> = conn
        .query_row(
            "SELECT user_edited_text FROM transcription_history WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| DataCliError::from_error("history_query_failed", error))?
        .flatten();
    let text_len = text
        .as_ref()
        .map(|text| text.chars().count())
        .or_else(|| {
            previous_user_edited_text
                .as_ref()
                .map(|text| text.chars().count())
        })
        .unwrap_or(0);
    let event_type = if text.is_some() {
        HISTORY_EVENT_USER_EDIT_SAVED
    } else {
        HISTORY_EVENT_USER_EDIT_CLEARED
    };
    let user_edited_at = text.as_ref().map(|_| Utc::now().timestamp());
    conn.execute(
        "UPDATE transcription_history SET user_edited_text = ?1, user_edited_at = ?2 WHERE id = ?3",
        params![text, user_edited_at, id],
    )
    .map_err(|error| DataCliError::from_error("history_update_failed", error))?;

    insert_history_event(
        conn,
        id,
        event_type,
        None,
        None,
        Some(json!({ "text_len": text_len }).to_string()),
    )?;
    get_history_entry(conn, id)
}

fn set_saved(conn: &mut Connection, id: i64, saved: bool) -> CliResult<HistoryEntry> {
    ensure_history_entry_exists(conn, id)?;
    conn.execute(
        "UPDATE transcription_history SET saved = ?1 WHERE id = ?2",
        params![saved, id],
    )
    .map_err(|error| DataCliError::from_error("history_update_failed", error))?;
    insert_history_event(
        conn,
        id,
        if saved {
            HISTORY_EVENT_SAVED
        } else {
            HISTORY_EVENT_UNSAVED
        },
        None,
        None,
        None,
    )?;
    get_history_entry(conn, id)
}

fn delete_history_entry(conn: &mut Connection, data_dir: &Path, id: i64) -> CliResult<Value> {
    let entry = get_history_entry(conn, id)?;
    conn.execute(
        "DELETE FROM history_events WHERE history_entry_id = ?1",
        params![id],
    )
    .map_err(|error| DataCliError::from_error("history_delete_failed", error))?;
    conn.execute(
        "DELETE FROM post_process_runs WHERE history_entry_id = ?1",
        params![id],
    )
    .map_err(|error| DataCliError::from_error("history_delete_failed", error))?;
    conn.execute(
        "DELETE FROM transcription_runs WHERE history_entry_id = ?1",
        params![id],
    )
    .map_err(|error| DataCliError::from_error("history_delete_failed", error))?;
    conn.execute(
        "DELETE FROM context_probe_runs WHERE history_entry_id = ?1",
        params![id],
    )
    .map_err(|error| DataCliError::from_error("history_delete_failed", error))?;
    conn.execute(
        "DELETE FROM transcription_history WHERE id = ?1",
        params![id],
    )
    .map_err(|error| DataCliError::from_error("history_delete_failed", error))?;

    let audio_path = data_dir.join(RECORDINGS_DIR).join(&entry.file_name);
    let audio_deleted = if audio_path.exists() {
        fs::remove_file(&audio_path).is_ok()
    } else {
        false
    };

    Ok(json!({
        "id": id,
        "deleted": true,
        "audio_deleted": audio_deleted
    }))
}

fn insert_history_event(
    conn: &Connection,
    history_entry_id: i64,
    event_type: &str,
    run_type: Option<&str>,
    run_id: Option<i64>,
    payload_json: Option<String>,
) -> CliResult<()> {
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
            run_type,
            run_id,
            event_type,
            CLI_EVENT_SOURCE,
            payload_json,
            Utc::now().timestamp(),
        ],
    )
    .map_err(|error| DataCliError::from_error("history_event_failed", error))?;
    Ok(())
}

fn ensure_history_entry_exists(conn: &Connection, id: i64) -> CliResult<()> {
    let exists = conn
        .query_row(
            "SELECT 1 FROM transcription_history WHERE id = ?1",
            params![id],
            |_| Ok(()),
        )
        .optional()
        .map_err(|error| DataCliError::from_error("history_query_failed", error))?
        .is_some();

    if exists {
        Ok(())
    } else {
        Err(DataCliError::new(
            "history_entry_not_found",
            format!("History entry {id} not found"),
        ))
    }
}

fn get_history_entry(conn: &Connection, id: i64) -> CliResult<HistoryEntry> {
    conn.query_row(
        &format!(
            "SELECT {} FROM transcription_history WHERE id = ?1",
            history_entry_columns()
        ),
        params![id],
        map_history_entry,
    )
    .optional()
    .map_err(|error| DataCliError::from_error("history_query_failed", error))?
    .ok_or_else(|| {
        DataCliError::new(
            "history_entry_not_found",
            format!("History entry {id} not found"),
        )
    })
}

fn history_entry_columns() -> &'static str {
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

fn context_probe_columns() -> &'static str {
    "id, history_entry_id, captured_at, source, status, confidence, latency_ms, app_name, bundle_id, pid, window_title, element_role, element_subrole, is_secure, value_text, before_text, selected_text, after_text, selected_location_utf16, selected_length_utf16, number_of_characters, available_attributes_json, failure_reason, truncated"
}

fn map_context_probe_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<ContextProbeRun> {
    let status: String = row.get("status")?;
    let confidence: String = row.get("confidence")?;

    Ok(ContextProbeRun {
        id: row.get("id")?,
        history_entry_id: row.get("history_entry_id")?,
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

fn get_focused_context_for_entry(
    conn: &Connection,
    history_entry_id: i64,
) -> CliResult<Option<ContextProbeRun>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {}
             FROM context_probe_runs
             WHERE history_entry_id = ?1
             ORDER BY captured_at DESC, id DESC
             LIMIT 1",
            context_probe_columns()
        ))
        .map_err(|error| DataCliError::from_error("history_query_failed", error))?;
    stmt.query_row(params![history_entry_id], map_context_probe_run)
        .optional()
        .map_err(|error| DataCliError::from_error("history_query_failed", error))
}

fn final_text_and_layer(entry: &HistoryEntry) -> (String, String) {
    if let Some(text) = &entry.user_edited_text {
        return (text.clone(), "user_edited".to_string());
    }
    if let Some(text) = &entry.post_processed_text {
        return (text.clone(), "post_processed".to_string());
    }
    (entry.transcription_text.clone(), "raw".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managers::history::HISTORY_STATUS_COMPLETED;
    use crate::settings::get_default_settings;
    use tempfile::tempdir;

    fn create_history_schema(conn: &Connection) {
        conn.execute_batch(
            "
            CREATE TABLE transcription_history (
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
                history_entry_id INTEGER,
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
            ",
        )
        .unwrap();
    }

    fn insert_history_entry(conn: &Connection) -> i64 {
        conn.execute(
            "INSERT INTO transcription_history (
                file_name,
                timestamp,
                saved,
                title,
                transcription_text,
                post_processed_text,
                post_process_requested,
                asr_provider_id,
                asr_model,
                asr_language,
                status
            ) VALUES (?1, ?2, 0, ?3, ?4, ?5, 1, ?6, ?7, ?8, ?9)",
            params![
                "sample.wav",
                1_700_000_000_i64,
                "Sample",
                "raw text",
                "processed text",
                "built_in_local",
                "small",
                "en",
                HISTORY_STATUS_COMPLETED,
            ],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn config_patch_merges_nested_settings_and_writes_wrapper() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(SETTINGS_STORE_PATH);
        let settings = get_default_settings();
        fs::write(
            &path,
            serde_json::to_string(&json!({ "settings": settings })).unwrap(),
        )
        .unwrap();

        let patched = patch_settings(
            dir.path(),
            json!({
                "selected_language": "zh-Hans",
                "asr_family_settings": {
                    "whisper": {
                        "language": "zh-Hans"
                    }
                }
            }),
            true,
        )
        .unwrap();

        assert_eq!(patched.selected_language, "zh-Hans");
        assert_eq!(patched.asr_family_settings.whisper.language, "zh-Hans");

        let root: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert!(root.get("settings").is_some());
    }

    #[test]
    fn config_patch_rejects_invalid_settings_without_writing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(SETTINGS_STORE_PATH);
        let settings = get_default_settings();
        fs::write(
            &path,
            serde_json::to_string_pretty(&json!({ "settings": settings })).unwrap(),
        )
        .unwrap();
        let before = fs::read_to_string(&path).unwrap();

        let error = patch_settings(
            dir.path(),
            json!({
                "recording_retention_period": "not_a_valid_period"
            }),
            true,
        )
        .unwrap_err();

        assert_eq!(error.code, "settings_invalid");
        assert_eq!(fs::read_to_string(path).unwrap(), before);
    }

    #[test]
    fn history_list_and_get_read_existing_schema() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join(HISTORY_DB_FILE);
        let conn = Connection::open(db_path).unwrap();
        create_history_schema(&conn);
        let id = insert_history_entry(&conn);

        let page = list_history(&conn, 20, None).unwrap();
        assert_eq!(page.entries.len(), 1);
        assert_eq!(page.entries[0].id, id);
        assert_eq!(page.entries[0].final_text, "processed text");
        assert_eq!(page.entries[0].text_layer, "post_processed");

        let detail = get_history_detail(&conn, dir.path(), id).unwrap();
        assert_eq!(detail.entry.id, id);
        assert!(detail.audio_path.ends_with("recordings/sample.wav"));
    }

    #[test]
    fn history_mutations_update_rows_and_write_cli_events() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join(HISTORY_DB_FILE);
        let mut conn = Connection::open(db_path).unwrap();
        create_history_schema(&conn);
        let id = insert_history_entry(&conn);

        let edited = update_user_edit(&mut conn, id, Some("edited text".to_string())).unwrap();
        assert_eq!(edited.final_text, "edited text");

        let saved = set_saved(&mut conn, id, true).unwrap();
        assert!(saved.saved);

        let events: Vec<(String, String)> = conn
            .prepare(
                "SELECT event_type, source FROM history_events WHERE history_entry_id = ?1 ORDER BY id",
            )
            .unwrap()
            .query_map(params![id], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();

        assert_eq!(
            events,
            vec![
                (
                    HISTORY_EVENT_USER_EDIT_SAVED.to_string(),
                    CLI_EVENT_SOURCE.to_string()
                ),
                (
                    HISTORY_EVENT_SAVED.to_string(),
                    CLI_EVENT_SOURCE.to_string()
                ),
            ]
        );

        let deleted = delete_history_entry(&mut conn, dir.path(), id).unwrap();
        assert_eq!(deleted["deleted"], true);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM transcription_history", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }
}

use crate::context_awareness::ContextProbeRun;
use crate::managers::history::HistoryManager;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};

#[tauri::command]
#[specta::specta]
pub fn capture_focused_context(app: AppHandle, source: String) -> Result<ContextProbeRun, String> {
    capture_and_store(app, source)
}

#[tauri::command]
#[specta::specta]
pub async fn capture_focused_context_after_delay(
    app: AppHandle,
    source: String,
    delay_ms: u32,
) -> Result<ContextProbeRun, String> {
    let delay_ms = delay_ms.min(10_000);
    if delay_ms > 0 {
        tokio::time::sleep(Duration::from_millis(u64::from(delay_ms))).await;
    }

    capture_and_store(app, source)
}

fn capture_and_store(app: AppHandle, source: String) -> Result<ContextProbeRun, String> {
    let run = crate::context_awareness::capture_focused_context(source);
    let history_manager = app.state::<Arc<HistoryManager>>();
    history_manager
        .save_context_probe_run(run)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn get_context_probe_runs(app: AppHandle, limit: u32) -> Result<Vec<ContextProbeRun>, String> {
    let history_manager = app.state::<Arc<HistoryManager>>();
    history_manager
        .get_context_probe_runs(limit)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn clear_context_probe_runs(app: AppHandle) -> Result<(), String> {
    let history_manager = app.state::<Arc<HistoryManager>>();
    history_manager
        .clear_context_probe_runs()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

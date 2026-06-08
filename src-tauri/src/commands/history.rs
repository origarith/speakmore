use crate::actions::process_transcription_output;
use crate::managers::{
    history::{
        AsrHistoryMetadata, HistoryEntry, HistoryEntryDetail, HistoryManager, NewHistoryEvent,
        NewTranscriptionRun, PaginatedHistory, HISTORY_EVENT_RETRY_REQUESTED,
        HISTORY_EVENT_SOURCE_BACKEND, HISTORY_EVENT_SOURCE_FRONTEND, HISTORY_STATUS_COMPLETED,
        HISTORY_STATUS_EMPTY, HISTORY_STATUS_FAILED, TRANSCRIPTION_RUN_STATUS_EMPTY,
        TRANSCRIPTION_RUN_STATUS_FAILED, TRANSCRIPTION_RUN_STATUS_SUCCESS,
    },
    transcription::TranscriptionManager,
};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, State};

fn elapsed_ms(started_at: Instant) -> i64 {
    started_at.elapsed().as_millis().min(i64::MAX as u128) as i64
}

#[tauri::command]
#[specta::specta]
pub async fn get_history_entries(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    cursor: Option<i64>,
    limit: Option<usize>,
) -> Result<PaginatedHistory, String> {
    history_manager
        .get_history_entries(cursor, limit)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_history_entry_detail(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    id: i64,
) -> Result<HistoryEntryDetail, String> {
    history_manager
        .get_entry_detail(id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("History entry {} not found", id))
}

#[tauri::command]
#[specta::specta]
pub async fn toggle_history_entry_saved(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    id: i64,
) -> Result<(), String> {
    history_manager
        .toggle_saved_status(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_audio_file_path(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    file_name: String,
) -> Result<String, String> {
    let path = history_manager.get_audio_file_path(&file_name);
    path.to_str()
        .ok_or_else(|| "Invalid file path".to_string())
        .map(|s| s.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_history_entry(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    id: i64,
) -> Result<(), String> {
    history_manager
        .delete_entry(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn update_history_entry_user_edit(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    id: i64,
    text: Option<String>,
) -> Result<HistoryEntry, String> {
    history_manager
        .update_user_edit(id, text)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn record_history_event(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    id: i64,
    event_type: String,
    run_type: Option<String>,
    run_id: Option<i64>,
    source: Option<String>,
    payload_json: Option<String>,
) -> Result<(), String> {
    history_manager
        .record_history_event(
            id,
            NewHistoryEvent {
                run_type,
                run_id,
                event_type,
                source: source.unwrap_or_else(|| HISTORY_EVENT_SOURCE_FRONTEND.to_string()),
                payload_json,
            },
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn retry_history_entry_transcription(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
    id: i64,
) -> Result<(), String> {
    let entry = history_manager
        .get_entry_by_id(id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("History entry {} not found", id))?;
    history_manager
        .record_history_event(
            id,
            NewHistoryEvent {
                run_type: None,
                run_id: None,
                event_type: HISTORY_EVENT_RETRY_REQUESTED.to_string(),
                source: HISTORY_EVENT_SOURCE_BACKEND.to_string(),
                payload_json: None,
            },
        )
        .map_err(|e| e.to_string())?;

    let audio_path = history_manager.get_audio_file_path(&entry.file_name);
    let samples = crate::audio_toolkit::read_wav_samples(&audio_path)
        .map_err(|e| format!("Failed to load audio: {}", e))?;

    if samples.is_empty() {
        return Err("Recording has no audio samples".to_string());
    }

    if crate::asr::is_active_asr_provider_local(&app) {
        transcription_manager.initiate_model_load();
    }

    let tm = Arc::clone(&transcription_manager);
    let transcription_started_at = Instant::now();
    let asr_result = match crate::asr::transcribe_with_active_provider(&app, &tm, samples).await {
        Ok(result) => result,
        Err(err) => {
            let metadata = crate::asr::active_provider_metadata(&app);
            let run = NewTranscriptionRun {
                provider_id: metadata
                    .as_ref()
                    .map(|metadata| metadata.provider_id.clone()),
                model: metadata.as_ref().map(|metadata| metadata.model.clone()),
                language: metadata.as_ref().map(|metadata| metadata.language.clone()),
                status: TRANSCRIPTION_RUN_STATUS_FAILED.to_string(),
                transcript_text: String::new(),
                latency_ms: elapsed_ms(transcription_started_at),
                error_summary: Some(err.to_string()),
            };
            history_manager
                .save_transcription_run(id, run, HISTORY_STATUS_FAILED.to_string())
                .map_err(|e| e.to_string())?;
            return Err(err.to_string());
        }
    };
    let transcription = asr_result.text;
    let asr_metadata = AsrHistoryMetadata {
        provider_id: asr_result.provider_id,
        model: asr_result.model,
        language: asr_result.language,
    };
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
    let transcription_run = history_manager
        .save_transcription_run(
            id,
            NewTranscriptionRun {
                provider_id: Some(asr_metadata.provider_id.clone()),
                model: Some(asr_metadata.model.clone()),
                language: Some(asr_metadata.language.clone()),
                status: transcription_run_status.to_string(),
                transcript_text: transcription.clone(),
                latency_ms: elapsed_ms(transcription_started_at),
                error_summary: None,
            },
            history_status.to_string(),
        )
        .map_err(|e| e.to_string())?;

    if transcription.is_empty() {
        history_manager
            .update_transcription(
                id,
                transcription,
                None,
                None,
                None,
                None,
                Some(asr_metadata),
                HISTORY_STATUS_EMPTY.to_string(),
                Some(transcription_run.id),
            )
            .map_err(|e| e.to_string())?;
        return Err("Recording contains no speech".to_string());
    }

    let focused_context = if entry.post_process_requested {
        history_manager
            .get_focused_context_for_entry(id)
            .map_err(|e| e.to_string())?
            .and_then(|context| context.focused_text_context())
    } else {
        None
    };
    let processed = process_transcription_output(
        &app,
        &transcription,
        entry.post_process_requested,
        focused_context.as_ref(),
    )
    .await;
    history_manager
        .update_transcription(
            id,
            transcription,
            processed.post_processed_text.clone(),
            processed.post_process_prompt.clone(),
            processed.post_process_preset_id.clone(),
            processed.post_process_preset_version,
            Some(asr_metadata),
            HISTORY_STATUS_COMPLETED.to_string(),
            Some(transcription_run.id),
        )
        .map_err(|e| e.to_string())
        .and_then(|entry| {
            if let Some(run) = processed.post_process_run {
                history_manager
                    .save_post_process_run(entry.id, run)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            } else {
                Ok(())
            }
        })
}

#[tauri::command]
#[specta::specta]
pub async fn update_history_limit(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    limit: usize,
) -> Result<(), String> {
    let mut settings = crate::settings::get_settings(&app);
    settings.history_limit = limit;
    crate::settings::write_settings(&app, settings);

    history_manager
        .cleanup_old_entries()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn update_recording_retention_period(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    period: String,
) -> Result<(), String> {
    use crate::settings::RecordingRetentionPeriod;

    let retention_period = match period.as_str() {
        "never" => RecordingRetentionPeriod::Never,
        "preserve_limit" => RecordingRetentionPeriod::PreserveLimit,
        "days3" => RecordingRetentionPeriod::Days3,
        "weeks2" => RecordingRetentionPeriod::Weeks2,
        "months3" => RecordingRetentionPeriod::Months3,
        _ => return Err(format!("Invalid retention period: {}", period)),
    };

    let mut settings = crate::settings::get_settings(&app);
    settings.recording_retention_period = retention_period;
    crate::settings::write_settings(&app, settings);

    history_manager
        .cleanup_old_entries()
        .map_err(|e| e.to_string())?;

    Ok(())
}

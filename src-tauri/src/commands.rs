use photo_core::{
    DuplicateGroupDetail, DuplicateGroupSummary, ReviewStatus, ScanConfig, ScanPreset,
    ScanProgress, SessionSummary, TrashResult,
};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use crate::state::AppState;

#[tauri::command]
pub fn list_sessions(state: State<'_, AppState>) -> Result<Vec<SessionSummary>, String> {
    state
        .engine
        .list_sessions()
        .map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionRequest {
    pub root_path: String,
    pub name: Option<String>,
    pub preset: Option<ScanPreset>,
    pub config: Option<ScanConfig>,
}

#[tauri::command]
pub fn create_session(
    state: State<'_, AppState>,
    request: CreateSessionRequest,
) -> Result<String, String> {
    let preset = request.preset.unwrap_or(ScanPreset::VisualSimilar);
    state
        .engine
        .create_session(
            &request.root_path,
            request.name.as_deref(),
            preset,
            request.config,
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_session(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    state
        .engine
        .delete_session(&session_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_scan_progress(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<ScanProgress, String> {
    state
        .engine
        .get_scan_progress(&session_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_scan(
    app: AppHandle,
    session_id: String,
) -> Result<(), String> {
    let app_handle = app.clone();
    let session_for_thread = session_id.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        let engine = Arc::clone(&state.engine);
        engine
            .start_scan(&session_for_thread, {
                let emit_handle = app_handle.clone();
                move |progress| {
                    let _ = emit_handle.emit("scan:progress", progress);
                }
            })
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;

    let _ = app.emit("scan:complete", session_id);
    Ok(())
}

#[tauri::command]
pub fn pause_scan(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    state
        .engine
        .pause_scan(&session_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn resume_scan(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    state
        .engine
        .resume_scan(&session_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn stop_scan(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    state
        .engine
        .stop_scan(&session_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_duplicate_groups(
    state: State<'_, AppState>,
    session_id: String,
    review_status: Option<ReviewStatus>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<DuplicateGroupSummary>, String> {
    state
        .engine
        .list_duplicate_groups(
            &session_id,
            review_status,
            limit.unwrap_or(100),
            offset.unwrap_or(0),
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_group_detail(
    state: State<'_, AppState>,
    group_id: i64,
) -> Result<DuplicateGroupDetail, String> {
    state
        .engine
        .get_group_detail(group_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_keepers(
    state: State<'_, AppState>,
    group_id: i64,
    keeper_file_ids: Vec<i64>,
) -> Result<(), String> {
    state
        .engine
        .set_keepers(group_id, &keeper_file_ids)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn keep_all_in_group(state: State<'_, AppState>, group_id: i64) -> Result<(), String> {
    state
        .engine
        .keep_all_in_group(group_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn move_duplicates_to_trash(
    state: State<'_, AppState>,
    group_id: i64,
) -> Result<TrashResult, String> {
    state
        .engine
        .move_duplicates_to_trash(group_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn keep_selected_and_trash(
    state: State<'_, AppState>,
    group_id: i64,
    keeper_file_ids: Vec<i64>,
) -> Result<TrashResult, String> {
    state
        .engine
        .keep_selected_and_trash(group_id, &keeper_file_ids)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ensure_thumbnail(
    state: State<'_, AppState>,
    source_path: String,
    cache_key: String,
) -> Result<String, String> {
    state
        .engine
        .ensure_thumbnail(&source_path, &cache_key)
        .map_err(|e| e.to_string())
}

use crate::state::AppState;
use photo_core::{
    DuplicateGroupDetail, DuplicateGroupSummary, ReviewStatus, ScanConfig, ScanPreset,
    ScanProgress, SessionSummary, TrashResult,
};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

#[tauri::command]
pub fn list_sessions(state: State<'_, AppState>) -> Result<Vec<SessionSummary>, String> {
    state.engine.list_sessions().map_err(|e| e.to_string())
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
    let jobs = state.jobs.lock().map_err(|e| e.to_string())?;
    if jobs.scan.as_deref() == Some(&session_id)
        || jobs
            .review
            .as_ref()
            .is_some_and(|(id, _)| id == &session_id)
    {
        return Err("Stop the active scan or review before deleting this session".into());
    }
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
pub async fn start_scan(app: AppHandle, session_id: String) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        let mut jobs = state.jobs.lock().map_err(|e| e.to_string())?;
        if jobs.scan.is_some() || jobs.review.is_some() {
            return Err("Another scan or review is already running".into());
        }
        jobs.scan = Some(session_id.clone());
    }
    let app_handle = app.clone();
    let session_for_thread = session_id.clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
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
    .map_err(|e| e.to_string())
    .and_then(|r| r);

    if let Ok(mut jobs) = app.state::<AppState>().jobs.lock() {
        jobs.scan = None;
    }
    result?;

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
    let mut groups = state
        .engine
        .list_duplicate_groups(
            &session_id,
            review_status,
            limit.unwrap_or(100),
            offset.unwrap_or(0),
        )
        .map_err(|e| e.to_string())?;
    let db = scan_engine::db::Database::open().map_err(|e| e.to_string())?;
    let settings = ai_culling::load_settings()?;
    for group in &mut groups {
        if let Some(json) = db.get_ai_result(group.id).map_err(|e| e.to_string())? {
            if let (Ok(mut result), Ok(detail)) = (
                serde_json::from_str::<ai_culling::GroupResult>(&json),
                db.get_group_detail(group.id),
            ) {
                let features = result.features.clone();
                if settings.tier1_enabled {
                    ai_culling::apply_local_analysis(
                        &mut result,
                        &detail,
                        &features,
                        settings.preset,
                    );
                }
                group.ai_bucket = Some(result.bucket);
            }
        }
    }
    Ok(groups)
}

#[tauri::command]
pub fn get_group_detail(
    state: State<'_, AppState>,
    group_id: i64,
) -> Result<DuplicateGroupDetail, String> {
    let mut detail = state
        .engine
        .get_group_detail(group_id)
        .map_err(|e| e.to_string())?;
    let db = scan_engine::db::Database::open().map_err(|e| e.to_string())?;
    let result = db
        .get_ai_result(group_id)
        .map_err(|e| e.to_string())?
        .and_then(|v| serde_json::from_str(&v).ok());
    ai_culling::apply_result(&mut detail, result, &ai_culling::load_settings()?);
    Ok(detail)
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
    let detail = get_group_detail(state.clone(), group_id)?;
    check_trash_safety(
        &detail,
        &detail
            .members
            .iter()
            .filter(|m| m.is_keeper == Some(true))
            .map(|m| m.file_id)
            .collect::<Vec<_>>(),
    )?;
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
    let detail = get_group_detail(state.clone(), group_id)?;
    check_trash_safety(&detail, &keeper_file_ids)?;
    state
        .engine
        .keep_selected_and_trash(group_id, &keeper_file_ids)
        .map_err(|e| e.to_string())
}

fn check_trash_safety(detail: &DuplicateGroupDetail, keepers: &[i64]) -> Result<(), String> {
    if keepers.is_empty()
        || keepers
            .iter()
            .any(|id| !detail.members.iter().any(|m| m.file_id == *id))
    {
        return Err("Select at least one valid photo to keep".into());
    }
    if detail
        .members
        .iter()
        .any(|m| !m.protected.is_empty() && !keepers.contains(&m.file_id))
    {
        return Err("Protected photos must be kept. Review their protection labels first.".into());
    }
    let db = scan_engine::db::Database::open().map_err(|e| e.to_string())?;
    if db
        .get_ai_result(detail.id)
        .map_err(|e| e.to_string())?
        .is_some()
        && detail.ai_bucket.is_none()
    {
        return Err("A photo has changed since the review. Run the review again before moving files to Trash.".into());
    }
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiCapabilities {
    tier1_available: bool,
    reason: Option<String>,
}

#[tauri::command]
pub fn get_ai_capabilities() -> AiCapabilities {
    match scan_engine::native_helper::capabilities() {
        Ok(v) if v.get("vision").and_then(|v| v.as_bool()) == Some(true) => AiCapabilities {
            tier1_available: true,
            reason: None,
        },
        Ok(_) => AiCapabilities {
            tier1_available: false,
            reason: Some("Local Vision analysis is unavailable on this Mac.".into()),
        },
        Err(e) => AiCapabilities {
            tier1_available: false,
            reason: Some(format!("Local analysis unavailable: {e}")),
        },
    }
}

#[tauri::command]
pub fn get_ai_settings() -> Result<ai_culling::Settings, String> {
    ai_culling::load_settings()
}

#[tauri::command]
pub fn set_ai_settings(
    state: State<'_, AppState>,
    settings: ai_culling::Settings,
) -> Result<ai_culling::Settings, String> {
    let jobs = state.jobs.lock().map_err(|e| e.to_string())?;
    if jobs.review.is_some() {
        return Err("Wait for the review to finish before changing settings".into());
    }
    ai_culling::save_settings(&settings)?;
    Ok(settings)
}

#[tauri::command]
pub fn clear_ai_cache(state: State<'_, AppState>) -> Result<(), String> {
    let jobs = state.jobs.lock().map_err(|e| e.to_string())?;
    if jobs.review.is_some() || jobs.scan.is_some() {
        return Err("Wait for the current job to finish before clearing the cache".into());
    }
    scan_engine::db::Database::open()
        .and_then(|db| db.clear_ai_cache())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_ai_review_progress(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<crate::state::AiProgress, String> {
    state
        .jobs
        .lock()
        .map_err(|e| e.to_string())?
        .progress
        .get(&session_id)
        .cloned()
        .ok_or_else(|| "No review has been started for this session".into())
}

#[tauri::command]
pub fn cancel_ai_review(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    let jobs = state.jobs.lock().map_err(|e| e.to_string())?;
    if let Some((id, cancel)) = &jobs.review {
        if *id == session_id {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            return Ok(());
        }
    }
    Err("No review is running for this session".into())
}

#[tauri::command]
pub fn start_ai_review(app: AppHandle, session_id: String) -> Result<(), String> {
    let settings = ai_culling::load_settings()?;
    let db = scan_engine::db::Database::open().map_err(|e| e.to_string())?;
    db.get_session_root(&session_id)
        .map_err(|e| e.to_string())?;
    let total = db.count_groups(&session_id).map_err(|e| e.to_string())?;
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let state = app.state::<AppState>();
        let mut jobs = state.jobs.lock().map_err(|e| e.to_string())?;
        if jobs.scan.is_some() || jobs.review.is_some() {
            return Err("Another scan or review is already running".into());
        }
        jobs.review = Some((session_id.clone(), cancel.clone()));
        jobs.progress.insert(
            session_id.clone(),
            crate::state::AiProgress {
                session_id: session_id.clone(),
                status: "running".into(),
                groups_processed: 0,
                groups_total: total,
                error: None,
            },
        );
    }
    tauri::async_runtime::spawn_blocking(move || {
        let outcome = run_ai_review(&app, &session_id, &settings, &cancel);
        let state = app.state::<AppState>();
        if let Ok(mut jobs) = state.jobs.lock() {
            if let Some(progress) = jobs.progress.get_mut(&session_id) {
                match outcome {
                    Ok(()) if cancel.load(std::sync::atomic::Ordering::Relaxed) => {
                        progress.status = "cancelled".into()
                    }
                    Ok(()) => progress.status = "complete".into(),
                    Err(e) => {
                        progress.status = "failed".into();
                        progress.error = Some(e);
                    }
                }
                let _ = app.emit("ai:complete", progress.clone());
            }
            jobs.review = None;
        };
    });
    Ok(())
}

fn run_ai_review(
    app: &AppHandle,
    session_id: &str,
    settings: &ai_culling::Settings,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    let db = scan_engine::db::Database::open().map_err(|e| e.to_string())?;
    db.clear_session_ai_results(session_id)
        .map_err(|e| e.to_string())?;
    let groups = db
        .list_duplicate_groups(session_id, None, u32::MAX, 0)
        .map_err(|e| e.to_string())?;
    let version = if settings.tier1_enabled {
        scan_engine::native_helper::capabilities()
            .ok()
            .and_then(|v| {
                v.get("extractor_version")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
            })
            .map(|v| {
                let os = std::process::Command::new("sw_vers")
                    .arg("-buildVersion")
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
                    .unwrap_or_else(|| "unknown-os".into());
                format!("{v}:{os}")
            })
    } else {
        None
    };
    for group in groups {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let mut detail = db.get_group_detail(group.id).map_err(|e| e.to_string())?;
        let mut hashes = std::collections::HashMap::new();
        let mut feature_map = std::collections::HashMap::new();
        for member in &mut detail.members {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            if member.exif.is_none() {
                member.exif = scan_engine::exif::read_exif(std::path::Path::new(&member.path));
            }
            if let Ok(hash) = scan_engine::hash::blake3_file(std::path::Path::new(&member.path)) {
                hashes.insert(member.file_id, hash);
            }
        }
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let key = format!(
            "{:?}:{}",
            detail.kind,
            ai_culling::group_key(&hashes.values().cloned().collect::<Vec<_>>())
        );
        let unique = hashes.len() == detail.members.len()
            && hashes
                .values()
                .collect::<std::collections::HashSet<_>>()
                .len()
                == hashes.len();
        let cached = if unique {
            db.get_cached_verdict(&key, "rules-v1")
                .map_err(|e| e.to_string())?
                .and_then(|v| serde_json::from_str::<ai_culling::CachedVerdict>(&v).ok())
        } else {
            None
        };
        let mut result = if let Some(cached) = cached {
            cached.materialize(hashes)
        } else {
            ai_culling::decide(&detail, hashes)
        };
        if unique {
            if let Some(cached) = ai_culling::CachedVerdict::from_result(&result) {
                db.put_cached_verdict(
                    &key,
                    "rules-v1",
                    &serde_json::to_string(&cached).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
            }
        }
        // Rules resolve exact copies and metadata variants without invoking Vision.
        if result.bucket == "needs_you" {
            if let Some(version) = &version {
                for member in &detail.members {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    if let Some(hash) = result.hashes.get(&member.file_id) {
                        if let Some(mut value) = ai_culling::cached_features(
                            &db,
                            std::path::Path::new(&member.path),
                            hash,
                            version,
                            scan_engine::native_helper::analyze,
                        )? {
                            if let Some(obj) = value.as_object_mut() {
                                obj.remove("feature_print");
                            }
                            feature_map.insert(member.file_id, value);
                        }
                    }
                }
            }
        }
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        result.features = feature_map;
        db.set_ai_result(
            group.id,
            &serde_json::to_string(&result).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let state = app.state::<AppState>();
        if let Ok(mut jobs) = state.jobs.lock() {
            if let Some(progress) = jobs.progress.get_mut(session_id) {
                progress.groups_processed += 1;
                let _ = app.emit("ai:progress", progress.clone());
            }
        };
    }
    Ok(())
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

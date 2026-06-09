use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use photo_core::{
    new_session_id, AppResult, DuplicateGroupDetail, DuplicateGroupSummary, ReviewStatus,
    ScanConfig, ScanPreset, ScanProgress, SessionSummary, TrashResult,
};

use crate::db::Database;
use crate::paths;
use crate::scan::{run_scan, ScanCallbacks, ScanController};
use crate::thumbnail;

pub struct ScanEngine {
    db: Mutex<Database>,
    active_scan: Mutex<Option<ActiveScan>>,
    live_progress: Arc<Mutex<HashMap<String, ScanProgress>>>,
}

struct ActiveScan {
    session_id: String,
    controller: ScanController,
}

impl ScanEngine {
    pub fn new() -> AppResult<Self> {
        paths::ensure_app_dirs().map_err(photo_core::AppError::Io)?;
        Ok(Self {
            db: Mutex::new(Database::open()?),
            active_scan: Mutex::new(None),
            live_progress: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn list_sessions(&self) -> AppResult<Vec<SessionSummary>> {
        self.db.lock().list_sessions()
    }

    pub fn create_session(
        &self,
        root_path: &str,
        name: Option<&str>,
        preset: ScanPreset,
        config: Option<ScanConfig>,
    ) -> AppResult<String> {
        let id = new_session_id();
        let session_name = name
            .map(str::to_string)
            .unwrap_or_else(|| Path::new(root_path).file_name().and_then(|n| n.to_str()).unwrap_or("Scan").to_string());
        let mut scan_config = preset.to_config();
        if let Some(custom) = config {
            if preset == ScanPreset::Custom {
                scan_config = custom;
            } else {
                scan_config.include_raw = custom.include_raw;
            }
        }

        self.db
            .lock()
            .create_session(&id, &session_name, root_path, &scan_config)?;
        Ok(id)
    }

    pub fn delete_session(&self, session_id: &str) -> AppResult<()> {
        self.db.lock().delete_session(session_id)
    }

    pub fn get_scan_progress(&self, session_id: &str) -> AppResult<ScanProgress> {
        if let Some(progress) = self.live_progress.lock().get(session_id).cloned() {
            return Ok(progress);
        }
        self.db.lock().get_checkpoint(session_id)
    }

    pub fn list_duplicate_groups(
        &self,
        session_id: &str,
        review_status: Option<ReviewStatus>,
        limit: u32,
        offset: u32,
    ) -> AppResult<Vec<DuplicateGroupSummary>> {
        self.db
            .lock()
            .list_duplicate_groups(session_id, review_status, limit, offset)
    }

    pub fn get_group_detail(&self, group_id: i64) -> AppResult<DuplicateGroupDetail> {
        self.db.lock().get_group_detail(group_id)
    }

    pub fn set_keepers(&self, group_id: i64, keeper_file_ids: &[i64]) -> AppResult<()> {
        self.db.lock().set_keepers(group_id, keeper_file_ids)
    }

    pub fn keep_all_in_group(&self, group_id: i64) -> AppResult<()> {
        self.db.lock().keep_all_in_group(group_id)
    }

    pub fn move_duplicates_to_trash(&self, group_id: i64) -> AppResult<TrashResult> {
        self.db.lock().move_duplicates_to_trash(group_id)
    }

    pub fn keep_selected_and_trash(
        &self,
        group_id: i64,
        keeper_file_ids: &[i64],
    ) -> AppResult<TrashResult> {
        self.db
            .lock()
            .keep_selected_and_trash(group_id, keeper_file_ids)
    }

    pub fn ensure_thumbnail(&self, source_path: &str, cache_key: &str) -> AppResult<String> {
        let path = thumbnail::ensure_thumbnail(Path::new(source_path), cache_key)?;
        Ok(path.to_string_lossy().to_string())
    }

    pub fn start_scan<F>(
        &self,
        session_id: &str,
        on_progress: F,
    ) -> AppResult<()>
    where
        F: Fn(ScanProgress) + Send + Sync + 'static,
    {
        let (root_path, config) = {
            let db = self.db.lock();
            let root = db.get_session_root(session_id)?;
            let config = db.get_session_config(session_id)?;
            (root, config)
        };

        let controller = ScanController::new();
        {
            let mut active = self.active_scan.lock();
            if let Some(existing) = active.as_ref() {
                if existing.session_id == session_id {
                    existing.controller.request_cancel();
                }
            }
            *active = Some(ActiveScan {
                session_id: session_id.to_string(),
                controller: controller.clone(),
            });
        }

        let live_progress = Arc::clone(&self.live_progress);
        let callbacks = ScanCallbacks {
            on_progress: Arc::new(move |progress| {
                live_progress
                    .lock()
                    .insert(progress.session_id.clone(), progress.clone());
                on_progress(progress);
            }),
        };

        let result = run_scan(
            &self.db,
            session_id,
            Path::new(&root_path),
            &config,
            &controller,
            &callbacks,
        );

        self.live_progress.lock().remove(session_id);
        self.active_scan.lock().take();
        result
    }

    pub fn pause_scan(&self, session_id: &str) -> AppResult<()> {
        if let Some(active) = self.active_scan.lock().as_ref() {
            if active.session_id == session_id {
                active.controller.request_pause();
            }
        }
        Ok(())
    }

    pub fn resume_scan(&self, session_id: &str) -> AppResult<()> {
        if let Some(active) = self.active_scan.lock().as_ref() {
            if active.session_id == session_id {
                active.controller.resume();
            }
        }
        Ok(())
    }

    pub fn stop_scan(&self, session_id: &str) -> AppResult<()> {
        let signalled_active = {
            let active = self.active_scan.lock();
            if let Some(existing) = active.as_ref() {
                if existing.session_id == session_id {
                    existing.controller.request_cancel();
                    true
                } else {
                    false
                }
            } else {
                false
            }
        };

        // When a scan is running it already holds the DB lock; the scan thread
        // updates session status when it observes the cancel flag.
        if !signalled_active {
            self.db
                .lock()
                .update_session_status(session_id, photo_core::SessionStatus::Paused)?;
        }
        Ok(())
    }
}

impl ScanController {
    pub fn clone(&self) -> Self {
        Self {
            cancel: Arc::clone(&self.cancel),
            pause: Arc::clone(&self.pause),
        }
    }
}

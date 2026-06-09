use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use jwalk::WalkDir;
use parking_lot::Mutex;
use photo_core::{AppError, AppResult, ScanConfig, ScanPhase, ScanProgress, SessionStatus};
use rayon::prelude::*;

use crate::cluster::{cluster_duplicates, file_record_from_row};
use crate::db::Database;
use crate::exif::exif_to_json;
use crate::hash::{blake3_file, image_dimensions, is_supported_image, perceptual_hashes};
use crate::raw::{plan_scan_entries, ScanEntryPlan};

const BATCH_SIZE: usize = 500;
const CHECKPOINT_INTERVAL: u64 = 1000;
const PROGRESS_INTERVAL_MS: u64 = 2500;

struct ProgressReporter {
    session_id: String,
    files_processed: Arc<AtomicU64>,
    batch_in_progress: Arc<AtomicU64>,
    current_path: Arc<Mutex<Option<String>>>,
    total_estimate: u64,
    start: Instant,
    callbacks: ScanCallbacks,
    stop: Arc<AtomicBool>,
}

impl ProgressReporter {
    fn new(
        session_id: &str,
        total_estimate: u64,
        start: Instant,
        callbacks: ScanCallbacks,
    ) -> Self {
        Self {
            session_id: session_id.to_string(),
            files_processed: Arc::new(AtomicU64::new(0)),
            batch_in_progress: Arc::new(AtomicU64::new(0)),
            current_path: Arc::new(Mutex::new(None)),
            total_estimate,
            start,
            callbacks,
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    fn set_files_processed(&self, count: u64) {
        self.files_processed.store(count, Ordering::Relaxed);
    }

    fn set_current_path(&self, path: Option<String>) {
        *self.current_path.lock() = path;
    }

    fn begin_batch(&self) {
        self.batch_in_progress.store(0, Ordering::Relaxed);
    }

    fn snapshot(&self) -> (u64, Option<String>) {
        let processed = self.files_processed.load(Ordering::Relaxed)
            + self.batch_in_progress.load(Ordering::Relaxed);
        let path = self.current_path.lock().clone();
        (processed, path)
    }

    fn spawn_ticker(self: &Arc<Self>) -> JoinHandle<()> {
        let reporter = Arc::clone(self);
        std::thread::spawn(move || {
            while !reporter.stop.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(PROGRESS_INTERVAL_MS));
                if reporter.stop.load(Ordering::Relaxed) {
                    break;
                }

                let (processed, current_path) = reporter.snapshot();
                let elapsed = reporter.start.elapsed().as_secs_f64().max(0.001);
                (reporter.callbacks.on_progress)(ScanProgress {
                    session_id: reporter.session_id.clone(),
                    phase: ScanPhase::Hashing,
                    files_processed: processed,
                    files_total_estimate: reporter.total_estimate,
                    files_per_sec: processed as f64 / elapsed,
                    current_path,
                    groups_found: 0,
                });
            }
        })
    }

    fn stop_ticker(&self, handle: JoinHandle<()>) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = handle.join();
    }
}

#[derive(Clone)]
pub struct ScanCallbacks {
    pub on_progress: Arc<dyn Fn(ScanProgress) + Send + Sync>,
}

pub struct ScanController {
    pub cancel: Arc<AtomicBool>,
    pub pause: Arc<AtomicBool>,
}

impl ScanController {
    pub fn new() -> Self {
        Self {
            cancel: Arc::new(AtomicBool::new(false)),
            pause: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn request_pause(&self) {
        self.pause.store(true, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.pause.store(false, Ordering::SeqCst);
    }

    fn wait_if_paused(&self) {
        while self.pause.load(Ordering::SeqCst) && !self.cancel.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}

pub fn run_scan(
    db: &Mutex<Database>,
    session_id: &str,
    root_path: &Path,
    config: &ScanConfig,
    controller: &ScanController,
    callbacks: &ScanCallbacks,
) -> AppResult<()> {
    let scan_generation = db.lock().bump_scan_generation(session_id)?;
    db.lock()
        .update_session_status(session_id, SessionStatus::Scanning)?;
    db.lock().clear_groups_for_session(session_id)?;

    let start = Instant::now();
    let mut files_processed = 0u64;

    emit_progress(
        db,
        session_id,
        ScanPhase::Walking,
        files_processed,
        0,
        None,
        0.0,
        0,
        callbacks,
    );

    let all_files: Vec<PathBuf> = WalkDir::new(root_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.path().to_path_buf())
        .filter(|path| {
            if !config.include_raw {
                return is_supported_image(path) && !crate::raw::is_standalone_raw_path(path);
            }
            is_supported_image(path)
                || crate::raw::extension_lower(path)
                    .is_some_and(|ext| crate::raw::is_raw_extension(&ext))
        })
        .collect();

    let ScanEntryPlan {
        entries,
        companion_raw_by_image,
    } = plan_scan_entries(&all_files, config);

    let total_estimate = entries.len() as u64;
    db.lock().upsert_checkpoint(
        session_id,
        ScanPhase::Hashing,
        0,
        total_estimate,
        None,
    )?;

    emit_progress(
        db,
        session_id,
        ScanPhase::Hashing,
        0,
        total_estimate,
        None,
        0.0,
        0,
        callbacks,
    );

    let reporter = Arc::new(ProgressReporter::new(
        session_id,
        total_estimate,
        start,
        callbacks.clone(),
    ));
    let ticker = reporter.spawn_ticker();

    let mut batch = Vec::with_capacity(BATCH_SIZE);
    let mut last_path: Option<String> = None;

    for path in entries {
        if controller.cancel.load(Ordering::SeqCst) {
            db.lock()
                .update_session_status(session_id, SessionStatus::Paused)?;
            db.lock().upsert_checkpoint(
                session_id,
                ScanPhase::Hashing,
                files_processed,
                total_estimate,
                path.to_str(),
            )?;
            return Ok(());
        }

        controller.wait_if_paused();
        if controller.cancel.load(Ordering::SeqCst) {
            db.lock()
                .update_session_status(session_id, SessionStatus::Paused)?;
            return Ok(());
        }

        let path_str = path.to_string_lossy().to_string();
        let metadata = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let size = metadata.len();
        let mtime = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0);

        if let Some((_, existing_size, existing_mtime)) = db
            .lock()
            .get_existing_file_meta(session_id, &path_str)?
        {
            if existing_size == size && existing_mtime == mtime {
                files_processed += 1;
                last_path = Some(path_str.clone());
                reporter.set_files_processed(files_processed);
                reporter.set_current_path(last_path.clone());
                continue;
            }
        }

        batch.push(path);
        last_path = Some(path_str.clone());
        reporter.set_current_path(last_path.clone());

        if batch.len() >= BATCH_SIZE {
            files_processed += process_batch(
                db,
                session_id,
                scan_generation,
                &batch,
                config,
                &companion_raw_by_image,
                &reporter,
            )?;
            batch.clear();
            reporter.set_files_processed(files_processed);

            if files_processed % CHECKPOINT_INTERVAL == 0 {
                db.lock().upsert_checkpoint(
                    session_id,
                    ScanPhase::Hashing,
                    files_processed,
                    total_estimate,
                    last_path.as_deref(),
                )?;
            }
        }
    }

    if !batch.is_empty() {
        files_processed += process_batch(
            db,
            session_id,
            scan_generation,
            &batch,
            config,
            &companion_raw_by_image,
            &reporter,
        )?;
        reporter.set_files_processed(files_processed);
    }

    reporter.stop_ticker(ticker);

    db.lock()
        .mark_missing_deleted(session_id, scan_generation)?;

    emit_progress(
        db,
        session_id,
        ScanPhase::Clustering,
        files_processed,
        total_estimate,
        None,
        files_processed as f64 / start.elapsed().as_secs_f64().max(0.001),
        0,
        callbacks,
    );

    let rows = db.lock().files_for_clustering(session_id)?;
    let records: Vec<_> = rows
        .into_iter()
        .map(
            |(id, path, size, width, height, blake3, dhash, phash, exif_json)| {
                file_record_from_row(id, path, size, width, height, blake3, dhash, phash, exif_json)
            },
        )
        .collect();

    let clusters = cluster_duplicates(records, config);
    for cluster in clusters {
        db.lock().insert_duplicate_group(
            session_id,
            cluster.kind,
            cluster.confidence,
            &cluster.file_ids,
        )?;
    }

    let groups_found = db.lock().count_groups(session_id)?;
    db.lock().upsert_checkpoint(
        session_id,
        ScanPhase::Complete,
        files_processed,
        total_estimate,
        None,
    )?;
    db.lock()
        .update_session_status(session_id, SessionStatus::Reviewing)?;

    emit_progress(
        db,
        session_id,
        ScanPhase::Complete,
        files_processed,
        total_estimate,
        None,
        files_processed as f64 / start.elapsed().as_secs_f64().max(0.001),
        groups_found,
        callbacks,
    );

    Ok(())
}

fn process_batch(
    db: &Mutex<Database>,
    session_id: &str,
    scan_generation: i64,
    paths: &[PathBuf],
    config: &ScanConfig,
    companion_raw_by_image: &std::collections::HashMap<PathBuf, PathBuf>,
    reporter: &ProgressReporter,
) -> AppResult<u64> {
    reporter.begin_batch();
    let batch_in_progress = Arc::clone(&reporter.batch_in_progress);
    let current_path = Arc::clone(&reporter.current_path);

    let results: Vec<_> = paths
        .par_iter()
        .filter_map(|path| {
            let result = process_file(path, config).ok()?;
            batch_in_progress.fetch_add(1, Ordering::Relaxed);
            *current_path.lock() = Some(result.path.clone());
            Some(result)
        })
        .collect();

    let db_guard = db.lock();
    let tx = db_guard.begin_transaction()?;
    let mut count = 0u64;

    for result in results {
        let companion_raw_path = companion_raw_by_image.get(Path::new(&result.path));
        let companion_raw = companion_raw_path.map(|p| p.to_string_lossy().to_string());
        let companion_raw_size = companion_raw_path.and_then(|p| {
            std::fs::metadata(p)
                .ok()
                .map(|metadata| metadata.len())
        });

        let file_id = db_guard.upsert_file(
            &tx,
            session_id,
            &result.path,
            result.size,
            result.mtime_ns,
            result.width,
            result.height,
            result.inode,
            scan_generation,
            result.created_at.as_deref(),
            result.modified_at.as_deref(),
            companion_raw.as_deref(),
            companion_raw_size,
        )?;

        db_guard.upsert_fingerprint(
            &tx,
            file_id,
            result.blake3.as_deref(),
            result.dhash,
            result.phash,
            result.exif_json.as_deref(),
        )?;

        count += 1;
    }

    tx.commit().map_err(|e| AppError::Database(e.to_string()))?;
    Ok(count)
}

struct ProcessedFile {
    path: String,
    size: u64,
    mtime_ns: i64,
    width: Option<u32>,
    height: Option<u32>,
    inode: Option<u64>,
    created_at: Option<String>,
    modified_at: Option<String>,
    blake3: Option<String>,
    dhash: Option<u64>,
    phash: Option<u64>,
    exif_json: Option<String>,
}

fn process_file(path: &Path, config: &ScanConfig) -> AppResult<ProcessedFile> {
    let metadata = std::fs::metadata(path).map_err(AppError::Io)?;
    let size = metadata.len();
    let mtime_ns = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0);

    #[cfg(unix)]
    let inode = {
        use std::os::unix::fs::MetadataExt;
        Some(metadata.ino())
    };
    #[cfg(not(unix))]
    let inode = None;

    let created_at = metadata.created().ok().map(format_system_time);
    let modified_at = metadata.modified().ok().map(format_system_time);

    let (width, height) = image_dimensions(path).unwrap_or((None, None));

    let blake3 = if config.exact_hash {
        Some(blake3_file(path)?)
    } else {
        None
    };

    let (dhash, phash) = if config.visual_similar {
        let (d, p) = perceptual_hashes(path)?;
        (Some(d), Some(p))
    } else {
        (None, None)
    };

    let exif_json = if config.burst_detection {
        exif_to_json(path)
    } else {
        None
    };

    Ok(ProcessedFile {
        path: path.to_string_lossy().to_string(),
        size,
        mtime_ns,
        width,
        height,
        inode,
        created_at,
        modified_at,
        blake3,
        dhash,
        phash,
        exif_json,
    })
}

fn format_system_time(time: std::time::SystemTime) -> String {
    let datetime: DateTime<Utc> = time.into();
    datetime.to_rfc3339()
}

fn emit_progress(
    db: &Mutex<Database>,
    session_id: &str,
    phase: ScanPhase,
    files_processed: u64,
    files_total_estimate: u64,
    current_path: Option<&str>,
    files_per_sec: f64,
    groups_found: u64,
    callbacks: &ScanCallbacks,
) {
    let _ = db.lock().upsert_checkpoint(
        session_id,
        phase,
        files_processed,
        files_total_estimate,
        current_path,
    );

    (callbacks.on_progress)(ScanProgress {
        session_id: session_id.to_string(),
        phase,
        files_processed,
        files_total_estimate,
        files_per_sec,
        current_path: current_path.map(str::to_string),
        groups_found,
    });
}

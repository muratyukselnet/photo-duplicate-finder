use std::path::Path;

use chrono::{DateTime, Utc};
use photo_core::{
    AppError, AppResult, DuplicateGroupDetail, DuplicateGroupSummary, DuplicateKind, ExifData,
    FileMember, ReviewStatus, ScanConfig, ScanPhase, ScanProgress, SessionStatus, SessionSummary,
    TrashResult,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde_json;

use crate::paths;

const SCHEMA: &str = include_str!("schema.sql");
const SCHEMA_VERSION: i32 = 2;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open() -> AppResult<Self> {
        paths::ensure_app_dirs().map_err(AppError::Io)?;
        let db_path = paths::database_path();
        let conn = Connection::open(db_path).map_err(|e| AppError::Database(e.to_string()))?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_in_memory() -> AppResult<Self> {
        let conn = Connection::open_in_memory().map_err(|e| AppError::Database(e.to_string()))?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> AppResult<()> {
        self.conn
            .execute_batch(SCHEMA)
            .map_err(|e| AppError::Database(e.to_string()))?;

        let version: Option<i32> = self
            .conn
            .query_row(
                "SELECT version FROM schema_version LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| AppError::Database(e.to_string()))?;

        let current = version.unwrap_or(0);
        if current == 0 {
            self.conn
                .execute(
                    "INSERT INTO schema_version (version) VALUES (?1)",
                    params![SCHEMA_VERSION],
                )
                .map_err(|e| AppError::Database(e.to_string()))?;
        } else if current < 2 {
            self.conn
                .execute(
                    "ALTER TABLE files ADD COLUMN companion_raw_path TEXT",
                    [],
                )
                .map_err(|e| AppError::Database(e.to_string()))?;
            self.conn
                .execute(
                    "ALTER TABLE files ADD COLUMN companion_raw_size INTEGER",
                    [],
                )
                .map_err(|e| AppError::Database(e.to_string()))?;
            self.conn
                .execute(
                    "UPDATE schema_version SET version = ?1",
                    params![SCHEMA_VERSION],
                )
                .map_err(|e| AppError::Database(e.to_string()))?;
        }

        Ok(())
    }

    pub fn create_session(
        &self,
        id: &str,
        name: &str,
        root_path: &str,
        config: &ScanConfig,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let config_json =
            serde_json::to_string(config).map_err(|e| AppError::Database(e.to_string()))?;

        self.conn
            .execute(
                "INSERT INTO sessions (id, name, root_path, status, config_json, scan_generation, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7)",
                params![
                    id,
                    name,
                    root_path,
                    status_to_str(SessionStatus::Scanning),
                    config_json,
                    now,
                    now
                ],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;

        self.upsert_checkpoint(id, ScanPhase::Walking, 0, 0, None)?;
        Ok(())
    }

    pub fn list_sessions(&self) -> AppResult<Vec<SessionSummary>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT s.id, s.name, s.root_path, s.status, s.created_at, s.updated_at,
                        (SELECT COUNT(*) FROM files f WHERE f.session_id = s.id AND f.deleted_at IS NULL) as files_scanned,
                        (SELECT COUNT(*) FROM duplicate_groups g WHERE g.session_id = s.id AND g.review_status = 'pending') as groups_pending,
                        (SELECT COUNT(*) FROM duplicate_groups g WHERE g.session_id = s.id) as groups_total
                 FROM sessions s
                 ORDER BY s.updated_at DESC",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(SessionSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    root_path: row.get(2)?,
                    status: str_to_status(&row.get::<_, String>(3)?),
                    created_at: parse_dt(&row.get::<_, String>(4)?),
                    updated_at: parse_dt(&row.get::<_, String>(5)?),
                    files_scanned: row.get::<_, i64>(6)? as u64,
                    groups_pending: row.get::<_, i64>(7)? as u64,
                    groups_total: row.get::<_, i64>(8)? as u64,
                })
            })
            .map_err(|e| AppError::Database(e.to_string()))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Database(e.to_string()))
    }

    pub fn get_session_config(&self, session_id: &str) -> AppResult<ScanConfig> {
        let config_json: String = self
            .conn
            .query_row(
                "SELECT config_json FROM sessions WHERE id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .map_err(|_| AppError::SessionNotFound(session_id.to_string()))?;

        serde_json::from_str(&config_json).map_err(|e| AppError::Config(e.to_string()))
    }

    pub fn get_session_root(&self, session_id: &str) -> AppResult<String> {
        self.conn
            .query_row(
                "SELECT root_path FROM sessions WHERE id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .map_err(|_| AppError::SessionNotFound(session_id.to_string()))
    }

    pub fn update_session_status(&self, session_id: &str, status: SessionStatus) -> AppResult<()> {
        self.conn
            .execute(
                "UPDATE sessions SET status = ?1, updated_at = ?2 WHERE id = ?3",
                params![status_to_str(status), Utc::now().to_rfc3339(), session_id],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn delete_session(&self, session_id: &str) -> AppResult<()> {
        self.conn
            .execute("DELETE FROM sessions WHERE id = ?1", params![session_id])
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn bump_scan_generation(&self, session_id: &str) -> AppResult<i64> {
        self.conn
            .execute(
                "UPDATE sessions SET scan_generation = scan_generation + 1, updated_at = ?1 WHERE id = ?2",
                params![Utc::now().to_rfc3339(), session_id],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;

        let gen: i64 = self
            .conn
            .query_row(
                "SELECT scan_generation FROM sessions WHERE id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(gen)
    }

    pub fn get_existing_file_meta(
        &self,
        session_id: &str,
        path: &str,
    ) -> AppResult<Option<(i64, u64, i64)>> {
        self.conn
            .query_row(
                "SELECT id, size, mtime_ns FROM files WHERE session_id = ?1 AND path = ?2 AND deleted_at IS NULL",
                params![session_id, path],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|e| AppError::Database(e.to_string()))
    }

    pub fn upsert_file(
        &self,
        tx: &Transaction<'_>,
        session_id: &str,
        path: &str,
        size: u64,
        mtime_ns: i64,
        width: Option<u32>,
        height: Option<u32>,
        inode: Option<u64>,
        scan_generation: i64,
        created_at: Option<&str>,
        modified_at: Option<&str>,
        companion_raw_path: Option<&str>,
        companion_raw_size: Option<u64>,
    ) -> AppResult<i64> {
        tx.execute(
            "INSERT INTO files (session_id, path, size, mtime_ns, width, height, inode, scan_generation, created_at, modified_at, companion_raw_path, companion_raw_size)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(session_id, path) DO UPDATE SET
                size = excluded.size,
                mtime_ns = excluded.mtime_ns,
                width = excluded.width,
                height = excluded.height,
                inode = excluded.inode,
                scan_generation = excluded.scan_generation,
                created_at = excluded.created_at,
                modified_at = excluded.modified_at,
                companion_raw_path = excluded.companion_raw_path,
                companion_raw_size = excluded.companion_raw_size,
                deleted_at = NULL",
            params![
                session_id,
                path,
                size as i64,
                mtime_ns,
                width,
                height,
                inode.map(|v| v as i64),
                scan_generation,
                created_at,
                modified_at,
                companion_raw_path,
                companion_raw_size.map(|v| v as i64)
            ],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        let file_id: i64 = tx
            .query_row(
                "SELECT id FROM files WHERE session_id = ?1 AND path = ?2",
                params![session_id, path],
                |row| row.get(0),
            )
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(file_id)
    }

    pub fn upsert_fingerprint(
        &self,
        tx: &Transaction<'_>,
        file_id: i64,
        blake3: Option<&str>,
        dhash: Option<u64>,
        phash: Option<u64>,
        exif_json: Option<&str>,
    ) -> AppResult<()> {
        tx.execute(
            "INSERT INTO fingerprints (file_id, blake3, dhash, phash, exif_json)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(file_id) DO UPDATE SET
                blake3 = excluded.blake3,
                dhash = excluded.dhash,
                phash = excluded.phash,
                exif_json = excluded.exif_json",
            params![
                file_id,
                blake3,
                dhash.map(|v| v as i64),
                phash.map(|v| v as i64),
                exif_json
            ],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn begin_transaction(&self) -> AppResult<Transaction<'_>> {
        self.conn
            .unchecked_transaction()
            .map_err(|e| AppError::Database(e.to_string()))
    }

    pub fn mark_missing_deleted(&self, session_id: &str, scan_generation: i64) -> AppResult<u64> {
        let updated = self
            .conn
            .execute(
                "UPDATE files SET deleted_at = ?1
                 WHERE session_id = ?2 AND scan_generation < ?3 AND deleted_at IS NULL",
                params![Utc::now().to_rfc3339(), session_id, scan_generation],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(updated as u64)
    }

    pub fn clear_groups_for_session(&self, session_id: &str) -> AppResult<()> {
        self.conn
            .execute(
                "DELETE FROM duplicate_groups WHERE session_id = ?1",
                params![session_id],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn insert_duplicate_group(
        &self,
        session_id: &str,
        kind: DuplicateKind,
        confidence: f32,
        file_ids: &[i64],
    ) -> AppResult<i64> {
        let tx = self.begin_transaction()?;
        tx.execute(
            "INSERT INTO duplicate_groups (session_id, kind, confidence, review_status)
             VALUES (?1, ?2, ?3, 'pending')",
            params![session_id, kind_to_str(kind), confidence],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        let group_id = tx.last_insert_rowid();
        for file_id in file_ids {
            tx.execute(
                "INSERT OR IGNORE INTO duplicate_members (group_id, file_id) VALUES (?1, ?2)",
                params![group_id, file_id],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        }

        tx.commit().map_err(|e| AppError::Database(e.to_string()))?;
        Ok(group_id)
    }

    pub fn list_duplicate_groups(
        &self,
        session_id: &str,
        review_status: Option<ReviewStatus>,
        limit: u32,
        offset: u32,
    ) -> AppResult<Vec<DuplicateGroupSummary>> {
        let status_filter = review_status.map(|s| review_status_to_str(s).to_string());

        let sql = if status_filter.is_some() {
            "SELECT g.id, g.kind, g.confidence, g.review_status,
                    (SELECT COUNT(*) FROM duplicate_members m WHERE m.group_id = g.id) as member_count
             FROM duplicate_groups g
             WHERE g.session_id = ?1 AND g.review_status = ?2
             ORDER BY g.confidence DESC, g.id ASC
             LIMIT ?3 OFFSET ?4"
        } else {
            "SELECT g.id, g.kind, g.confidence, g.review_status,
                    (SELECT COUNT(*) FROM duplicate_members m WHERE m.group_id = g.id) as member_count
             FROM duplicate_groups g
             WHERE g.session_id = ?1
             ORDER BY g.confidence DESC, g.id ASC
             LIMIT ?2 OFFSET ?3"
        };

        if let Some(status) = status_filter {
            let mut stmt = self
                .conn
                .prepare(sql)
                .map_err(|e| AppError::Database(e.to_string()))?;
            let rows = stmt
                .query_map(params![session_id, status, limit, offset], map_group_summary)
                .map_err(|e| AppError::Database(e.to_string()))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| AppError::Database(e.to_string()))?;
            let mut summaries = rows;
            for summary in &mut summaries {
                summary.bytes_recoverable = self.group_bytes_recoverable(summary.id)?;
            }
            return Ok(summaries);
        }

        let mut stmt = self
            .conn
            .prepare(sql)
            .map_err(|e| AppError::Database(e.to_string()))?;
        let mut summaries = stmt
            .query_map(params![session_id, limit, offset], map_group_summary)
            .map_err(|e| AppError::Database(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Database(e.to_string()))?;

        for summary in &mut summaries {
            summary.bytes_recoverable = self.group_bytes_recoverable(summary.id)?;
        }

        Ok(summaries)
    }

    fn group_bytes_recoverable(&self, group_id: i64) -> AppResult<u64> {
        let bytes: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(SUM(f.size + COALESCE(f.companion_raw_size, 0)), 0)
                 FROM duplicate_members m
                 JOIN files f ON f.id = m.file_id
                 WHERE m.group_id = ?1 AND (m.is_keeper IS NULL OR m.is_keeper = 0)",
                params![group_id],
                |row| row.get(0),
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(bytes.max(0) as u64)
    }

    pub fn get_group_detail(&self, group_id: i64) -> AppResult<DuplicateGroupDetail> {
        let (session_id, kind, confidence, review_status): (String, String, f32, String) = self
            .conn
            .query_row(
                "SELECT session_id, kind, confidence, review_status FROM duplicate_groups WHERE id = ?1",
                params![group_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(|_| AppError::GroupNotFound(group_id))?;

        let _ = session_id;

        let mut stmt = self
            .conn
            .prepare(
                "SELECT f.id, f.path, f.size, f.width, f.height, f.created_at, f.modified_at,
                        fp.exif_json, m.is_keeper, f.companion_raw_path, f.companion_raw_size
                 FROM duplicate_members m
                 JOIN files f ON f.id = m.file_id
                 LEFT JOIN fingerprints fp ON fp.file_id = f.id
                 WHERE m.group_id = ?1 AND f.deleted_at IS NULL
                 ORDER BY f.size DESC",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;

        let members = stmt
            .query_map(params![group_id], |row| {
                let path: String = row.get(1)?;
                let file_name = Path::new(&path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&path)
                    .to_string();
                let exif_json: Option<String> = row.get(7)?;
                let exif = exif_json
                    .as_deref()
                    .and_then(|j| serde_json::from_str::<ExifData>(j).ok());
                let is_keeper: Option<i64> = row.get(8)?;
                let companion_raw_path: Option<String> = row.get(9)?;
                let companion_raw_size: Option<i64> = row.get(10)?;
                let thumbnail_key = thumbnail_key_for(&path, row.get::<_, i64>(2)? as u64);

                Ok(FileMember {
                    file_id: row.get(0)?,
                    path,
                    file_name,
                    size: row.get::<_, i64>(2)? as u64,
                    width: row.get(3)?,
                    height: row.get(4)?,
                    created_at: row
                        .get::<_, Option<String>>(5)?
                        .map(|s| parse_dt(&s)),
                    modified_at: row
                        .get::<_, Option<String>>(6)?
                        .map(|s| parse_dt(&s)),
                    exif,
                    is_keeper: is_keeper.map(|v| v != 0),
                    thumbnail_key,
                    companion_raw_path,
                    companion_raw_size: companion_raw_size.map(|v| v.max(0) as u64),
                })
            })
            .map_err(|e| AppError::Database(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Database(e.to_string()))?;

        let bytes_recoverable = self.group_bytes_recoverable(group_id)?;

        Ok(DuplicateGroupDetail {
            id: group_id,
            kind: str_to_kind(&kind),
            confidence,
            review_status: str_to_review_status(&review_status),
            members,
            bytes_recoverable,
        })
    }

    pub fn set_keepers(&self, group_id: i64, keeper_file_ids: &[i64]) -> AppResult<()> {
        let tx = self.begin_transaction()?;

        tx.execute(
            "UPDATE duplicate_members SET is_keeper = 0 WHERE group_id = ?1",
            params![group_id],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        for file_id in keeper_file_ids {
            tx.execute(
                "UPDATE duplicate_members SET is_keeper = 1 WHERE group_id = ?1 AND file_id = ?2",
                params![group_id, file_id],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        }

        tx.commit().map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn keep_all_in_group(&self, group_id: i64) -> AppResult<()> {
        self.conn
            .execute(
                "UPDATE duplicate_members SET is_keeper = 1 WHERE group_id = ?1",
                params![group_id],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        self.resolve_group(group_id)?;
        Ok(())
    }

    pub fn keep_selected_and_trash(
        &self,
        group_id: i64,
        keeper_file_ids: &[i64],
    ) -> AppResult<TrashResult> {
        self.set_keepers(group_id, keeper_file_ids)?;
        self.move_duplicates_to_trash(group_id)
    }

    pub fn resolve_group(&self, group_id: i64) -> AppResult<()> {
        self.conn
            .execute(
                "UPDATE duplicate_groups SET review_status = 'resolved' WHERE id = ?1",
                params![group_id],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn move_duplicates_to_trash(&self, group_id: i64) -> AppResult<TrashResult> {
        let detail = self.get_group_detail(group_id)?;
        let mut trashed_count = 0u32;
        let mut bytes_freed = 0u64;
        let mut errors = Vec::new();

        let keepers: Vec<i64> = detail
            .members
            .iter()
            .filter(|m| m.is_keeper == Some(true))
            .map(|m| m.file_id)
            .collect();

        if keepers.is_empty() {
            return Err(AppError::Other(
                "Select at least one photo to keep before moving duplicates to Trash".into(),
            ));
        }

        for member in &detail.members {
            if member.is_keeper == Some(true) {
                continue;
            }

            match trash::delete(&member.path) {
                Ok(()) => {
                    trashed_count += 1;
                    bytes_freed += member.size;
                    let _ = self.conn.execute(
                        "UPDATE files SET deleted_at = ?1 WHERE id = ?2",
                        params![Utc::now().to_rfc3339(), member.file_id],
                    );

                    if let Some(ref raw_path) = member.companion_raw_path {
                        match trash::delete(raw_path) {
                            Ok(()) => {
                                trashed_count += 1;
                                bytes_freed += companion_raw_bytes(&member);
                            }
                            Err(e) => errors.push(format!("{raw_path}: {e}")),
                        }
                    }
                }
                Err(e) => errors.push(format!("{}: {e}", member.path)),
            }
        }

        if trashed_count > 0 {
            self.resolve_group(group_id)?;
        }

        Ok(TrashResult {
            trashed_count,
            bytes_freed,
            errors,
        })
    }

    pub fn upsert_checkpoint(
        &self,
        session_id: &str,
        phase: ScanPhase,
        files_processed: u64,
        files_total_estimate: u64,
        last_path: Option<&str>,
    ) -> AppResult<()> {
        self.conn
            .execute(
                "INSERT INTO scan_checkpoints (session_id, phase, files_processed, files_total_estimate, last_path, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(session_id) DO UPDATE SET
                    phase = excluded.phase,
                    files_processed = excluded.files_processed,
                    files_total_estimate = excluded.files_total_estimate,
                    last_path = excluded.last_path,
                    updated_at = excluded.updated_at",
                params![
                    session_id,
                    phase_to_str(phase),
                    files_processed as i64,
                    files_total_estimate as i64,
                    last_path,
                    Utc::now().to_rfc3339()
                ],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn get_checkpoint(&self, session_id: &str) -> AppResult<ScanProgress> {
        let (phase, processed, total, last_path): (String, i64, i64, Option<String>) = self
            .conn
            .query_row(
                "SELECT phase, files_processed, files_total_estimate, last_path
                 FROM scan_checkpoints WHERE session_id = ?1",
                params![session_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(|_| AppError::SessionNotFound(session_id.to_string()))?;

        let groups_found: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM duplicate_groups WHERE session_id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok(ScanProgress {
            session_id: session_id.to_string(),
            phase: str_to_phase(&phase),
            files_processed: processed.max(0) as u64,
            files_total_estimate: total.max(0) as u64,
            files_per_sec: 0.0,
            current_path: last_path,
            groups_found: groups_found.max(0) as u64,
        })
    }

    pub fn files_for_clustering(
        &self,
        session_id: &str,
    ) -> AppResult<Vec<(i64, String, u64, Option<u32>, Option<u32>, Option<String>, Option<u64>, Option<u64>, Option<String>)>>
    {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT f.id, f.path, f.size, f.width, f.height, fp.blake3, fp.dhash, fp.phash, fp.exif_json
                 FROM files f
                 JOIN fingerprints fp ON fp.file_id = f.id
                 WHERE f.session_id = ?1 AND f.deleted_at IS NULL",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;

        let rows = stmt
            .query_map(params![session_id], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get::<_, i64>(2)? as u64,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get::<_, Option<i64>>(6)?.map(|v| v as u64),
                    row.get::<_, Option<i64>>(7)?.map(|v| v as u64),
                    row.get(8)?,
                ))
            })
            .map_err(|e| AppError::Database(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(rows)
    }

    pub fn count_groups(&self, session_id: &str) -> AppResult<u64> {
        let count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM duplicate_groups WHERE session_id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(count.max(0) as u64)
    }
}

fn map_group_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<DuplicateGroupSummary> {
    Ok(DuplicateGroupSummary {
        id: row.get(0)?,
        kind: str_to_kind(&row.get::<_, String>(1)?),
        confidence: row.get(2)?,
        review_status: str_to_review_status(&row.get::<_, String>(3)?),
        member_count: row.get::<_, i64>(4)? as u32,
        bytes_recoverable: 0,
    })
}

fn companion_raw_bytes(member: &FileMember) -> u64 {
    member.companion_raw_size.unwrap_or_else(|| {
        member
            .companion_raw_path
            .as_ref()
            .and_then(|path| std::fs::metadata(path).ok())
            .map(|metadata| metadata.len())
            .unwrap_or(0)
    })
}

pub fn thumbnail_key_for(path: &str, size: u64) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(path.as_bytes());
    hasher.update(size.to_le_bytes());
    hex::encode(hasher.finalize())
}

fn parse_dt(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn status_to_str(status: SessionStatus) -> &'static str {
    match status {
        SessionStatus::Scanning => "scanning",
        SessionStatus::Reviewing => "reviewing",
        SessionStatus::Paused => "paused",
        SessionStatus::Completed => "completed",
    }
}

fn str_to_status(s: &str) -> SessionStatus {
    match s {
        "reviewing" => SessionStatus::Reviewing,
        "paused" => SessionStatus::Paused,
        "completed" => SessionStatus::Completed,
        _ => SessionStatus::Scanning,
    }
}

fn phase_to_str(phase: ScanPhase) -> &'static str {
    match phase {
        ScanPhase::Walking => "walking",
        ScanPhase::Hashing => "hashing",
        ScanPhase::Clustering => "clustering",
        ScanPhase::Complete => "complete",
    }
}

fn str_to_phase(s: &str) -> ScanPhase {
    match s {
        "hashing" => ScanPhase::Hashing,
        "clustering" => ScanPhase::Clustering,
        "complete" => ScanPhase::Complete,
        _ => ScanPhase::Walking,
    }
}

fn kind_to_str(kind: DuplicateKind) -> &'static str {
    match kind {
        DuplicateKind::Exact => "exact",
        DuplicateKind::Visual => "visual",
        DuplicateKind::Burst => "burst",
        DuplicateKind::Metadata => "metadata",
    }
}

fn str_to_kind(s: &str) -> DuplicateKind {
    match s {
        "visual" => DuplicateKind::Visual,
        "burst" => DuplicateKind::Burst,
        "metadata" => DuplicateKind::Metadata,
        _ => DuplicateKind::Exact,
    }
}

fn review_status_to_str(status: ReviewStatus) -> &'static str {
    match status {
        ReviewStatus::Pending => "pending",
        ReviewStatus::Resolved => "resolved",
    }
}

fn str_to_review_status(s: &str) -> ReviewStatus {
    if s == "resolved" {
        ReviewStatus::Resolved
    } else {
        ReviewStatus::Pending
    }
}

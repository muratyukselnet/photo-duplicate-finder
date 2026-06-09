use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Scanning,
    Reviewing,
    Paused,
    Completed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScanPhase {
    Walking,
    Hashing,
    Clustering,
    Complete,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DuplicateKind {
    Exact,
    Visual,
    Burst,
    Metadata,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Pending,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub status: SessionStatus,
    pub files_scanned: u64,
    pub groups_pending: u64,
    pub groups_total: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub session_id: String,
    pub phase: ScanPhase,
    pub files_processed: u64,
    pub files_total_estimate: u64,
    pub files_per_sec: f64,
    pub current_path: Option<String>,
    pub groups_found: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExifData {
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub iso: Option<u32>,
    pub aperture: Option<String>,
    pub shutter_speed: Option<String>,
    pub focal_length: Option<String>,
    pub date_taken: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMember {
    pub file_id: i64,
    pub path: String,
    pub file_name: String,
    pub size: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub created_at: Option<DateTime<Utc>>,
    pub modified_at: Option<DateTime<Utc>>,
    pub exif: Option<ExifData>,
    pub is_keeper: Option<bool>,
    pub thumbnail_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub companion_raw_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub companion_raw_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroupSummary {
    pub id: i64,
    pub kind: DuplicateKind,
    pub confidence: f32,
    pub member_count: u32,
    pub review_status: ReviewStatus,
    pub bytes_recoverable: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroupDetail {
    pub id: i64,
    pub kind: DuplicateKind,
    pub confidence: f32,
    pub review_status: ReviewStatus,
    pub members: Vec<FileMember>,
    pub bytes_recoverable: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashResult {
    pub trashed_count: u32,
    pub bytes_freed: u64,
    pub errors: Vec<String>,
}

pub fn new_session_id() -> String {
    Uuid::new_v4().to_string()
}

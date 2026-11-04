use scan_engine::ScanEngine;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::{atomic::AtomicBool, Mutex};

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProgress {
    pub session_id: String,
    pub status: String,
    pub groups_processed: u64,
    pub groups_total: u64,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct Jobs {
    pub scan: Option<String>,
    pub review: Option<(String, Arc<AtomicBool>)>,
    pub progress: HashMap<String, AiProgress>,
}

pub struct AppState {
    pub engine: Arc<ScanEngine>,
    pub jobs: Mutex<Jobs>,
}

impl AppState {
    pub fn new() -> Result<Self, String> {
        ScanEngine::new()
            .map(|engine| Self {
                engine: Arc::new(engine),
                jobs: Mutex::new(Jobs::default()),
            })
            .map_err(|e| e.to_string())
    }
}

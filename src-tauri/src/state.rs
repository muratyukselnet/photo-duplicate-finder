use scan_engine::ScanEngine;
use std::sync::Arc;

pub struct AppState {
    pub engine: Arc<ScanEngine>,
}

impl AppState {
    pub fn new() -> Result<Self, String> {
        ScanEngine::new()
            .map(|engine| Self {
                engine: Arc::new(engine),
            })
            .map_err(|e| e.to_string())
    }
}

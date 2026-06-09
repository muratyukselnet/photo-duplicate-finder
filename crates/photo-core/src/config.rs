use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScanConfig {
    pub exact_hash: bool,
    pub visual_similar: bool,
    pub burst_detection: bool,
    pub filename_ranking: bool,
    pub phash_threshold: u8,
    pub burst_window_secs: u32,
    #[serde(default)]
    pub include_raw: bool,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            exact_hash: true,
            visual_similar: true,
            burst_detection: true,
            filename_ranking: true,
            phash_threshold: 6,
            burst_window_secs: 2,
            include_raw: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScanPreset {
    #[default]
    VisualSimilar,
    ExactOnly,
    BurstTimeWindow,
    Custom,
}

impl ScanPreset {
    pub fn to_config(self) -> ScanConfig {
        match self {
            Self::ExactOnly => ScanConfig {
                exact_hash: true,
                visual_similar: false,
                burst_detection: false,
                filename_ranking: false,
                ..ScanConfig::default()
            },
            Self::VisualSimilar => ScanConfig::default(),
            Self::BurstTimeWindow => ScanConfig {
                exact_hash: true,
                visual_similar: true,
                burst_detection: true,
                filename_ranking: true,
                burst_window_secs: 3,
                ..ScanConfig::default()
            },
            Self::Custom => ScanConfig::default(),
        }
    }
}

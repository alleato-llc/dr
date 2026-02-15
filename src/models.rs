use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackResult {
    pub dr: u32,
    pub peak_db: f64,
    pub rms_db: f64,
    pub duration_secs: f64,
    pub title: String,
    pub filename: String,
    #[serde(default)]
    pub file_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumResult {
    pub tracks: Vec<TrackResult>,
    pub overall_dr: u32,
    pub album: Option<String>,
}

/// Sent from analysis thread to TUI for live progress
pub enum AnalysisEvent {
    TrackStarted { index: usize },
    TrackProgress { index: usize, percent: f32 },
    TrackCompleted { index: usize, result: TrackResult },
    AlbumCompleted { result: AlbumResult },
    Error { index: usize, message: String },
}

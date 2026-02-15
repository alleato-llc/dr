use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::models::{AlbumResult, TrackResult};

#[derive(Debug, Clone)]
pub enum TrackStatus {
    Pending,
    Analyzing(f32),
    Complete(TrackResult),
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum View {
    Main,
    About,
    Export,
    Info,
    RegenerateConfirm,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExportFormat {
    Text,
    Json,
    Csv,
}

#[derive(Debug, Clone)]
pub struct TrackTiming {
    pub elapsed: Duration,
    pub file_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct BenchmarkStats {
    pub total_elapsed: Duration,
    pub track_timings: Vec<TrackTiming>,
}

impl BenchmarkStats {
    pub fn total_mb(&self) -> f64 {
        let total_bytes: u64 = self.track_timings.iter().map(|t| t.file_bytes).sum();
        total_bytes as f64 / (1024.0 * 1024.0)
    }

    pub fn avg_per_track(&self) -> Duration {
        if self.track_timings.is_empty() {
            Duration::ZERO
        } else {
            self.total_elapsed / self.track_timings.len() as u32
        }
    }

    pub fn mb_per_sec(&self) -> f64 {
        let secs = self.total_elapsed.as_secs_f64();
        if secs > 0.0 {
            self.total_mb() / secs
        } else {
            0.0
        }
    }
}

pub struct App {
    pub tracks: Vec<(String, TrackStatus)>,
    pub album_result: Option<AlbumResult>,
    pub view: View,
    pub selected: usize,
    pub scroll_offset: usize,
    pub should_quit: bool,
    pub album_title: Option<String>,
    pub path: PathBuf,
    pub export_format: ExportFormat,
    pub export_message: Option<String>,
    /// Visible height of the track table (updated each frame by the renderer)
    pub visible_rows: usize,
    pub loaded_from_cache: bool,
    pub benchmark: Option<BenchmarkStats>,
    pub analysis_start: Option<Instant>,
    pub track_start_times: Vec<Option<Instant>>,
    pub track_elapsed: Vec<Option<Duration>>,
    pub jobs: usize,
}

impl App {
    pub fn new(filenames: Vec<String>, path: PathBuf, jobs: usize) -> Self {
        let count = filenames.len();
        let tracks = filenames
            .into_iter()
            .map(|name| (name, TrackStatus::Pending))
            .collect();
        Self {
            tracks,
            album_result: None,
            view: View::Main,
            selected: 0,
            scroll_offset: 0,
            should_quit: false,
            album_title: None,
            path,
            export_format: ExportFormat::Text,
            export_message: None,
            visible_rows: 20,
            loaded_from_cache: false,
            benchmark: None,
            analysis_start: None,
            track_start_times: vec![None; count],
            track_elapsed: vec![None; count],
            jobs,
        }
    }

    pub fn load_from_cache(&mut self, result: AlbumResult) {
        for (i, track_result) in result.tracks.iter().enumerate() {
            if let Some(track) = self.tracks.get_mut(i) {
                track.1 = TrackStatus::Complete(track_result.clone());
            }
        }
        self.album_title = result.album.clone();
        self.album_result = Some(result);
        self.loaded_from_cache = true;
    }

    pub fn reset_for_regeneration(&mut self, filenames: Vec<String>) {
        let count = filenames.len();
        self.tracks = filenames
            .into_iter()
            .map(|name| (name, TrackStatus::Pending))
            .collect();
        self.album_result = None;
        self.album_title = None;
        self.selected = 0;
        self.scroll_offset = 0;
        self.loaded_from_cache = false;
        self.benchmark = None;
        self.analysis_start = Some(Instant::now());
        self.track_start_times = vec![None; count];
        self.track_elapsed = vec![None; count];
        self.export_message = None;
        self.view = View::Main;
    }

    pub fn completed_count(&self) -> usize {
        self.tracks
            .iter()
            .filter(|(_, s)| matches!(s, TrackStatus::Complete(_)))
            .count()
    }

    pub fn select_next(&mut self) {
        if !self.tracks.is_empty() {
            self.selected = (self.selected + 1).min(self.tracks.len() - 1);
            self.ensure_visible();
        }
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        self.ensure_visible();
    }

    /// Adjust scroll_offset so that self.selected is within the visible window.
    fn ensure_visible(&mut self) {
        if self.visible_rows == 0 {
            return;
        }
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + self.visible_rows {
            self.scroll_offset = self.selected - self.visible_rows + 1;
        }
    }

    pub fn cycle_export_format(&mut self) {
        self.export_format = match self.export_format {
            ExportFormat::Text => ExportFormat::Json,
            ExportFormat::Json => ExportFormat::Csv,
            ExportFormat::Csv => ExportFormat::Text,
        };
    }
}

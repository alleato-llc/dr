use std::path::PathBuf;

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
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExportFormat {
    Text,
    Json,
    Csv,
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
}

impl App {
    pub fn new(filenames: Vec<String>, path: PathBuf) -> Self {
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
        }
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

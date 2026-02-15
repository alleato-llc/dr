pub mod app;
pub mod ui;

use std::io;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::analyzer::{analyze_directory_async, scan_audio_files};
use crate::format;
use crate::models::AnalysisEvent;

use app::{App, ExportFormat, TrackStatus, View};

pub fn run(path: &Path, jobs: usize) -> Result<()> {
    let files = scan_audio_files(path);
    if files.is_empty() {
        anyhow::bail!("No audio files found in {}", path.display());
    }

    let filenames: Vec<String> = files
        .iter()
        .filter_map(|p| p.file_name().and_then(|f| f.to_str()).map(String::from))
        .collect();

    let mut app = App::new(filenames, path.to_path_buf());

    // Spawn analysis thread
    let (tx, rx) = mpsc::channel::<AnalysisEvent>();
    let analysis_path = path.to_path_buf();
    std::thread::spawn(move || {
        let _ = analyze_directory_async(&analysis_path, tx, jobs);
    });

    // Setup terminal
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, &mut app, rx);

    // Restore terminal
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    rx: mpsc::Receiver<AnalysisEvent>,
) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::render(frame, app))?;
        // Note: ui::render updates app.visible_rows each frame

        // Drain analysis events
        while let Ok(event) = rx.try_recv() {
            match event {
                AnalysisEvent::TrackStarted { index } => {
                    if let Some(track) = app.tracks.get_mut(index) {
                        track.1 = TrackStatus::Analyzing(0.0);
                    }
                }
                AnalysisEvent::TrackProgress { index, percent } => {
                    if let Some(track) = app.tracks.get_mut(index) {
                        track.1 = TrackStatus::Analyzing(percent);
                    }
                }
                AnalysisEvent::TrackCompleted { index, result } => {
                    if let Some(track) = app.tracks.get_mut(index) {
                        if app.album_title.is_none() {
                            // Try to use the title hint
                        }
                        track.1 = TrackStatus::Complete(result);
                    }
                }
                AnalysisEvent::AlbumCompleted { result } => {
                    app.album_title = result.album.clone();
                    app.album_result = Some(result);
                }
                AnalysisEvent::Error { index, message } => {
                    if let Some(track) = app.tracks.get_mut(index) {
                        track.1 = TrackStatus::Error(message);
                    }
                }
            }
        }

        // Poll for key events
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match app.view {
                    View::Main => match key.code {
                        KeyCode::Char('q') => {
                            app.should_quit = true;
                            break;
                        }
                        KeyCode::Char('e') => {
                            if app.album_result.is_some() {
                                app.view = View::Export;
                                app.export_message = None;
                            }
                        }
                        KeyCode::Char('a') => {
                            app.view = View::About;
                        }
                        KeyCode::Char('j') | KeyCode::Down => {
                            app.select_next();
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            app.select_prev();
                        }
                        _ => {}
                    },
                    View::About => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            app.view = View::Main;
                        }
                        _ => {}
                    },
                    View::Export => match key.code {
                        KeyCode::Esc => {
                            app.view = View::Main;
                        }
                        KeyCode::Tab => {
                            app.cycle_export_format();
                            app.export_message = None;
                        }
                        KeyCode::Enter => {
                            if let Some(ref result) = app.album_result {
                                let (content, ext) = match app.export_format {
                                    ExportFormat::Text => (format::format_table(result), "txt"),
                                    ExportFormat::Json => (format::format_json(result), "json"),
                                    ExportFormat::Csv => (format::format_csv(result), "csv"),
                                };
                                let output_path = app.path.join(format!("dr_report.{}", ext));
                                match std::fs::write(&output_path, &content) {
                                    Ok(_) => {
                                        app.export_message = Some(format!(
                                            "Saved to {}",
                                            output_path.display()
                                        ));
                                    }
                                    Err(e) => {
                                        app.export_message =
                                            Some(format!("Error: {}", e));
                                    }
                                }
                            }
                        }
                        _ => {}
                    },
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

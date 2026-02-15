pub mod app;
pub mod ui;

use std::io;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::analyzer::{analyze_directory_async, scan_audio_files};
use crate::cache;
use crate::format;
use crate::models::AnalysisEvent;

use app::{App, BenchmarkStats, ExportFormat, TrackStatus, TrackTiming, View};

pub fn run(path: &Path, jobs: usize, regenerate: bool) -> Result<()> {
    let files = scan_audio_files(path);
    if files.is_empty() {
        anyhow::bail!("No audio files found in {}", path.display());
    }

    let filenames: Vec<String> = files
        .iter()
        .filter_map(|p| p.file_name().and_then(|f| f.to_str()).map(String::from))
        .collect();

    let mut app = App::new(filenames, path.to_path_buf(), jobs);

    // Check for cached report
    let rx = if !regenerate {
        if let Some(cached) = cache::load_cached_report(path) {
            app.load_from_cache(cached);
            // Create a dummy channel that will never receive
            let (_tx, rx) = mpsc::channel::<AnalysisEvent>();
            rx
        } else {
            spawn_analysis(&mut app, path, jobs)
        }
    } else {
        spawn_analysis(&mut app, path, jobs)
    };

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

fn spawn_analysis(
    app: &mut App,
    path: &Path,
    jobs: usize,
) -> mpsc::Receiver<AnalysisEvent> {
    let (tx, rx) = mpsc::channel::<AnalysisEvent>();
    let analysis_path = path.to_path_buf();
    app.analysis_start = Some(Instant::now());
    std::thread::spawn(move || {
        let _ = analyze_directory_async(&analysis_path, tx, jobs);
    });
    rx
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    rx: mpsc::Receiver<AnalysisEvent>,
) -> Result<()> {
    let mut rx = rx;

    loop {
        terminal.draw(|frame| ui::render(frame, app))?;

        // Drain analysis events
        while let Ok(event) = rx.try_recv() {
            match event {
                AnalysisEvent::TrackStarted { index } => {
                    if let Some(track) = app.tracks.get_mut(index) {
                        track.1 = TrackStatus::Analyzing(0.0);
                    }
                    if index < app.track_start_times.len() {
                        app.track_start_times[index] = Some(Instant::now());
                    }
                }
                AnalysisEvent::TrackProgress { index, percent } => {
                    if let Some(track) = app.tracks.get_mut(index) {
                        track.1 = TrackStatus::Analyzing(percent);
                    }
                }
                AnalysisEvent::TrackCompleted { index, result } => {
                    // Record timing
                    if index < app.track_start_times.len() {
                        if let Some(start) = app.track_start_times[index] {
                            let elapsed = start.elapsed();
                            if index < app.track_elapsed.len() {
                                app.track_elapsed[index] = Some(elapsed);
                            }
                        }
                    }
                    if let Some(track) = app.tracks.get_mut(index) {
                        track.1 = TrackStatus::Complete(result);
                    }
                }
                AnalysisEvent::AlbumCompleted { result } => {
                    // Build benchmark stats
                    if let Some(analysis_start) = app.analysis_start {
                        let total_elapsed = analysis_start.elapsed();
                        let track_timings: Vec<TrackTiming> = result
                            .tracks
                            .iter()
                            .enumerate()
                            .map(|(i, t)| TrackTiming {
                                elapsed: app
                                    .track_elapsed
                                    .get(i)
                                    .copied()
                                    .flatten()
                                    .unwrap_or(Duration::ZERO),
                                file_bytes: t.file_bytes,
                            })
                            .collect();
                        app.benchmark = Some(BenchmarkStats {
                            total_elapsed,
                            track_timings,
                        });
                    }

                    // Auto-save cache
                    let _ = cache::save_report(&app.path, &result);

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
                        KeyCode::Char('i') => {
                            if app.album_result.is_some() {
                                app.view = View::Info;
                            }
                        }
                        KeyCode::Char('r') => {
                            if app.album_result.is_some() {
                                app.view = View::RegenerateConfirm;
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
                    View::About | View::Info => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            app.view = View::Main;
                        }
                        _ => {}
                    },
                    View::RegenerateConfirm => match key.code {
                        KeyCode::Char('y') | KeyCode::Enter => {
                            // Rescan files and regenerate
                            let files = scan_audio_files(&app.path);
                            let filenames: Vec<String> = files
                                .iter()
                                .filter_map(|p| {
                                    p.file_name()
                                        .and_then(|f| f.to_str())
                                        .map(String::from)
                                })
                                .collect();
                            let jobs = app.jobs;
                            let path = app.path.clone();
                            app.reset_for_regeneration(filenames);
                            rx = spawn_analysis(app, &path, jobs);
                        }
                        KeyCode::Char('n') | KeyCode::Esc => {
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

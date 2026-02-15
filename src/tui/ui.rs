use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table};
use ratatui::Frame;

use super::app::{App, ExportFormat, TrackStatus, View};
use crate::format::format_duration;

const ACCENT: Color = Color::Cyan;
const DIM: Color = Color::DarkGray;
const COMPLETE_COLOR: Color = Color::Green;
const ERROR_COLOR: Color = Color::Red;
const PROGRESS_COLOR: Color = Color::Yellow;

pub fn render(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(5),   // Track table
            Constraint::Length(3), // Summary
            Constraint::Length(1), // Footer
        ])
        .split(frame.area());

    render_header(frame, app, chunks[0]);
    render_track_table(frame, app, chunks[1]);
    render_summary(frame, app, chunks[2]);
    render_footer(frame, app, chunks[3]);

    // Overlays
    match app.view {
        View::About => render_about_overlay(frame),
        View::Export => render_export_overlay(frame, app),
        View::Main => {}
    }
}

fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let album_text = app
        .album_title
        .as_deref()
        .unwrap_or("Unknown Album");
    let path_text = app.path.display().to_string();

    let text = vec![
        Line::from(vec![
            Span::styled("Album: ", Style::default().fg(DIM)),
            Span::styled(album_text, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled("Path: ", Style::default().fg(DIM)),
            Span::styled(path_text, Style::default().fg(DIM)),
        ]),
    ];

    let block = Block::default()
        .title(Span::styled(
            " DR Meter ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT));

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn render_track_table(frame: &mut Frame, app: &mut App, area: Rect) {
    // 2 for borders, 1 for header
    let inner_height = area.height.saturating_sub(3) as usize;
    app.visible_rows = inner_height;

    // Build a scroll indicator for the block title
    let total = app.tracks.len();
    let scroll_info = if total > inner_height {
        let has_above = app.scroll_offset > 0;
        let has_below = app.scroll_offset + inner_height < total;
        match (has_above, has_below) {
            (true, true) => format!(" [{}-{}/{}] \u{2191}\u{2193} ", app.scroll_offset + 1, (app.scroll_offset + inner_height).min(total), total),
            (true, false) => format!(" [{}-{}/{}] \u{2191} ", app.scroll_offset + 1, total, total),
            (false, true) => format!(" [1-{}/{}] \u{2193} ", inner_height.min(total), total),
            (false, false) => String::new(),
        }
    } else {
        String::new()
    };

    let header = Row::new(vec![
        Cell::from("#").style(Style::default().fg(DIM)),
        Cell::from("Track").style(Style::default().fg(DIM)),
        Cell::from("DR").style(Style::default().fg(DIM)),
        Cell::from("Peak").style(Style::default().fg(DIM)),
        Cell::from("RMS").style(Style::default().fg(DIM)),
        Cell::from("Duration").style(Style::default().fg(DIM)),
        Cell::from("").style(Style::default().fg(DIM)),
    ])
    .height(1);

    // Only render the visible slice of tracks
    let end = (app.scroll_offset + inner_height).min(app.tracks.len());
    let visible_slice = &app.tracks[app.scroll_offset..end];

    let rows: Vec<Row> = visible_slice
        .iter()
        .enumerate()
        .map(|(vi, (name, status))| {
            let actual_index = app.scroll_offset + vi;
            let num = format!("{}", actual_index + 1);
            let is_selected = actual_index == app.selected;
            let style = if is_selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            match status {
                TrackStatus::Pending => Row::new(vec![
                    Cell::from(num),
                    Cell::from(name.as_str()),
                    Cell::from("\u{00b7}").style(Style::default().fg(DIM)),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from("\u{00b7}").style(Style::default().fg(DIM)),
                ])
                .style(style),
                TrackStatus::Analyzing(pct) => {
                    let bar_width = 12;
                    let filled = (pct * bar_width as f32) as usize;
                    let empty = bar_width - filled;
                    let bar = format!(
                        "{}{} {:>3}%",
                        "\u{2588}".repeat(filled),
                        "\u{2591}".repeat(empty),
                        (pct * 100.0) as u32
                    );
                    Row::new(vec![
                        Cell::from(num),
                        Cell::from(name.as_str()),
                        Cell::from(bar).style(Style::default().fg(PROGRESS_COLOR)),
                        Cell::from(""),
                        Cell::from(""),
                        Cell::from(""),
                        Cell::from("\u{27f3}").style(Style::default().fg(PROGRESS_COLOR)),
                    ])
                    .style(style)
                }
                TrackStatus::Complete(result) => Row::new(vec![
                    Cell::from(num),
                    Cell::from(result.title.as_str()),
                    Cell::from(format!("DR{}", result.dr))
                        .style(Style::default().fg(dr_color(result.dr))),
                    Cell::from(format!("{:.2}dB", result.peak_db)),
                    Cell::from(format!("{:.2}dB", result.rms_db)),
                    Cell::from(format_duration(result.duration_secs)),
                    Cell::from("\u{2713}").style(Style::default().fg(COMPLETE_COLOR)),
                ])
                .style(style),
                TrackStatus::Error(msg) => Row::new(vec![
                    Cell::from(num),
                    Cell::from(name.as_str()),
                    Cell::from("ERR").style(Style::default().fg(ERROR_COLOR)),
                    Cell::from(msg.as_str()).style(Style::default().fg(ERROR_COLOR)),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from("\u{2717}").style(Style::default().fg(ERROR_COLOR)),
                ])
                .style(style),
            }
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Min(20),
        Constraint::Length(18),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(8),
        Constraint::Length(2),
    ];

    let block = Block::default()
        .title(Span::styled(
            scroll_info,
            Style::default().fg(DIM),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT));

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(Style::default().bg(Color::DarkGray));

    frame.render_widget(table, area);
}

fn render_summary(frame: &mut Frame, app: &App, area: Rect) {
    let completed = app.completed_count();
    let total = app.tracks.len();

    let dr_text = if let Some(ref album) = app.album_result {
        format!("Overall DR: DR{}", album.overall_dr)
    } else if completed > 0 {
        // Compute running average
        let sum: u32 = app
            .tracks
            .iter()
            .filter_map(|(_, s)| match s {
                TrackStatus::Complete(r) => Some(r.dr),
                _ => None,
            })
            .sum();
        format!("Overall DR: ~DR{}", sum / completed as u32)
    } else {
        "Overall DR: --".to_string()
    };

    let text = format!("{} ({}/{} complete)", dr_text, completed, total);
    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        );
    frame.render_widget(paragraph, area);
}

fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let keys = match app.view {
        View::Main => "[e]xport  [a]bout  [q]uit",
        View::About | View::Export => "[Esc] close",
    };
    let footer = Paragraph::new(keys)
        .style(Style::default().fg(DIM))
        .alignment(Alignment::Center);
    frame.render_widget(footer, area);
}

fn render_about_overlay(frame: &mut Frame) {
    let area = centered_rect(40, 10, frame.area());
    frame.render_widget(Clear, area);

    let text = vec![
        Line::from(Span::styled(
            "DR Meter",
            Style::default()
                .fg(ACCENT)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!("Version {}", env!("CARGO_PKG_VERSION"))),
        Line::from(""),
        Line::from("Dynamic range analyzer for audio files."),
        Line::from("Uses the Pleasurize Music / DR Database algorithm."),
        Line::from(""),
        Line::from(Span::styled("[Esc] close", Style::default().fg(DIM))),
    ];

    let block = Block::default()
        .title(" About ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT));

    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(block);
    frame.render_widget(paragraph, area);
}

fn render_export_overlay(frame: &mut Frame, app: &App) {
    let area = centered_rect(50, 12, frame.area());
    frame.render_widget(Clear, area);

    let format_name = match app.export_format {
        ExportFormat::Text => "Text (DR Database table)",
        ExportFormat::Json => "JSON",
        ExportFormat::Csv => "CSV",
    };

    let ext = match app.export_format {
        ExportFormat::Text => "txt",
        ExportFormat::Json => "json",
        ExportFormat::Csv => "csv",
    };

    let output_path = app.path.join(format!("dr_report.{}", ext));

    let mut text = vec![
        Line::from(Span::styled(
            "Export Report",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Format: ", Style::default().fg(DIM)),
            Span::styled(format_name, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Output: ", Style::default().fg(DIM)),
            Span::styled(
                output_path.display().to_string(),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "[Tab] cycle format  [Enter] save  [Esc] cancel",
            Style::default().fg(DIM),
        )),
    ];

    if let Some(ref msg) = app.export_message {
        text.push(Line::from(""));
        text.push(Line::from(Span::styled(
            msg.as_str(),
            Style::default().fg(COMPLETE_COLOR),
        )));
    }

    let block = Block::default()
        .title(" Export ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT));

    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(block);
    frame.render_widget(paragraph, area);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

/// Color-code DR values: high DR = green, medium = yellow, low = red.
fn dr_color(dr: u32) -> Color {
    match dr {
        0..=7 => Color::Red,
        8..=11 => Color::Yellow,
        _ => Color::Green,
    }
}

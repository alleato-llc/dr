use crate::models::{AlbumResult, TrackResult};

/// Format a duration in seconds as "M:SS".
pub fn format_duration(secs: f64) -> String {
    let total_secs = secs.round() as u64;
    let minutes = total_secs / 60;
    let seconds = total_secs % 60;
    format!("{}:{:02}", minutes, seconds)
}

/// Format a single track result as a DR Database-style table.
pub fn format_table_single(result: &TrackResult) -> String {
    let separator = "\u{2500}".repeat(58);
    format!(
        "{:<10} {:>10} {:>10} {:>10}  {}\n\
         {}\n\
         DR{:<8} {:>7.2} dB {:>7.2} dB {:>10}  {}\n\
         {}\n\
         Official DR value: DR{}",
        "DR", "Peak", "RMS", "Duration", "Track",
        separator,
        result.dr,
        result.peak_db,
        result.rms_db,
        format_duration(result.duration_secs),
        result.title,
        separator,
        result.dr,
    )
}

/// Format an album result as a DR Database-style table.
pub fn format_table(result: &AlbumResult) -> String {
    let separator = "\u{2500}".repeat(58);
    let mut output = String::new();

    // Header
    output.push_str(&format!(
        "{:<10} {:>10} {:>10} {:>10}  {}\n",
        "DR", "Peak", "RMS", "Duration", "Track"
    ));
    output.push_str(&separator);
    output.push('\n');

    // Track rows
    for track in &result.tracks {
        output.push_str(&format!(
            "DR{:<8} {:>7.2} dB {:>7.2} dB {:>10}  {}\n",
            track.dr,
            track.peak_db,
            track.rms_db,
            format_duration(track.duration_secs),
            track.title,
        ));
    }

    output.push_str(&separator);
    output.push('\n');

    // Footer
    output.push_str(&format!(
        "Number of tracks:  {}\n\
         Official DR value: DR{}",
        result.tracks.len(),
        result.overall_dr,
    ));

    output
}

/// Format a single track result as pretty-printed JSON.
pub fn format_json_single(result: &TrackResult) -> String {
    serde_json::to_string_pretty(result).unwrap_or_else(|_| "{}".to_string())
}

/// Format an album result as pretty-printed JSON.
pub fn format_json(result: &AlbumResult) -> String {
    serde_json::to_string_pretty(result).unwrap_or_else(|_| "{}".to_string())
}

/// Format an album result as CSV.
pub fn format_csv(result: &AlbumResult) -> String {
    let mut output = String::from("DR,Peak dB,RMS dB,Duration,Track\n");
    for track in &result.tracks {
        output.push_str(&format!(
            "{},{:.2},{:.2},{},{}\n",
            track.dr,
            track.peak_db,
            track.rms_db,
            format_duration(track.duration_secs),
            track.title,
        ));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0.0), "0:00");
        assert_eq!(format_duration(61.0), "1:01");
        assert_eq!(format_duration(125.0), "2:05");
        assert_eq!(format_duration(3661.0), "61:01");
    }

    #[test]
    fn test_format_table_columns() {
        let result = AlbumResult {
            tracks: vec![TrackResult {
                dr: 14,
                peak_db: -0.10,
                rms_db: -16.78,
                duration_secs: 263.0,
                title: "Test Track".to_string(),
                filename: "test.flac".to_string(),
                file_bytes: 0,
            }],
            overall_dr: 14,
            album: Some("Test Album".to_string()),
        };
        let table = format_table(&result);
        assert!(table.contains("DR14"));
        assert!(table.contains("-0.10 dB"));
        assert!(table.contains("-16.78 dB"));
        assert!(table.contains("4:23"));
        assert!(table.contains("Test Track"));
        assert!(table.contains("Official DR value: DR14"));
        assert!(table.contains("Number of tracks:  1"));
    }

    #[test]
    fn test_format_json_roundtrip() {
        let track = TrackResult {
            dr: 12,
            peak_db: -0.30,
            rms_db: -14.56,
            duration_secs: 225.0,
            title: "My Track".to_string(),
            filename: "track.flac".to_string(),
            file_bytes: 0,
        };
        let json = format_json_single(&track);
        let parsed: TrackResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.dr, 12);
        assert_eq!(parsed.title, "My Track");
    }

    #[test]
    fn test_format_csv() {
        let result = AlbumResult {
            tracks: vec![
                TrackResult {
                    dr: 14,
                    peak_db: -0.10,
                    rms_db: -16.78,
                    duration_secs: 263.0,
                    title: "Track One".to_string(),
                    filename: "01.flac".to_string(),
                    file_bytes: 0,
                },
                TrackResult {
                    dr: 12,
                    peak_db: -0.30,
                    rms_db: -14.56,
                    duration_secs: 225.0,
                    title: "Track Two".to_string(),
                    filename: "02.flac".to_string(),
                    file_bytes: 0,
                },
            ],
            overall_dr: 13,
            album: None,
        };
        let csv = format_csv(&result);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "DR,Peak dB,RMS dB,Duration,Track");
        assert!(lines[1].starts_with("14,"));
        assert!(lines[2].starts_with("12,"));
    }
}

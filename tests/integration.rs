use std::io::Write;
use std::path::PathBuf;

use dr::analyzer;
use dr::format;
use dr::models::{AlbumResult, TrackResult};

/// Generate a WAV file with a pure sine wave at a given frequency and amplitude.
/// Returns the path to the temporary WAV file.
fn generate_sine_wav(
    dir: &std::path::Path,
    filename: &str,
    frequency: f64,
    amplitude: f32,
    duration_secs: f64,
    sample_rate: u32,
) -> PathBuf {
    let num_channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let num_samples = (sample_rate as f64 * duration_secs) as usize;
    let byte_rate = sample_rate * num_channels as u32 * bits_per_sample as u32 / 8;
    let block_align = num_channels * bits_per_sample / 8;
    let data_size = (num_samples * num_channels as usize * (bits_per_sample as usize / 8)) as u32;

    let path = dir.join(filename);
    let mut file = std::fs::File::create(&path).unwrap();

    // RIFF header
    file.write_all(b"RIFF").unwrap();
    file.write_all(&(36 + data_size).to_le_bytes()).unwrap();
    file.write_all(b"WAVE").unwrap();

    // fmt chunk
    file.write_all(b"fmt ").unwrap();
    file.write_all(&16u32.to_le_bytes()).unwrap(); // chunk size
    file.write_all(&1u16.to_le_bytes()).unwrap(); // PCM
    file.write_all(&num_channels.to_le_bytes()).unwrap();
    file.write_all(&sample_rate.to_le_bytes()).unwrap();
    file.write_all(&byte_rate.to_le_bytes()).unwrap();
    file.write_all(&block_align.to_le_bytes()).unwrap();
    file.write_all(&bits_per_sample.to_le_bytes()).unwrap();

    // data chunk
    file.write_all(b"data").unwrap();
    file.write_all(&data_size.to_le_bytes()).unwrap();

    for i in 0..num_samples {
        let t = i as f64 / sample_rate as f64;
        let sample = amplitude * (2.0 * std::f64::consts::PI * frequency * t).sin() as f32;
        let sample_i16 = (sample * i16::MAX as f32) as i16;
        file.write_all(&sample_i16.to_le_bytes()).unwrap();
    }

    path
}

#[test]
fn test_sine_wave_dr() {
    let dir = tempfile::tempdir().unwrap();

    // With the TT DR standard's sqrt(2) RMS calibration, a pure sine wave's
    // DR-RMS equals its peak, so DR should be ~0 (crest factor cancelled out).
    // Using 2nd-highest block peak (which equals the highest for a steady sine)
    // and per-channel processing, a mono sine should yield DR0.
    let path = generate_sine_wav(dir.path(), "sine.wav", 440.0, 0.99, 12.0, 44100);

    let result = analyzer::analyze_file(&path).unwrap();

    // A pure sine wave should have DR ~0 with the sqrt(2) calibration
    assert!(
        result.dr <= 1,
        "Pure sine wave DR should be ~0, got DR{}",
        result.dr
    );

    // Peak should be near 0 dBFS
    assert!(
        result.peak_db > -1.0,
        "Peak should be near 0 dBFS, got {:.2}",
        result.peak_db
    );
}

#[test]
fn test_album_dr_is_average() {
    let dir = tempfile::tempdir().unwrap();

    // Create two tracks with same sine wave (both should have ~DR3)
    generate_sine_wav(dir.path(), "01-track.wav", 440.0, 0.99, 12.0, 44100);
    generate_sine_wav(dir.path(), "02-track.wav", 880.0, 0.99, 12.0, 44100);

    let result = analyzer::analyze_directory(dir.path(), 2).unwrap();

    assert_eq!(result.tracks.len(), 2);

    // Album DR should be average of individual DRs
    let expected_avg =
        (result.tracks[0].dr as f64 + result.tracks[1].dr as f64) / 2.0;
    let expected_rounded = expected_avg.round() as u32;
    assert_eq!(
        result.overall_dr, expected_rounded,
        "Album DR should be average of track DRs"
    );
}

#[test]
fn test_table_formatter_columns() {
    let result = AlbumResult {
        tracks: vec![
            TrackResult {
                dr: 14,
                peak_db: -0.10,
                rms_db: -16.78,
                duration_secs: 263.0,
                title: "First Track".to_string(),
                filename: "01.flac".to_string(),
            },
            TrackResult {
                dr: 12,
                peak_db: -0.30,
                rms_db: -14.56,
                duration_secs: 225.0,
                title: "Second Track".to_string(),
                filename: "02.flac".to_string(),
            },
        ],
        overall_dr: 13,
        album: Some("Test Album".to_string()),
    };

    let table = format::format_table(&result);

    // Verify header
    assert!(table.contains("DR"));
    assert!(table.contains("Peak"));
    assert!(table.contains("RMS"));
    assert!(table.contains("Duration"));
    assert!(table.contains("Track"));

    // Verify track data
    assert!(table.contains("DR14"));
    assert!(table.contains("DR12"));
    assert!(table.contains("First Track"));
    assert!(table.contains("Second Track"));
    assert!(table.contains("4:23"));
    assert!(table.contains("3:45"));

    // Verify footer
    assert!(table.contains("Number of tracks:  2"));
    assert!(table.contains("Official DR value: DR13"));
}

#[test]
fn test_json_roundtrip() {
    let track = TrackResult {
        dr: 14,
        peak_db: -0.10,
        rms_db: -16.78,
        duration_secs: 263.0,
        title: "Test Track".to_string(),
        filename: "test.flac".to_string(),
    };

    let json = format::format_json_single(&track);
    let parsed: TrackResult = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.dr, track.dr);
    assert_eq!(parsed.title, track.title);
    assert_eq!(parsed.filename, track.filename);
    assert!((parsed.peak_db - track.peak_db).abs() < 0.001);
    assert!((parsed.rms_db - track.rms_db).abs() < 0.001);
}

#[test]
fn test_album_json_roundtrip() {
    let result = AlbumResult {
        tracks: vec![TrackResult {
            dr: 10,
            peak_db: -0.50,
            rms_db: -12.00,
            duration_secs: 180.0,
            title: "A Track".to_string(),
            filename: "a.flac".to_string(),
        }],
        overall_dr: 10,
        album: Some("My Album".to_string()),
    };

    let json = format::format_json(&result);
    let parsed: AlbumResult = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.overall_dr, 10);
    assert_eq!(parsed.album, Some("My Album".to_string()));
    assert_eq!(parsed.tracks.len(), 1);
}

#[test]
fn test_scan_audio_files() {
    let dir = tempfile::tempdir().unwrap();

    // Create various files
    std::fs::write(dir.path().join("track.flac"), b"fake").unwrap();
    std::fs::write(dir.path().join("track.mp3"), b"fake").unwrap();
    std::fs::write(dir.path().join("cover.jpg"), b"fake").unwrap();
    std::fs::write(dir.path().join("notes.txt"), b"fake").unwrap();

    let files = analyzer::scan_audio_files(dir.path());
    assert_eq!(files.len(), 2);
    assert!(files[0].extension().unwrap() == "flac");
    assert!(files[1].extension().unwrap() == "mp3");
}

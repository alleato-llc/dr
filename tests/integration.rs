use std::io::Write;
use std::path::PathBuf;

use assert_cmd::cargo::cargo_bin_cmd;
use dr::analyzer;
use dr::cache;
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
                file_bytes: 0,
            },
            TrackResult {
                dr: 12,
                peak_db: -0.30,
                rms_db: -14.56,
                duration_secs: 225.0,
                title: "Second Track".to_string(),
                filename: "02.flac".to_string(),
                file_bytes: 0,
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
        file_bytes: 0,
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
            file_bytes: 0,
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

// --- Cache helper tests ---

#[test]
fn test_reports_exist_no_files() {
    let dir = tempfile::tempdir().unwrap();
    // Neither file exists
    assert!(!cache::reports_exist(dir.path(), true, false));
    assert!(!cache::reports_exist(dir.path(), false, true));
    assert!(!cache::reports_exist(dir.path(), true, true));
    // Neither requested — vacuously true
    assert!(cache::reports_exist(dir.path(), false, false));
}

#[test]
fn test_reports_exist_json_only() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("dr_report.json"), "{}").unwrap();

    assert!(cache::reports_exist(dir.path(), true, false));
    assert!(!cache::reports_exist(dir.path(), true, true));
    assert!(!cache::reports_exist(dir.path(), false, true));
}

#[test]
fn test_reports_exist_txt_only() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("dr_report.txt"), "report").unwrap();

    assert!(cache::reports_exist(dir.path(), false, true));
    assert!(!cache::reports_exist(dir.path(), true, true));
    assert!(!cache::reports_exist(dir.path(), true, false));
}

#[test]
fn test_reports_exist_both() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("dr_report.json"), "{}").unwrap();
    std::fs::write(dir.path().join("dr_report.txt"), "report").unwrap();

    assert!(cache::reports_exist(dir.path(), true, true));
    assert!(cache::reports_exist(dir.path(), true, false));
    assert!(cache::reports_exist(dir.path(), false, true));
}

#[test]
fn test_save_text_report() {
    let dir = tempfile::tempdir().unwrap();
    let content = "DR14  -0.10 dB  -16.78 dB  4:23  Test Track";
    cache::save_text_report(dir.path(), content).unwrap();

    let path = dir.path().join("dr_report.txt");
    assert!(path.exists());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
}

// --- CLI validation tests ---

#[test]
fn test_bulk_and_tui_conflict() {
    cargo_bin_cmd!("dr")
        .args([".", "--bulk", "--tui"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--bulk and --tui cannot be used together"));
}

#[test]
fn test_bulk_requires_output_format() {
    cargo_bin_cmd!("dr")
        .args([".", "--bulk"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "--bulk requires at least one output format",
        ));
}

// --- Bulk mode integration tests ---

/// Helper: create a temp directory with two "album" subdirectories, each containing a sine WAV.
fn setup_bulk_dir() -> tempfile::TempDir {
    let base = tempfile::tempdir().unwrap();

    let album_a = base.path().join("Album A");
    let album_b = base.path().join("Album B");
    std::fs::create_dir(&album_a).unwrap();
    std::fs::create_dir(&album_b).unwrap();

    generate_sine_wav(&album_a, "01-track.wav", 440.0, 0.99, 12.0, 44100);
    generate_sine_wav(&album_b, "01-track.wav", 880.0, 0.99, 12.0, 44100);

    base
}

#[test]
fn test_bulk_json_only() {
    let base = setup_bulk_dir();

    cargo_bin_cmd!("dr")
        .args([base.path().to_str().unwrap(), "--bulk", "--json"])
        .assert()
        .success();

    // JSON reports should exist in both album directories
    assert!(base.path().join("Album A/dr_report.json").exists());
    assert!(base.path().join("Album B/dr_report.json").exists());
    // Text reports should NOT exist
    assert!(!base.path().join("Album A/dr_report.txt").exists());
    assert!(!base.path().join("Album B/dr_report.txt").exists());

    // Verify JSON is valid
    let json_a = std::fs::read_to_string(base.path().join("Album A/dr_report.json")).unwrap();
    let parsed: AlbumResult = serde_json::from_str(&json_a).unwrap();
    assert_eq!(parsed.tracks.len(), 1);
}

#[test]
fn test_bulk_txt_only() {
    let base = setup_bulk_dir();

    cargo_bin_cmd!("dr")
        .args([base.path().to_str().unwrap(), "--bulk", "--txt"])
        .assert()
        .success();

    // Text reports should exist
    assert!(base.path().join("Album A/dr_report.txt").exists());
    assert!(base.path().join("Album B/dr_report.txt").exists());
    // JSON reports should NOT exist
    assert!(!base.path().join("Album A/dr_report.json").exists());
    assert!(!base.path().join("Album B/dr_report.json").exists());

    // Verify text content has expected table structure
    let txt = std::fs::read_to_string(base.path().join("Album A/dr_report.txt")).unwrap();
    assert!(txt.contains("DR"));
    assert!(txt.contains("Official DR value"));
}

#[test]
fn test_bulk_json_and_txt() {
    let base = setup_bulk_dir();

    cargo_bin_cmd!("dr")
        .args([base.path().to_str().unwrap(), "--bulk", "--json", "--txt"])
        .assert()
        .success();

    // Both report types should exist in both albums
    for album in &["Album A", "Album B"] {
        assert!(base.path().join(album).join("dr_report.json").exists());
        assert!(base.path().join(album).join("dr_report.txt").exists());
    }
}

#[test]
fn test_bulk_skips_existing_reports() {
    let base = setup_bulk_dir();

    // First run — generates reports
    cargo_bin_cmd!("dr")
        .args([base.path().to_str().unwrap(), "--bulk", "--json"])
        .assert()
        .success();

    // Record modification time of Album A's report
    let report_path = base.path().join("Album A/dr_report.json");
    let mtime_before = std::fs::metadata(&report_path).unwrap().modified().unwrap();

    // Brief sleep to ensure mtime would differ if file were rewritten
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Second run — should skip
    cargo_bin_cmd!("dr")
        .args([base.path().to_str().unwrap(), "--bulk", "--json"])
        .assert()
        .success()
        .stderr(predicates::str::contains("Skipping"));

    let mtime_after = std::fs::metadata(&report_path).unwrap().modified().unwrap();
    assert_eq!(mtime_before, mtime_after, "Report should not have been rewritten");
}

#[test]
fn test_bulk_regenerate_overwrites() {
    let base = setup_bulk_dir();

    // First run
    cargo_bin_cmd!("dr")
        .args([base.path().to_str().unwrap(), "--bulk", "--json"])
        .assert()
        .success();

    let report_path = base.path().join("Album A/dr_report.json");
    let mtime_before = std::fs::metadata(&report_path).unwrap().modified().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(50));

    // Second run with --regenerate — should re-analyze
    cargo_bin_cmd!("dr")
        .args([base.path().to_str().unwrap(), "--bulk", "--json", "--regenerate"])
        .assert()
        .success()
        .stderr(predicates::str::contains("Analyzing"));

    let mtime_after = std::fs::metadata(&report_path).unwrap().modified().unwrap();
    assert_ne!(mtime_before, mtime_after, "Report should have been rewritten");
}

#[test]
fn test_bulk_no_subdirectories() {
    let base = tempfile::tempdir().unwrap();
    // Empty dir — no subdirectories
    std::fs::write(base.path().join("file.txt"), "not a dir").unwrap();

    cargo_bin_cmd!("dr")
        .args([base.path().to_str().unwrap(), "--bulk", "--json"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("No subdirectories found"));
}

#[test]
fn test_bulk_summary_line() {
    let base = setup_bulk_dir();

    cargo_bin_cmd!("dr")
        .args([base.path().to_str().unwrap(), "--bulk", "--json"])
        .assert()
        .success()
        .stderr(predicates::str::contains("Done: 2 analyzed, 0 skipped, 0 failed (out of 2 total)"));
}

// --- Single-directory --txt test ---

#[test]
fn test_single_dir_txt_flag() {
    let dir = tempfile::tempdir().unwrap();
    generate_sine_wav(dir.path(), "track.wav", 440.0, 0.99, 12.0, 44100);

    cargo_bin_cmd!("dr")
        .args([dir.path().to_str().unwrap(), "--txt"])
        .assert()
        .success();

    // Both JSON (auto-saved) and TXT should exist
    assert!(dir.path().join("dr_report.json").exists());
    assert!(dir.path().join("dr_report.txt").exists());

    let txt = std::fs::read_to_string(dir.path().join("dr_report.txt")).unwrap();
    assert!(txt.contains("Official DR value"));
}

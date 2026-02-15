use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;

use dr::analyzer;
use dr::cache;
use dr::format;
use dr::models::AlbumResult;

#[derive(Parser)]
#[command(name = "dr", about = "Dynamic range meter for audio files")]
struct Cli {
    /// Audio file, directory, or "-" for STDIN
    path: Option<String>,

    /// Format hint for STDIN (e.g. flac, mp3, opus)
    #[arg(long)]
    format: Option<String>,

    /// Output as JSON instead of table
    #[arg(long)]
    json: bool,

    /// Launch interactive TUI
    #[arg(long)]
    tui: bool,

    /// Number of parallel analysis jobs (default: number of CPU cores)
    #[arg(short = 'j', long)]
    jobs: Option<usize>,

    /// Re-analyze even if a cached report exists
    #[arg(long)]
    regenerate: bool,

    /// Analyze all immediate subdirectories as separate albums
    #[arg(long)]
    bulk: bool,

    /// Write a text report (dr_report.txt) alongside JSON
    #[arg(long)]
    txt: bool,
}

fn print_benchmark(result: &AlbumResult, elapsed: std::time::Duration) {
    let total_bytes: u64 = result.tracks.iter().map(|t| t.file_bytes).sum();
    let total_mb = total_bytes as f64 / (1024.0 * 1024.0);
    let secs = elapsed.as_secs_f64();
    let avg_per_track = if result.tracks.is_empty() {
        0.0
    } else {
        secs / result.tracks.len() as f64
    };
    let mb_per_sec = if secs > 0.0 { total_mb / secs } else { 0.0 };

    eprintln!(
        "Processed {} tracks ({:.1} MB) in {:.2}s | {:.2}s/track | {:.1} MB/s",
        result.tracks.len(),
        total_mb,
        secs,
        avg_per_track,
        mb_per_sec,
    );
}

fn run_bulk(base_path: &Path, jobs: usize, write_json: bool, write_txt: bool, regenerate: bool) -> Result<()> {
    let mut subdirs: Vec<_> = std::fs::read_dir(base_path)
        .with_context(|| format!("Failed to read directory: {}", base_path.display()))?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            if entry.path().is_dir() {
                Some(entry.path())
            } else {
                None
            }
        })
        .collect();

    subdirs.sort();

    if subdirs.is_empty() {
        anyhow::bail!("No subdirectories found in '{}'", base_path.display());
    }

    let total = subdirs.len();
    let mut analyzed = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;

    for (i, subdir) in subdirs.iter().enumerate() {
        let album_name = subdir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| subdir.display().to_string());

        if !regenerate && cache::reports_exist(subdir, write_json, write_txt) {
            eprintln!("[{}/{}] Skipping (reports exist): {}", i + 1, total, album_name);
            skipped += 1;
            continue;
        }

        eprintln!("[{}/{}] Analyzing: {}", i + 1, total, album_name);

        match analyzer::analyze_directory(subdir, jobs) {
            Ok(result) => {
                if write_json {
                    if let Err(e) = cache::save_report(subdir, &result) {
                        eprintln!("  Warning: failed to save JSON report: {}", e);
                    }
                }
                if write_txt {
                    if let Err(e) = cache::save_text_report(subdir, &format::format_table(&result)) {
                        eprintln!("  Warning: failed to save text report: {}", e);
                    }
                }
                analyzed += 1;
            }
            Err(e) => {
                eprintln!("  Warning: failed to analyze: {}", e);
                failed += 1;
            }
        }
    }

    eprintln!(
        "Done: {} analyzed, {} skipped, {} failed (out of {} total)",
        analyzed, skipped, failed, total
    );

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.bulk && cli.tui {
        anyhow::bail!("--bulk and --tui cannot be used together");
    }
    if cli.bulk && !cli.json && !cli.txt {
        anyhow::bail!("--bulk requires at least one output format: --json and/or --txt");
    }

    let jobs = cli.jobs.unwrap_or_else(analyzer::default_jobs);
    let path_str = cli.path.as_deref().unwrap_or(".");

    // STDIN mode
    if path_str == "-" {
        let fmt = cli
            .format
            .as_deref()
            .context("--format is required when reading from STDIN (e.g. --format flac)")?;
        let result = analyzer::analyze_stdin(fmt)?;
        if cli.json {
            println!("{}", format::format_json_single(&result));
        } else {
            println!("{}", format::format_table_single(&result));
        }
        return Ok(());
    }

    let path = Path::new(path_str);

    // Single file mode
    if path.is_file() {
        let result = analyzer::analyze_file(path)?;
        if cli.json {
            println!("{}", format::format_json_single(&result));
        } else {
            println!("{}", format::format_table_single(&result));
        }
        return Ok(());
    }

    // Directory mode
    if path.is_dir() {
        if cli.tui {
            return dr::tui::run(path, jobs, cli.regenerate);
        }

        if cli.bulk {
            return run_bulk(path, jobs, cli.json, cli.txt, cli.regenerate);
        }

        // Check for cached report
        if !cli.regenerate {
            if let Some(cached) = cache::load_cached_report(path) {
                eprintln!("(loaded from cached report)");
                if cli.json {
                    println!("{}", format::format_json(&cached));
                } else {
                    println!("{}", format::format_table(&cached));
                }
                return Ok(());
            }
        }

        let start = Instant::now();
        let result = analyzer::analyze_directory(path, jobs)?;
        let elapsed = start.elapsed();

        // Auto-save cache
        if let Err(e) = cache::save_report(path, &result) {
            eprintln!("Warning: failed to save cache: {}", e);
        }

        if cli.txt {
            if let Err(e) = cache::save_text_report(path, &format::format_table(&result)) {
                eprintln!("Warning: failed to save text report: {}", e);
            }
        }

        if cli.json {
            println!("{}", format::format_json(&result));
        } else {
            println!("{}", format::format_table(&result));
        }

        print_benchmark(&result, elapsed);

        return Ok(());
    }

    anyhow::bail!("Path '{}' is not a file or directory", path_str);
}

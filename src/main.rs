use std::path::Path;

use anyhow::{Context, Result};
use clap::Parser;

use dr::analyzer;
use dr::format;

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
}

fn main() -> Result<()> {
    let cli = Cli::parse();

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
            return dr::tui::run(path, jobs);
        }

        let result = analyzer::analyze_directory(path, jobs)?;
        if cli.json {
            println!("{}", format::format_json(&result));
        } else {
            println!("{}", format::format_table(&result));
        }
        return Ok(());
    }

    anyhow::bail!("Path '{}' is not a file or directory", path_str);
}

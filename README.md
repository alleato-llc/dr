# dr

[![CI](https://github.com/alleato-llc/dr/actions/workflows/ci.yml/badge.svg)](https://github.com/alleato-llc/dr/actions/workflows/ci.yml)
[![GitHub Release](https://img.shields.io/github/v/release/nycjv321/dr)](https://github.com/alleato-llc/dr/releases)
[![License](https://img.shields.io/github/license/nycjv321/dr)](LICENSE)
[![Built with Claude](https://img.shields.io/badge/Built%20with-Claude-blueviolet)](https://claude.ai)

A dynamic range meter for audio files, implementing the Pleasurize Music Foundation / TT DR standard.

## Features

- Measure dynamic range (DR) of individual audio files or entire albums
- Parallel multi-threaded analysis for directories
- Interactive TUI with live progress and color-coded DR values
- Multiple output formats: table, JSON, CSV
- STDIN piping support for integration with other tools
- Pure Rust — no C dependencies

## Installation

### Binary Download

Download pre-built binaries from the [latest release](https://github.com/alleato-llc/dr/releases/latest):

| Platform | Binary |
|----------|--------|
| macOS (Apple Silicon) | `dr-darwin-arm64` |
| Linux (x86_64) | `dr-linux-amd64` |
| Windows (x86_64) | `dr-windows-amd64.exe` |

### Build from Source

```bash
# Clone
git clone https://github.com/alleato-llc/dr.git
cd dr

# Build
cargo build --release

# The binary is at target/release/dr
```

## Usage

### Single File

```bash
dr track.flac
```

```
DR              Peak        RMS   Duration  Track
──────────────────────────────────────────────────────────
DR14        -0.10 dB  -16.78 dB      4:23  Track Title
──────────────────────────────────────────────────────────
Official DR value: DR14
```

### Directory (Album)

```bash
dr ~/Music/Artist/Album/
```

```
DR              Peak        RMS   Duration  Track
──────────────────────────────────────────────────────────
DR14        -0.10 dB  -16.78 dB      4:23  Track One
DR12        -0.30 dB  -14.56 dB      3:45  Track Two
DR15        -0.05 dB  -18.20 dB      5:01  Track Three
──────────────────────────────────────────────────────────
Number of tracks:  3
Official DR value: DR14
```

### STDIN

```bash
cat track.flac | dr - --format flac
```

### Interactive TUI

```bash
dr ~/Music/Artist/Album/ --tui
```

Launches a terminal interface with live analysis progress, scrollable track table, and export dialog.

## Options

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON instead of table |
| `--tui` | Launch interactive TUI |
| `--format <fmt>` | Format hint for STDIN (e.g. flac, mp3, opus) |
| `-j, --jobs <n>` | Number of parallel analysis jobs (default: CPU cores) |

## TUI Keybindings

| Key | Action |
|-----|--------|
| `j` / `Down` | Select next track |
| `k` / `Up` | Select previous track |
| `e` | Open export dialog |
| `a` | Open about dialog |
| `q` | Quit |
| `Tab` | Cycle export format (in export dialog) |
| `Enter` | Save export (in export dialog) |
| `Esc` | Close dialog |

## Algorithm

`dr` implements the TT Dynamic Range measurement standard:

1. Decode audio to f32 samples
2. Split into 3-second non-overlapping blocks
3. Per block, per channel: compute DR-RMS (`sqrt(2 * sum(x²) / N)`) and peak
4. Per channel: quadratic-mean the top 20% of block RMS values; take the 2nd-highest block peak
5. Per channel: `DR = 20 * log10(2nd_peak / top20%_rms)`
6. Final DR = mean of per-channel DR values, rounded

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full algorithm description and design details.

## Testing

```bash
cargo test
```

Tests include unit tests for the DR computation, format output, and audio file scanning, plus integration tests that generate real WAV files and verify DR measurements.

## Dependencies

| Crate | Purpose |
|-------|---------|
| [symphonia](https://crates.io/crates/symphonia) | Audio decoding (pure Rust) |
| [clap](https://crates.io/crates/clap) | CLI argument parsing |
| [ratatui](https://crates.io/crates/ratatui) | Terminal UI framework |
| [crossterm](https://crates.io/crates/crossterm) | Terminal input/output |
| [serde](https://crates.io/crates/serde) / [serde_json](https://crates.io/crates/serde_json) | JSON serialization |
| [anyhow](https://crates.io/crates/anyhow) | Error handling |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[MIT](LICENSE)

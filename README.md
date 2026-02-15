# dr

[![CI](https://github.com/alleato-llc/dr/actions/workflows/ci.yml/badge.svg)](https://github.com/alleato-llc/dr/actions/workflows/ci.yml)
[![GitHub Release](https://img.shields.io/github/v/release/alleato-llc/dr)](https://github.com/alleato-llc/dr/releases)
[![License](https://img.shields.io/github/license/nycjv321/dr)](LICENSE)
[![Built with Claude](https://img.shields.io/badge/Built%20with-Claude-blueviolet)](https://claude.ai)

A dynamic range meter for audio files, implementing the Pleasurize Music Foundation / TT DR standard.

### Preview

#### In Progress
```
┌ DR Meter ───────────────────────────────────────────────────────────────────────────────────────────────────┐
│Album: Unknown Album  Path: /Users/nycjv321/Desktop/McCartney (Hi-res Unlimited Version)                     │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
┌ [1-13/20] ↓ ────────────────────────────────────────────────────────────────────────────────────────────────┐
│#    Track                                               DR                 Peak       RMS        Duration   │
│1    01-Suicide [Out-take].flac                          ███░░░░░░░░░  29%                                 ⟳ │
│2    The Lovely Linda                                    DR11               -3.11dB    -14.40dB   0:46     ✓ │
│3    02-Maybe I'm Amazed [From One Hand Clapping].flac   ██░░░░░░░░░░  17%                                 ⟳ │
│4    02-That Would Be Something.flac                     ███░░░░░░░░░  31%                                 ⟳ │
│5    03-Every Night (Live At Glasgow, 1979).flac         ██░░░░░░░░░░  19%                                 ⟳ │
│6    03-Valentine Day.flac                               ██████░░░░░░  53%                                 ⟳ │
│7    04-Every Night.flac                                 ████░░░░░░░░  37%                                 ⟳ │
│8    04-Hot As Sun (Live At Glasgow, 1979).flac          ████░░░░░░░░  33%                                 ⟳ │
│9    05-Hot As Sun _ Glasses.flac                        █████░░░░░░░  46%                                 ⟳ │
│10   05-Maybe I'm Amazed (Live At Glasgow, 1979).flac    ██░░░░░░░░░░  17%                                 ⟳ │
│11   06-Don't Cry Baby [Out-take].flac                   ███░░░░░░░░░  28%                                 ⟳ │
│12   06-Junk.flac                                        █████░░░░░░░  45%                                 ⟳ │
│13   07-Man We Was Lonely.flac                           ███░░░░░░░░░  31%                                 ⟳ │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                      Overall DR: ~DR11 (1/20 complete)                                      │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
                                           [e]xport  [a]bout  [q]uit
```

#### Calculated Dynamic Range
```
┌ DR Meter ───────────────────────────────────────────────────────────────────────────────────────────────────┐
│Album: McCartney (Hi-res Unlimited Version)  Path: .//McCartney (Hi-res Unlimited Versio│
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
┌ [1-13/20] ↓ ────────────────────────────────────────────────────────────────────────────────────────────────┐
│#    Track                                               DR                 Peak       RMS        Duration   │
│1    Suicide [Out-take]                                  DR13               -3.70dB    -16.94dB   2:50     ✓ │
│2    The Lovely Linda                                    DR11               -3.11dB    -14.40dB   0:46     ✓ │
│3    Maybe I'm Amazed [From One Hand Clapping]           DR11               -2.10dB    -12.12dB   4:54     ✓ │
│4    That Would Be Something                             DR11               -0.67dB    -12.86dB   2:41     ✓ │
│5    Every Night (Live At Glasgow, 1979)                 DR13               -0.10dB    -12.75dB   4:32     ✓ │
│6    Valentine Day                                       DR11               -2.58dB    -14.18dB   1:44     ✓ │
│7    Every Night                                         DR11               -0.11dB    -12.38dB   2:35     ✓ │
│8    Hot As Sun (Live At Glasgow, 1979)                  DR12               -0.16dB    -12.50dB   2:28     ✓ │
│9    Hot As Sun / Glasses                                DR11               -1.81dB    -14.82dB   2:09     ✓ │
│10   Maybe I'm Amazed (Live At Glasgow, 1979)            DR11               -1.04dB    -12.77dB   5:13     ✓ │
│11   Don't Cry Baby [Out-take]                           DR12               -2.30dB    -12.55dB   3:08     ✓ │
│12   Junk                                                DR15               -0.61dB    -14.80dB   1:57     ✓ │
│13   Man We Was Lonely                                   DR10               -2.31dB    -12.69dB   3:00     ✓ │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                      Overall DR: DR12 (20/20 complete)                                      │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
                                           [e]xport  [a]bout  [q]uit
```

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



### Directory (Album)

```bash
dr ~/Music/Artist/Album/
```


### STDIN

```bash
cat track.flac | dr - --format flac
```

### Bulk Mode

Analyze an entire music library at once. Each immediate subdirectory is treated as a separate album:

```bash
dr ~/Music/ --bulk --json --txt
```

This writes `dr_report.json` and/or `dr_report.txt` into each album subdirectory. On subsequent runs, albums with existing reports are automatically skipped:

```
[1/47] Analyzing: Abbey Road
[2/47] Analyzing: Dark Side of the Moon
[3/47] Skipping (reports exist): Kind of Blue
...
Done: 45 analyzed, 1 skipped, 1 failed (out of 47 total)
```

Use `--regenerate` to force re-analysis of all albums.

### Interactive TUI

```bash
dr ~/Music/Artist/Album/ --tui
```

Launches a terminal interface with live analysis progress, scrollable track table, and export dialog.

## Options

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON instead of table |
| `--txt` | Write a text report (`dr_report.txt`) alongside JSON |
| `--tui` | Launch interactive TUI |
| `--bulk` | Analyze all immediate subdirectories as separate albums |
| `--regenerate` | Re-analyze even if cached reports exist |
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

# Architecture

## Module Structure

```
src/
├── main.rs          CLI entry point — argument parsing, mode dispatch
├── lib.rs           Public module exports
├── analyzer.rs      Audio decoding + DR computation engine
├── format.rs        Output formatters (table, JSON, CSV)
├── models.rs        Data types (TrackResult, AlbumResult, AnalysisEvent)
└── tui/
    ├── mod.rs       Terminal setup, event loop, key handling
    ├── app.rs       Application state (App, TrackStatus, View, ExportFormat)
    └── ui.rs        Ratatui widget rendering
```

## Operating Modes

### Single File

```
dr track.flac
```

Decodes the file via symphonia, computes DR, prints result as table or JSON.

### Directory

```
dr ~/Music/Album/
```

Scans for audio files (flac, mp3, wav, ogg, m4a, opus, wv, aif, aiff), analyzes in parallel using a work-stealing thread pool, computes per-track and album DR.

### STDIN

```
cat track.flac | dr - --format flac
```

Reads audio from standard input with a required `--format` hint. Useful for piping from other tools.

### TUI

```
dr ~/Music/Album/ --tui
```

Launches an interactive terminal UI with live progress, track table, export dialog, and about overlay.

## Algorithm: TT Dynamic Range Standard

`dr` implements the Pleasurize Music Foundation / TT Dynamic Range measurement standard. The algorithm operates as follows:

### 1. Decode to f32

Audio is decoded to interleaved f32 samples using symphonia. All codec-specific details are handled by symphonia's format and codec layers.

### 2. Split into 3-second blocks

The sample stream is divided into non-overlapping 3-second blocks. The final partial block (less than 3 seconds) is discarded.

### 3. Per block, per channel: DR-RMS and peak

For each block and each channel independently:

- **DR-RMS** = `sqrt(2 * sum(x²) / N)` where N is the number of frames in the block
- **Peak** = maximum absolute sample value

### 4. Per channel: top 20% RMS and 2nd-highest peak

For each channel across all blocks:

- Sort block RMS values descending, take the top 20% (ceiling), combine via **quadratic mean** (RMS of RMS values)
- Sort block peaks descending, use the **2nd-highest** block peak (falls back to highest if fewer than 2 blocks)

### 5. Per channel DR

```
DR_channel = 20 * log10(2nd_peak / top20%_rms)
```

### 6. Final DR

The final DR value is the **mean of per-channel DR values**, rounded to the nearest integer.

### The sqrt(2) Calibration Factor

Standard RMS of a full-scale sine wave is `1/sqrt(2)` ≈ 0.707, which gives -3.01 dBFS. The DR standard multiplies sum-of-squares by 2 before taking the square root, which calibrates a full-scale sine wave to exactly 0 dB RMS. This means DR-RMS equals peak for a pure sine, cancelling its crest factor and yielding DR0.

### Peak Reporting

Two different peak values are used:

- **For DR computation**: the 2nd-highest block peak per channel (reduces sensitivity to isolated transients)
- **For display (peak_dB)**: the absolute peak across the entire track (what the user expects to see)

### Album DR

Album DR is the **mean of all track DR values**, rounded to the nearest integer.

## References

- "Measuring Dynamic Range — DR standard v3" (Pleasurize Music Foundation)
- [dr14_t.meter](https://github.com/simon-r/dr14_t.meter) — Python reference implementation
- [adiblol/dr_meter](https://github.com/adiblol/dr_meter) — C++ implementation
- [Robhub/TTDR](https://github.com/Robhub/TTDR) — JavaScript implementation

## TUI Architecture

### Data Flow

```
┌─────────────────┐    mpsc     ┌────────────┐    render    ┌──────────┐
│  Worker Threads  │───────────▶│ Event Loop │───────────▶│ Terminal │
│  (analyzer)      │  channel   │  (mod.rs)  │            │ (ratatui)│
└─────────────────┘            └────────────┘            └──────────┘
                                     │
                                     ▼
                                ┌─────────┐
                                │   App   │
                                │ (state) │
                                └─────────┘
```

1. **Worker threads** run `analyze_directory_async`, sending `AnalysisEvent` messages through an `mpsc` channel
2. **Event loop** (`run_loop`) drains the channel each tick, updating `App` state
3. **Key events** are polled via crossterm (100ms timeout) and dispatched based on current `View`
4. **Rendering** calls `ui::render` which reads `App` state and draws ratatui widgets

### App State

- `tracks: Vec<(String, TrackStatus)>` — per-track filename and analysis status
- `album_result: Option<AlbumResult>` — final result after all tracks complete
- `view: View` — current screen (Main, About, Export)
- `selected` / `scroll_offset` — cursor position and virtual scroll
- `export_format` / `export_message` — export dialog state

### Rendering

The UI is composed of four layout sections:

| Section | Widget | Content |
|---------|--------|---------|
| Header | Paragraph | Album title + path |
| Track table | Table | Per-track DR, peak, RMS, duration with scroll |
| Summary | Paragraph | Overall DR + completion count |
| Footer | Paragraph | Keybinding hints |

Overlays (About, Export) render centered over the main layout using `Clear` + `Paragraph`.

DR values are color-coded: green (DR12+), yellow (DR8–11), red (DR0–7).

## Audio Decoding

All audio decoding uses [symphonia](https://github.com/pdeljanov/Symphonia), a pure-Rust audio decoding library. This means:

- No C dependencies (no FFmpeg, libsndfile, etc.)
- Cross-platform without system library requirements
- Supports FLAC, MP3, WAV, OGG/Vorbis, AAC/M4A, Opus, WavPack, AIFF

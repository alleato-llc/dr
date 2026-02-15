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

Audio is decoded to interleaved f32 samples using symphonia. All codec-specific details are handled by symphonia's format and codec layers. Samples are processed in a streaming fashion — each decoded packet is fed directly into `StreamingDrState` without buffering the full track.

### 2. Split into 3-second blocks

The sample stream is divided into non-overlapping 3-second blocks via streaming accumulation. The final partial block (less than 3 seconds) is discarded.

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

## Streaming Analyzer Architecture

The analyzer uses a streaming architecture (`StreamingDrState`) that computes DR statistics block-by-block as samples arrive from the decoder, rather than buffering the entire decoded track in memory.

### Design

```
┌──────────┐   packets   ┌──────────┐  interleaved  ┌──────────────────┐
│ Symphonia │────────────▶│ Sample   │──────────────▶│ StreamingDrState │
│ Decoder   │             │ Buffer   │   samples     │                  │
└──────────┘             └──────────┘               └──────────────────┘
                          (reused)                    accumulates blocks
                                                      ──▶ finalize()
```

### Key Optimizations

**1. Streaming block accumulation**

`StreamingDrState` maintains per-channel accumulators (sum-of-squares and peak) for the current in-progress 3-second block. As interleaved samples are fed via `push_samples()`, they are processed immediately. When a block fills, its RMS and peak are stored and accumulators reset. Only the per-block summary statistics are retained — the raw samples are never stored.

Memory usage is bounded to ~1 block of accumulator state plus 1 packet of residual samples, regardless of track length. For a 5-minute stereo 24-bit/96kHz track, this reduces peak memory from ~230 MB to ~2 MB.

**2. SampleBuffer reuse**

Symphonia's `SampleBuffer<f32>` is allocated once on the first decoded packet and reused across all subsequent packets. A new buffer is only allocated if a later packet requires more capacity than the current buffer. This eliminates per-packet heap allocations in the decode loop.

**3. Inline global peak tracking**

The absolute peak across all samples and all channels is tracked during `push_samples()` as part of the same loop that computes per-block sum-of-squares and peak. This eliminates the need for a separate full-track scan after block computation.

**4. Residual handling**

Decoder packet boundaries don't align with 3-second block boundaries. `StreamingDrState` handles this by accumulating partial frames into the current block's accumulators and tracking how many frames have been accumulated (`current_block_frames`). Sub-frame residuals from packet boundaries are buffered and prepended to the next `push_samples()` call.

### Performance

Benchmarked on a 20-track hi-res album (1215 MB, 24-bit/96kHz FLAC) on an Apple M4 Max (16-core, 128 GB RAM):

| Metric | Before (buffered) | After (streaming) |
|--------|--------------------|--------------------|
| Throughput | ~54 MB/s | ~2400 MB/s |
| Peak memory | ~230 MB per track | ~2 MB per track |
| Allocations/packet | 1 SampleBuffer | 0 (reused) |
| Peak scans | 2 (block + full-track) | 1 (inline) |

At ~2.4 GB/s the analyzer is **memory-bandwidth bound**. Benchmarks with `RUSTFLAGS="-C target-cpu=native"` (enabling AVX/NEON) show no measurable improvement (~1% within run-to-run variance), confirming that the bottleneck is memory throughput, not compute. The compiler's default auto-vectorization is sufficient.

### API

All changes are internal to `src/analyzer.rs`. The public API (`analyze_file`, `analyze_file_with_progress`, `analyze_stdin`, `analyze_directory`, `analyze_directory_async`) and `TrackResult` struct are unchanged.

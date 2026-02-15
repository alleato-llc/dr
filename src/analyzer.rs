use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

use anyhow::{Context, Result};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::{MediaSourceStream, ReadOnlySource};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::models::{AlbumResult, AnalysisEvent, TrackResult};

const AUDIO_EXTENSIONS: &[&str] = &[
    "flac", "mp3", "wav", "ogg", "m4a", "opus", "wv", "aif", "aiff",
];

/// Check if a path has a recognized audio file extension.
fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| AUDIO_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Scan a directory for audio files, sorted by filename.
pub fn scan_audio_files(path: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_audio_file(p))
        .collect();
    files.sort();
    files
}

/// Convert a linear amplitude to dBFS.
fn db_fs(linear: f64) -> f64 {
    if linear <= 0.0 {
        -f64::INFINITY
    } else {
        20.0 * linear.log10()
    }
}

/// Streaming DR state that accumulates statistics block-by-block
/// without buffering the entire track in memory.
struct StreamingDrState {
    channels: usize,
    sample_rate: usize,
    block_frames: usize,
    // Per-channel accumulators for the current in-progress block
    current_block_frames: usize,
    ch_sum_sq: Vec<f64>,
    ch_peak: Vec<f64>,
    // Completed block stats
    block_rms: Vec<Vec<f64>>,
    block_peaks: Vec<Vec<f64>>,
    // Global absolute peak (across all samples, all channels)
    global_peak: f64,
    // Residual buffer for partial frames from packet boundaries
    residual: Vec<f32>,
}

impl StreamingDrState {
    fn new(channels: usize, sample_rate: usize) -> Self {
        Self {
            channels,
            sample_rate,
            block_frames: 3 * sample_rate,
            current_block_frames: 0,
            ch_sum_sq: vec![0.0; channels],
            ch_peak: vec![0.0; channels],
            block_rms: (0..channels).map(|_| Vec::new()).collect(),
            block_peaks: (0..channels).map(|_| Vec::new()).collect(),
            global_peak: 0.0,
            residual: Vec::new(),
        }
    }

    /// Feed interleaved samples into the streaming state.
    /// Processes complete 3-second blocks as they fill up.
    fn push_samples(&mut self, interleaved: &[f32]) {
        let channels = self.channels;
        if channels == 0 {
            return;
        }
        let block_size = self.block_frames * channels;

        // If we have residual samples from the previous packet, prepend them
        let samples: &[f32] = if self.residual.is_empty() {
            interleaved
        } else {
            self.residual.extend_from_slice(interleaved);
            // We'll process from residual; it will be replaced at the end
            &[] // placeholder — handled below
        };

        // Unify: work from a single slice
        let work = if samples.is_empty() {
            // residual was extended above
            // Take ownership of the data for processing
            let data = std::mem::take(&mut self.residual);
            // Process and store leftover back
            self.process_slice(&data, block_size);
            return;
        } else {
            samples
        };

        self.process_slice(work, block_size);
    }

    fn process_slice(&mut self, data: &[f32], block_size: usize) {
        let channels = self.channels;
        let mut offset = 0;

        while offset + block_size <= data.len() + self.current_block_frames * channels {
            // How many more samples do we need to complete the current block?
            let remaining_frames = self.block_frames - self.current_block_frames;
            let remaining_samples = remaining_frames * channels;

            if offset + remaining_samples > data.len() {
                break;
            }

            let chunk = &data[offset..offset + remaining_samples];

            // Accumulate into current block
            for ch in 0..channels {
                let mut sum_sq = self.ch_sum_sq[ch];
                let mut peak = self.ch_peak[ch];
                for &s in chunk.iter().skip(ch).step_by(channels) {
                    let v = s as f64;
                    sum_sq += v * v;
                    let abs_v = v.abs();
                    if abs_v > peak {
                        peak = abs_v;
                    }
                    if abs_v > self.global_peak {
                        self.global_peak = abs_v;
                    }
                }
                self.ch_sum_sq[ch] = sum_sq;
                self.ch_peak[ch] = peak;
            }

            self.current_block_frames = self.block_frames;

            // Block complete — store RMS and peak, reset accumulators
            let block_frames = self.block_frames as f64;
            for ch in 0..channels {
                let rms = (2.0 * self.ch_sum_sq[ch] / block_frames).sqrt();
                self.block_rms[ch].push(rms);
                self.block_peaks[ch].push(self.ch_peak[ch]);
            }
            self.ch_sum_sq.iter_mut().for_each(|v| *v = 0.0);
            self.ch_peak.iter_mut().for_each(|v| *v = 0.0);
            self.current_block_frames = 0;

            offset += remaining_samples;
        }

        // Process leftover partial block samples (update accumulators but don't finalize)
        let leftover = &data[offset..];
        if !leftover.is_empty() {
            let leftover_frames = leftover.len() / channels;
            for ch in 0..channels {
                let mut sum_sq = self.ch_sum_sq[ch];
                let mut peak = self.ch_peak[ch];
                for &s in leftover.iter().skip(ch).step_by(channels) {
                    let v = s as f64;
                    sum_sq += v * v;
                    let abs_v = v.abs();
                    if abs_v > peak {
                        peak = abs_v;
                    }
                    if abs_v > self.global_peak {
                        self.global_peak = abs_v;
                    }
                }
                self.ch_sum_sq[ch] = sum_sq;
                self.ch_peak[ch] = peak;
            }
            self.current_block_frames += leftover_frames;
        }

        // Store any sub-frame residual (shouldn't happen with well-formed data,
        // but be safe)
        let consumed = offset + (leftover.len() / channels) * channels;
        if consumed < data.len() {
            self.residual = data[consumed..].to_vec();
        } else {
            self.residual.clear();
        }
    }

    /// Finalize and compute DR stats. Discards any partial final block
    /// per the TT DR standard.
    fn finalize(self, total_frames: usize) -> (u32, f64, f64, f64) {
        let channels = self.channels;
        let duration_secs = total_frames as f64 / self.sample_rate as f64;

        let num_blocks = if channels > 0 {
            self.block_rms[0].len()
        } else {
            0
        };

        if num_blocks == 0 || channels == 0 {
            return (0, db_fs(self.global_peak), -f64::INFINITY, duration_secs);
        }

        let mut channel_drs: Vec<f64> = Vec::with_capacity(channels);
        let mut report_rms = 0.0f64;

        for ch in 0..channels {
            let mut ch_rms: Vec<f64> = self.block_rms[ch].clone();
            let mut ch_peaks: Vec<f64> = self.block_peaks[ch].clone();

            // Sort RMS descending
            ch_rms.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

            // Top 20% RMS — combine via quadratic mean (RMS of RMS values)
            let top_count = ((num_blocks as f64 * 0.2).ceil() as usize).max(1);
            let sum_sq: f64 = ch_rms.iter().take(top_count).map(|v| v * v).sum();
            let combined_rms = (sum_sq / top_count as f64).sqrt();

            // Sort peaks descending, use 2nd-highest (fall back to highest if < 2 blocks)
            ch_peaks.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
            let peak = if ch_peaks.len() >= 2 {
                ch_peaks[1]
            } else {
                ch_peaks[0]
            };

            // Per-channel DR
            if peak > 0.0 && combined_rms > 0.0 {
                channel_drs.push(20.0 * (peak / combined_rms).log10());
            } else {
                channel_drs.push(0.0);
            }

            if combined_rms > report_rms {
                report_rms = combined_rms;
            }
        }

        // Final DR = mean of per-channel DR values, rounded
        let dr = if channel_drs.is_empty() {
            0
        } else {
            let mean_dr: f64 = channel_drs.iter().sum::<f64>() / channel_drs.len() as f64;
            mean_dr.round() as u32
        };

        (dr, db_fs(self.global_peak), db_fs(report_rms), duration_secs)
    }
}

#[cfg(test)]
/// Per-channel data for a single 3-second block.
struct BlockStats {
    /// DR-RMS per channel: sqrt(2 * sum(x²) / N)
    rms: Vec<f64>,
    /// Peak (max absolute sample) per channel
    peak: Vec<f64>,
}

#[cfg(test)]
/// Compute per-channel DR-RMS and peak for a block of interleaved samples.
/// DR-RMS uses the sqrt(2) factor per the Pleasurize Music / TT DR standard,
/// which calibrates a full-scale sine wave to 0 dB RMS.
fn compute_block_stats(samples: &[f32], channels: usize) -> BlockStats {
    let frames = samples.len() / channels;
    let mut rms = vec![0.0f64; channels];
    let mut peak = vec![0.0f64; channels];

    for ch in 0..channels {
        let mut sum_sq = 0.0f64;
        let mut ch_peak = 0.0f64;
        for &s in samples.iter().skip(ch).step_by(channels) {
            let v = s as f64;
            sum_sq += v * v;
            let abs_v = v.abs();
            if abs_v > ch_peak {
                ch_peak = abs_v;
            }
        }
        // DR-RMS: sqrt(2) factor calibrates sine peak = sine RMS
        rms[ch] = (2.0 * sum_sq / frames as f64).sqrt();
        peak[ch] = ch_peak;
    }

    BlockStats { rms, peak }
}

#[cfg(test)]
/// Compute DR, peak_dB, and rms_dB from decoded interleaved samples.
///
/// Per the TT DR standard, everything is computed independently per channel:
/// 1. Split into 3-second blocks, discard final partial block
/// 2. Per block per channel: compute DR-RMS (with sqrt(2)) and peak
/// 3. Per channel: sort RMS descending, quadratic-mean the top 20%
/// 4. Per channel: sort peaks descending, use the 2nd-highest
/// 5. Per channel: DR_ch = 20 * log10(2nd_peak / top20%_rms)
/// 6. Final DR = mean of per-channel DR values, rounded
fn compute_dr(all_samples: &[f32], channels: usize, sample_rate: usize) -> (u32, f64, f64) {
    let block_frames = 3 * sample_rate;
    let block_size = block_frames * channels;

    if all_samples.len() < block_size || channels == 0 {
        // Not enough data for even one full block
        let peak: f64 = all_samples
            .iter()
            .map(|&s| (s as f64).abs())
            .fold(0.0_f64, f64::max);
        return (0, db_fs(peak), -f64::INFINITY);
    }

    // Gather per-block stats
    let blocks: Vec<BlockStats> = all_samples
        .chunks_exact(block_size)
        .map(|chunk| compute_block_stats(chunk, channels))
        .collect();

    let num_blocks = blocks.len();
    let mut channel_drs: Vec<f64> = Vec::with_capacity(channels);
    // Track overall peak and RMS for reporting (take the max across channels)
    let mut report_peak = 0.0f64;
    let mut report_rms = 0.0f64;

    for ch in 0..channels {
        // Collect per-block RMS and peak for this channel
        let mut ch_rms: Vec<f64> = blocks.iter().map(|b| b.rms[ch]).collect();
        let mut ch_peaks: Vec<f64> = blocks.iter().map(|b| b.peak[ch]).collect();

        // Sort RMS descending
        ch_rms.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

        // Top 20% RMS — combine via quadratic mean (RMS of RMS values)
        let top_count = ((num_blocks as f64 * 0.2).ceil() as usize).max(1);
        let sum_sq: f64 = ch_rms.iter().take(top_count).map(|v| v * v).sum();
        let combined_rms = (sum_sq / top_count as f64).sqrt();

        // Sort peaks descending, use 2nd-highest (fall back to highest if < 2 blocks)
        ch_peaks.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        let peak = if ch_peaks.len() >= 2 {
            ch_peaks[1]
        } else {
            ch_peaks[0]
        };

        // Per-channel DR
        if peak > 0.0 && combined_rms > 0.0 {
            channel_drs.push(20.0 * (peak / combined_rms).log10());
        } else {
            channel_drs.push(0.0);
        }

        // Track reporting values (max across channels)
        if peak > report_peak {
            report_peak = peak;
        }
        if combined_rms > report_rms {
            report_rms = combined_rms;
        }
    }

    // Final DR = mean of per-channel DR values, rounded
    let dr = if channel_drs.is_empty() {
        0
    } else {
        let mean_dr: f64 = channel_drs.iter().sum::<f64>() / channel_drs.len() as f64;
        mean_dr.round() as u32
    };

    // For peak_dB reporting, use the actual absolute peak across entire track
    // (not the 2nd-highest block peak, which is only for DR computation)
    let absolute_peak: f64 = all_samples
        .iter()
        .map(|&s| (s as f64).abs())
        .fold(0.0_f64, f64::max);

    (dr, db_fs(absolute_peak), db_fs(report_rms))
}

/// Extract a track title from metadata, falling back to filename stem.
fn extract_title(format: &mut dyn FormatReader, path: &Path) -> String {
    // Try metadata from the format reader
    if let Some(metadata) = format.metadata().current() {
        for tag in metadata.tags() {
            if tag.std_key == Some(symphonia::core::meta::StandardTagKey::TrackTitle) {
                return tag.value.to_string();
            }
        }
    }
    // Fall back to filename stem
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string()
}

/// Extract album name from metadata.
fn extract_album(format: &mut dyn FormatReader) -> Option<String> {
    if let Some(metadata) = format.metadata().current() {
        for tag in metadata.tags() {
            if tag.std_key == Some(symphonia::core::meta::StandardTagKey::Album) {
                return Some(tag.value.to_string());
            }
        }
    }
    None
}

/// Analyze a single audio file and return its DR measurement.
pub fn analyze_file(path: &Path) -> Result<TrackResult> {
    analyze_file_with_progress(path, |_| {})
}

/// Analyze a single audio file, reporting progress via callback.
pub fn analyze_file_with_progress(
    path: &Path,
    on_progress: impl Fn(f32),
) -> Result<TrackResult> {
    let file = File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let file_size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .with_context(|| format!("Failed to probe {}", path.display()))?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .context("No audio track found")?;

    let codec_params = track.codec_params.clone();
    let track_id = track.id;
    let sample_rate = codec_params.sample_rate.unwrap_or(44100) as usize;
    let channels = codec_params
        .channels
        .map(|c| c.count())
        .unwrap_or(2);

    let title = extract_title(format.as_mut(), path);
    let album_name = extract_album(format.as_mut());
    let _ = album_name; // album is used at directory level

    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .context("Failed to create decoder")?;

    let mut state = StreamingDrState::new(channels, sample_rate);
    let mut bytes_decoded: u64 = 0;
    let mut sample_buf: Option<SampleBuffer<f32>> = None;
    let mut sample_buf_capacity: u64 = 0;
    let mut total_frames: usize = 0;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(symphonia::core::errors::Error::ResetRequired) => {
                break;
            }
            Err(e) => return Err(e.into()),
        };

        if packet.track_id() != track_id {
            continue;
        }

        bytes_decoded += packet.data.len() as u64;
        if file_size > 0 {
            on_progress((bytes_decoded as f32 / file_size as f32).min(1.0));
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(e) => return Err(e.into()),
        };

        let spec = *decoded.spec();
        let num_frames = decoded.frames() as u64;
        total_frames += num_frames as usize;

        // Reuse SampleBuffer across packets; only reallocate if capacity is insufficient
        let buf = if let Some(ref mut buf) = sample_buf {
            if sample_buf_capacity < num_frames {
                *buf = SampleBuffer::new(num_frames, spec);
                sample_buf_capacity = num_frames;
            }
            buf
        } else {
            sample_buf = Some(SampleBuffer::new(num_frames, spec));
            sample_buf_capacity = num_frames;
            sample_buf.as_mut().unwrap()
        };

        buf.copy_interleaved_ref(decoded);
        state.push_samples(buf.samples());
    }

    on_progress(1.0);

    let (dr_value, peak_db, rms_db, duration_secs) = state.finalize(total_frames);

    let filename = path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(TrackResult {
        dr: dr_value,
        peak_db,
        rms_db,
        duration_secs,
        title,
        filename,
        file_bytes: file_size,
    })
}

/// Analyze audio from STDIN with a format hint.
pub fn analyze_stdin(format_hint: &str) -> Result<TrackResult> {
    let stdin = std::io::stdin();
    let source = ReadOnlySource::new(stdin);
    let mss = MediaSourceStream::new(Box::new(source), Default::default());

    let mut hint = Hint::new();
    hint.with_extension(format_hint);

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .context("Failed to probe STDIN stream")?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .context("No audio track found in STDIN")?;

    let codec_params = track.codec_params.clone();
    let track_id = track.id;
    let sample_rate = codec_params.sample_rate.unwrap_or(44100) as usize;
    let channels = codec_params.channels.map(|c| c.count()).unwrap_or(2);

    let title = "STDIN".to_string();

    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .context("Failed to create decoder")?;

    let mut state = StreamingDrState::new(channels, sample_rate);
    let mut sample_buf: Option<SampleBuffer<f32>> = None;
    let mut sample_buf_capacity: u64 = 0;
    let mut total_frames: usize = 0;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(symphonia::core::errors::Error::ResetRequired) => break,
            Err(e) => return Err(e.into()),
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(e) => return Err(e.into()),
        };

        let spec = *decoded.spec();
        let num_frames = decoded.frames() as u64;
        total_frames += num_frames as usize;

        let buf = if let Some(ref mut buf) = sample_buf {
            if sample_buf_capacity < num_frames {
                *buf = SampleBuffer::new(num_frames, spec);
                sample_buf_capacity = num_frames;
            }
            buf
        } else {
            sample_buf = Some(SampleBuffer::new(num_frames, spec));
            sample_buf_capacity = num_frames;
            sample_buf.as_mut().unwrap()
        };

        buf.copy_interleaved_ref(decoded);
        state.push_samples(buf.samples());
    }

    let (dr_value, peak_db, rms_db, duration_secs) = state.finalize(total_frames);

    Ok(TrackResult {
        dr: dr_value,
        peak_db,
        rms_db,
        duration_secs,
        title,
        filename: "STDIN".to_string(),
        file_bytes: 0,
    })
}

/// Return the default number of parallel jobs (number of CPU cores).
pub fn default_jobs() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// Analyze all audio files in a directory in parallel.
pub fn analyze_directory(path: &Path, jobs: usize) -> Result<AlbumResult> {
    let files = scan_audio_files(path);
    if files.is_empty() {
        anyhow::bail!("No audio files found in {}", path.display());
    }

    let jobs = jobs.max(1);
    let files = Arc::new(files);
    let next_index = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    for _ in 0..jobs.min(files.len()) {
        let files = Arc::clone(&files);
        let next_index = Arc::clone(&next_index);
        handles.push(std::thread::spawn(move || {
            let mut results = Vec::new();
            loop {
                let idx = next_index.fetch_add(1, Ordering::SeqCst);
                if idx >= files.len() {
                    break;
                }
                match analyze_file(&files[idx]) {
                    Ok(result) => results.push((idx, Ok(result))),
                    Err(e) => results.push((idx, Err(e))),
                }
            }
            results
        }));
    }

    // Collect results from all threads and sort by original index
    let mut indexed_results: Vec<(usize, Result<TrackResult>)> = Vec::new();
    for handle in handles {
        indexed_results.extend(handle.join().unwrap());
    }
    indexed_results.sort_by_key(|(idx, _)| *idx);

    let mut tracks = Vec::with_capacity(indexed_results.len());
    for (_, result) in indexed_results {
        tracks.push(result?);
    }

    let album_name = extract_album_from_file(files.first().unwrap());

    let overall_dr = if tracks.is_empty() {
        0
    } else {
        let sum: f64 = tracks.iter().map(|t| t.dr as f64).sum();
        (sum / tracks.len() as f64).round() as u32
    };

    Ok(AlbumResult {
        tracks,
        overall_dr,
        album: album_name,
    })
}

/// Analyze a directory in parallel, sending progress events through a channel for TUI use.
pub fn analyze_directory_async(
    path: &Path,
    sender: Sender<AnalysisEvent>,
    jobs: usize,
) -> Result<()> {
    let files = scan_audio_files(path);
    if files.is_empty() {
        anyhow::bail!("No audio files found in {}", path.display());
    }

    let jobs = jobs.max(1);
    let files = Arc::new(files);
    let next_index = Arc::new(AtomicUsize::new(0));
    let sender = Arc::new(sender);

    let mut handles = Vec::new();
    for _ in 0..jobs.min(files.len()) {
        let files = Arc::clone(&files);
        let next_index = Arc::clone(&next_index);
        let sender = Arc::clone(&sender);
        handles.push(std::thread::spawn(move || {
            let mut results = Vec::new();
            loop {
                let index = next_index.fetch_add(1, Ordering::SeqCst);
                if index >= files.len() {
                    break;
                }
                let _ = sender.send(AnalysisEvent::TrackStarted { index });
                let sender_ref = &sender;
                match analyze_file_with_progress(&files[index], |percent| {
                    let _ = sender_ref.send(AnalysisEvent::TrackProgress { index, percent });
                }) {
                    Ok(result) => {
                        let _ = sender.send(AnalysisEvent::TrackCompleted {
                            index,
                            result: result.clone(),
                        });
                        results.push(result);
                    }
                    Err(e) => {
                        let _ = sender.send(AnalysisEvent::Error {
                            index,
                            message: e.to_string(),
                        });
                    }
                }
            }
            results
        }));
    }

    let mut tracks: Vec<TrackResult> = Vec::new();
    for handle in handles {
        tracks.extend(handle.join().unwrap());
    }

    let album_name = extract_album_from_file(files.first().unwrap());

    let overall_dr = if tracks.is_empty() {
        0
    } else {
        let sum: f64 = tracks.iter().map(|t| t.dr as f64).sum();
        (sum / tracks.len() as f64).round() as u32
    };

    let _ = sender.send(AnalysisEvent::AlbumCompleted {
        result: AlbumResult {
            tracks,
            overall_dr,
            album: album_name,
        },
    });

    Ok(())
}

/// Extract album name by probing the first file's metadata.
fn extract_album_from_file(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .ok()?;
    extract_album(probed.format.as_mut())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_audio_file() {
        assert!(is_audio_file(Path::new("track.flac")));
        assert!(is_audio_file(Path::new("track.MP3")));
        assert!(is_audio_file(Path::new("track.wav")));
        assert!(is_audio_file(Path::new("track.ogg")));
        assert!(is_audio_file(Path::new("track.m4a")));
        assert!(!is_audio_file(Path::new("readme.txt")));
        assert!(!is_audio_file(Path::new("image.png")));
    }

    #[test]
    fn test_db_fs() {
        assert!((db_fs(1.0) - 0.0).abs() < 0.001);
        assert!((db_fs(0.5) - (-6.0206)).abs() < 0.01);
        assert!(db_fs(0.0).is_infinite());
    }

    #[test]
    fn test_compute_block_stats() {
        // Mono silence — RMS should be 0, peak should be 0
        let samples = vec![0.0f32; 44100];
        let stats = compute_block_stats(&samples, 1);
        assert_eq!(stats.rms[0], 0.0);
        assert_eq!(stats.peak[0], 0.0);

        // Mono constant value 0.5:
        // DR-RMS = sqrt(2 * sum(0.25) / N) = sqrt(2 * 0.25) = sqrt(0.5) ≈ 0.7071
        let samples = vec![0.5f32; 44100];
        let stats = compute_block_stats(&samples, 1);
        let expected_rms = (2.0 * 0.25_f64).sqrt(); // sqrt(0.5) ≈ 0.7071
        assert!((stats.rms[0] - expected_rms).abs() < 0.001,
            "DR-RMS of constant 0.5 should be {:.4}, got {:.4}", expected_rms, stats.rms[0]);
        assert!((stats.peak[0] - 0.5).abs() < 0.001);

        // Stereo: left=0.8 right=0.2 — channels computed independently
        let mut stereo = Vec::new();
        for _ in 0..44100 {
            stereo.push(0.8f32);
            stereo.push(0.2f32);
        }
        let stats = compute_block_stats(&stereo, 2);
        let expected_left = (2.0 * 0.64_f64).sqrt();  // sqrt(1.28) ≈ 1.1314
        let expected_right = (2.0 * 0.04_f64).sqrt(); // sqrt(0.08) ≈ 0.2828
        assert!((stats.rms[0] - expected_left).abs() < 0.001,
            "Left channel DR-RMS should be {:.4}, got {:.4}", expected_left, stats.rms[0]);
        assert!((stats.rms[1] - expected_right).abs() < 0.001,
            "Right channel DR-RMS should be {:.4}, got {:.4}", expected_right, stats.rms[1]);
        assert!((stats.peak[0] - 0.8).abs() < 0.001);
        assert!((stats.peak[1] - 0.2).abs() < 0.001);
    }

    #[test]
    fn test_compute_dr_sine_wave() {
        // A full-scale sine wave: with the sqrt(2) calibration, DR-RMS equals peak
        // so DR should be ~0 (the crest factor is cancelled out by the sqrt(2) factor)
        let sample_rate = 44100;
        let duration = 12.0;
        let num_samples = (sample_rate as f64 * duration) as usize;
        let mut samples = Vec::with_capacity(num_samples);
        for i in 0..num_samples {
            let t = i as f64 / sample_rate as f64;
            samples.push((2.0 * std::f64::consts::PI * 440.0 * t).sin() as f32);
        }
        let (dr, peak_db, rms_db) = compute_dr(&samples, 1, sample_rate);
        // Pure sine with sqrt(2) calibration: DR should be 0
        assert!(dr <= 1, "Pure sine DR should be ~0 with sqrt(2) calibration, got DR{}", dr);
        assert!(peak_db > -0.1, "Peak should be near 0 dBFS, got {:.2}", peak_db);
        // RMS should also be near 0 dBFS due to sqrt(2) factor
        assert!(rms_db > -1.0, "DR-RMS of sine should be near 0 dBFS, got {:.2}", rms_db);
    }
}

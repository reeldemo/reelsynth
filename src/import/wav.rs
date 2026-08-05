//! WAV import: single-cycle folders and Serum-style multicycle tables.

use crate::wavetable::{WavetableBank, DEFAULT_FRAME_SIZE, DEFAULT_NUM_FRAMES};
use std::path::Path;

/// Preferred cycle lengths when slicing a concatenated multicycle WAV (Serum-style).
const FRAME_SIZE_CANDIDATES: &[usize] = &[2048, 1024, 4096, 512, 256];

pub fn import_wav_folder(path: &str) -> Result<WavetableBank, String> {
    let dir = Path::new(path);
    if !dir.is_dir() {
        return Err(format!("not a directory: {path}"));
    }
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|x| x.to_str())
                .map(|x| x.eq_ignore_ascii_case("wav"))
                .unwrap_or(false)
        })
        .collect();
    files.sort();
    if files.is_empty() {
        return Err("no .wav files in folder".into());
    }

    let num_frames = files.len().min(DEFAULT_NUM_FRAMES);
    let mut bank = WavetableBank::new(num_frames, DEFAULT_FRAME_SIZE);

    for (fi, file) in files.iter().take(num_frames).enumerate() {
        let cycle = decode_wav_mono(file)?;
        if !cycle.is_empty() {
            bank.set_frame_from_cycle(fi, &cycle);
        }
    }
    Ok(bank)
}

/// Import a Serum / Echo Sound Works style multicycle WAV (frames concatenated in one file).
///
/// Frame size is inferred (prefers 2048). More than [`DEFAULT_NUM_FRAMES`] frames are
/// subsampled evenly down to that cap. Non-multiple lengths are treated as a single cycle
/// and resampled into one [`DEFAULT_FRAME_SIZE`] frame.
pub fn import_wav_multicycle(path: &str) -> Result<WavetableBank, String> {
    let file = Path::new(path);
    if !file.is_file() {
        return Err(format!("not a WAV file: {path}"));
    }
    let samples = decode_wav_mono(file)?;
    if samples.is_empty() {
        return Err("empty WAV".into());
    }

    let Some(frame_size) = infer_frame_size(samples.len()) else {
        let mut bank = WavetableBank::new(1, DEFAULT_FRAME_SIZE);
        bank.set_frame_from_cycle(0, &samples);
        return Ok(bank);
    };

    let raw_frames = samples.len() / frame_size;
    if raw_frames == 0 {
        return Err("WAV shorter than one wavetable frame".into());
    }

    if raw_frames <= DEFAULT_NUM_FRAMES {
        let flat = samples[..raw_frames * frame_size].to_vec();
        return WavetableBank::from_flat(raw_frames, frame_size, flat);
    }

    // Subsample along the frame axis so morph still spans the table.
    let mut bank = WavetableBank::new(DEFAULT_NUM_FRAMES, frame_size);
    let denom = (DEFAULT_NUM_FRAMES - 1).max(1);
    for fi in 0..DEFAULT_NUM_FRAMES {
        let src = fi * (raw_frames - 1) / denom;
        let start = src * frame_size;
        bank.frame_mut(fi)
            .copy_from_slice(&samples[start..start + frame_size]);
    }
    Ok(bank)
}

fn infer_frame_size(num_samples: usize) -> Option<usize> {
    for &fs in FRAME_SIZE_CANDIDATES {
        if num_samples % fs == 0 {
            let frames = num_samples / fs;
            if (1..=2048).contains(&frames) {
                return Some(fs);
            }
        }
    }
    None
}

fn decode_wav_mono(path: &Path) -> Result<Vec<f32>, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    if data.len() < 44 {
        return Err("truncated wav".into());
    }
    if &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err("not a WAV file".into());
    }
    let mut offset = 12usize;
    let mut sample_rate = 44100u32;
    let mut channels = 1u16;
    let mut bits = 16u16;
    let mut format_tag = 1u16; // 1 = PCM, 3 = IEEE float
    let mut pcm_offset = 0usize;
    let mut pcm_len = 0usize;

    while offset + 8 <= data.len() {
        let chunk_id = &data[offset..offset + 4];
        let chunk_size = u32::from_le_bytes([
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]) as usize;
        let chunk_data = offset + 8;
        if chunk_id == b"fmt " && chunk_size >= 16 && chunk_data + 16 <= data.len() {
            format_tag = u16::from_le_bytes([data[chunk_data], data[chunk_data + 1]]);
            channels = u16::from_le_bytes([data[chunk_data + 2], data[chunk_data + 3]]);
            sample_rate = u32::from_le_bytes([
                data[chunk_data + 4],
                data[chunk_data + 5],
                data[chunk_data + 6],
                data[chunk_data + 7],
            ]);
            bits = u16::from_le_bytes([data[chunk_data + 14], data[chunk_data + 15]]);
        } else if chunk_id == b"data" {
            pcm_offset = chunk_data;
            pcm_len = chunk_size.min(data.len().saturating_sub(chunk_data));
            break;
        }
        offset = chunk_data + chunk_size + (chunk_size % 2);
    }

    if pcm_len == 0 {
        return Err("no PCM data in wav".into());
    }
    let bytes_per_sample = (bits / 8) as usize;
    if bytes_per_sample == 0 || channels == 0 {
        return Err("invalid WAV format".into());
    }
    let frame_bytes = bytes_per_sample * channels as usize;
    let mut out = Vec::new();
    let mut i = pcm_offset;
    while i + frame_bytes <= pcm_offset + pcm_len {
        let sample = decode_interleaved_sample(&data[i..i + frame_bytes], channels, bits, format_tag);
        out.push(sample);
        i += frame_bytes;
    }
    let _ = sample_rate;
    Ok(out)
}

fn decode_interleaved_sample(frame: &[u8], channels: u16, bits: u16, format_tag: u16) -> f32 {
    let ch = channels as usize;
    let bps = (bits / 8) as usize;
    let mut sum = 0.0f32;
    for c in 0..ch {
        let o = c * bps;
        let s = if format_tag == 3 && bits == 32 {
            f32::from_le_bytes([frame[o], frame[o + 1], frame[o + 2], frame[o + 3]])
        } else if bits == 16 {
            i16::from_le_bytes([frame[o], frame[o + 1]]) as f32 / 32768.0
        } else if bits == 32 {
            // 32-bit integer PCM
            i32::from_le_bytes([frame[o], frame[o + 1], frame[o + 2], frame[o + 3]]) as f32
                / 2147483648.0
        } else if bits == 24 && o + 2 < frame.len() {
            let v = i32::from_le_bytes([frame[o], frame[o + 1], frame[o + 2], 0]) >> 8;
            v as f32 / 8388608.0
        } else {
            0.0
        };
        sum += s;
    }
    sum / ch as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    fn write_float32_mono_wav(path: &Path, samples: &[f32], sample_rate: u32) {
        let data_len = (samples.len() * 4) as u32;
        let mut out = Vec::with_capacity(44 + samples.len() * 4);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&(sample_rate * 4).to_le_bytes());
        out.extend_from_slice(&4u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for &s in samples {
            out.extend_from_slice(&s.to_le_bytes());
        }
        std::fs::write(path, out).unwrap();
    }

    #[test]
    fn rejects_empty_dir() {
        let dir = std::env::temp_dir().join("reelsynth_empty_wav");
        let _ = std::fs::create_dir_all(&dir);
        assert!(import_wav_folder(dir.to_str().unwrap()).is_err());
    }

    #[test]
    fn import_multicycle_serum_style() {
        let n_frames = 8usize;
        let frame_size = DEFAULT_FRAME_SIZE;
        let mut samples = Vec::with_capacity(n_frames * frame_size);
        for fi in 0..n_frames {
            for i in 0..frame_size {
                let phase = i as f32 / frame_size as f32;
                samples.push(((fi + 1) as f32 * 0.1) * (phase * TAU).sin());
            }
        }
        let path = std::env::temp_dir().join("reelsynth_mc_import.wav");
        write_float32_mono_wav(&path, &samples, 44100);

        let bank = import_wav_multicycle(path.to_str().unwrap()).expect("import multicycle");
        assert_eq!(bank.num_frames, n_frames);
        assert_eq!(bank.frame_size, frame_size);
        for fi in 0..n_frames {
            let frame = bank.frame(fi);
            let mid = frame_size / 4;
            let expected_mid =
                ((fi + 1) as f32 * 0.1) * ((mid as f32 / frame_size as f32) * TAU).sin();
            assert!(
                (frame[mid] - expected_mid).abs() < 1e-5,
                "frame {fi} mid mismatch"
            );
        }
    }

    #[test]
    fn import_multicycle_rejects_missing_file() {
        assert!(import_wav_multicycle("/no/such/table.wav").is_err());
    }

    #[test]
    fn import_multicycle_two_frame_table() {
        let frame_size = DEFAULT_FRAME_SIZE;
        let mut samples = vec![0.0f32; 2 * frame_size];
        samples[..frame_size].fill(0.5);
        samples[frame_size..].fill(-0.5);
        let path = std::env::temp_dir().join("reelsynth_mc_two.wav");
        write_float32_mono_wav(&path, &samples, 44100);
        let bank = import_wav_multicycle(path.to_str().unwrap()).unwrap();
        assert_eq!(bank.num_frames, 2);
        assert!((bank.frame(0)[0] - 0.5).abs() < 1e-6);
        assert!((bank.frame(1)[0] + 0.5).abs() < 1e-6);
    }

    #[test]
    fn import_esw_core_tables_if_present() {
        // Set REELSYNTH_ESW_TABLES to the pack root (folder containing Analog/, Basics/, …).
        let root = std::env::var_os("REELSYNTH_ESW_TABLES")
            .map(std::path::PathBuf::from)
            .filter(|p| p.is_dir());
        let Some(root) = root else {
            eprintln!("skip: set REELSYNTH_ESW_TABLES to run Echo Sound Works smoke test");
            return;
        };
        let cases = [
            ("Analog/ESW Analog - 303 Saw.wav", 256, 2048),
            ("Analog/ESW Analog - ARP Pulse.wav", 2, 2048),
        ];
        for (rel, expect_frames, expect_size) in cases {
            let path = root.join(rel);
            let bank = import_wav_multicycle(path.to_str().unwrap())
                .unwrap_or_else(|e| panic!("import {rel}: {e}"));
            assert_eq!(bank.num_frames, expect_frames, "{rel} frames");
            assert_eq!(bank.frame_size, expect_size, "{rel} frame_size");

            let out = std::env::temp_dir().join(format!(
                "reelsynth_esw_{}.reelwt",
                path.file_stem().unwrap().to_string_lossy().replace(' ', "_")
            ));
            bank.write_file(out.to_str().unwrap()).unwrap();
            let reloaded = WavetableBank::read_file(out.to_str().unwrap()).unwrap();
            assert_eq!(reloaded.num_frames, bank.num_frames);

            let wav_out = out.with_extension("wav");
            let report = crate::export_wav_multicycle(&bank, &wav_out);
            assert!(report.success, "{rel} export: {:?}", report.errors);
            let round = import_wav_multicycle(wav_out.to_str().unwrap()).unwrap();
            assert_eq!(round.num_frames, bank.num_frames);
            let mut max_err = 0.0f32;
            for (a, b) in bank.frames.iter().zip(round.frames.iter()) {
                max_err = max_err.max((a - b).abs());
            }
            assert!(max_err < 1e-5, "{rel} round-trip max_err={max_err}");
        }

        // Spot-check: every category folder has at least one importable table.
        let mut imported = 0usize;
        for entry in std::fs::read_dir(&root).unwrap().filter_map(|e| e.ok()) {
            if !entry.path().is_dir() {
                continue;
            }
            let Some(wav) = std::fs::read_dir(entry.path())
                .ok()
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .find(|p| {
                    p.extension()
                        .and_then(|x| x.to_str())
                        .map(|x| x.eq_ignore_ascii_case("wav"))
                        .unwrap_or(false)
                })
            else {
                continue;
            };
            let bank = import_wav_multicycle(wav.to_str().unwrap())
                .unwrap_or_else(|e| panic!("import {}: {e}", wav.display()));
            assert!(bank.num_frames >= 1, "{}", wav.display());
            assert!(bank.frame_size >= 256, "{}", wav.display());
            imported += 1;
        }
        assert!(imported >= 8, "expected category coverage, got {imported}");
    }
}

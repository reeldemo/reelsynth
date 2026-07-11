//! WAV single-cycle folder import.

use crate::wavetable::{WavetableBank, DEFAULT_FRAME_SIZE, DEFAULT_NUM_FRAMES};
use std::path::Path;

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

fn decode_wav_mono(path: &Path) -> Result<Vec<f32>, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    if data.len() < 44 {
        return Err("truncated wav".into());
    }
    // Minimal RIFF/WAV parser (PCM 16-bit mono/stereo)
    if &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err("not a WAV file".into());
    }
    let mut offset = 12usize;
    let mut sample_rate = 44100u32;
    let mut channels = 1u16;
    let mut bits = 16u16;
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
    let frame_bytes = bytes_per_sample * channels as usize;
    let mut out = Vec::new();
    let mut i = pcm_offset;
    while i + frame_bytes <= pcm_offset + pcm_len {
        let sample = if bits == 16 {
            let v = i16::from_le_bytes([data[i], data[i + 1]]) as f32 / 32768.0;
            if channels > 1 {
                let v2 = i16::from_le_bytes([data[i + 2], data[i + 3]]) as f32 / 32768.0;
                (v + v2) * 0.5
            } else {
                v
            }
        } else if bits == 32 {
            f32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]])
        } else {
            0.0
        };
        out.push(sample);
        i += frame_bytes;
    }
    let _ = sample_rate;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_dir() {
        let dir = std::env::temp_dir().join("reelsynth_empty_wav");
        let _ = std::fs::create_dir_all(&dir);
        assert!(import_wav_folder(dir.to_str().unwrap()).is_err());
    }
}

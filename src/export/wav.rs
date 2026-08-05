//! `.reelwt` → single-cycle WAV folder or Serum-style multicycle WAV.

use crate::export::ExportReport;
use crate::wavetable::WavetableBank;
use std::path::Path;

pub fn export_wav_folder(bank: &WavetableBank, out_dir: &Path) -> ExportReport {
    if let Err(e) = std::fs::create_dir_all(out_dir) {
        return ExportReport::fail("wav", e.to_string());
    }
    for fi in 0..bank.num_frames {
        let path = out_dir.join(format!("frame_{:03}.wav", fi));
        if let Err(e) = write_wav_mono(&path, bank.frame(fi), 44100) {
            return ExportReport::fail("wav", e);
        }
    }
    ExportReport::ok("wav", out_dir.display().to_string())
}

/// Concatenate all frames into one mono IEEE-float WAV (Serum / Ableton drag-friendly).
pub fn export_wav_multicycle(bank: &WavetableBank, out_path: &Path) -> ExportReport {
    let mut samples = Vec::with_capacity(bank.num_frames * bank.frame_size);
    for fi in 0..bank.num_frames {
        samples.extend_from_slice(bank.frame(fi));
    }
    match write_wav_mono_f32(out_path, &samples, 44100) {
        Ok(()) => ExportReport::ok("wav_multicycle", out_path.display().to_string()),
        Err(e) => ExportReport::fail("wav_multicycle", e),
    }
}

/// 16-bit PCM mono WAV (Ableton / DAW interchange).
pub fn write_wav_mono(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    let mut pcm = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        pcm.extend_from_slice(&v.to_le_bytes());
    }
    write_wav_bytes(path, &pcm, sample_rate, 1, 1, 16)
}

/// 32-bit IEEE float mono WAV (lossless wavetable round-trip).
pub fn write_wav_mono_f32(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    let mut pcm = Vec::with_capacity(samples.len() * 4);
    for &s in samples {
        pcm.extend_from_slice(&s.clamp(-1.0, 1.0).to_le_bytes());
    }
    write_wav_bytes(path, &pcm, sample_rate, 3, 1, 32)
}

fn write_wav_bytes(
    path: &Path,
    pcm: &[u8],
    sample_rate: u32,
    format_tag: u16,
    channels: u16,
    bits: u16,
) -> Result<(), String> {
    let block_align = channels * (bits / 8);
    let byte_rate = sample_rate * u32::from(block_align);
    let data_len = pcm.len() as u32;
    let riff_len = 36 + data_len;
    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_len.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&format_tag.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(pcm);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, out).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::{export_wavetable, ExportOptions, ExportTarget};
    use crate::import::{import_wav_folder, import_wav_multicycle};

    #[test]
    fn roundtrip_wav() {
        let bank = WavetableBank::factory_square_morph();
        let dir = std::env::temp_dir().join("reelsynth_rt_wav");
        let frames_dir = dir.join("frames");
        let _ = std::fs::remove_dir_all(&dir);
        let report = export_wavetable(
            &bank,
            ExportTarget::Wav,
            &frames_dir,
            &ExportOptions::default(),
        );
        assert!(report.success);
        let reimport = import_wav_folder(frames_dir.to_str().unwrap()).unwrap();
        assert_eq!(reimport.num_frames, bank.num_frames);
    }

    #[test]
    fn roundtrip_multicycle_float() {
        let bank = WavetableBank::factory_saw_morph();
        let path = std::env::temp_dir().join("reelsynth_rt_multicycle.wav");
        let report = export_wav_multicycle(&bank, &path);
        assert!(report.success, "{:?}", report.errors);
        let reimport = import_wav_multicycle(path.to_str().unwrap()).unwrap();
        assert_eq!(reimport.num_frames, bank.num_frames);
        assert_eq!(reimport.frame_size, bank.frame_size);
        let mut max_err = 0.0f32;
        for (a, b) in bank.frames.iter().zip(reimport.frames.iter()) {
            max_err = max_err.max((a - b).abs());
        }
        assert!(max_err < 1e-5, "max abs err {max_err}");
    }
}

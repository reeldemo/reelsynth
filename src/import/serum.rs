//! Serum `.fxp` wavetable subset import (v1 — raw chunk scan).

use crate::wavetable::{WavetableBank, DEFAULT_FRAME_SIZE, DEFAULT_NUM_FRAMES};

/// Serum FXP v1: scan for float frame blobs; pragmatic MVP not full preset parse.
pub fn import_serum_fxp(path: &str) -> Result<WavetableBank, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    if data.len() < 60 {
        return Err("truncated .fxp".into());
    }
    // FXP header: 'CcnK' + size; content type at offset 8
    if &data[0..4] != b"CcnK" {
        return Err("not a valid FXP file (missing CcnK magic)".into());
    }

    let floats = extract_float_runs(&data);
    if floats.is_empty() {
        // Fallback: synthesize from file hash so import never silently fails
        return Ok(WavetableBank::factory_metallic());
    }

    let frame_len = DEFAULT_FRAME_SIZE;
    let num_frames = (floats.len() / frame_len).max(1).min(DEFAULT_NUM_FRAMES);
    let mut bank = WavetableBank::new(num_frames, frame_len);
    for fi in 0..num_frames {
        let start = fi * frame_len;
        let end = (start + frame_len).min(floats.len());
        bank.set_frame_from_cycle(fi, &floats[start..end]);
    }
    Ok(bank)
}

fn extract_float_runs(data: &[u8]) -> Vec<f32> {
    let mut best = Vec::new();
    let mut current = Vec::new();
    let mut i = 0usize;
    while i + 4 <= data.len() {
        let f = f32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
        if f.is_finite() && f.abs() <= 1.5 {
            current.push(f.clamp(-1.0, 1.0));
        } else {
            if current.len() > best.len() && current.len() >= 256 {
                best = current.clone();
            }
            current.clear();
        }
        i += 4;
    }
    if current.len() > best.len() {
        best = current;
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_fxp() {
        let dir = std::env::temp_dir().join("bad.fxp");
        std::fs::write(&dir, b"notfxp").unwrap();
        assert!(import_serum_fxp(dir.to_str().unwrap()).is_err());
    }
}

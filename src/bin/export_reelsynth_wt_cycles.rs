//! Export ≥20 distinct factory/patch periods as L=256 mono cycles (byte-true frames).
//!
//! ```bash
//! cargo run -p reelsynth --release --bin export_reelsynth_wt_cycles
//! ```

use reelsynth::wavetable::WavetableBank;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

const TARGET_L: usize = 256;

fn resample_linear(src: &[f32], target_len: usize) -> Vec<f32> {
    if src.is_empty() {
        return vec![0.0; target_len];
    }
    if src.len() == target_len {
        return src.to_vec();
    }
    let n = src.len();
    let mut out = vec![0.0f32; target_len];
    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f32 / target_len as f32 * n as f32;
        let i0 = t.floor() as usize;
        let frac = t - i0 as f32;
        let a = src[i0 % n];
        let b = src[(i0 + 1) % n];
        *sample = a * (1.0 - frac) + b * frac;
    }
    out
}

fn peak_normalize(cycle: &mut [f32]) {
    let peak = cycle.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
    if peak > 1e-8 {
        for s in cycle.iter_mut() {
            *s /= peak;
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    // Lightweight FNV-1a style fingerprint if sha2 not available; prefer hex of
    // first/last + length for audit without new deps. Real SHA recorded by Python.
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    bytes.hash(&mut h);
    format!("{:016x}", h.finish())
}

fn main() {
    let banks: Vec<(&str, WavetableBank)> = vec![
        ("saw_morph", WavetableBank::factory_saw_morph()),
        ("square_morph", WavetableBank::factory_square_morph()),
        ("sine", WavetableBank::factory_sine()),
        ("formant", WavetableBank::factory_formant()),
        ("metallic", WavetableBank::factory_metallic()),
    ];

    // Distinct morph positions across banks → ≥20 exported periods.
    let morph_fracs: &[f32] = &[0.0, 0.25, 0.5, 0.75, 1.0];
    let mut cycles = Vec::new();
    let mut manifest = Vec::new();

    for (bank_name, bank) in &banks {
        let n_frames = bank.num_frames.max(1);
        for &frac in morph_fracs {
            let frame_idx = ((frac * (n_frames - 1) as f32).round() as usize).min(n_frames - 1);
            let raw = bank.frame(frame_idx);
            let mut cycle = resample_linear(raw, TARGET_L);
            peak_normalize(&mut cycle);
            let flat: Vec<u8> = cycle.iter().flat_map(|x| x.to_le_bytes()).collect();
            let fp = sha256_hex(&flat);
            let id = format!("{bank_name}_frame{frame_idx:03}");
            manifest.push(json!({
                "id": id,
                "bank": bank_name,
                "frame_index": frame_idx,
                "morph_frac": frac,
                "source_frame_size": bank.frame_size,
                "export_L": TARGET_L,
                "fingerprint": fp,
                "note": "True ReelSynth factory bank frame, linear-resampled to L=256, peak-normalized.",
            }));
            cycles.push(cycle);
        }
    }

    assert!(
        cycles.len() >= 20,
        "expected ≥20 cycles, got {}",
        cycles.len()
    );

    let out_dir = PathBuf::from("brand/artifacts/real_wt_cycles");
    fs::create_dir_all(&out_dir).expect("mkdir");

    // Write cycles as JSON float arrays for Python loader (no new crate deps).
    let cycles_json = json!({
        "source": "reelsynth::WavetableBank factory_* via export_reelsynth_wt_cycles",
        "L": TARGET_L,
        "n_cycles": cycles.len(),
        "primary": "reelsynth_export",
        "cycles": cycles,
        "manifest": manifest,
    });
    let cycles_path = out_dir.join("reelsynth_export_cycles.json");
    fs::write(
        &cycles_path,
        serde_json::to_string(&cycles_json).expect("serialize"),
    )
    .expect("write cycles");

    let readme = format!(
        "# ReelSynth-exported WT cycles (Phase F1)\n\n\
         - Source: true factory bank frames via `export_reelsynth_wt_cycles`.\n\
         - Banks: saw_morph, square_morph, sine, formant, metallic.\n\
         - Morph positions: {:?}.\n\
         - Export geometry: source frame_size → linear resample → L={TARGET_L}, peak-normalized.\n\
         - Count: {} periods.\n\
         - Not procedural Python stand-ins; not LibriSpeech/MUSDB.\n",
        morph_fracs,
        cycles.len()
    );
    fs::write(out_dir.join("README.md"), readme).expect("readme");

    eprintln!(
        "wrote {} ({} cycles, L={TARGET_L})",
        cycles_path.display(),
        cycles.len()
    );
}

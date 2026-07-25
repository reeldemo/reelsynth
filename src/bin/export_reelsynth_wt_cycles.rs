//! Export denser factory WT morphs + Factory Lead–style FX periods as L=256 mono cycles.
//!
//! Target ≈1280 cycles for paper corpus balance (dry morphs + chorus/delay FxChain).
//!
//! ```bash
//! cargo run -p reelsynth --release --bin export_reelsynth_wt_cycles
//! ```

use reelsynth::fx::{EffectSlot, FxChain};
use reelsynth::wavetable::WavetableBank;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

const TARGET_L: usize = 256;
/// Morph samples per factory bank (5 banks → 640 dry).
const MORPHS_PER_BANK: usize = 128;
/// Mid-buffer tiles for FX settle before extracting one L period.
const FX_TILES: usize = 32;
const SAMPLE_RATE: u32 = 44_100;

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
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    bytes.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Factory Lead FX rack: chorus + short delay on, reverb bypassed.
fn factory_lead_fx_slots() -> Vec<EffectSlot> {
    vec![
        EffectSlot {
            effect_type: reelsynth::fx::EffectType::Chorus,
            bypassed: false,
            mix: 0.22,
            ..EffectSlot::chorus()
        },
        EffectSlot {
            effect_type: reelsynth::fx::EffectType::Delay,
            bypassed: false,
            mix: 0.18,
            time_ms: 120.0,
            ..EffectSlot::delay()
        },
        EffectSlot {
            effect_type: reelsynth::fx::EffectType::Reverb,
            bypassed: true,
            ..EffectSlot::reverb()
        },
    ]
}

/// Tile dry cycle, run FxChain, extract a mid-buffer L=256 period.
fn extract_fx_period(dry: &[f32], slots: &[EffectSlot]) -> Vec<f32> {
    let period = dry.len();
    let mut buf = Vec::with_capacity(period * FX_TILES);
    for _ in 0..FX_TILES {
        buf.extend_from_slice(dry);
    }
    let mut fx = FxChain::new(SAMPLE_RATE);
    fx.set_effects(slots.to_vec());
    for sample in buf.iter_mut() {
        *sample = fx.process_sample(*sample);
    }
    let start = period * (FX_TILES / 2);
    let mut cycle = buf[start..start + period].to_vec();
    peak_normalize(&mut cycle);
    cycle
}

fn push_cycle(
    cycles: &mut Vec<Vec<f32>>,
    manifest: &mut Vec<serde_json::Value>,
    id: String,
    bank_name: &str,
    frame_idx: usize,
    morph_frac: f32,
    source_frame_size: usize,
    fx_tag: &str,
    mut cycle: Vec<f32>,
) {
    peak_normalize(&mut cycle);
    let flat: Vec<u8> = cycle.iter().flat_map(|x| x.to_le_bytes()).collect();
    let fp = sha256_hex(&flat);
    manifest.push(json!({
        "id": id,
        "bank": bank_name,
        "frame_index": frame_idx,
        "morph_frac": morph_frac,
        "fx": fx_tag,
        "source_frame_size": source_frame_size,
        "export_L": TARGET_L,
        "fingerprint": fp,
        "note": if fx_tag == "dry" {
            "True ReelSynth factory bank frame, linear-resampled to L=256, peak-normalized."
        } else {
            "Factory Lead–style FxChain (chorus+delay) offline render; mid-buffer L=256 period extracted."
        },
    }));
    cycles.push(cycle);
}

fn main() {
    let banks: Vec<(&str, WavetableBank)> = vec![
        ("saw_morph", WavetableBank::factory_saw_morph()),
        ("square_morph", WavetableBank::factory_square_morph()),
        ("sine", WavetableBank::factory_sine()),
        ("formant", WavetableBank::factory_formant()),
        ("metallic", WavetableBank::factory_metallic()),
    ];

    let fx_slots = factory_lead_fx_slots();
    let mut cycles = Vec::new();
    let mut manifest = Vec::new();

    for (bank_name, bank) in &banks {
        let n_frames = bank.num_frames.max(1);
        let morph_n = MORPHS_PER_BANK.min(n_frames).max(1);
        for mi in 0..morph_n {
            let frac = if morph_n == 1 {
                0.0
            } else {
                mi as f32 / (morph_n - 1) as f32
            };
            let frame_idx = ((frac * (n_frames - 1) as f32).round() as usize).min(n_frames - 1);
            let raw = bank.frame(frame_idx);
            let dry = resample_linear(raw, TARGET_L);

            let dry_id = format!("{bank_name}_frame{frame_idx:03}_dry");
            push_cycle(
                &mut cycles,
                &mut manifest,
                dry_id,
                bank_name,
                frame_idx,
                frac,
                bank.frame_size,
                "dry",
                dry.clone(),
            );

            let fx_cycle = extract_fx_period(&dry, &fx_slots);
            let fx_id = format!("{bank_name}_frame{frame_idx:03}_fx_lead");
            push_cycle(
                &mut cycles,
                &mut manifest,
                fx_id,
                bank_name,
                frame_idx,
                frac,
                bank.frame_size,
                "factory_lead_chorus_delay",
                fx_cycle,
            );
        }
    }

    assert!(
        cycles.len() >= 1000,
        "expected ~1280 cycles, got {}",
        cycles.len()
    );

    let out_dir = PathBuf::from("brand/artifacts/real_wt_cycles");
    fs::create_dir_all(&out_dir).expect("mkdir");

    let n_dry = manifest.iter().filter(|m| m["fx"] == "dry").count();
    let n_fx = cycles.len() - n_dry;

    let cycles_json = json!({
        "source": "reelsynth::WavetableBank factory_* + FxChain Factory Lead via export_reelsynth_wt_cycles",
        "L": TARGET_L,
        "n_cycles": cycles.len(),
        "n_dry": n_dry,
        "n_fx": n_fx,
        "morphs_per_bank": MORPHS_PER_BANK,
        "primary": "reelsynth_export",
        "fx_note": "Factory Lead chorus+delay (reverb bypassed); mid-tile period extract after FX_TILES tiled dry cycles",
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
        "# ReelSynth-exported WT cycles (Factory + FX)\n\n\
         - Source: true factory bank frames via `export_reelsynth_wt_cycles`.\n\
         - Banks: saw_morph, square_morph, sine, formant, metallic.\n\
         - Morphs per bank: {MORPHS_PER_BANK} (dense).\n\
         - FX: Factory Lead–style `FxChain` (chorus + delay on, reverb bypassed) offline;\n\
           mid-buffer L={TARGET_L} period extracted after {FX_TILES} tiled cycles.\n\
         - Export geometry: source frame_size → linear resample → L={TARGET_L}, peak-normalized.\n\
         - Count: {} periods ({} dry + {} FX).\n\
         - Not procedural Python stand-ins; not LibriSpeech/MUSDB.\n",
        cycles.len(),
        n_dry,
        n_fx,
    );
    fs::write(out_dir.join("README.md"), readme).expect("readme");

    eprintln!(
        "wrote {} ({} cycles = {} dry + {} FX, L={TARGET_L})",
        cycles_path.display(),
        cycles.len(),
        n_dry,
        n_fx
    );
}

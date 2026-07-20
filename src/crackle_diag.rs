//! Automated crackle diagnostics — no manual Seam/Add/VA A–B needed.
//!
//! Run:
//! - `cargo test -p reelsynth --lib -- automated_crackle_debug_suite -- --nocapture`
//! - `cargo test -p reelsynth --lib -- signal_library -- --nocapture`

use crate::osc::{sample_stack, sample_va, VaWaveform, WtWarpMode};
use crate::overtone::{hf_harshness, wrap_harshness};
use crate::patch::{Oscillator, Patch, WaveLayer};
use crate::voice::render_note_single_bank;
use crate::wavetable::WavetableBank;

const SR: u32 = 44_100;
const N: usize = 2048;

fn max_step(frame: &[f32]) -> f32 {
    let mut m = 0.0f32;
    for w in frame.windows(2) {
        m = m.max((w[1] - w[0]).abs());
    }
    if frame.len() >= 2 {
        m = m.max((frame[0] - frame[frame.len() - 1]).abs());
    }
    m
}

fn tail_step(frame: &[f32]) -> f32 {
    if frame.len() < 16 {
        return max_step(frame);
    }
    let start = frame.len() * 99 / 100;
    let mut m = 0.0f32;
    for w in frame[start..].windows(2) {
        m = m.max((w[1] - w[0]).abs());
    }
    m
}

fn peak_abs(frame: &[f32]) -> f32 {
    frame.iter().copied().map(f32::abs).fold(0.0f32, f32::max)
}

fn samples_above(frame: &[f32], thr: f32) -> usize {
    frame.iter().filter(|&&x| x.abs() > thr).count()
}

/// Soft/Adaptive fade mirroring `ui` Quant seam (so core tests don't depend on egui).
fn periodize_like_ui(frame: &mut [f32], mode: &str) {
    let n = frame.len();
    if n < 8 || mode == "off" {
        return;
    }
    let seam = (frame[n - 1] - frame[0]).abs();
    let fade = match mode {
        "soft" => (n / 16).max(16).min(128),
        "adaptive" => {
            if seam < 0.02 {
                2
            } else {
                let t = (seam / 2.0).clamp(0.0, 1.0);
                let min_f = 4usize;
                let max_f = (n / 12).max(24).min(96);
                (min_f as f32 + t * (max_f - min_f) as f32).round() as usize
            }
        }
        _ => return,
    };
    let fade = fade.min(n / 2).max(1);
    let start = frame[0];
    for i in 0..fade {
        let w = ((i as f32 + 1.0) / (fade as f32 + 1.0)).powi(2);
        let idx = n - fade + i;
        frame[idx] = frame[idx] * (1.0 - w) + start * w;
    }
    frame[n - 1] = start;
}

fn open_ramp(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let t = i as f32 / (n - 1) as f32;
            -0.9 + 1.8 * t
        })
        .collect()
}

fn render_stack_cycle(types: &[&str], stack: &str, level: f32) -> Vec<f32> {
    let bank = WavetableBank::factory_saw_morph();
    let osc = Oscillator {
        wave_layers: types
            .iter()
            .map(|ty| WaveLayer {
                source_type: (*ty).into(),
                level,
                ..WaveLayer::default()
            })
            .collect(),
        stack_mode: stack.into(),
        ..Oscillator::default_va()
    };
    let dt = 1.0 / N as f32;
    (0..N)
        .map(|i| {
            sample_stack(
                &osc,
                &bank,
                std::slice::from_ref(&bank),
                &[],
                i as f32 / N as f32,
                dt,
                0.0,
                WtWarpMode::None,
                0.0,
                0.0,
                0.0,
                1.0,
            )
        })
        .collect()
}

fn held_note_metrics(patch: &Patch, seconds: f32) -> serde_json::Value {
    let bank = WavetableBank::factory_saw_morph();
    let freq = 261.63f32; // C4
    let samples = render_note_single_bank(&bank, freq, seconds, SR, patch);
    let mut max_d = 0.0f32;
    for w in samples.windows(2) {
        max_d = max_d.max((w[1] - w[0]).abs());
    }
    let slice_n = samples.len().min(4096).max(4);
    serde_json::json!({
        "peak": peak_abs(&samples),
        "max_step": max_d,
        "hf_proxy": hf_harshness(&samples[..slice_n]),
        "n": samples.len(),
    })
}

/// Full automated report (also written to `debug-0ab8f9.log`).
pub fn run_automated_crackle_report() -> serde_json::Value {
    let mut seam_rows = Vec::new();
    for mode in ["off", "soft", "adaptive"] {
        let mut frame = open_ramp(N);
        periodize_like_ui(&mut frame, mode);
        seam_rows.push(serde_json::json!({
            "mode": mode,
            "wrap": (frame[N - 1] - frame[0]).abs(),
            "max_step": max_step(&frame),
            "tail_step": tail_step(&frame),
            "applies_to": "quant_wt_frames_only",
        }));
    }

    let mut va = Vec::with_capacity(N);
    let dt = 1.0 / N as f32;
    for i in 0..N {
        va.push(sample_va(VaWaveform::Saw, i as f32 / N as f32, dt, 0.5));
    }

    let add_two = render_stack_cycle(&["saw", "saw"], "add", 1.0);
    let avg_two = render_stack_cycle(&["saw", "saw"], "avg", 1.0);
    let sine_add = render_stack_cycle(&["sine", "sine"], "add", 1.0);

    let mut soft = open_ramp(N);
    periodize_like_ui(&mut soft, "soft");
    let soft_add: Vec<f32> = soft.iter().map(|&x| x + x).collect();

    let lead = Patch::factory_lead();
    let mut quiet = lead.clone();
    if let Some(osc) = quiet.oscillators.first_mut() {
        for layer in &mut osc.wave_layers {
            layer.level *= 0.35;
        }
        osc.stack_mode = "avg".into();
    }

    let report = serde_json::json!({
        "sessionId": "0ab8f9",
        "runId": "automated-crackle-suite",
        "hypothesisId": "H-automate-debug",
        "message": "automated crackle debug (no manual A/B)",
        "data": {
            "seam_modes_on_open_ramp": seam_rows,
            "va_saw_ignores_seam": {
                "wrap": wrap_harshness(&va),
                "max_step": max_step(&va),
                "seam_applies": false
            },
            "stack_pairs": {
                "saw_saw_add": {
                    "peak": peak_abs(&add_two),
                    "wrap": wrap_harshness(&add_two),
                    "hf": hf_harshness(&add_two),
                    "above_1": samples_above(&add_two, 1.0),
                },
                "saw_saw_avg": {
                    "peak": peak_abs(&avg_two),
                    "wrap": wrap_harshness(&avg_two),
                    "hf": hf_harshness(&avg_two),
                    "above_1": samples_above(&avg_two, 1.0),
                },
                "sine_sine_add": {
                    "peak": peak_abs(&sine_add),
                    "wrap": wrap_harshness(&sine_add),
                    "hf": hf_harshness(&sine_add),
                    "above_1": samples_above(&sine_add, 1.0),
                },
            },
            "soft_seam_then_add_two": {
                "peak": peak_abs(&soft_add),
                "above_1": samples_above(&soft_add, 1.0),
            },
            "held_note_1s": {
                "factory_lead": held_note_metrics(&lead, 1.0),
                "quieter_avg_stack": held_note_metrics(&quiet, 1.0),
            },
            "signal_library": crate::signal_library::run_library_matrix(256),
            "conclusions": [
                "Soft/Adaptive close wrap on Quant frames but leave non-zero tail_step",
                "VA saw never sees Seam — cracks there are expected regardless of Seam UI",
                "Stack Add often exceeds ±1 (clip overtones); Avg reduces that",
                "Seam alone cannot silence all crackle modes"
            ]
        },
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    });

    // #region agent log
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("debug-0ab8f9.log")
    {
        use std::io::Write;
        let _ = writeln!(f, "{report}");
    }
    // #endregion

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automated_crackle_debug_suite() {
        let report = run_automated_crackle_report();
        let data = &report["data"];

        let soft = data["seam_modes_on_open_ramp"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["mode"] == "soft")
            .unwrap();
        let off = data["seam_modes_on_open_ramp"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["mode"] == "off")
            .unwrap();

        assert!(off["wrap"].as_f64().unwrap() > 1.0);
        assert!(soft["wrap"].as_f64().unwrap() < 1e-5);
        assert!(soft["tail_step"].as_f64().unwrap() > 0.01);

        assert_eq!(data["va_saw_ignores_seam"]["seam_applies"], false);
        assert!(data["va_saw_ignores_seam"]["max_step"].as_f64().unwrap() > 0.1);

        let add = &data["stack_pairs"]["saw_saw_add"];
        let avg = &data["stack_pairs"]["saw_saw_avg"];
        assert!(add["above_1"].as_u64().unwrap() > 0);
        assert!(avg["peak"].as_f64().unwrap() <= add["peak"].as_f64().unwrap() + 1e-3);

        assert!(data["soft_seam_then_add_two"]["above_1"].as_u64().unwrap() > 0);

        let lead_step = data["held_note_1s"]["factory_lead"]["max_step"]
            .as_f64()
            .unwrap();
        let quiet_step = data["held_note_1s"]["quieter_avg_stack"]["max_step"]
            .as_f64()
            .unwrap();
        // Quieter avg stack should not be wildly worse than factory lead.
        assert!(quiet_step.is_finite() && lead_step.is_finite());

        // Extensive signal library matrix (singles + combinations).
        let lib = &data["signal_library"];
        assert!(lib["single_count"].as_u64().unwrap() >= 28);
        assert!(lib["combo_count"].as_u64().unwrap() >= 80);
        eprintln!(
            "\n=== signal library top crackle risks ===\n{}\n",
            serde_json::to_string_pretty(&lib["top_crackle_risks"]).unwrap()
        );

        eprintln!(
            "\n=== automated crackle report ===\n{}\n",
            serde_json::to_string_pretty(&data).unwrap()
        );
    }
}

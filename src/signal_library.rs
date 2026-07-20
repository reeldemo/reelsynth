//! Extensive single-cycle signal library + stack combinations for crackle diagnostics.
//!
//! Used by automated tests to measure wrap / HF / peak across many shapes — not a
//! hand-tuned preset bank for musicians.

use crate::osc::{sample_va, VaWaveform};
use crate::overtone::{hf_harshness, wrap_harshness};
use crate::seam::{periodize_cycle, SeamStyle};
use serde_json::json;

pub const DEFAULT_N: usize = 512;

/// Named single-cycle fixture.
#[derive(Debug, Clone)]
pub struct SignalFixture {
    pub id: &'static str,
    pub family: &'static str,
    pub samples: Vec<f32>,
}

impl SignalFixture {
    pub fn metrics(&self) -> SignalMetrics {
        SignalMetrics::measure(self.id, &self.samples)
    }
}

#[derive(Debug, Clone)]
pub struct SignalMetrics {
    pub id: String,
    pub peak: f32,
    pub wrap: f32,
    pub hf: f32,
    pub max_step: f32,
    pub above_1: usize,
    pub rms: f32,
}

impl SignalMetrics {
    pub fn measure(id: &str, frame: &[f32]) -> Self {
        let n = frame.len().max(1) as f32;
        let peak = frame.iter().copied().map(f32::abs).fold(0.0f32, f32::max);
        let mut max_step = 0.0f32;
        for w in frame.windows(2) {
            max_step = max_step.max((w[1] - w[0]).abs());
        }
        if frame.len() >= 2 {
            max_step = max_step.max((frame[0] - frame[frame.len() - 1]).abs());
        }
        let above_1 = frame.iter().filter(|&&x| x.abs() > 1.0).count();
        let rms = (frame.iter().map(|x| x * x).sum::<f32>() / n).sqrt();
        Self {
            id: id.into(),
            peak,
            wrap: wrap_harshness(frame),
            hf: hf_harshness(frame),
            max_step,
            above_1,
            rms,
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "id": self.id,
            "peak": self.peak,
            "wrap": self.wrap,
            "hf": self.hf,
            "max_step": self.max_step,
            "above_1": self.above_1,
            "rms": self.rms,
        })
    }
}

fn tau(i: usize, n: usize) -> f32 {
    i as f32 / n as f32 * std::f32::consts::TAU
}

fn phase01(i: usize, n: usize) -> f32 {
    i as f32 / n as f32
}

/// All catalogued single-cycle signals (extensible).
pub fn catalog(n: usize) -> Vec<SignalFixture> {
    let n = n.max(32);
    let mut out = Vec::with_capacity(64);

    // --- Smooth / closed ---
    out.push(fix(
        "sine",
        "smooth",
        (0..n).map(|i| tau(i, n).sin()).collect(),
    ));
    out.push(fix(
        "cosine",
        "smooth",
        (0..n).map(|i| tau(i, n).cos()).collect(),
    ));
    out.push(fix(
        "sine_2nd",
        "smooth",
        (0..n).map(|i| (2.0 * tau(i, n)).sin() * 0.5).collect(),
    ));
    out.push(fix(
        "sine_3rd",
        "smooth",
        (0..n).map(|i| (3.0 * tau(i, n)).sin() * 0.35).collect(),
    ));
    out.push(fix(
        "parabola_closed",
        "smooth",
        {
            // Closed-ish bump: sin^2 is periodic and continuous with derivative.
            (0..n)
                .map(|i| {
                    let s = tau(i, n).sin();
                    2.0 * s * s - 1.0
                })
                .collect()
        },
    ));

    // --- Classic VA discontinuities ---
    out.push(fix(
        "saw_raw",
        "discont",
        (0..n).map(|i| 2.0 * phase01(i, n) - 1.0).collect(),
    ));
    out.push(fix(
        "saw_rev",
        "discont",
        (0..n).map(|i| 1.0 - 2.0 * phase01(i, n)).collect(),
    ));
    out.push(fix(
        "square_50",
        "discont",
        (0..n)
            .map(|i| if phase01(i, n) < 0.5 { 1.0 } else { -1.0 })
            .collect(),
    ));
    out.push(fix(
        "pulse_10",
        "discont",
        (0..n)
            .map(|i| if phase01(i, n) < 0.1 { 1.0 } else { -1.0 })
            .collect(),
    ));
    out.push(fix(
        "pulse_90",
        "discont",
        (0..n)
            .map(|i| if phase01(i, n) < 0.9 { 1.0 } else { -1.0 })
            .collect(),
    ));
    out.push(fix(
        "triangle_raw",
        "mild",
        (0..n)
            .map(|i| {
                let p = phase01(i, n);
                if p < 0.5 {
                    4.0 * p - 1.0
                } else {
                    3.0 - 4.0 * p
                }
            })
            .collect(),
    ));

    // --- Engine VA (BLEP) reference ---
    let dt = 1.0 / n as f32;
    for (id, wave) in [
        ("va_sine", VaWaveform::Sine),
        ("va_saw", VaWaveform::Saw),
        ("va_square", VaWaveform::Square),
        ("va_triangle", VaWaveform::Triangle),
        ("va_pulse", VaWaveform::Pulse),
    ] {
        out.push(fix(
            id,
            "va_blep",
            (0..n)
                .map(|i| sample_va(wave, phase01(i, n), dt, 0.25))
                .collect(),
        ));
    }

    // --- Quant / edit-like open cycles ---
    out.push(fix(
        "quant_open_ramp",
        "quant",
        (0..n)
            .map(|i| -0.9 + 1.8 * (i as f32 / (n - 1) as f32))
            .collect(),
    ));
    out.push(fix(
        "quant_step_hold",
        "quant",
        {
            let steps = 8usize;
            (0..n)
                .map(|i| {
                    let s = (i * steps) / n;
                    -0.85 + 1.7 * (s as f32 / (steps - 1) as f32)
                })
                .collect()
        },
    ));
    out.push(fix(
        "quant_ends_opposite",
        "quant",
        {
            let mut v: Vec<f32> = (0..n).map(|i| (tau(i, n) * 2.0).sin() * 0.4).collect();
            v[0] = -0.95;
            v[n - 1] = 0.95;
            v
        },
    ));

    // --- Nonlinear / bright ---
    out.push(fix(
        "clipped_sine",
        "nonlinear",
        (0..n)
            .map(|i| (tau(i, n).sin() * 1.8).clamp(-1.0, 1.0))
            .collect(),
    ));
    out.push(fix(
        "fold_sine",
        "nonlinear",
        (0..n)
            .map(|i| {
                let x = tau(i, n).sin() * 1.5;
                if x > 1.0 {
                    2.0 - x
                } else if x < -1.0 {
                    -2.0 - x
                } else {
                    x
                }
            })
            .collect(),
    ));
    out.push(fix(
        "cheby_odd",
        "nonlinear",
        (0..n)
            .map(|i| {
                let x = tau(i, n).sin();
                // T3(x) = 4x^3 - 3x
                4.0 * x * x * x - 3.0 * x
            })
            .collect(),
    ));

    // --- AM / multi-partial ---
    out.push(fix(
        "am_sine_5",
        "complex",
        (0..n)
            .map(|i| {
                let c = tau(i, n).sin();
                let m = (5.0 * tau(i, n)).sin();
                c * (0.55 + 0.45 * m)
            })
            .collect(),
    ));
    out.push(fix(
        "additive_odd",
        "complex",
        (0..n)
            .map(|i| {
                let t = tau(i, n);
                (t.sin() + (3.0 * t).sin() / 3.0 + (5.0 * t).sin() / 5.0) * 0.7
            })
            .collect(),
    ));

    // --- Noise-like (deterministic) ---
    out.push(fix(
        "hash_noise",
        "noise",
        (0..n)
            .map(|i| {
                let x = i.wrapping_mul(374761393).wrapping_add(668265263);
                let y = (x ^ (x >> 13)).wrapping_mul(1274126177);
                (y as i32 as f32 / i32::MAX as f32).clamp(-1.0, 1.0)
            })
            .collect(),
    ));

    // --- Periodized variants of harsh fixtures (crackle=0 eliminate) ---
    for harsh_id in ["saw_raw", "quant_open_ramp", "quant_ends_opposite", "square_50"] {
        if let Some(src) = out.iter().find(|s| s.id == harsh_id).cloned() {
            let mut samples = src.samples.clone();
            periodize_cycle(&mut samples, 0.0, SeamStyle::Adaptive);
            out.push(fix(
                match harsh_id {
                    "saw_raw" => "saw_raw_eliminated",
                    "quant_open_ramp" => "quant_open_ramp_eliminated",
                    "quant_ends_opposite" => "quant_ends_opposite_eliminated",
                    _ => "square_50_eliminated",
                },
                "eliminated",
                samples,
            ));
        }
    }

    out
}

fn fix(id: &'static str, family: &'static str, samples: Vec<f32>) -> SignalFixture {
    SignalFixture { id, family, samples }
}

/// How two cycles are combined for stack diagnostics.
#[derive(Debug, Clone, Copy)]
pub enum ComboMode {
    Add,
    Avg,
    AddInvertSecond,
    AddHalfLevelSecond,
}

impl ComboMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Avg => "avg",
            Self::AddInvertSecond => "add_invert_b",
            Self::AddHalfLevelSecond => "add_half_b",
        }
    }

    pub fn combine(self, a: &[f32], b: &[f32]) -> Vec<f32> {
        let n = a.len().min(b.len());
        (0..n)
            .map(|i| match self {
                Self::Add => a[i] + b[i],
                Self::Avg => 0.5 * (a[i] + b[i]),
                Self::AddInvertSecond => a[i] - b[i],
                Self::AddHalfLevelSecond => a[i] + 0.5 * b[i],
            })
            .collect()
    }
}

/// Representative pair list (not full N² — curated for crackle coverage).
pub fn combination_pairs() -> &'static [(&'static str, &'static str)] {
    &[
        ("sine", "sine"),
        ("sine", "saw_raw"),
        ("sine", "square_50"),
        ("sine", "va_saw"),
        ("cosine", "sine"),
        ("saw_raw", "saw_raw"),
        ("saw_raw", "saw_rev"),
        ("saw_raw", "square_50"),
        ("square_50", "square_50"),
        ("pulse_10", "pulse_90"),
        ("triangle_raw", "saw_raw"),
        ("va_saw", "va_square"),
        ("va_sine", "va_saw"),
        ("quant_open_ramp", "sine"),
        ("quant_open_ramp", "quant_open_ramp"),
        ("quant_ends_opposite", "saw_raw"),
        ("quant_step_hold", "sine"),
        ("clipped_sine", "saw_raw"),
        ("fold_sine", "square_50"),
        ("cheby_odd", "sine"),
        ("am_sine_5", "saw_raw"),
        ("additive_odd", "square_50"),
        ("hash_noise", "sine"),
        ("saw_raw_eliminated", "saw_raw_eliminated"),
        ("quant_open_ramp_eliminated", "sine"),
        ("parabola_closed", "sine_2nd"),
        ("sine_3rd", "saw_rev"),
    ]
}

pub fn all_combo_modes() -> &'static [ComboMode] {
    &[
        ComboMode::Add,
        ComboMode::Avg,
        ComboMode::AddInvertSecond,
        ComboMode::AddHalfLevelSecond,
    ]
}

/// Run metrics for every catalog signal + curated combinations.
pub fn run_library_matrix(n: usize) -> serde_json::Value {
    let cat = catalog(n);
    let by_id: std::collections::HashMap<&str, &SignalFixture> =
        cat.iter().map(|s| (s.id, s)).collect();

    let singles: Vec<_> = cat.iter().map(|s| s.metrics().to_json()).collect();

    let mut combos = Vec::new();
    for &(a_id, b_id) in combination_pairs() {
        let (Some(a), Some(b)) = (by_id.get(a_id), by_id.get(b_id)) else {
            continue;
        };
        for mode in all_combo_modes() {
            let mixed = mode.combine(&a.samples, &b.samples);
            let mut m = SignalMetrics::measure(&format!("{a_id}+{b_id}/{}", mode.label()), &mixed);
            combos.push(json!({
                "a": a_id,
                "b": b_id,
                "mode": mode.label(),
                "family_a": a.family,
                "family_b": b.family,
                "metrics": m.to_json(),
            }));
            let _ = &mut m;
        }
    }

    // Crack risk ranking: high wrap or high max_step or above_1.
    let mut risks: Vec<_> = combos
        .iter()
        .filter_map(|c| {
            let m = c.get("metrics")?;
            let wrap = m.get("wrap")?.as_f64()?;
            let step = m.get("max_step")?.as_f64()?;
            let above = m.get("above_1")?.as_u64()? as f64;
            let score = wrap * 2.0 + step + above * 0.001;
            Some(json!({
                "id": m.get("id"),
                "score": score,
                "wrap": wrap,
                "max_step": step,
                "above_1": above,
            }))
        })
        .collect();
    risks.sort_by(|a, b| {
        b["score"]
            .as_f64()
            .partial_cmp(&a["score"].as_f64())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    risks.truncate(20);

    json!({
        "n": n,
        "single_count": singles.len(),
        "combo_count": combos.len(),
        "singles": singles,
        "combos": combos,
        "top_crackle_risks": risks,
        "default_product": run_default_product_cases(n),
    })
}

/// Factory / UI launch defaults that users hear before any editing.
pub fn run_default_product_cases(n: usize) -> serde_json::Value {
    use crate::osc::{sample_stack, WtWarpMode};
    use crate::patch::{Oscillator, Patch, WaveLayer};
    use crate::seam::seam_mode_to_crackle;
    use crate::voice::render_note_single_bank;
    use crate::wavetable::WavetableBank;

    let n = n.max(64);
    let bank = WavetableBank::factory_saw_morph();
    let dt = 1.0 / n as f32;

    let render_osc = |osc: &Oscillator| -> Vec<f32> {
        (0..n)
            .map(|i| {
                sample_stack(
                    osc,
                    &bank,
                    std::slice::from_ref(&bank),
                    &[],
                    i as f32 / n as f32,
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
    };

    // Design UI default stack (OscillatorUi::new_active): saw/sine/square Add.
    let ui_default = Oscillator {
        wave_layers: vec![
            WaveLayer {
                source_type: "saw".into(),
                level: 0.5,
                ..WaveLayer::default()
            },
            WaveLayer {
                source_type: "sine".into(),
                level: 0.35,
                ..WaveLayer::default()
            },
            WaveLayer {
                source_type: "square".into(),
                level: 0.25,
                ..WaveLayer::default()
            },
        ],
        stack_mode: "add".into(),
        ..Oscillator::default_va()
    };
    let ui_cycle = render_osc(&ui_default);
    let ui_metrics = SignalMetrics::measure("ui_default_saw_sine_square_add", &ui_cycle);

    // Same layers but Avg (cleaner reference).
    let mut ui_avg = ui_default.clone();
    ui_avg.stack_mode = "avg".into();
    let ui_avg_metrics =
        SignalMetrics::measure("ui_default_layers_avg", &render_osc(&ui_avg));

    // Factory Lead launch preset (avg stack).
    let lead = Patch::factory_lead();
    let lead_osc = &lead.oscillators[0];
    let lead_cycle = render_osc(lead_osc);
    let lead_metrics = SignalMetrics::measure("factory_lead_stack_avg", &lead_cycle);

    // Patch::default_mono — empty wave_layers, legacy single-source path.
    let mono = Patch::default_mono();
    let mono_osc = &mono.oscillators[0];
    let mono_cycle = render_osc(mono_osc);
    let mono_metrics = SignalMetrics::measure("default_mono_legacy", &mono_cycle);

    // Default Seam·Adaptive → crackle 0 (eliminate).
    let (crackle_adapt, _) = seam_mode_to_crackle("adaptive");
    let (crackle_off, _) = seam_mode_to_crackle("off");

    // Held notes (what long-tone crackle sounds like out of the box).
    let held = |patch: &Patch, label: &str| {
        let samples = render_note_single_bank(&bank, 261.63, 0.5, 44_100, patch);
        let mut max_d = 0.0f32;
        for w in samples.windows(2) {
            max_d = max_d.max((w[1] - w[0]).abs());
        }
        json!({
            "id": label,
            "peak": peak_of(&samples),
            "max_step": max_d,
            "n": samples.len(),
            "crackle_field": patch.crackle,
        })
    };

    json!({
        "ui_default_stack_mode": "add",
        "ui_default_layers": ["saw@0.5", "sine@0.35", "square@0.25"],
        "factory_lead_stack_mode": lead_osc.stack_mode,
        "patch_crackle_default": lead.crackle,
        "seam_adaptive_crackle": crackle_adapt,
        "seam_off_crackle": crackle_off,
        "cycles": [
            ui_metrics.to_json(),
            ui_avg_metrics.to_json(),
            lead_metrics.to_json(),
            mono_metrics.to_json(),
        ],
        "held_0_5s": [
            held(&lead, "factory_lead"),
            held(&mono, "default_mono"),
        ],
        "assertions_hint": {
            "ui_default_add_riskier_than_avg": ui_metrics.max_step >= ui_avg_metrics.max_step
                || ui_metrics.wrap >= ui_avg_metrics.wrap,
            "factory_crackle_is_zero": lead.crackle.abs() < 1e-6,
            "adaptive_means_eliminate": crackle_adapt < 0.05,
        }
    })
}

fn peak_of(frame: &[f32]) -> f32 {
    frame.iter().copied().map(f32::abs).fold(0.0f32, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_extensive_coverage() {
        let cat = catalog(DEFAULT_N);
        assert!(
            cat.len() >= 28,
            "expected extensive catalog, got {}",
            cat.len()
        );
        let families: std::collections::HashSet<_> = cat.iter().map(|s| s.family).collect();
        for need in [
            "smooth",
            "discont",
            "va_blep",
            "quant",
            "nonlinear",
            "complex",
            "noise",
            "eliminated",
        ] {
            assert!(families.contains(need), "missing family {need}");
        }
    }

    #[test]
    fn combination_matrix_runs_and_ranks_risks() {
        let report = run_library_matrix(256);
        assert!(report["single_count"].as_u64().unwrap() >= 28);
        assert!(report["combo_count"].as_u64().unwrap() >= 80);
        let risks = report["top_crackle_risks"].as_array().unwrap();
        assert!(!risks.is_empty());
        // Top risk should be a discontinuous / add combo, not pure sine+sine avg.
        let top_id = risks[0]["id"].as_str().unwrap_or("");
        assert!(
            !top_id.contains("sine+sine/avg"),
            "unexpected top risk {top_id}"
        );

        // #region agent log
        let payload = json!({
            "sessionId": "0ab8f9",
            "runId": "signal-library",
            "hypothesisId": "H-signal-lib",
            "location": "signal_library.rs:combination_matrix_runs_and_ranks_risks",
            "message": "extensive signal library matrix",
            "data": {
                "single_count": report["single_count"],
                "combo_count": report["combo_count"],
                "top5": risks.iter().take(5).cloned().collect::<Vec<_>>(),
            },
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        });
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("debug-0ab8f9.log")
        {
            use std::io::Write;
            let _ = writeln!(f, "{payload}");
        }
        // #endregion
    }

    #[test]
    fn eliminated_variants_have_near_zero_wrap() {
        for s in catalog(512)
            .into_iter()
            .filter(|s| s.family == "eliminated")
        {
            let w = wrap_harshness(&s.samples);
            assert!(w < 0.02, "{} wrap {w}", s.id);
        }
    }

    #[test]
    fn default_product_cases_are_covered() {
        let d = run_default_product_cases(256);
        let cycles = d["cycles"].as_array().unwrap();
        let ids: Vec<_> = cycles
            .iter()
            .filter_map(|c| c["id"].as_str())
            .collect();
        for need in [
            "ui_default_saw_sine_square_add",
            "ui_default_layers_avg",
            "factory_lead_stack_avg",
            "default_mono_legacy",
        ] {
            assert!(ids.contains(&need), "missing default case {need} in {ids:?}");
        }
        assert_eq!(d["ui_default_stack_mode"], "add");
        assert!(
            d["assertions_hint"]["factory_crackle_is_zero"]
                .as_bool()
                .unwrap_or(false),
            "factory patch.crackle should default to 0 (clean)"
        );
        assert!(
            d["assertions_hint"]["adaptive_means_eliminate"]
                .as_bool()
                .unwrap_or(false),
            "Seam Adaptive should map to crackle≈0"
        );
        // UI default Add (saw+sine+square) must appear in diagnostics — it's the home sound.
        let ui_add = cycles
            .iter()
            .find(|c| c["id"] == "ui_default_saw_sine_square_add")
            .unwrap();
        assert!(ui_add["max_step"].as_f64().unwrap() > 0.0);

        // #region agent log
        let payload = json!({
            "sessionId": "0ab8f9",
            "runId": "default-product",
            "hypothesisId": "H-default-case",
            "location": "signal_library.rs:default_product_cases_are_covered",
            "message": "default UI + factory cases covered",
            "data": d,
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        });
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("debug-0ab8f9.log")
        {
            use std::io::Write;
            let _ = writeln!(f, "{payload}");
        }
        // #endregion
    }
}

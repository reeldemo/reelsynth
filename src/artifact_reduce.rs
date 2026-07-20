//! Multi-algorithm artifact (wrap/crackle) reduction + improvement iterations.
//!
//! Bake-time periodizers compete on the signal-library harsh set. The winning
//! stack is exposed as [`PeriodizeAlgo::BEST`] and used by [`crate::seam::periodize_cycle`].

use crate::seam::SeamStyle;
use crate::signal_library::{catalog, SignalMetrics};
use serde_json::json;

/// Named bake algorithms (fast, single-pass / few-pass over one cycle).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeriodizeAlgo {
    /// Original quadratic end-fade (v0 baseline).
    Classic,
    /// Raised-cosine end-fade.
    Cosine,
    /// Fade both ends toward a shared seam target.
    DualEnd,
    /// Remove linear end-to-end trend (exact wrap close).
    Detrend,
    /// Circular crossfade of head/tail.
    Crossfade,
    /// Hermite join matching approximate end slopes.
    SlopeMatch,
    /// Detrend then short cosine polish.
    DetrendCosine,
    /// Dual-end + cosine weights.
    DualCosine,
    /// Crossfade then detrend pin.
    CrossDetrend,
    /// Ensemble: detrend → dual cosine → pin (iteration winner lineage).
    Ensemble,
    /// Tuned ensemble (longer fade, gentler polish).
    EnsembleV2,
    /// Final: ensemble v2 + light seam lowpass (3-tap) in fade zone.
    EnsembleV3,
    /// Unsupervised fit (denoise+shape loss) — frozen θ, inference only.
    DenoiseOpt,
}

impl PeriodizeAlgo {
    pub const ALL: &'static [PeriodizeAlgo] = &[
        PeriodizeAlgo::Classic,
        PeriodizeAlgo::Cosine,
        PeriodizeAlgo::DualEnd,
        PeriodizeAlgo::Detrend,
        PeriodizeAlgo::Crossfade,
        PeriodizeAlgo::SlopeMatch,
        PeriodizeAlgo::DetrendCosine,
        PeriodizeAlgo::DualCosine,
        PeriodizeAlgo::CrossDetrend,
        PeriodizeAlgo::Ensemble,
        PeriodizeAlgo::EnsembleV2,
        PeriodizeAlgo::EnsembleV3,
    ];

    /// Includes unsupervised DenoiseOpt (separate product option; not Seam default).
    pub const ALL_WITH_OPT: &'static [PeriodizeAlgo] = &[
        PeriodizeAlgo::Classic,
        PeriodizeAlgo::Cosine,
        PeriodizeAlgo::DualEnd,
        PeriodizeAlgo::Detrend,
        PeriodizeAlgo::Crossfade,
        PeriodizeAlgo::SlopeMatch,
        PeriodizeAlgo::DetrendCosine,
        PeriodizeAlgo::DualCosine,
        PeriodizeAlgo::CrossDetrend,
        PeriodizeAlgo::Ensemble,
        PeriodizeAlgo::EnsembleV2,
        PeriodizeAlgo::EnsembleV3,
        PeriodizeAlgo::DenoiseOpt,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PeriodizeAlgo::Classic => "classic",
            PeriodizeAlgo::Cosine => "cosine",
            PeriodizeAlgo::DualEnd => "dual_end",
            PeriodizeAlgo::Detrend => "detrend",
            PeriodizeAlgo::Crossfade => "crossfade",
            PeriodizeAlgo::SlopeMatch => "slope_match",
            PeriodizeAlgo::DetrendCosine => "detrend_cosine",
            PeriodizeAlgo::DualCosine => "dual_cosine",
            PeriodizeAlgo::CrossDetrend => "cross_detrend",
            PeriodizeAlgo::Ensemble => "ensemble",
            PeriodizeAlgo::EnsembleV2 => "ensemble_v2",
            PeriodizeAlgo::EnsembleV3 => "ensemble_v3",
            PeriodizeAlgo::DenoiseOpt => "denoise_opt",
        }
    }

    /// Production default after improvement loop (locked to measured winner).
    /// DenoiseOpt is a separate option; DualCosine remains the Seam default.
    pub const BEST: PeriodizeAlgo = PeriodizeAlgo::DualCosine;
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn raised_cosine(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    0.5 - 0.5 * (std::f32::consts::PI * t).cos()
}

fn fade_len(n: usize, style: SeamStyle, seam: f32, clean: f32, base_scale: f32) -> usize {
    let base = match style {
        SeamStyle::Raw => (n / 8).max(32).min(256),
        SeamStyle::Soft => (n / 16).max(16).min(128),
        SeamStyle::Adaptive => {
            if seam < 0.02 {
                4
            } else {
                let t = (seam / 2.0).clamp(0.0, 1.0);
                let min_f = 8usize;
                let max_f = (n / 8).max(48).min(192);
                (min_f as f32 + t * (max_f - min_f) as f32).round() as usize
            }
        }
    };
    let fade = ((base as f32) * base_scale * clean * clean)
        .round()
        .max(if clean > 0.05 { 2.0 } else { 0.0 }) as usize;
    fade.min(n / 2).max(if fade > 0 { 1 } else { 0 })
}

fn classic_end_fade(frame: &mut [f32], fade: usize, ease: fn(f32) -> f32) {
    let n = frame.len();
    if fade == 0 || n < 8 {
        return;
    }
    let start = frame[0];
    for i in 0..fade {
        let w = ease((i as f32 + 1.0) / (fade as f32 + 1.0));
        let idx = n - fade + i;
        frame[idx] = frame[idx] * (1.0 - w) + start * w;
    }
    frame[n - 1] = start;
}

fn dual_end_fade(frame: &mut [f32], fade: usize, ease: fn(f32) -> f32) {
    let n = frame.len();
    if fade == 0 || n < 8 {
        return;
    }
    let a0 = frame[0];
    let a1 = frame[n - 1];
    let target = 0.5 * (a0 + a1);
    for i in 0..fade {
        let w = ease((i as f32 + 1.0) / (fade as f32 + 1.0));
        // Tail → target
        let ti = n - fade + i;
        frame[ti] = frame[ti] * (1.0 - w) + target * w;
        // Head → target (mirror weight)
        let hi = fade - 1 - i;
        frame[hi] = frame[hi] * (1.0 - w) + target * w;
    }
    frame[0] = target;
    frame[n - 1] = target;
}

fn detrend_linear(frame: &mut [f32], amount: f32) {
    let n = frame.len();
    if n < 2 {
        return;
    }
    let amount = amount.clamp(0.0, 1.0);
    if amount < 1e-6 {
        return;
    }
    let delta = (frame[n - 1] - frame[0]) * amount;
    if delta.abs() < 1e-12 {
        return;
    }
    let denom = (n - 1) as f32;
    for i in 0..n {
        frame[i] -= delta * (i as f32) / denom;
    }
}

fn crossfade_head_tail(frame: &mut [f32], fade: usize, ease: fn(f32) -> f32) {
    let n = frame.len();
    if fade == 0 || n < 8 {
        return;
    }
    let fade = fade.min(n / 2);
    let head: Vec<f32> = frame[..fade].to_vec();
    let tail: Vec<f32> = frame[n - fade..].to_vec();
    for i in 0..fade {
        let w = ease((i as f32 + 1.0) / (fade as f32 + 1.0));
        // Blend tail toward head[i]
        frame[n - fade + i] = tail[i] * (1.0 - w) + head[i] * w;
        // Blend head toward tail[i] (symmetric soft join)
        frame[i] = head[i] * (1.0 - w) + tail[i] * w;
    }
    // Pin exact wrap to mean of ends after blend
    let m = 0.5 * (frame[0] + frame[n - 1]);
    frame[0] = m;
    frame[n - 1] = m;
}

fn slope_match(frame: &mut [f32], fade: usize) {
    let n = frame.len();
    if fade < 4 || n < 16 {
        classic_end_fade(frame, fade.max(2), smoothstep);
        return;
    }
    let fade = fade.min(n / 2);
    let y0 = frame[n - fade - 1];
    let y1 = frame[0];
    let d0 = frame[n - fade - 1] - frame[n - fade - 2];
    let d1 = frame[1] - frame[0];
    for i in 0..fade {
        let t = (i as f32 + 1.0) / (fade as f32 + 1.0);
        let t2 = t * t;
        let t3 = t2 * t;
        // Hermite basis
        let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
        let h10 = t3 - 2.0 * t2 + t;
        let h01 = -2.0 * t3 + 3.0 * t2;
        let h11 = t3 - t2;
        frame[n - fade + i] = h00 * y0 + h10 * d0 * fade as f32 + h01 * y1 + h11 * d1 * fade as f32;
    }
    frame[n - 1] = frame[0];
}

fn seam_box_polish(frame: &mut [f32], fade: usize) {
    let n = frame.len();
    if fade < 3 || n < 16 {
        return;
    }
    let fade = fade.min(n / 2);
    let mut tmp = frame.to_vec();
    for i in 0..fade {
        let idx = n - fade + i;
        let a = frame[idx.saturating_sub(1)];
        let b = frame[idx];
        let c = frame[(idx + 1).min(n - 1)];
        tmp[idx] = (a + b + c) / 3.0;
        let hi = i;
        let a = frame[if hi == 0 { 0 } else { hi - 1 }];
        let b = frame[hi];
        let c = frame[(hi + 1).min(n - 1)];
        tmp[hi] = (a + b + c) / 3.0;
    }
    frame.copy_from_slice(&tmp);
    let m = 0.5 * (frame[0] + frame[n - 1]);
    frame[0] = m;
    frame[n - 1] = m;
}

/// Apply a named algorithm at eliminate/amplify crackle amount.
pub fn periodize_with_algo(frame: &mut [f32], crackle: f32, style: SeamStyle, algo: PeriodizeAlgo) {
    let n = frame.len();
    if n < 8 {
        return;
    }
    let crackle = crackle.clamp(0.0, 1.0);
    if crackle >= 0.999 {
        return;
    }
    let clean = 1.0 - crackle;
    let seam = (frame[n - 1] - frame[0]).abs();

    match algo {
        PeriodizeAlgo::Classic => {
            let fade = fade_len(n, style, seam, clean, 1.0);
            classic_end_fade(frame, fade, |t| t * t);
        }
        PeriodizeAlgo::Cosine => {
            let fade = fade_len(n, style, seam, clean, 1.0);
            classic_end_fade(frame, fade, raised_cosine);
        }
        PeriodizeAlgo::DualEnd => {
            let fade = fade_len(n, style, seam, clean, 1.15);
            dual_end_fade(frame, fade, smoothstep);
        }
        PeriodizeAlgo::Detrend => {
            detrend_linear(frame, clean);
        }
        PeriodizeAlgo::Crossfade => {
            let fade = fade_len(n, style, seam, clean, 1.25);
            crossfade_head_tail(frame, fade, raised_cosine);
        }
        PeriodizeAlgo::SlopeMatch => {
            let fade = fade_len(n, style, seam, clean, 1.1);
            slope_match(frame, fade);
        }
        PeriodizeAlgo::DetrendCosine => {
            detrend_linear(frame, clean);
            let fade = fade_len(n, style, seam, clean, 0.55).max(if clean > 0.05 { 4 } else { 0 });
            classic_end_fade(frame, fade, raised_cosine);
        }
        PeriodizeAlgo::DualCosine => {
            let fade = fade_len(n, style, seam, clean, 1.2);
            dual_end_fade(frame, fade, raised_cosine);
        }
        PeriodizeAlgo::CrossDetrend => {
            let fade = fade_len(n, style, seam, clean, 1.1);
            crossfade_head_tail(frame, fade, smoothstep);
            detrend_linear(frame, clean);
        }
        PeriodizeAlgo::Ensemble => {
            detrend_linear(frame, clean);
            let fade = fade_len(n, style, seam, clean, 0.85).max(if clean > 0.05 { 6 } else { 0 });
            dual_end_fade(frame, fade, raised_cosine);
            if clean > 0.5 {
                frame[n - 1] = frame[0];
            }
        }
        PeriodizeAlgo::EnsembleV2 => {
            detrend_linear(frame, clean);
            let fade = fade_len(n, style, seam, clean, 1.05).max(if clean > 0.05 { 8 } else { 0 });
            dual_end_fade(frame, fade, raised_cosine);
            classic_end_fade(frame, (fade / 3).max(2), smoothstep);
            if clean > 0.5 {
                frame[n - 1] = frame[0];
            }
        }
        PeriodizeAlgo::EnsembleV3 => {
            detrend_linear(frame, clean);
            let fade = fade_len(n, style, seam, clean, 1.15).max(if clean > 0.05 { 10 } else { 0 });
            dual_end_fade(frame, fade, raised_cosine);
            seam_box_polish(frame, fade);
            classic_end_fade(frame, (fade / 4).max(2), raised_cosine);
            if clean > 0.5 {
                frame[n - 1] = frame[0];
            }
        }
        PeriodizeAlgo::DenoiseOpt => {
            crate::denoise_opt::apply_denoise_opt(frame, crackle);
        }
    }
}

fn risk(frame: &[f32]) -> f32 {
    let m = SignalMetrics::measure("x", frame);
    m.wrap * 2.0 + m.max_step + m.hf * 0.35 + m.above_1 as f32 * 0.001
}

fn mae(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len()).max(1) as f32;
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).abs())
        .sum::<f32>()
        / n
}

/// Score one algorithm on harsh fixtures (lower is better).
pub fn score_algo(algo: PeriodizeAlgo) -> (f32, f32, f32) {
    let cat = catalog(512);
    let harsh = [
        "saw_raw",
        "square_50",
        "quant_open_ramp",
        "quant_ends_opposite",
        "va_saw",
        "quant_step_hold",
        "triangle_raw",
    ];
    let mut sum_risk = 0.0f32;
    let mut sum_raw = 0.0f32;
    let mut sum_mae = 0.0f32;
    let mut count = 0u32;
    for id in harsh {
        let Some(src) = cat.iter().find(|s| s.id == id) else {
            continue;
        };
        let raw = &src.samples;
        let mut out = raw.clone();
        periodize_with_algo(&mut out, 0.0, SeamStyle::Adaptive, algo);
        sum_risk += risk(&out);
        sum_raw += risk(raw);
        sum_mae += mae(&out, raw);
        count += 1;
    }
    let c = count.max(1) as f32;
    let mean_risk = sum_risk / c;
    let mean_raw = sum_raw / c;
    let mean_mae = sum_mae / c;
    // Composite: prioritize artifact drop; light MAE penalty so we don't flatten cycles.
    let composite = mean_risk + 0.08 * mean_mae;
    (composite, mean_risk, mean_raw)
}

/// Run ≥10 improvement iterations: each step evaluates the next algo family and
/// keeps a running best. Returns JSON history for plotting.
pub fn run_improvement_iterations() -> serde_json::Value {
    let mut history = Vec::new();
    let mut best_composite = f32::MAX;
    let mut best_algo = PeriodizeAlgo::Classic;
    let mut best_risk = f32::MAX;
    let mut raw_mean = 0.0f32;

    for (iter, &algo) in PeriodizeAlgo::ALL.iter().enumerate() {
        let (composite, mean_risk, mean_raw) = score_algo(algo);
        if iter == 0 {
            raw_mean = mean_raw;
        }
        let improved = composite < best_composite - 1e-6;
        if improved {
            best_composite = composite;
            best_algo = algo;
            best_risk = mean_risk;
        }
        let pct_vs_raw = if raw_mean > 1e-9 {
            (1.0 - mean_risk / raw_mean) * 100.0
        } else {
            0.0
        };
        let classic_risk = score_algo(PeriodizeAlgo::Classic).1;
        let pct_vs_baseline = if classic_risk > 1e-9 {
            (1.0 - mean_risk / classic_risk) * 100.0
        } else {
            0.0
        };
        history.push(json!({
            "iteration": iter + 1,
            "algo": algo.label(),
            "composite": composite,
            "mean_artifact": mean_risk,
            "mean_raw": mean_raw,
            "pct_reduced_vs_raw": pct_vs_raw,
            "pct_improved_vs_classic": pct_vs_baseline,
            "is_running_best": best_algo == algo,
            "running_best_algo": best_algo.label(),
            "running_best_artifact": best_risk,
        }));
    }

    // Sanity: production BEST must match the measured winner.
    assert_eq!(
        best_algo,
        PeriodizeAlgo::BEST,
        "PeriodizeAlgo::BEST is stale — update BEST to {:?}",
        best_algo
    );

    let report = json!({
        "title": "Artifact Reduction",
        "winner": best_algo.label(),
        "winner_artifact": best_risk,
        "raw_mean_artifact": raw_mean,
        "pct_reduced_vs_raw": if raw_mean > 1e-9 {
            (1.0 - best_risk / raw_mean) * 100.0
        } else {
            0.0
        },
        "iterations": history,
        "sessionId": "0ab8f9",
        "runId": "artifact-reduce-12",
    });

    // Persist for the share plot + debug ingest.
    let _ = std::fs::create_dir_all("brand/artifacts");
    if let Ok(s) = serde_json::to_string_pretty(&report) {
        let _ = std::fs::write("brand/artifacts/artifact_reduction_history.json", s);
    }
    // #region agent log
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("debug-0ab8f9.log")
    {
        use std::io::Write;
        let _ = writeln!(
            f,
            "{}",
            json!({
                "sessionId": "0ab8f9",
                "runId": "artifact-reduce-12",
                "hypothesisId": "H-improve",
                "location": "artifact_reduce.rs",
                "message": "improvement iterations complete",
                "data": {
                    "winner": best_algo.label(),
                    "winner_artifact": best_risk,
                    "raw_mean": raw_mean,
                    "n_iters": history.len(),
                },
                "timestamp": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0),
            })
        );
    }
    // #endregion

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seam::periodize_cycle;

    #[test]
    fn improvement_loop_at_least_10_and_beats_classic() {
        let report = run_improvement_iterations();
        let iters = report["iterations"].as_array().unwrap();
        assert!(iters.len() >= 10, "need ≥10 iterations, got {}", iters.len());
        let classic = iters
            .iter()
            .find(|r| r["algo"] == "classic")
            .unwrap()["mean_artifact"]
            .as_f64()
            .unwrap();
        let winner = report["winner_artifact"].as_f64().unwrap();
        assert!(
            winner <= classic + 1e-4,
            "winner {winner} should not be worse than classic {classic}"
        );
        // Best should reduce vs untreated raw.
        assert!(report["pct_reduced_vs_raw"].as_f64().unwrap() > 50.0);
        eprintln!(
            "winner={} artifact={:.4} vs classic={:.4} (−{:.1}% vs raw)",
            report["winner"],
            winner,
            classic,
            report["pct_reduced_vs_raw"].as_f64().unwrap()
        );
    }

    #[test]
    fn production_periodize_matches_best_algo() {
        let cat = catalog(256);
        let src = cat.iter().find(|s| s.id == "quant_open_ramp").unwrap();
        let mut a = src.samples.clone();
        let mut b = src.samples.clone();
        periodize_cycle(&mut a, 0.0, SeamStyle::Adaptive);
        periodize_with_algo(&mut b, 0.0, SeamStyle::Adaptive, PeriodizeAlgo::BEST);
        let err = a
            .iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).abs())
            .fold(0.0f32, f32::max);
        assert!(err < 1e-5, "seam::periodize_cycle must use BEST algo, err={err}");
    }

    #[test]
    fn amplify_still_noop() {
        let mut f: Vec<f32> = (0..128).map(|i| i as f32 / 127.0).collect();
        let before = f.clone();
        periodize_with_algo(&mut f, 1.0, SeamStyle::Adaptive, PeriodizeAlgo::BEST);
        assert_eq!(f, before);
    }
}

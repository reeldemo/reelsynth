//! Unsupervised crackle denoise: fit θ once on denoise+shape loss, infer with frozen θ.
//!
//! No labeled data. Offline coordinate descent minimizes
//! `L = (1 − denoise) + λ(1 − shape)` on the harsh signal matrix.
//!
//! **Shape invariant:** mid-cycle samples are copied from the input; only head/tail
//! seam zones are rewritten. That is how aggressive denoise stays shape-safe.

use crate::signal_library::{catalog, SignalMetrics};
use serde_json::json;

pub const N_THETA: usize = 12;
pub const LAMBDA_SHAPE: f32 = 1.0;

/// Frozen after 1500-trial residual-objective bi-level meta (champion meta_top1 /
/// `evo_explore_515`). Primary score: prolonged residual vs ideal. Regenerate via
/// `bench_denoise_meta`.
pub const FROZEN_THETA: [f32; N_THETA] = [
    0.6743, // detrend / seam pull
    0.3573, // fade length scale
    0.7695, // dual target blend
    0.2606, // raised-cosine weight
    0.7395, // secondary tail fade
    0.4262, // ease gamma
    0.2884, // polish wet
    0.0,    // reserved (mid always dry)
    0.4770, // head/tail asymmetry
    0.6788, // wrap pin
    0.4341, // base fade scale knob
    0.4200, // second polish wet
];

#[derive(Debug, Clone, Copy)]
pub struct QualityScores {
    pub denoise: f32,
    pub shape: f32,
    pub quality: f32,
    pub loss: f32,
    pub crackle_raw: f32,
    pub crackle_out: f32,
}

fn crackle_c(frame: &[f32]) -> f32 {
    let m = SignalMetrics::measure("x", frame);
    m.wrap * 2.0 + m.max_step + m.hf * 0.35 + m.above_1 as f32 * 0.001
}

fn mae_band(a: &[f32], b: &[f32], lo: usize, hi: usize) -> f32 {
    if hi <= lo {
        return 0.0;
    }
    let n = (hi - lo) as f32;
    let mut s = 0.0f32;
    for i in lo..hi {
        s += (a[i] - b[i]).abs();
    }
    s / n
}

fn rms(frame: &[f32]) -> f32 {
    let n = frame.len().max(1) as f32;
    (frame.iter().map(|x| x * x).sum::<f32>() / n).sqrt()
}

/// Default periods when scoring prolonged cyclic playback vs an ideal reference.
pub const RESIDUAL_PROLONG_PERIODS: usize = 16;

/// Tile one baked cycle `periods` times (engine cyclic / wrap playback).
pub fn tile_cycle(cycle: &[f32], periods: usize) -> Vec<f32> {
    let periods = periods.max(1);
    let n = cycle.len();
    let mut out = Vec::with_capacity(n * periods);
    for _ in 0..periods {
        out.extend_from_slice(cycle);
    }
    out
}

/// Residual score ∈ [0, 1] (1 = best): prolonged engine playback vs ideal reference.
///
/// `score = clamp(1 − residual_rms / max(ideal_rms, ε), 0, 1)`
///
/// Monotone: lower residual energy → higher score. Stable across amplitude families
/// because residual is normalized by ideal RMS.
pub fn residual_score(ideal: &[f32], rendered: &[f32]) -> f32 {
    let n = ideal.len().min(rendered.len());
    if n == 0 {
        return 0.0;
    }
    let mut e_res = 0.0f32;
    let mut e_id = 0.0f32;
    for i in 0..n {
        let r = rendered[i] - ideal[i];
        e_res += r * r;
        e_id += ideal[i] * ideal[i];
    }
    let inv_n = 1.0 / n as f32;
    let residual_rms = (e_res * inv_n).sqrt();
    let ideal_rms = (e_id * inv_n).sqrt();
    (1.0 - residual_rms / ideal_rms.max(1e-6)).clamp(0.0, 1.0)
}

/// Prolonged residual score: tile `out` vs tile `ideal_cycle` over `periods`.
pub fn residual_score_prolonged(ideal_cycle: &[f32], out_cycle: &[f32], periods: usize) -> f32 {
    let ideal = tile_cycle(ideal_cycle, periods);
    let rendered = tile_cycle(out_cycle, periods);
    residual_score(&ideal, &rendered)
}

pub fn score_cycle(raw: &[f32], out: &[f32]) -> QualityScores {
    let c_raw = crackle_c(raw);
    let c_out = crackle_c(out);
    let denoise = if c_raw < 1e-6 {
        1.0
    } else {
        ((c_raw - c_out) / c_raw).clamp(0.0, 1.0)
    };
    let n = raw.len();
    let guard = (n / 8).max(4).min(n / 3);
    let mae = mae_band(out, raw, guard, n.saturating_sub(guard));
    let shape = 1.0 - (mae / (rms(raw) + 1e-6)).clamp(0.0, 1.0);
    let loss = (1.0 - denoise) + LAMBDA_SHAPE * (1.0 - shape);
    QualityScores {
        denoise,
        shape,
        quality: 0.5 * (denoise + shape),
        loss,
        crackle_raw: c_raw,
        crackle_out: c_out,
    }
}

fn raised_cosine(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    0.5 - 0.5 * (std::f32::consts::PI * t).cos()
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn ease_mix(t: f32, w_cos: f32, gamma: f32) -> f32 {
    let e = raised_cosine(t) * w_cos + smoothstep(t) * (1.0 - w_cos);
    let g = 0.5 + gamma * 1.5;
    e.powf(g)
}

/// Seam-local deep stack. Mid-cycle is never written (shape conserved by construction).
pub fn apply_denoise_theta(frame: &mut [f32], crackle: f32, theta: &[f32; N_THETA]) {
    let n = frame.len();
    if n < 16 {
        return;
    }
    let crackle = crackle.clamp(0.0, 1.0);
    if crackle >= 0.999 {
        return;
    }
    let clean = 1.0 - crackle;
    let dry = frame.to_vec();

    let t0 = theta[0];
    let t1 = theta[1];
    let t2 = theta[2];
    let t3 = theta[3];
    let t4 = theta[4];
    let t5 = theta[5];
    let t6 = theta[6];
    let t8 = theta[8];
    let t9 = theta[9];
    let t10 = 0.45 + theta[10] * 1.1; // ~0.45..1.55 fade scale
    let t11 = theta[11];

    let seam = (dry[n - 1] - dry[0]).abs();
    let base = if seam < 0.02 {
        8usize
    } else {
        let u = (seam / 2.0).clamp(0.0, 1.0);
        (8.0 + u * ((n / 7).max(56).min(220) as f32 - 8.0)).round() as usize
    };
    let fade = ((base as f32) * t10 * t1 * clean * clean)
        .round()
        .max(if clean > 0.05 { 6.0 } else { 0.0 }) as usize;
    let fade = fade.min(n / 3).max(if fade > 0 { 4 } else { 0 });
    if fade == 0 {
        return;
    }

    let fade_h = ((fade as f32) * (0.55 + 0.9 * t8)).round() as usize;
    let fade_h = fade_h.min(n / 3).max(4);
    let fade_t = ((fade as f32) * (1.45 - 0.9 * t8)).round() as usize;
    let fade_t = fade_t.min(n / 3).max(4);

    // Work copy for seam zones only
    let mut work = dry.clone();

    // Linear detrend contribution only expressed through seam rewrite (full-frame
    // detrend would move mid). Approximate: blend ends toward closed after fade.
    let a0 = dry[0];
    let a1 = dry[n - 1];
    let closed0 = a0 - t0 * clean * (a1 - a0) * 0.0; // keep a0; close via target
    let _ = closed0;
    let target = a0 * (1.0 - t2) + (0.5 * (a0 + a1)) * t2;

    for i in 0..fade_t {
        let w = ease_mix((i as f32 + 1.0) / (fade_t as f32 + 1.0), t3, t5) * clean;
        let ti = n - fade_t + i;
        // Also pull toward periodized end: mix dry[ti] with target and with dry[i] mirror
        let mirror = dry[i.min(fade_h.saturating_sub(1))];
        let goal = target * (0.65 + 0.35 * t0) + mirror * (0.35 * (1.0 - t0));
        work[ti] = dry[ti] * (1.0 - w) + goal * w;
    }
    for i in 0..fade_h {
        let w = ease_mix((i as f32 + 1.0) / (fade_h as f32 + 1.0), t3, t5) * clean;
        let hi = fade_h - 1 - i;
        let mirror = dry[(n - 1 - i).min(n - 1)];
        let goal = target * (0.65 + 0.35 * t0) + mirror * (0.35 * (1.0 - t0));
        work[hi] = dry[hi] * (1.0 - w) + goal * w;
    }

    // Secondary tail fade toward frame[0]
    let fade2 = ((fade_t as f32) * t4 * 0.75).round() as usize;
    let fade2 = fade2.min(n / 3).max(0);
    if fade2 >= 3 {
        let start = work[0];
        for i in 0..fade2 {
            let w = ease_mix((i as f32 + 1.0) / (fade2 as f32 + 1.0), t3, t5) * clean;
            let idx = n - fade2 + i;
            work[idx] = work[idx] * (1.0 - w) + start * w;
        }
    }

    // Polish only inside seam zones
    let polish_zone = |work: &mut [f32], wet: f32, width: usize| {
        if wet * clean < 1e-4 || width < 3 {
            return;
        }
        let width = width.min(n / 3);
        let src = work.to_vec();
        let wet = wet * clean;
        for i in 0..width {
            for &idx in &[i, n - width + i] {
                let a = src[idx.saturating_sub(1)];
                let b = src[idx];
                let c = src[(idx + 1).min(n - 1)];
                work[idx] = b * (1.0 - wet) + ((a + b + c) / 3.0) * wet;
            }
        }
    };
    polish_zone(&mut work, t6, fade.max(6));
    polish_zone(&mut work, t11, (fade / 2).max(4));

    // Pin wrap
    if t9 * clean > 1e-4 {
        let pin = 0.5 * (work[0] + work[n - 1]);
        let w = (t9 * clean).clamp(0.0, 1.0);
        work[0] = work[0] * (1.0 - w) + pin * w;
        work[n - 1] = work[n - 1] * (1.0 - w) + pin * w;
    }

    // Commit: seam zones from work, mid-cycle exact dry copy
    let lo = fade_h.max(fade_t);
    let hi = n.saturating_sub(fade_t.max(fade_h));
    for i in 0..n {
        if i < lo || i >= hi {
            frame[i] = work[i];
        } else {
            frame[i] = dry[i];
        }
    }
}

pub fn apply_denoise_opt(frame: &mut [f32], crackle: f32) {
    apply_denoise_theta(frame, crackle, &FROZEN_THETA);
}

fn harsh_ids() -> &'static [&'static str] {
    &[
        "saw_raw",
        "square_50",
        "quant_open_ramp",
        "quant_ends_opposite",
        "va_saw",
        "quant_step_hold",
        "triangle_raw",
    ]
}

pub fn eval_theta(theta: &[f32; N_THETA]) -> (f32, f32, f32, f32) {
    let cat = catalog(512);
    let mut sum_l = 0.0f32;
    let mut sum_d = 0.0f32;
    let mut sum_s = 0.0f32;
    let mut sum_q = 0.0f32;
    let mut n = 0u32;
    for id in harsh_ids() {
        let Some(src) = cat.iter().find(|s| s.id == *id) else {
            continue;
        };
        let mut out = src.samples.clone();
        apply_denoise_theta(&mut out, 0.0, theta);
        let q = score_cycle(&src.samples, &out);
        sum_l += q.loss;
        sum_d += q.denoise;
        sum_s += q.shape;
        sum_q += q.quality;
        n += 1;
    }
    let c = n.max(1) as f32;
    (sum_l / c, sum_d / c, sum_s / c, sum_q / c)
}

fn clamp_theta(theta: &mut [f32; N_THETA]) {
    for t in theta.iter_mut() {
        *t = t.clamp(0.0, 1.0);
    }
}

pub fn fit_denoise_theta(restarts: usize, sweeps: usize) -> ([f32; N_THETA], f32, f32, f32, f32) {
    let mut best = FROZEN_THETA;
    let (mut best_l, mut best_d, mut best_s, mut best_q) = eval_theta(&best);
    let step_grid: [f32; 5] = [0.25, 0.12, 0.06, 0.03, 0.015];

    let mut seed = 0xA11CEu64;
    let mut rnd = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 33) as f32 / (u32::MAX as f32)
    };

    for r in 0..restarts {
        let mut theta = if r == 0 {
            FROZEN_THETA
        } else if r == 1 {
            [0.5; N_THETA]
        } else if r == 2 {
            [1.0, 1.0, 0.5, 1.0, 0.5, 0.5, 0.8, 0.0, 0.5, 1.0, 1.0, 0.5]
        } else {
            let mut t = [0.0; N_THETA];
            for x in t.iter_mut() {
                *x = rnd();
            }
            t[7] = 0.0; // mid always dry
            t
        };
        clamp_theta(&mut theta);
        let (mut cur_l, _, _, _) = eval_theta(&theta);

        for &step in &step_grid {
            for _ in 0..sweeps {
                for i in 0..N_THETA {
                    if i == 7 {
                        continue; // reserved
                    }
                    let base = theta[i];
                    let mut local_best = cur_l;
                    let mut local_val = base;
                    for &delta in &[-step, step] {
                        theta[i] = (base + delta).clamp(0.0, 1.0);
                        let (l, _, _, _) = eval_theta(&theta);
                        if l + 1e-7 < local_best {
                            local_best = l;
                            local_val = theta[i];
                        }
                    }
                    theta[i] = local_val;
                    cur_l = local_best;
                }
            }
        }

        let (l, d, s, q) = eval_theta(&theta);
        if l < best_l - 1e-6 {
            best_l = l;
            best_d = d;
            best_s = s;
            best_q = q;
            best = theta;
        }
    }
    (best, best_l, best_d, best_s, best_q)
}

pub fn run_quality_gate_report() -> serde_json::Value {
    use crate::artifact_reduce::{periodize_with_algo, PeriodizeAlgo};
    use crate::seam::SeamStyle;

    let cat = catalog(512);
    let mut rows = Vec::new();
    let mut sum_opt = [0.0f32; 3];
    let mut sum_dual = [0.0f32; 3];
    let mut sum_classic = [0.0f32; 3];
    let mut n = 0u32;

    for id in harsh_ids() {
        let Some(src) = cat.iter().find(|s| s.id == *id) else {
            continue;
        };
        let raw = &src.samples;
        let mut opt = raw.clone();
        apply_denoise_opt(&mut opt, 0.0);
        let mut dual = raw.clone();
        periodize_with_algo(&mut dual, 0.0, SeamStyle::Adaptive, PeriodizeAlgo::DualCosine);
        let mut classic = raw.clone();
        periodize_with_algo(&mut classic, 0.0, SeamStyle::Adaptive, PeriodizeAlgo::Classic);

        let qo = score_cycle(raw, &opt);
        let qd = score_cycle(raw, &dual);
        let qc = score_cycle(raw, &classic);
        sum_opt[0] += qo.denoise;
        sum_opt[1] += qo.shape;
        sum_opt[2] += qo.quality;
        sum_dual[0] += qd.denoise;
        sum_dual[1] += qd.shape;
        sum_dual[2] += qd.quality;
        sum_classic[0] += qc.denoise;
        sum_classic[1] += qc.shape;
        sum_classic[2] += qc.quality;
        n += 1;
        rows.push(json!({
            "id": id,
            "opt": { "denoise": qo.denoise, "shape": qo.shape, "quality": qo.quality },
            "dual_cosine": { "denoise": qd.denoise, "shape": qd.shape, "quality": qd.quality },
            "classic": { "denoise": qc.denoise, "shape": qc.shape, "quality": qc.quality },
        }));
    }
    let c = n.max(1) as f32;
    for s in [&mut sum_opt, &mut sum_dual, &mut sum_classic] {
        s[0] /= c;
        s[1] /= c;
        s[2] /= c;
    }

    let pass = sum_opt[2] + 1e-4 >= sum_dual[2] - 0.02
        && sum_opt[0] + 0.05 >= sum_dual[0]
        && sum_opt[1] >= 0.95;

    let report = json!({
        "sessionId": "0ab8f9",
        "runId": "denoise-opt-gate",
        "pass": pass,
        "opt": { "denoise": sum_opt[0], "shape": sum_opt[1], "quality": sum_opt[2] },
        "dual_cosine": { "denoise": sum_dual[0], "shape": sum_dual[1], "quality": sum_dual[2] },
        "classic": { "denoise": sum_classic[0], "shape": sum_classic[1], "quality": sum_classic[2] },
        "frozen_theta": FROZEN_THETA.as_slice(),
        "fixtures": rows,
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
    let _ = std::fs::create_dir_all("brand/artifacts");
    if let Ok(s) = serde_json::to_string_pretty(&report) {
        let _ = std::fs::write("brand/artifacts/denoise_opt_gate.json", s);
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_improves_and_gate_passes() {
        let (theta, loss, denoise, shape, quality) = fit_denoise_theta(4, 2);
        eprintln!(
            "fit θ loss={loss:.4} denoise={denoise:.4} shape={shape:.4} quality={quality:.4}"
        );
        eprintln!("FITTED_THETA={theta:?}");
        assert!(denoise > 0.55, "aggressive denoise expected, got {denoise}");
        assert!(shape > 0.85, "mid-cycle shape must stay, got {shape}");
        assert!(quality > 0.70, "quality={quality}");

        let report = run_quality_gate_report();
        eprintln!(
            "gate pass={} opt={:?} dual={:?}",
            report["pass"], report["opt"], report["dual_cosine"]
        );
        assert!(
            report["pass"].as_bool().unwrap_or(false),
            "DenoiseOpt must pass quality gate vs DualCosine: {}",
            serde_json::to_string_pretty(&report).unwrap()
        );
    }

    #[test]
    fn amplify_noop() {
        let mut f: Vec<f32> = (0..64).map(|i| i as f32 / 63.0).collect();
        let before = f.clone();
        apply_denoise_opt(&mut f, 1.0);
        assert_eq!(f, before);
    }

    #[test]
    fn mid_cycle_unchanged() {
        let mut f: Vec<f32> = (0..256)
            .map(|i| (i as f32 / 255.0 * std::f32::consts::TAU).sin())
            .collect();
        // open the wrap
        f[0] = -0.9;
        f[255] = 0.9;
        let before = f.clone();
        apply_denoise_opt(&mut f, 0.0);
        let lo = 256 / 3;
        let hi = 256 - 256 / 3;
        for i in lo..hi {
            assert!(
                (f[i] - before[i]).abs() < 1e-6,
                "mid[{i}] changed"
            );
        }
    }

    #[test]
    fn inference_is_fast_budget() {
        let mut f: Vec<f32> = (0..2048)
            .map(|i| -1.0 + 2.0 * i as f32 / 2047.0)
            .collect();
        let t0 = std::time::Instant::now();
        for _ in 0..200 {
            apply_denoise_opt(&mut f, 0.0);
            for (i, x) in f.iter_mut().enumerate() {
                *x = -1.0 + 2.0 * i as f32 / 2047.0;
            }
        }
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        eprintln!("200×2048 denoise_opt: {ms:.2} ms");
        assert!(ms < 500.0, "inference too slow: {ms} ms");
    }

    #[test]
    fn residual_score_perfect_match_is_one() {
        let wave: Vec<f32> = (0..128)
            .map(|i| (i as f32 / 128.0 * std::f32::consts::TAU).sin())
            .collect();
        let s = residual_score_prolonged(&wave, &wave, 16);
        assert!((s - 1.0).abs() < 1e-5, "got {s}");
    }

    #[test]
    fn residual_score_wrap_cliff_near_zero() {
        let ideal: Vec<f32> = (0..128)
            .map(|i| (i as f32 / 128.0 * std::f32::consts::TAU).sin())
            .collect();
        let mut bad = ideal.clone();
        bad[0] = -3.0;
        bad[127] = 3.0;
        let s = residual_score_prolonged(&ideal, &bad, 16);
        assert!(s < 0.55, "wrap cliff residual score={s}");
        assert!(s < 0.99, "cliff must score below perfect");
        assert!((0.0..=1.0).contains(&s));
    }

    #[test]
    fn residual_score_clamped_unit_interval() {
        let ideal = [0.5f32; 32];
        let huge: Vec<f32> = (0..32).map(|i| if i == 0 { 100.0 } else { -100.0 }).collect();
        let s = residual_score(&ideal, &huge);
        assert!((0.0..=1.0).contains(&s), "got {s}");
        assert_eq!(residual_score(&[], &[1.0]), 0.0);
    }
}

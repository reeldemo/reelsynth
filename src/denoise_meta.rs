//! Meta-learning + literature-informed hyperparameter / algorithm search.
//!
//! ## Two entry points
//! - [`run_meta_learning_search_n`] — legacy 1500-trial θ/λ search (prior buckets).
//! - [`run_lit_combo_meta_n`] — lit-family catalog + combinatorial hybrids; each
//!   trial **fits until convergence** (see [`CONV_EPS`] / [`CONV_PATIENCE`]).
//!
//! ## Literature-derived algorithm families (short citation names)
//! | Family | Inspiration | Role here |
//! |--------|-------------|-----------|
//! | `baseline_bake` | DualCosine / Classic / Soft / Ensemble* (`artifact_reduce`) | Fixed bake baselines |
//! | `bayes_local` | Bayesian / local HPO (Snoek/BOHB-style densify) | Sample near good λ |
//! | `pbt_exploit` | PBT (Jaderberg et al.) | Exploit elite + mutate |
//! | `irace_racing` | irace / racing (López-Ibáñez) | Progressive discard |
//! | `moead_shape` | MOEA/D (Zhang & Li) | Weighted residual↔shape |
//! | `evo_explore` | Evolutionary wide search | Broad θ mutation |
//! | `n2n_unsup` | Noise2Noise (Lehtinen et al.) | Label-free loss fit |
//! | `bilevel_nested` | Nested / bi-level opt | Inner L, outer residual |
//!
//! Combinatorial hybrids sample pairs/triples of operator components
//! (e.g. `race+pbt`, `evo+loss_opt`, `mo_shape+residual_primary`).
//!
//! **Primary meta objective:** prolonged residual score ∈ [0, 1] (1 = best).

use crate::artifact_reduce::{periodize_with_algo, PeriodizeAlgo};
use crate::denoise_opt::{
    apply_denoise_opt, apply_denoise_theta, residual_score_prolonged, FROZEN_THETA, N_THETA,
    RESIDUAL_PROLONG_PERIODS,
};
use crate::seam::SeamStyle;
use crate::sound_bench::{
    crackle_fast, generate_sound, generate_sound_ideal, BenchFamily, BENCH_N,
};
use serde_json::json;

const N_TRIALS_DEFAULT: usize = 1500;
const VAL_FAST_DEFAULT: usize = 400;
const VAL_FINAL_DEFAULT: usize = 2000;
const PROLONG: usize = RESIDUAL_PROLONG_PERIODS;
/// Seeds used for each trial's inner unsupervised loss fit.
const INNER_FIT_COUNT: usize = 48;
const INNER_FIT_START: u64 = 40_000;

/// Relative improvement threshold for inner convergence.
/// Stop when `|L_prev - L_cur| / max(|L_prev|, 1e-6) < CONV_EPS` for
/// [`CONV_PATIENCE`] consecutive coordinate sweeps (or residual analog).
pub const CONV_EPS: f32 = 1e-4;
/// Consecutive plateau sweeps required before declaring convergence.
pub const CONV_PATIENCE: usize = 3;
/// Hard cap on inner coordinate sweeps (early-stop still applies).
pub const CONV_MAX_SWEEPS: usize = 16;
/// Step schedule for coordinate descent (coarse → fine).
const CONV_STEPS: [f32; 3] = [0.12, 0.06, 0.03];

/// Lit-informed operator atoms that may be combined into hybrid algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum OpAtom {
    BayesLocal,
    PbtExploit,
    IraceRacing,
    MoeadShape,
    EvoExplore,
    N2nUnsup,
    BilevelNested,
    ResidualPrimary,
    BakeDualCosine,
    BakeClassic,
    BakeSoft,
    BakeEnsembleV3,
    BakeCrossfade,
}

impl OpAtom {
    fn label(self) -> &'static str {
        match self {
            OpAtom::BayesLocal => "bayes_local",
            OpAtom::PbtExploit => "pbt_exploit",
            OpAtom::IraceRacing => "irace_racing",
            OpAtom::MoeadShape => "moead_shape",
            OpAtom::EvoExplore => "evo_explore",
            OpAtom::N2nUnsup => "n2n_unsup",
            OpAtom::BilevelNested => "bilevel_nested",
            OpAtom::ResidualPrimary => "residual_primary",
            OpAtom::BakeDualCosine => "bake_dual_cosine",
            OpAtom::BakeClassic => "bake_classic",
            OpAtom::BakeSoft => "bake_soft",
            OpAtom::BakeEnsembleV3 => "bake_ensemble_v3",
            OpAtom::BakeCrossfade => "bake_crossfade",
        }
    }

    const SEARCH: &'static [OpAtom] = &[
        OpAtom::BayesLocal,
        OpAtom::PbtExploit,
        OpAtom::IraceRacing,
        OpAtom::MoeadShape,
        OpAtom::EvoExplore,
        OpAtom::N2nUnsup,
        OpAtom::BilevelNested,
        OpAtom::ResidualPrimary,
    ];

    const BAKE: &'static [OpAtom] = &[
        OpAtom::BakeDualCosine,
        OpAtom::BakeClassic,
        OpAtom::BakeSoft,
        OpAtom::BakeEnsembleV3,
        OpAtom::BakeCrossfade,
    ];
}

#[derive(Debug, Clone)]
struct TrialHp {
    name: String,
    lambda_shape: f32,
    fade_scale_bias: f32,
    polish_bias: f32,
    pin_bias: f32,
    detrend_bias: f32,
    ease_bias: f32,
    /// Inner coordinate-descent sweeps (0 = mutate-only baseline).
    loss_opt_sweeps: usize,
    /// If true, also refine λ by ±δ after θ fit (joint nested).
    refine_lambda: bool,
    algo_seed: u64,
    prior: &'static str,
}

/// Hybrid / combo trial configuration for lit-combo meta.
#[derive(Debug, Clone)]
pub(crate) struct ComboTrial {
    pub name: String,
    pub family: String,
    pub ops: Vec<&'static str>,
    pub lambda_shape: f32,
    pub residual_primary: bool,
    pub use_racing: bool,
    pub use_pbt: bool,
    pub use_mo_weight: f32,
    pub bake: Option<PeriodizeAlgo>,
    pub seam: SeamStyle,
    pub theta_polish: bool,
    pub refine_lambda: bool,
    pub algo_seed: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct FitResult {
    pub theta: [f32; N_THETA],
    pub lambda: f32,
    pub fit_loss: f32,
    pub conv_steps: usize,
    pub converged: bool,
}

/// Auxiliary D/S wrap-energy proxy (kept for reports; not meta ranking).
pub(crate) fn score_with_lambda(raw: &[f32], out: &[f32], lambda: f32) -> (f32, f32, f32, f32) {
    let c_raw = crackle_fast(raw);
    let c_out = crackle_fast(out);
    let denoise = if c_raw < 1e-6 {
        1.0
    } else {
        ((c_raw - c_out) / c_raw).clamp(0.0, 1.0)
    };
    let n = raw.len();
    let guard = (n / 8).max(4).min(n / 3);
    let mut mae = 0.0f32;
    let mut cnt = 0u32;
    for i in guard..n.saturating_sub(guard) {
        mae += (out[i] - raw[i]).abs();
        cnt += 1;
    }
    mae /= cnt.max(1) as f32;
    let rms = (raw.iter().map(|x| x * x).sum::<f32>() / n as f32).sqrt();
    let shape = 1.0 - (mae / (rms + 1e-6)).clamp(0.0, 1.0);
    let loss = (1.0 - denoise) + lambda * (1.0 - shape);
    (loss, denoise, shape, 0.5 * (denoise + shape))
}

pub(crate) fn residual_for_cycle(ideal: &[f32], out: &[f32]) -> f32 {
    residual_score_prolonged(ideal, out, PROLONG)
}

pub(crate) const PROLONG_PERIODS: usize = PROLONG;
pub(crate) const INNER_FIT_COUNT_PUB: usize = INNER_FIT_COUNT;
pub(crate) const INNER_FIT_START_PUB: u64 = INNER_FIT_START;

/// Soft shape gate: keep mid-cycle preservation as a secondary constraint.
pub(crate) fn meta_rank(residual: f32, shape: f32) -> f32 {
    if shape >= 0.97 {
        residual
    } else {
        residual * 0.45
    }
}

pub(crate) fn apply_pipeline(
    frame: &mut [f32],
    theta: &[f32; N_THETA],
    bake: Option<PeriodizeAlgo>,
    seam: SeamStyle,
    theta_polish: bool,
) {
    if let Some(algo) = bake {
        periodize_with_algo(frame, 0.0, seam, algo);
        if theta_polish {
            let mut polished = frame.to_vec();
            apply_denoise_theta(&mut polished, 0.0, theta);
            let wet = theta[6].clamp(0.0, 1.0);
            for (a, b) in frame.iter_mut().zip(polished.iter()) {
                *a = *a * (1.0 - wet) + *b * wet;
            }
        }
    } else {
        apply_denoise_theta(frame, 0.0, theta);
    }
}

fn mean_unsupervised_loss(
    theta: &[f32; N_THETA],
    start_seed: u64,
    count: usize,
    n: usize,
    lambda: f32,
) -> f32 {
    mean_unsupervised_loss_pipe(theta, start_seed, count, n, lambda, None, SeamStyle::Adaptive, false)
}

pub(crate) fn mean_unsupervised_loss_pipe(
    theta: &[f32; N_THETA],
    start_seed: u64,
    count: usize,
    n: usize,
    lambda: f32,
    bake: Option<PeriodizeAlgo>,
    seam: SeamStyle,
    theta_polish: bool,
) -> f32 {
    let mut sum = 0.0f32;
    for k in 0..count {
        let seed = start_seed + k as u64;
        let (_, raw) = generate_sound(seed, n);
        let mut out = raw.clone();
        apply_pipeline(&mut out, theta, bake, seam, theta_polish);
        let (l, _, _, _) = score_with_lambda(&raw, &out, lambda);
        sum += l;
    }
    sum / count.max(1) as f32
}

pub(crate) fn mean_residual_fit(
    theta: &[f32; N_THETA],
    start_seed: u64,
    count: usize,
    n: usize,
    bake: Option<PeriodizeAlgo>,
    seam: SeamStyle,
    theta_polish: bool,
) -> f32 {
    let mut sum = 0.0f32;
    for k in 0..count {
        let seed = start_seed + k as u64;
        let (_, ideal) = generate_sound_ideal(seed, n);
        let (_, raw) = generate_sound(seed, n);
        let mut out = raw;
        apply_pipeline(&mut out, theta, bake, seam, theta_polish);
        sum += residual_for_cycle(&ideal, &out);
    }
    sum / count.max(1) as f32
}

fn mean_mo_objective(
    theta: &[f32; N_THETA],
    start_seed: u64,
    count: usize,
    n: usize,
    lambda: f32,
    mo_w: f32,
    bake: Option<PeriodizeAlgo>,
    seam: SeamStyle,
    theta_polish: bool,
) -> f32 {
    // Minimize: (1-w)*(1-residual) + w*L_unsup  (MOEA/D scalarization)
    let r = mean_residual_fit(theta, start_seed, count, n, bake, seam, theta_polish);
    let l = mean_unsupervised_loss_pipe(
        theta, start_seed, count, n, lambda, bake, seam, theta_polish,
    );
    (1.0 - mo_w) * (1.0 - r) + mo_w * l
}

/// One coordinate-descent sweep; returns new loss/objective (lower better unless residual_primary).
fn coord_sweep(
    theta: &mut [f32; N_THETA],
    lambda: f32,
    n: usize,
    step: f32,
    residual_primary: bool,
    mo_w: f32,
    bake: Option<PeriodizeAlgo>,
    seam: SeamStyle,
    theta_polish: bool,
    maximize: bool,
) -> f32 {
    let eval = |th: &[f32; N_THETA]| -> f32 {
        if residual_primary {
            mean_residual_fit(th, INNER_FIT_START, INNER_FIT_COUNT, n, bake, seam, theta_polish)
        } else if mo_w > 0.0 {
            mean_mo_objective(
                th, INNER_FIT_START, INNER_FIT_COUNT, n, lambda, mo_w, bake, seam, theta_polish,
            )
        } else {
            mean_unsupervised_loss_pipe(
                th, INNER_FIT_START, INNER_FIT_COUNT, n, lambda, bake, seam, theta_polish,
            )
        }
    };
    let mut cur = eval(theta);
    for i in 0..N_THETA {
        if i == 7 {
            continue;
        }
        let base = theta[i];
        let mut local_best = cur;
        let mut local_val = base;
        for &delta in &[-step, step] {
            theta[i] = (base + delta).clamp(0.0, 1.0);
            let v = eval(theta);
            let better = if maximize {
                v > local_best + 1e-7
            } else {
                v + 1e-7 < local_best
            };
            if better {
                local_best = v;
                local_val = theta[i];
            }
        }
        theta[i] = local_val;
        cur = local_best;
    }
    cur
}

fn refine_lambda_1d(
    theta: &[f32; N_THETA],
    mut lambda: f32,
    n: usize,
    bake: Option<PeriodizeAlgo>,
    seam: SeamStyle,
    theta_polish: bool,
) -> (f32, f32) {
    let mut best_lam = lambda;
    let mut best_l = mean_unsupervised_loss_pipe(
        theta, INNER_FIT_START, INNER_FIT_COUNT, n, lambda, bake, seam, theta_polish,
    );
    let base_lam = lambda;
    for &delta in &[-0.18f32, -0.08, 0.08, 0.18] {
        let cand = (base_lam + delta).clamp(0.25, 2.8);
        let l = mean_unsupervised_loss_pipe(
            theta, INNER_FIT_START, INNER_FIT_COUNT, n, cand, bake, seam, theta_polish,
        );
        if l + 1e-7 < best_l {
            best_l = l;
            best_lam = cand;
        }
    }
    lambda = best_lam;
    (lambda, best_l)
}

/// Fit θ (and optionally λ) until relative plateau for [`CONV_PATIENCE`] sweeps
/// or [`CONV_MAX_SWEEPS`] reached.
///
/// Convergence criterion (documented for paper / JSON):
/// - Track objective `J` (unsupervised L, MO scalarization, or residual score).
/// - After each full coordinate sweep, compute
///   `rel = |J_prev - J_cur| / max(|J_prev|, 1e-6)`.
/// - If `rel < CONV_EPS` (1e-4) for `CONV_PATIENCE` (3) consecutive sweeps → converged.
/// - Else continue until `CONV_MAX_SWEEPS` (16). Residual-primary **maximizes** J;
///   loss / MO modes **minimize** J.
pub(crate) fn fit_until_convergence(
    mut theta: [f32; N_THETA],
    mut lambda: f32,
    n: usize,
    residual_primary: bool,
    mo_w: f32,
    bake: Option<PeriodizeAlgo>,
    seam: SeamStyle,
    theta_polish: bool,
    refine_lambda: bool,
) -> FitResult {
    let maximize = residual_primary;
    let mut prev = if residual_primary {
        mean_residual_fit(
            &theta, INNER_FIT_START, INNER_FIT_COUNT, n, bake, seam, theta_polish,
        )
    } else if mo_w > 0.0 {
        mean_mo_objective(
            &theta, INNER_FIT_START, INNER_FIT_COUNT, n, lambda, mo_w, bake, seam, theta_polish,
        )
    } else {
        mean_unsupervised_loss_pipe(
            &theta, INNER_FIT_START, INNER_FIT_COUNT, n, lambda, bake, seam, theta_polish,
        )
    };

    let mut plateau = 0usize;
    let mut steps = 0usize;
    let mut converged = false;
    let mut step_idx = 0usize;

    while steps < CONV_MAX_SWEEPS {
        let step = CONV_STEPS[step_idx.min(CONV_STEPS.len() - 1)];
        let cur = coord_sweep(
            &mut theta,
            lambda,
            n,
            step,
            residual_primary,
            mo_w,
            bake,
            seam,
            theta_polish,
            maximize,
        );
        steps += 1;
        let denom = prev.abs().max(1e-6);
        let rel = (prev - cur).abs() / denom;
        if rel < CONV_EPS {
            plateau += 1;
            if plateau >= CONV_PATIENCE {
                converged = true;
                break;
            }
        } else {
            plateau = 0;
            // Advance coarseness only when making progress.
            if step_idx + 1 < CONV_STEPS.len() && steps % 3 == 0 {
                step_idx += 1;
            }
        }
        prev = cur;
    }

    if refine_lambda && !residual_primary {
        let (lam2, _) = refine_lambda_1d(&theta, lambda, n, bake, seam, theta_polish);
        lambda = lam2;
        // One fine re-sweep at refined λ.
        let _ = coord_sweep(
            &mut theta,
            lambda,
            n,
            0.04,
            false,
            mo_w,
            bake,
            seam,
            theta_polish,
            false,
        );
        steps += 1;
    }

    theta[7] = 0.0;
    let fit_loss = if residual_primary {
        // Store 1-residual as "loss-like" for logging.
        1.0 - mean_residual_fit(
            &theta, INNER_FIT_START, INNER_FIT_COUNT, n, bake, seam, theta_polish,
        )
    } else {
        mean_unsupervised_loss_pipe(
            &theta, INNER_FIT_START, INNER_FIT_COUNT, n, lambda, bake, seam, theta_polish,
        )
    };

    FitResult {
        theta,
        lambda,
        fit_loss,
        conv_steps: steps,
        converged,
    }
}

/// Inner coordinate descent on unsupervised loss (nested bi-level step) — legacy fixed sweeps.
fn inner_loss_optimize(
    mut theta: [f32; N_THETA],
    mut lambda: f32,
    n: usize,
    sweeps: usize,
    refine_lambda: bool,
) -> ([f32; N_THETA], f32, f32) {
    if sweeps == 0 {
        let l = mean_unsupervised_loss(&theta, INNER_FIT_START, INNER_FIT_COUNT, n, lambda);
        return (theta, lambda, l);
    }
    let steps = [0.15f32, 0.07, 0.03];
    let mut cur_l =
        mean_unsupervised_loss(&theta, INNER_FIT_START, INNER_FIT_COUNT, n, lambda);
    for &step in &steps {
        for _ in 0..sweeps {
            for i in 0..N_THETA {
                if i == 7 {
                    continue;
                }
                let base = theta[i];
                let mut local_best = cur_l;
                let mut local_val = base;
                for &delta in &[-step, step] {
                    theta[i] = (base + delta).clamp(0.0, 1.0);
                    let l = mean_unsupervised_loss(
                        &theta,
                        INNER_FIT_START,
                        INNER_FIT_COUNT,
                        n,
                        lambda,
                    );
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
    if refine_lambda {
        let base_lam = lambda;
        let mut best_lam = lambda;
        let mut best_l = cur_l;
        for &delta in &[-0.18f32, -0.08, 0.08, 0.18] {
            let cand = (base_lam + delta).clamp(0.25, 2.8);
            let l =
                mean_unsupervised_loss(&theta, INNER_FIT_START, INNER_FIT_COUNT, n, cand);
            if l + 1e-7 < best_l {
                best_l = l;
                best_lam = cand;
            }
        }
        lambda = best_lam;
        cur_l = best_l;
        let step = 0.05f32;
        for i in 0..N_THETA {
            if i == 7 {
                continue;
            }
            let base = theta[i];
            let mut local_best = cur_l;
            let mut local_val = base;
            for &delta in &[-step, step] {
                theta[i] = (base + delta).clamp(0.0, 1.0);
                let l = mean_unsupervised_loss(
                    &theta,
                    INNER_FIT_START,
                    INNER_FIT_COUNT,
                    n,
                    lambda,
                );
                if l + 1e-7 < local_best {
                    local_best = l;
                    local_val = theta[i];
                }
            }
            theta[i] = local_val;
            cur_l = local_best;
        }
    }
    theta[7] = 0.0;
    (theta, lambda, cur_l)
}

fn eval_theta_fast(
    theta: &[f32; N_THETA],
    start_seed: u64,
    count: usize,
    n: usize,
    lambda: f32,
) -> (f32, f32, f32, f32, f32) {
    eval_pipeline_fast(
        theta,
        start_seed,
        count,
        n,
        lambda,
        None,
        SeamStyle::Adaptive,
        false,
    )
}

pub(crate) fn eval_pipeline_fast(
    theta: &[f32; N_THETA],
    start_seed: u64,
    count: usize,
    n: usize,
    lambda: f32,
    bake: Option<PeriodizeAlgo>,
    seam: SeamStyle,
    theta_polish: bool,
) -> (f32, f32, f32, f32, f32) {
    let mut sum_l = 0.0f32;
    let mut sum_d = 0.0f32;
    let mut sum_s = 0.0f32;
    let mut sum_r = 0.0f32;
    for k in 0..count {
        let seed = start_seed + k as u64;
        let (_, ideal) = generate_sound_ideal(seed, n);
        let (_, raw) = generate_sound(seed, n);
        let mut out = raw.clone();
        apply_pipeline(&mut out, theta, bake, seam, theta_polish);
        let (l, d, s, _) = score_with_lambda(&raw, &out, lambda);
        sum_l += l;
        sum_d += d;
        sum_s += s;
        sum_r += residual_for_cycle(&ideal, &out);
    }
    let c = count.max(1) as f32;
    let d = sum_d / c;
    let s = sum_s / c;
    let r = sum_r / c;
    (sum_l / c, d, s, 0.5 * (d + s), r)
}

pub(crate) struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
        self.0
    }
    pub fn f01(&mut self) -> f32 {
        (self.next() >> 33) as f32 / (u32::MAX as f32)
    }
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f01()
    }
    pub fn usize(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() as usize) % n
        }
    }
}

/// Literature-informed prior families (HPO / PBT / multi-objective / bi-level).
fn sample_trial(rng: &mut Rng, idx: usize) -> (TrialHp, [f32; N_THETA]) {
    let bucket = idx % 7;
    let (prior, lam_lo, lam_hi, fade_lo, fade_hi, pol_lo, pol_hi, sweeps, refine_lam) =
        match bucket {
            0 => ("bayes_local", 0.55, 1.25, 0.9, 1.35, 0.7, 1.05, 1usize, true),
            1 => ("pbt_exploit", 0.7, 1.0, 1.05, 1.35, 0.85, 1.05, 2, true),
            2 => ("mo_shape", 1.2, 2.2, 0.65, 1.0, 0.4, 0.75, 1, false),
            3 => ("aggressive", 0.35, 0.75, 1.15, 1.5, 0.9, 1.1, 1, false),
            4 => ("evo_explore", 0.3, 2.5, 0.5, 1.6, 0.3, 1.1, 0, false),
            5 => ("racing_mid", 0.8, 1.4, 0.85, 1.2, 0.6, 0.95, 1, true),
            _ => ("bilevel_loss", 0.5, 1.6, 0.75, 1.35, 0.55, 1.05, 2, true),
        };
    let hp = TrialHp {
        name: format!("{prior}_{idx}"),
        lambda_shape: rng.range(lam_lo, lam_hi),
        fade_scale_bias: rng.range(fade_lo, fade_hi),
        polish_bias: rng.range(pol_lo, pol_hi),
        pin_bias: rng.range(0.8, 1.05),
        detrend_bias: rng.range(0.85, 1.05),
        ease_bias: rng.range(0.0, 1.0),
        loss_opt_sweeps: sweeps,
        refine_lambda: refine_lam,
        algo_seed: 10_000 + idx as u64,
        prior,
    };
    let mut theta = FROZEN_THETA;
    for t in theta.iter_mut() {
        let noise = (rng.f01() - 0.5) * 0.22;
        *t = (*t + noise).clamp(0.0, 1.0);
    }
    theta[0] = (theta[0] * hp.detrend_bias).clamp(0.0, 1.0);
    theta[1] = (theta[1] * hp.fade_scale_bias).clamp(0.0, 1.0);
    theta[3] = hp.ease_bias.clamp(0.0, 1.0);
    theta[6] = (theta[6] * hp.polish_bias).clamp(0.0, 1.0);
    theta[9] = (theta[9] * hp.pin_bias).clamp(0.0, 1.0);
    theta[11] = (theta[11] * hp.polish_bias).clamp(0.0, 1.0);
    theta[7] = 0.0;
    (hp, theta)
}

fn bake_from_atom(a: OpAtom) -> (Option<PeriodizeAlgo>, SeamStyle, bool) {
    match a {
        OpAtom::BakeDualCosine => (Some(PeriodizeAlgo::DualCosine), SeamStyle::Adaptive, true),
        OpAtom::BakeClassic => (Some(PeriodizeAlgo::Classic), SeamStyle::Adaptive, true),
        OpAtom::BakeSoft => (Some(PeriodizeAlgo::DualCosine), SeamStyle::Soft, true),
        OpAtom::BakeEnsembleV3 => (Some(PeriodizeAlgo::EnsembleV3), SeamStyle::Adaptive, true),
        OpAtom::BakeCrossfade => (Some(PeriodizeAlgo::Crossfade), SeamStyle::Adaptive, true),
        _ => (None, SeamStyle::Adaptive, false),
    }
}

/// Sample a combinatorial hybrid: 1–3 operator atoms (search ± bake).
pub(crate) fn sample_combo(rng: &mut Rng, idx: usize) -> (ComboTrial, [f32; N_THETA]) {
    let n_ops = 1 + rng.usize(3); // 1, 2, or 3
    let mut ops: Vec<OpAtom> = Vec::with_capacity(n_ops);
    // Always include at least one search atom.
    ops.push(OpAtom::SEARCH[rng.usize(OpAtom::SEARCH.len())]);
    while ops.len() < n_ops {
        let use_bake = rng.f01() < 0.35;
        let cand = if use_bake {
            OpAtom::BAKE[rng.usize(OpAtom::BAKE.len())]
        } else {
            OpAtom::SEARCH[rng.usize(OpAtom::SEARCH.len())]
        };
        if !ops.contains(&cand) {
            ops.push(cand);
        } else if ops.len() == 1 {
            // Force progress on collision.
            ops.push(OpAtom::SEARCH[(idx + ops.len()) % OpAtom::SEARCH.len()]);
            break;
        }
    }

    let mut residual_primary = false;
    let mut use_racing = false;
    let mut use_pbt = false;
    let mut use_mo_weight = 0.0f32;
    let mut refine_lambda = false;
    let mut bake = None;
    let mut seam = SeamStyle::Adaptive;
    let mut theta_polish = false;
    let mut lam_lo = 0.4f32;
    let mut lam_hi = 1.8f32;
    let mut noise_scale = 0.22f32;

    for &a in &ops {
        match a {
            OpAtom::BayesLocal => {
                lam_lo = 0.55;
                lam_hi = 1.25;
                noise_scale = 0.12;
                refine_lambda = true;
            }
            OpAtom::PbtExploit => {
                use_pbt = true;
                noise_scale = 0.10;
                refine_lambda = true;
            }
            OpAtom::IraceRacing => {
                use_racing = true;
            }
            OpAtom::MoeadShape => {
                use_mo_weight = rng.range(0.25, 0.65);
                lam_lo = 1.0;
                lam_hi = 2.2;
            }
            OpAtom::EvoExplore => {
                noise_scale = 0.35;
                lam_lo = 0.3;
                lam_hi = 2.5;
            }
            OpAtom::N2nUnsup => {
                refine_lambda = true;
                noise_scale = (noise_scale + 0.15).min(0.4);
            }
            OpAtom::BilevelNested => {
                refine_lambda = true;
            }
            OpAtom::ResidualPrimary => {
                residual_primary = true;
            }
            OpAtom::BakeDualCosine
            | OpAtom::BakeClassic
            | OpAtom::BakeSoft
            | OpAtom::BakeEnsembleV3
            | OpAtom::BakeCrossfade => {
                let (b, s, polish) = bake_from_atom(a);
                bake = b;
                seam = s;
                theta_polish = polish;
            }
        }
    }

    let labels: Vec<&'static str> = ops.iter().map(|o| o.label()).collect();
    let family = labels.join("+");
    let hp_lam = rng.range(lam_lo, lam_hi);

    let mut theta = FROZEN_THETA;
    for t in theta.iter_mut() {
        let noise = (rng.f01() - 0.5) * noise_scale;
        *t = (*t + noise).clamp(0.0, 1.0);
    }
    theta[7] = 0.0;

    let trial = ComboTrial {
        name: format!("combo_{idx}_{family}"),
        family,
        ops: labels,
        lambda_shape: hp_lam,
        residual_primary,
        use_racing,
        use_pbt,
        use_mo_weight,
        bake,
        seam,
        theta_polish,
        refine_lambda,
        algo_seed: 20_000 + idx as u64,
    };
    (trial, theta)
}

/// irace-style: race a small population on growing budgets; keep winners.
pub(crate) fn race_init_population(
    rng: &mut Rng,
    base: [f32; N_THETA],
    n_cand: usize,
) -> Vec<[f32; N_THETA]> {
    let mut pop = Vec::with_capacity(n_cand);
    pop.push(base);
    for _ in 1..n_cand {
        let mut t = base;
        for x in t.iter_mut() {
            *x = (*x + (rng.f01() - 0.5) * 0.18).clamp(0.0, 1.0);
        }
        t[7] = 0.0;
        pop.push(t);
    }
    pop
}

pub(crate) fn race_select(
    pop: &mut Vec<[f32; N_THETA]>,
    trial: &ComboTrial,
    n: usize,
    budgets: &[usize],
) {
    for &budget in budgets {
        let mut scored: Vec<(f32, [f32; N_THETA])> = pop
            .iter()
            .map(|th| {
                let score = if trial.residual_primary {
                    mean_residual_fit(
                        th,
                        INNER_FIT_START,
                        budget,
                        n,
                        trial.bake,
                        trial.seam,
                        trial.theta_polish,
                    )
                } else {
                    -mean_unsupervised_loss_pipe(
                        th,
                        INNER_FIT_START,
                        budget,
                        n,
                        trial.lambda_shape,
                        trial.bake,
                        trial.seam,
                        trial.theta_polish,
                    )
                };
                (score, *th)
            })
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        let keep = (scored.len() / 2).max(1);
        *pop = scored.into_iter().take(keep).map(|(_, t)| t).collect();
    }
}

/// PBT-style exploit: copy elite, mutate offspring.
pub(crate) fn pbt_exploit_mutate(rng: &mut Rng, elite: [f32; N_THETA], n_offspring: usize) -> Vec<[f32; N_THETA]> {
    let mut out = vec![elite];
    for _ in 0..n_offspring {
        let mut t = elite;
        for x in t.iter_mut() {
            if rng.f01() < 0.4 {
                *x = (*x + (rng.f01() - 0.5) * 0.2).clamp(0.0, 1.0);
            }
        }
        t[7] = 0.0;
        out.push(t);
    }
    out
}

fn family_stress(theta: &[f32; N_THETA], lambda: f32, n: usize) -> Vec<serde_json::Value> {
    family_stress_pipe(theta, lambda, n, None, SeamStyle::Adaptive, false)
}

pub(crate) fn family_stress_pipe(
    theta: &[f32; N_THETA],
    lambda: f32,
    n: usize,
    bake: Option<PeriodizeAlgo>,
    seam: SeamStyle,
    theta_polish: bool,
) -> Vec<serde_json::Value> {
    let mut fam_q = Vec::new();
    for fam in [
        BenchFamily::ExtremeOverlay,
        BenchFamily::OpenWrapBias,
        BenchFamily::Combo,
        BenchFamily::HarmonicFft,
        BenchFamily::Nonlinear,
    ] {
        let mut qd = 0.0f32;
        let mut qs = 0.0f32;
        let mut qr = 0.0f32;
        let mut cnt = 0u32;
        let mut seed = fam.index() as u64 + 80_000;
        while cnt < 120 {
            if BenchFamily::from_seed(seed) == fam {
                let (_, ideal) = generate_sound_ideal(seed, n);
                let (_, raw) = generate_sound(seed, n);
                let mut out = raw.clone();
                apply_pipeline(&mut out, theta, bake, seam, theta_polish);
                let (_, d, s, _) = score_with_lambda(&raw, &out, lambda);
                qd += d;
                qs += s;
                qr += residual_for_cycle(&ideal, &out);
                cnt += 1;
            }
            seed += BenchFamily::ALL.len() as u64;
        }
        let cc = cnt.max(1) as f32;
        fam_q.push(json!({
            "family": fam.label(),
            "denoise": qd / cc,
            "shape": qs / cc,
            "quality": 0.5 * (qd + qs) / cc,
            "residual": qr / cc,
        }));
    }
    fam_q
}

pub(crate) fn eval_bake_baseline(
    algo: PeriodizeAlgo,
    seam: SeamStyle,
    start_seed: u64,
    count: usize,
    n: usize,
) -> (f32, f32, f32, f32) {
    let mut sum_d = 0.0f32;
    let mut sum_s = 0.0f32;
    let mut sum_r = 0.0f32;
    for k in 0..count {
        let seed = start_seed + k as u64;
        let (_, ideal) = generate_sound_ideal(seed, n);
        let (_, raw) = generate_sound(seed, n);
        let mut out = raw.clone();
        periodize_with_algo(&mut out, 0.0, seam, algo);
        let (_, d, s, _) = score_with_lambda(&raw, &out, 1.0);
        sum_d += d;
        sum_s += s;
        sum_r += residual_for_cycle(&ideal, &out);
    }
    let c = count.max(1) as f32;
    let d = sum_d / c;
    let s = sum_s / c;
    (d, s, 0.5 * (d + s), sum_r / c)
}

/// Literature + combinatorial hybrid meta search with fit-until-convergence.
///
/// Wall-clock for the outer `n_trials` loop is recorded separately from post-hoc
/// refine / baseline matrix work.
pub fn run_lit_combo_meta_n(
    n_trials: usize,
    val_fast: usize,
    val_final: usize,
) -> serde_json::Value {
    let t_total = std::time::Instant::now();
    let n = BENCH_N;
    let mut rng = Rng(0x71C0_CB01); // lit-combo seed
    let val_start = 55_000u64;
    let n_trials = n_trials.max(1);
    let val_fast = val_fast.max(1);
    let val_final = val_final.max(1);

    // (meta_rank, trial, fit, loss, d, s, q, residual)
    let mut scored: Vec<(f32, ComboTrial, FitResult, f32, f32, f32, f32, f32)> =
        Vec::with_capacity(n_trials);

    let t_iters = std::time::Instant::now();
    for i in 0..n_trials {
        let (mut trial, theta0) = sample_combo(&mut rng, i);

        // Optional racing / PBT to choose init before convergence fit.
        let mut init = theta0;
        if trial.use_racing {
            let mut pop = race_init_population(&mut rng, init, 4);
            race_select(&mut pop, &trial, n, &[8, 16, 32]);
            init = pop[0];
        }
        if trial.use_pbt {
            let pop = pbt_exploit_mutate(&mut rng, init, 3);
            // Score residual-primary or -loss; keep best.
            let mut best_s = f32::NEG_INFINITY;
            let mut best_t = init;
            for th in &pop {
                let s = if trial.residual_primary {
                    mean_residual_fit(
                        th,
                        INNER_FIT_START,
                        24,
                        n,
                        trial.bake,
                        trial.seam,
                        trial.theta_polish,
                    )
                } else {
                    -mean_unsupervised_loss_pipe(
                        th,
                        INNER_FIT_START,
                        24,
                        n,
                        trial.lambda_shape,
                        trial.bake,
                        trial.seam,
                        trial.theta_polish,
                    )
                };
                if s > best_s {
                    best_s = s;
                    best_t = *th;
                }
            }
            init = best_t;
        }

        let fit = fit_until_convergence(
            init,
            trial.lambda_shape,
            n,
            trial.residual_primary,
            trial.use_mo_weight,
            trial.bake,
            trial.seam,
            trial.theta_polish,
            trial.refine_lambda,
        );
        trial.lambda_shape = fit.lambda;

        let (loss, d, s, q, residual) = eval_pipeline_fast(
            &fit.theta,
            val_start,
            val_fast,
            n,
            trial.lambda_shape,
            trial.bake,
            trial.seam,
            trial.theta_polish,
        );
        let meta = meta_rank(residual, s);
        scored.push((meta, trial, fit, loss, d, s, q, residual));

        if i % 50 == 0 || (n_trials <= 40 && i % 5 == 0) {
            eprintln!(
                "lit-combo progress {i}/{n_trials} best_residual_rank={:.4} last_conv_steps={}",
                scored.iter().map(|t| t.0).fold(0.0f32, f32::max),
                scored.last().map(|t| t.2.conv_steps).unwrap_or(0)
            );
        }
    }
    let iterations_elapsed = t_iters.elapsed();
    let iterations_elapsed_ms = iterations_elapsed.as_millis() as u64;
    let iterations_elapsed_sec = iterations_elapsed.as_secs_f64();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

    // Refine top 12
    let top_refine = scored.len().min(12);
    let mut refined = Vec::new();
    for (meta, trial, fit, _, _, _, _, _) in scored.iter().take(top_refine) {
        let (loss, d, s, q, residual) = eval_pipeline_fast(
            &fit.theta,
            70_000,
            val_final,
            n,
            trial.lambda_shape,
            trial.bake,
            trial.seam,
            trial.theta_polish,
        );
        let meta2 = meta_rank(residual, s);
        let fam = family_stress_pipe(
            &fit.theta,
            trial.lambda_shape,
            n,
            trial.bake,
            trial.seam,
            trial.theta_polish,
        );
        refined.push(json!({
            "name": trial.name,
            "family": trial.family,
            "ops": trial.ops,
            "meta_score_fast": meta,
            "meta_score": meta2,
            "residual": residual,
            "convergence": {
                "steps": fit.conv_steps,
                "converged": fit.converged,
                "fit_loss": fit.fit_loss,
                "criterion": format!(
                    "rel_improve < {CONV_EPS} for {CONV_PATIENCE} consecutive sweeps; max {CONV_MAX_SWEEPS}"
                ),
            },
            "hyper": {
                "lambda_shape": trial.lambda_shape,
                "residual_primary": trial.residual_primary,
                "use_racing": trial.use_racing,
                "use_pbt": trial.use_pbt,
                "mo_weight": trial.use_mo_weight,
                "bake": trial.bake.map(|a| a.label()),
                "seam": match trial.seam {
                    SeamStyle::Soft => "soft",
                    SeamStyle::Adaptive => "adaptive",
                    SeamStyle::Raw => "raw",
                },
                "theta_polish": trial.theta_polish,
                "refine_lambda": trial.refine_lambda,
                "algo_seed": trial.algo_seed,
            },
            "theta": fit.theta.as_slice(),
            "val": {
                "loss": loss,
                "denoise": d,
                "shape": s,
                "quality": q,
                "residual": residual,
            },
            "family_stress": fam,
        }));
    }
    refined.sort_by(|a, b| {
        b["meta_score"]
            .as_f64()
            .unwrap_or(0.0)
            .partial_cmp(&a["meta_score"].as_f64().unwrap_or(0.0))
            .unwrap()
    });

    let top4: Vec<_> = refined.iter().take(4).cloned().collect();

    // Bake baselines from artifact_reduce (+ Soft seam variant).
    let mut bake_baselines = Vec::new();
    for &algo in PeriodizeAlgo::ALL {
        let (d, s, q, r) = eval_bake_baseline(algo, SeamStyle::Adaptive, 60_000, val_final, n);
        bake_baselines.push(json!({
            "algo": algo.label(),
            "kind": "bake_baseline",
            "seam": "adaptive",
            "denoise": d,
            "shape": s,
            "quality": q,
            "residual": r,
        }));
    }
    let (d, s, q, r) =
        eval_bake_baseline(PeriodizeAlgo::DualCosine, SeamStyle::Soft, 60_000, val_final, n);
    bake_baselines.push(json!({
        "algo": "dual_cosine_soft",
        "kind": "bake_baseline",
        "seam": "soft",
        "denoise": d,
        "shape": s,
        "quality": q,
        "residual": r,
    }));

    let mut five = Vec::new();
    let naive = bake_baselines
        .iter()
        .find(|b| b["algo"] == "dual_cosine")
        .cloned()
        .unwrap_or(json!({}));
    five.push(json!({
        "algo": "naive_dual_cosine",
        "kind": "naive",
        "denoise": naive["denoise"],
        "shape": naive["shape"],
        "quality": naive["quality"],
        "residual": naive["residual"],
        "rank": 0,
    }));

    for (i, t) in top4.iter().enumerate() {
        five.push(json!({
            "algo": format!("meta_top{}", i + 1),
            "kind": "meta_combo",
            "lambda": t["hyper"]["lambda_shape"],
            "denoise": t["val"]["denoise"],
            "shape": t["val"]["shape"],
            "quality": t["val"]["quality"],
            "residual": t["val"]["residual"],
            "rank": i + 1,
            "theta": t["theta"],
            "family": t["family"],
            "ops": t["ops"],
            "trial_name": t["name"],
            "convergence_steps": t["convergence"]["steps"],
        }));
    }

    let mut fd = 0.0f32;
    let mut fs = 0.0f32;
    let mut fr = 0.0f32;
    for k in 0..val_final {
        let seed = 60_000 + k as u64;
        let (_, ideal) = generate_sound_ideal(seed, n);
        let (_, raw) = generate_sound(seed, n);
        let mut out = raw.clone();
        apply_denoise_opt(&mut out, 0.0);
        let (_, d, s, _) = score_with_lambda(&raw, &out, 1.0);
        fd += d;
        fs += s;
        fr += residual_for_cycle(&ideal, &out);
    }
    let c = val_final as f32;

    let artifact = if n_trials >= 500 {
        "brand/artifacts/denoise_opt_meta_lit_combo_500.json"
    } else {
        "brand/artifacts/denoise_opt_meta_lit_combo_sanity.json"
    };

    let mean_conv: f64 = if scored.is_empty() {
        0.0
    } else {
        scored.iter().map(|t| t.2.conv_steps as f64).sum::<f64>() / scored.len() as f64
    };
    let pct_converged = if scored.is_empty() {
        0.0
    } else {
        100.0 * scored.iter().filter(|t| t.2.converged).count() as f64 / scored.len() as f64
    };

    let total_elapsed = t_total.elapsed();
    let report = json!({
        "title": "DenoiseOpt lit-combo meta (fit-until-convergence + hybrid operators)",
        "n_trials": n_trials,
        "val_fast": val_fast,
        "val_final": val_final,
        "cycle_n": n,
        "prolong_periods": PROLONG,
        "inner_fit_count": INNER_FIT_COUNT,
        "iterations_elapsed_ms": iterations_elapsed_ms,
        "iterations_elapsed_sec": iterations_elapsed_sec,
        "total_elapsed_ms": total_elapsed.as_millis() as u64,
        "total_elapsed_sec": total_elapsed.as_secs_f64(),
        "seconds": iterations_elapsed_sec,
        "convergence": {
            "eps": CONV_EPS,
            "patience": CONV_PATIENCE,
            "max_sweeps": CONV_MAX_SWEEPS,
            "step_schedule": CONV_STEPS.as_slice(),
            "criterion": "relative |J_prev-J_cur|/max(|J_prev|,1e-6) < eps for patience consecutive coordinate sweeps; else stop at max_sweeps. Residual-primary maximizes residual; else minimize unsupervised L (or MOEA/D scalarization).",
            "mean_conv_steps": mean_conv,
            "pct_converged": pct_converged,
        },
        "literature_families": [
            "baseline_bake — DualCosine/Classic/Soft/Ensemble*/Crossfade (artifact_reduce seam race)",
            "bayes_local — Bayesian/local HPO densify around good λ (Snoek/BOHB-style)",
            "pbt_exploit — Population-Based Training exploit+mutate (Jaderberg)",
            "irace_racing — racing / irace progressive discard (López-Ibáñez)",
            "moead_shape — MOEA/D weighted residual↔shape (Zhang & Li)",
            "evo_explore — evolutionary wide θ mutation",
            "n2n_unsup — Noise2Noise-style unsupervised L fit (Lehtinen)",
            "bilevel_nested — nested inner L / outer residual",
            "residual_primary — converge by maximizing prolonged residual",
            "hybrids — pairs/triples of the above (race+pbt, evo+loss_opt, mo+residual, bake+θ polish, …)",
        ],
        "meta_objective": "outer maximize residual_score (soft gate S>=0.97); inner fit-until-convergence on L or residual/MO; D/S auxiliaries",
        "residual_formula": "score = clamp(1 - rms(engine_tiled - ideal_tiled) / max(rms(ideal_tiled), 1e-6), 0, 1)",
        "champion": top4.first().cloned().unwrap_or(json!({})),
        "top4": top4,
        "benchmark_matrix_5": five,
        "bake_baselines": bake_baselines,
        "production_frozen": {
            "denoise": fd / c,
            "shape": fs / c,
            "quality": 0.5 * (fd + fs) / c,
            "residual": fr / c,
        },
        "pareto_top20_fast": scored.iter().take(20).map(|(meta, trial, fit, loss, d, s, q, residual)| json!({
            "meta_score": meta,
            "residual": residual,
            "name": trial.name,
            "family": trial.family,
            "ops": trial.ops,
            "lambda": trial.lambda_shape,
            "conv_steps": fit.conv_steps,
            "converged": fit.converged,
            "val_fast": { "loss": loss, "denoise": d, "shape": s, "quality": q, "residual": residual },
            "theta": fit.theta.as_slice(),
        })).collect::<Vec<_>>(),
        "per_algo_scores": scored.iter().map(|(meta, trial, fit, loss, d, s, q, residual)| json!({
            "name": trial.name,
            "family": trial.family,
            "ops": trial.ops,
            "meta_score": meta,
            "residual": residual,
            "denoise": d,
            "shape": s,
            "quality": q,
            "loss": loss,
            "conv_steps": fit.conv_steps,
            "converged": fit.converged,
            "fit_loss": fit.fit_loss,
            "lambda": trial.lambda_shape,
        })).collect::<Vec<_>>(),
        "artifact_path": artifact,
        "sessionId": "0ab8f9",
        "runId": if n_trials >= 500 { "meta-lit-combo-500" } else { "meta-lit-combo-sanity" },
        "build_notes": "release recommended; BENCH_N cycle length; INNER_FIT_COUNT seeds per inner eval; prolong=16",
    });

    let _ = std::fs::create_dir_all("brand/artifacts");
    if let Ok(s) = serde_json::to_string_pretty(&report) {
        let _ = std::fs::write(artifact, &s);
    }
    report
}

/// Configurable meta search (use `n_trials=40` for a fast sanity check).
pub fn run_meta_learning_search_n(
    n_trials: usize,
    val_fast: usize,
    val_final: usize,
) -> serde_json::Value {
    let t0 = std::time::Instant::now();
    let n = BENCH_N;
    let mut rng = Rng(0x15A0_1500);
    let val_start = 55_000u64;
    let n_trials = n_trials.max(1);
    let val_fast = val_fast.max(1);
    let val_final = val_final.max(1);

    let mut scored: Vec<(f32, TrialHp, [f32; N_THETA], f32, f32, f32, f32, f32)> =
        Vec::with_capacity(n_trials);

    for i in 0..n_trials {
        let (mut hp, theta0) = sample_trial(&mut rng, i);
        let (theta, lambda, fit_loss) = inner_loss_optimize(
            theta0,
            hp.lambda_shape,
            n,
            hp.loss_opt_sweeps,
            hp.refine_lambda,
        );
        hp.lambda_shape = lambda;
        let (loss, d, s, q, residual) =
            eval_theta_fast(&theta, val_start, val_fast, n, hp.lambda_shape);
        let _ = fit_loss;
        let meta = meta_rank(residual, s);
        scored.push((meta, hp, theta, loss, d, s, q, residual));
        if i % 250 == 0 || (n_trials <= 100 && i % 10 == 0) {
            eprintln!(
                "meta progress {i}/{n_trials} best_residual_rank={:.4}",
                scored.iter().map(|t| t.0).fold(0.0f32, f32::max)
            );
        }
    }

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

    let top_refine = scored.len().min(12);
    let mut refined = Vec::new();
    for (meta, hp, theta, _, _, _, _, _) in scored.iter().take(top_refine) {
        let (loss, d, s, q, residual) =
            eval_theta_fast(theta, 70_000, val_final, n, hp.lambda_shape);
        let meta2 = meta_rank(residual, s);
        let fam = family_stress(theta, hp.lambda_shape, n);
        refined.push(json!({
            "name": hp.name,
            "prior": hp.prior,
            "meta_score_fast": meta,
            "meta_score": meta2,
            "residual": residual,
            "hyper": {
                "lambda_shape": hp.lambda_shape,
                "fade_scale_bias": hp.fade_scale_bias,
                "polish_bias": hp.polish_bias,
                "pin_bias": hp.pin_bias,
                "detrend_bias": hp.detrend_bias,
                "ease_bias": hp.ease_bias,
                "loss_opt_sweeps": hp.loss_opt_sweeps,
                "refine_lambda": hp.refine_lambda,
                "algo_seed": hp.algo_seed,
            },
            "theta": theta.as_slice(),
            "val": {
                "loss": loss,
                "denoise": d,
                "shape": s,
                "quality": q,
                "residual": residual,
            },
            "family_stress": fam,
        }));
    }
    refined.sort_by(|a, b| {
        b["meta_score"]
            .as_f64()
            .unwrap_or(0.0)
            .partial_cmp(&a["meta_score"].as_f64().unwrap_or(0.0))
            .unwrap()
    });

    let top4: Vec<_> = refined.iter().take(4).cloned().collect();
    let mut top4_algos = Vec::new();
    for (i, t) in top4.iter().enumerate() {
        let theta_arr: Vec<f32> = t["theta"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0) as f32)
            .collect();
        let mut th = [0.0f32; N_THETA];
        for (j, v) in theta_arr.iter().enumerate().take(N_THETA) {
            th[j] = *v;
        }
        let lam = t["hyper"]["lambda_shape"].as_f64().unwrap_or(1.0) as f32;
        top4_algos.push((format!("meta_top{}", i + 1), th, lam));
    }

    let mut five = Vec::new();
    let mut nd = 0.0f32;
    let mut ns = 0.0f32;
    let mut nr = 0.0f32;
    for k in 0..val_final {
        let seed = 60_000 + k as u64;
        let (_, ideal) = generate_sound_ideal(seed, n);
        let (_, raw) = generate_sound(seed, n);
        let mut out = raw.clone();
        periodize_with_algo(&mut out, 0.0, SeamStyle::Adaptive, PeriodizeAlgo::DualCosine);
        let (_, d, s, _) = score_with_lambda(&raw, &out, 1.0);
        nd += d;
        ns += s;
        nr += residual_for_cycle(&ideal, &out);
    }
    let c = val_final as f32;
    five.push(json!({
        "algo": "naive_dual_cosine",
        "kind": "naive",
        "denoise": nd / c,
        "shape": ns / c,
        "quality": 0.5 * (nd + ns) / c,
        "residual": nr / c,
        "rank": 0,
    }));
    let mut nc = 0.0f32;
    let mut ncs = 0.0f32;
    let mut ncr = 0.0f32;
    for k in 0..val_final {
        let seed = 60_000 + k as u64;
        let (_, ideal) = generate_sound_ideal(seed, n);
        let (_, raw) = generate_sound(seed, n);
        let mut out = raw.clone();
        periodize_with_algo(&mut out, 0.0, SeamStyle::Adaptive, PeriodizeAlgo::Classic);
        let (_, d, s, _) = score_with_lambda(&raw, &out, 1.0);
        nc += d;
        ncs += s;
        ncr += residual_for_cycle(&ideal, &out);
    }
    let classic_row = json!({
        "algo": "naive_classic",
        "kind": "naive_ref",
        "denoise": nc / c,
        "shape": ncs / c,
        "quality": 0.5 * (nc + ncs) / c,
        "residual": ncr / c,
    });

    for (i, (name, th, lam)) in top4_algos.iter().enumerate() {
        let (_, d, s, q, residual) = eval_theta_fast(th, 60_000, val_final, n, *lam);
        five.push(json!({
            "algo": name,
            "kind": "meta",
            "lambda": lam,
            "denoise": d,
            "shape": s,
            "quality": q,
            "residual": residual,
            "rank": i + 1,
            "theta": th.as_slice(),
            "prior": top4[i]["prior"],
            "trial_name": top4[i]["name"],
        }));
    }

    let mut fd = 0.0f32;
    let mut fs = 0.0f32;
    let mut fr = 0.0f32;
    for k in 0..val_final {
        let seed = 60_000 + k as u64;
        let (_, ideal) = generate_sound_ideal(seed, n);
        let (_, raw) = generate_sound(seed, n);
        let mut out = raw.clone();
        apply_denoise_opt(&mut out, 0.0);
        let (_, d, s, _) = score_with_lambda(&raw, &out, 1.0);
        fd += d;
        fs += s;
        fr += residual_for_cycle(&ideal, &out);
    }

    let artifact = if n_trials >= N_TRIALS_DEFAULT {
        "brand/artifacts/denoise_opt_meta_1500.json"
    } else {
        "brand/artifacts/denoise_opt_meta_sanity.json"
    };

    let report = json!({
        "title": "DenoiseOpt bi-level meta-learning (residual objective + nested loss opt)",
        "n_trials": n_trials,
        "val_fast": val_fast,
        "val_final": val_final,
        "cycle_n": n,
        "prolong_periods": PROLONG,
        "inner_fit_count": INNER_FIT_COUNT,
        "seconds": t0.elapsed().as_secs_f64(),
        "literature_priors": [
            "bayes_local — densify around good λ (Bayesian HPO) + 1-sweep loss opt",
            "pbt_exploit — mutate near champion + 2-sweep loss opt + λ refine",
            "mo_shape — higher λ multi-objective shape preference + 1-sweep",
            "aggressive — low λ long fade + 1-sweep",
            "evo_explore — wide evolutionary explore (mutate-only control)",
            "racing_mid — mid-band racing + 1-sweep + λ refine",
            "bilevel_loss — deeper nested θ fit + joint λ refine",
        ],
        "meta_objective": "bi-level: inner minimize L=(1-D)+λ(1-S); outer maximize residual_score on prolonged ideal vs engine; soft gate S>=0.97; D/S auxiliaries",
        "residual_formula": "score = clamp(1 - rms(engine_tiled - ideal_tiled) / max(rms(ideal_tiled), 1e-6), 0, 1); ideal = generate_sound_ideal (no open-wrap); engine = tile(DenoiseOpt(generate_sound), N=16)",
        "loss_opt": "per-trial coordinate descent on unsupervised loss (INNER_FIT_COUNT seeds); optional 1-D λ refine + re-sweep (bilevel_loss / pbt / bayes / racing)",
        "champion": top4.first().cloned().unwrap_or(json!({})),
        "top4": top4,
        "benchmark_matrix_5": five,
        "naive_classic_ref": classic_row,
        "production_frozen": {
            "denoise": fd / c,
            "shape": fs / c,
            "quality": 0.5 * (fd + fs) / c,
            "residual": fr / c,
        },
        "pareto_top20_fast": scored.iter().take(20).map(|(meta, hp, theta, loss, d, s, q, residual)| json!({
            "meta_score": meta,
            "residual": residual,
            "name": hp.name,
            "prior": hp.prior,
            "lambda": hp.lambda_shape,
            "loss_opt_sweeps": hp.loss_opt_sweeps,
            "refine_lambda": hp.refine_lambda,
            "val_fast": { "loss": loss, "denoise": d, "shape": s, "quality": q, "residual": residual },
            "theta": theta.as_slice(),
        })).collect::<Vec<_>>(),
        "artifact_path": artifact,
        "sessionId": "0ab8f9",
        "runId": if n_trials >= N_TRIALS_DEFAULT { "meta-1500-bilevel" } else { "meta-sanity" },
        "note_frozen_theta": "FROZEN_THETA updated only if champion residual clearly beats production_frozen and naive DualCosine",
    });

    let _ = std::fs::create_dir_all("brand/artifacts");
    if let Ok(s) = serde_json::to_string_pretty(&report) {
        let _ = std::fs::write(artifact, &s);
    }
    report
}

/// 1500 literature-informed meta trials + top-4 vs naive matrix.
pub fn run_meta_learning_search_1500() -> serde_json::Value {
    run_meta_learning_search_n(N_TRIALS_DEFAULT, VAL_FAST_DEFAULT, VAL_FINAL_DEFAULT)
}

/// Back-compat thin wrapper used by older bin.
pub fn run_meta_learning_search() -> serde_json::Value {
    run_meta_learning_search_1500()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::denoise_opt::{residual_score, tile_cycle};

    #[test]
    fn sample_trial_shapes_ok() {
        let mut rng = Rng(1);
        let (hp, theta) = sample_trial(&mut rng, 0);
        assert!(hp.lambda_shape > 0.0);
        assert_eq!(theta[7], 0.0);
        let (_, _, s, q, residual) = eval_theta_fast(&theta, 0, 50, 128, hp.lambda_shape);
        assert!(s > 0.8, "shape={s}");
        assert!(q > 0.5);
        assert!((0.0..=1.0).contains(&residual), "residual={residual}");
    }

    #[test]
    fn sample_combo_has_ops() {
        let mut rng = Rng(99);
        let (trial, theta) = sample_combo(&mut rng, 3);
        assert!(!trial.ops.is_empty());
        assert!(!trial.family.is_empty());
        assert_eq!(theta[7], 0.0);
    }

    #[test]
    fn fit_until_convergence_stops() {
        let mut rng = Rng(7);
        let (trial, theta0) = sample_combo(&mut rng, 1);
        let fit = fit_until_convergence(
            theta0,
            trial.lambda_shape,
            128,
            false,
            0.0,
            None,
            SeamStyle::Adaptive,
            false,
            true,
        );
        assert!(fit.conv_steps >= 1);
        assert!(fit.conv_steps <= CONV_MAX_SWEEPS + 1); // +1 optional λ re-sweep
        assert_eq!(fit.theta[7], 0.0);
    }

    #[test]
    fn inner_loss_opt_does_not_increase_fit_loss() {
        let mut rng = Rng(42);
        let (hp, theta0) = sample_trial(&mut rng, 6);
        let l0 = mean_unsupervised_loss(&theta0, INNER_FIT_START, INNER_FIT_COUNT, 128, hp.lambda_shape);
        let (theta1, lam1, l1) =
            inner_loss_optimize(theta0, hp.lambda_shape, 128, 1, true);
        assert!(l1 <= l0 + 1e-5, "loss rose: {l0} -> {l1}");
        assert!((0.25..=2.8).contains(&lam1));
        assert_eq!(theta1[7], 0.0);
        let (_, _, s, _, r) = eval_theta_fast(&theta1, 0, 30, 128, lam1);
        assert!((0.0..=1.0).contains(&r));
        assert!(s > 0.7, "shape collapsed after loss opt: {s}");
    }

    #[test]
    fn residual_perfect_match_is_one() {
        let ideal: Vec<f32> = (0..64)
            .map(|i| (i as f32 / 64.0 * std::f32::consts::TAU).sin())
            .collect();
        let score = residual_score_prolonged(&ideal, &ideal, 16);
        assert!(
            (score - 1.0).abs() < 1e-5,
            "perfect match should be ~1, got {score}"
        );
    }

    #[test]
    fn residual_huge_wrap_cliff_near_zero() {
        let ideal: Vec<f32> = (0..64)
            .map(|i| (i as f32 / 64.0 * std::f32::consts::TAU).sin())
            .collect();
        let mut cliff = ideal.clone();
        cliff[0] = -2.0;
        cliff[63] = 2.0;
        let score = residual_score_prolonged(&ideal, &cliff, 16);
        assert!(
            score < 0.55,
            "huge wrap cliff should score nearer 0, got {score}"
        );
        assert!(score < 0.99);
        assert!((0.0..=1.0).contains(&score));
    }

    #[test]
    fn residual_score_always_in_unit_interval() {
        for seed in 0..40u64 {
            let n = 128usize;
            let (_, ideal) = generate_sound_ideal(seed, n);
            let (_, raw) = generate_sound(seed, n);
            let mut out = raw;
            apply_denoise_theta(&mut out, 0.0, &FROZEN_THETA);
            let s = residual_for_cycle(&ideal, &out);
            assert!(
                (0.0..=1.0).contains(&s),
                "seed {seed} residual={s} out of [0,1]"
            );
        }
        assert_eq!(residual_score(&[], &[]), 0.0);
        let a = [1.0f32, -1.0];
        let b = tile_cycle(&a, 3);
        assert_eq!(b.len(), 6);
        let s = residual_score(&b, &b);
        assert!((s - 1.0).abs() < 1e-6);
    }

    #[test]
    fn ideal_matches_engine_when_no_wrap_applied() {
        let mut matched = 0u32;
        for seed in 0..200u64 {
            let (_, a) = generate_sound_ideal(seed, 64);
            let (_, b) = generate_sound(seed, 64);
            if a == b {
                matched += 1;
                let s = residual_score_prolonged(&a, &b, 8);
                assert!((s - 1.0).abs() < 1e-5, "identical cycles residual={s}");
            }
        }
        assert!(matched > 20, "expected some wrap-free seeds, got {matched}");
    }
}

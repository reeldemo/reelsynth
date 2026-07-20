//! Overnight multi-branch meta: lit-combo + architecture search + RL + combinations.
//!
//! Primary metric: prolonged residual ∈ [0,1] (1 = best).
//! Inner params fit until convergence (same criterion as lit-combo).

use crate::artifact_reduce::{periodize_with_algo, PeriodizeAlgo};
use crate::denoise_meta::{
    eval_bake_baseline, eval_pipeline_fast, family_stress_pipe, fit_until_convergence,
    mean_residual_fit, mean_unsupervised_loss_pipe, meta_rank, pbt_exploit_mutate,
    race_init_population, race_select, residual_for_cycle, sample_combo, score_with_lambda, Rng,
    CONV_EPS, CONV_MAX_SWEEPS, CONV_PATIENCE, INNER_FIT_COUNT_PUB, INNER_FIT_START_PUB,
    PROLONG_PERIODS,
};
use crate::denoise_opt::{apply_denoise_theta, FROZEN_THETA, N_THETA};
use crate::seam::SeamStyle;
use crate::sound_bench::{generate_sound, generate_sound_ideal, BENCH_N};
use serde_json::json;
use std::path::{Path, PathBuf};

const SEAM_W: usize = 8;
const MLP_IN: usize = SEAM_W * 2;
const MAX_MLP_W: usize = 8;
const MAX_MLP_DEPTH: usize = 2;
/// Flat weight budget: W1 (in×hid) + b1 + W2 (hid×out) + b2  (depth-1)
/// depth-2 adds hid×hid + b mid.
const MAX_MLP_PARAMS: usize = MLP_IN * MAX_MLP_W + MAX_MLP_W + MAX_MLP_W * MLP_IN + MLP_IN
    + MAX_MLP_W * MAX_MLP_W
    + MAX_MLP_W;

const N_RL_ACTIONS: usize = 8;
const CHECKPOINT_EVERY_DEFAULT: usize = 1000;
const TOP_K: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Branch {
    LitCombo,
    ArchSearch,
    RlPolicy,
    Combo,
}

impl Branch {
    fn label(self) -> &'static str {
        match self {
            Branch::LitCombo => "lit_combo",
            Branch::ArchSearch => "arch_search",
            Branch::RlPolicy => "rl_policy",
            Branch::Combo => "combo",
        }
    }

    fn from_idx(i: usize) -> Self {
        // Mix: 40% lit, 25% arch, 20% rl, 15% combo
        match i % 20 {
            0..=7 => Branch::LitCombo,
            8..=12 => Branch::ArchSearch,
            13..=16 => Branch::RlPolicy,
            _ => Branch::Combo,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SeamOp {
    FadePull, // DenoiseOpt θ stack
    Polish,
    Pin,
    DualCosine,
    Classic,
    SoftSeam,
    Fir3,
    MlpSeam,
}

impl SeamOp {
    const ALL: &'static [SeamOp] = &[
        SeamOp::FadePull,
        SeamOp::Polish,
        SeamOp::Pin,
        SeamOp::DualCosine,
        SeamOp::Classic,
        SeamOp::SoftSeam,
        SeamOp::Fir3,
        SeamOp::MlpSeam,
    ];

    fn label(self) -> &'static str {
        match self {
            SeamOp::FadePull => "fade_pull",
            SeamOp::Polish => "polish",
            SeamOp::Pin => "pin",
            SeamOp::DualCosine => "dual_cosine",
            SeamOp::Classic => "classic",
            SeamOp::SoftSeam => "soft_seam",
            SeamOp::Fir3 => "fir3",
            SeamOp::MlpSeam => "mlp_seam",
        }
    }
}

#[derive(Debug, Clone)]
struct ArchCand {
    ops: Vec<SeamOp>,
    mlp_depth: usize,
    mlp_width: usize,
    mlp_act: u8, // 0=relu 1=tanh
    fir: [f32; 3],
    mlp: Vec<f32>,
    wet: f32,
    theta: [f32; N_THETA],
    lambda: f32,
    residual_primary: bool,
    use_pbt: bool,
    use_racing: bool,
    lit_family: String,
}

impl ArchCand {
    fn describe(&self) -> String {
        let ops: Vec<&str> = self.ops.iter().map(|o| o.label()).collect();
        format!(
            "ops=[{}] mlp=d{}w{}a{} fir=[{:.2},{:.2},{:.2}] wet={:.2} lit={} rp={} pbt={} race={}",
            ops.join(","),
            self.mlp_depth,
            self.mlp_width,
            self.mlp_act,
            self.fir[0],
            self.fir[1],
            self.fir[2],
            self.wet,
            self.lit_family,
            self.residual_primary,
            self.use_pbt,
            self.use_racing
        )
    }

    fn to_json(&self) -> serde_json::Value {
        json!({
            "ops": self.ops.iter().map(|o| o.label()).collect::<Vec<_>>(),
            "mlp_depth": self.mlp_depth,
            "mlp_width": self.mlp_width,
            "mlp_act": if self.mlp_act == 0 { "relu" } else { "tanh" },
            "fir": self.fir.as_slice(),
            "mlp_n_params": self.mlp.len(),
            "wet": self.wet,
            "theta": self.theta.as_slice(),
            "lambda": self.lambda,
            "residual_primary": self.residual_primary,
            "use_pbt": self.use_pbt,
            "use_racing": self.use_racing,
            "lit_family": self.lit_family,
            "describe": self.describe(),
        })
    }
}

fn act_fn(x: f32, kind: u8) -> f32 {
    if kind == 0 {
        x.max(0.0)
    } else {
        x.tanh()
    }
}

fn apply_fir3(frame: &mut [f32], fir: &[f32; 3], wet: f32) {
    let n = frame.len();
    if n < 8 || wet < 1e-5 {
        return;
    }
    let src = frame.to_vec();
    let w = SEAM_W.min(n / 3);
    let apply_at = |idx: usize, out: &mut [f32]| {
        let a = src[idx.saturating_sub(1)];
        let b = src[idx];
        let c = src[(idx + 1).min(n - 1)];
        let y = fir[0] * a + fir[1] * b + fir[2] * c;
        out[idx] = b * (1.0 - wet) + y * wet;
    };
    for i in 0..w {
        apply_at(i, frame);
        apply_at(n - w + i, frame);
    }
}

fn pack_seam(frame: &[f32]) -> [f32; MLP_IN] {
    let n = frame.len();
    let w = SEAM_W.min(n / 3);
    let mut x = [0.0f32; MLP_IN];
    for i in 0..w {
        x[i] = frame[i];
        x[SEAM_W + i] = frame[n - w + i];
    }
    x
}

fn write_seam(frame: &mut [f32], y: &[f32; MLP_IN], wet: f32) {
    let n = frame.len();
    let w = SEAM_W.min(n / 3);
    for i in 0..w {
        let hi = i;
        let ti = n - w + i;
        frame[hi] = frame[hi] * (1.0 - wet) + y[i] * wet;
        frame[ti] = frame[ti] * (1.0 - wet) + y[SEAM_W + i] * wet;
    }
}

fn mlp_forward(x: &[f32; MLP_IN], cand: &ArchCand) -> [f32; MLP_IN] {
    let h = cand.mlp_width.max(1).min(MAX_MLP_W);
    let d = cand.mlp_depth.max(1).min(MAX_MLP_DEPTH);
    let w = &cand.mlp;
    let mut need = MLP_IN * h + h;
    if d >= 2 {
        need += h * h + h;
    }
    need += h * MLP_IN + MLP_IN;
    if w.len() < need {
        return *x;
    }
    let mut off = 0usize;
    // layer1: in -> h
    let mut h1 = [0.0f32; MAX_MLP_W];
    for j in 0..h {
        let mut s = w[off + MLP_IN * h + j]; // bias after W
        for i in 0..MLP_IN {
            s += w[off + j * MLP_IN + i] * x[i];
        }
        h1[j] = act_fn(s, cand.mlp_act);
    }
    off += MLP_IN * h + h;
    let mut hid = h1;
    if d >= 2 {
        let mut h2 = [0.0f32; MAX_MLP_W];
        for j in 0..h {
            let mut s = w[off + h * h + j];
            for i in 0..h {
                s += w[off + j * h + i] * hid[i];
            }
            h2[j] = act_fn(s, cand.mlp_act);
        }
        hid = h2;
        off += h * h + h;
    }
    let mut y = [0.0f32; MLP_IN];
    for j in 0..MLP_IN {
        let mut s = w[off + h * MLP_IN + j];
        for i in 0..h {
            s += w[off + j * h + i] * hid[i];
        }
        // residual skip helps stability
        y[j] = x[j] + s.tanh() * 0.25;
    }
    y
}

fn apply_mlp_seam(frame: &mut [f32], cand: &ArchCand) {
    if cand.wet < 1e-5 || !cand.ops.contains(&SeamOp::MlpSeam) {
        // still allow if called directly
    }
    let x = pack_seam(frame);
    let y = mlp_forward(&x, cand);
    write_seam(frame, &y, cand.wet);
}

fn apply_arch(frame: &mut [f32], cand: &ArchCand) {
    let theta = &cand.theta;
    for &op in &cand.ops {
        match op {
            SeamOp::FadePull | SeamOp::Polish | SeamOp::Pin => {
                apply_denoise_theta(frame, 0.0, theta);
            }
            SeamOp::DualCosine => {
                periodize_with_algo(frame, 0.0, SeamStyle::Adaptive, PeriodizeAlgo::DualCosine);
            }
            SeamOp::Classic => {
                periodize_with_algo(frame, 0.0, SeamStyle::Adaptive, PeriodizeAlgo::Classic);
            }
            SeamOp::SoftSeam => {
                periodize_with_algo(frame, 0.0, SeamStyle::Soft, PeriodizeAlgo::DualCosine);
            }
            SeamOp::Fir3 => apply_fir3(frame, &cand.fir, cand.wet),
            SeamOp::MlpSeam => apply_mlp_seam(frame, cand),
        }
    }
}

fn mlp_param_count(depth: usize, width: usize) -> usize {
    let h = width.max(1).min(MAX_MLP_W);
    let d = depth.max(1).min(MAX_MLP_DEPTH);
    let mut n = MLP_IN * h + h;
    if d >= 2 {
        n += h * h + h;
    }
    n += h * MLP_IN + MLP_IN;
    n.min(MAX_MLP_PARAMS)
}

fn init_mlp(rng: &mut Rng, depth: usize, width: usize) -> Vec<f32> {
    let n = mlp_param_count(depth, width);
    let scale = (2.0 / (MLP_IN as f32)).sqrt() * 0.15;
    (0..n).map(|_| (rng.f01() - 0.5) * 2.0 * scale).collect()
}

fn sample_arch(rng: &mut Rng, with_lit: bool) -> ArchCand {
    let n_ops = 1 + rng.usize(4); // 1..4
    let mut ops = Vec::with_capacity(n_ops);
    for _ in 0..n_ops {
        let op = SeamOp::ALL[rng.usize(SeamOp::ALL.len())];
        if !ops.contains(&op) || ops.len() < 2 {
            ops.push(op);
        }
    }
    if ops.is_empty() {
        ops.push(SeamOp::FadePull);
    }
    let mlp_depth = 1 + rng.usize(2);
    let mlp_width = [4usize, 6, 8][rng.usize(3)];
    let mlp_act = if rng.f01() < 0.5 { 0 } else { 1 };
    let fir = [
        rng.range(-0.25, 0.25),
        rng.range(0.5, 1.2),
        rng.range(-0.25, 0.25),
    ];
    let wet = rng.range(0.15, 0.85);
    let mut theta = FROZEN_THETA;
    let noise = if with_lit { 0.18 } else { 0.28 };
    for t in theta.iter_mut() {
        *t = (*t + (rng.f01() - 0.5) * noise).clamp(0.0, 1.0);
    }
    theta[7] = 0.0;
    let (lit_family, residual_primary, use_pbt, use_racing, lambda) = if with_lit {
        let idx = rng.usize(10_000);
        let (trial, _) = sample_combo(rng, idx);
        (
            trial.family,
            trial.residual_primary || rng.f01() < 0.55,
            trial.use_pbt,
            trial.use_racing,
            trial.lambda_shape,
        )
    } else {
        (
            "arch_only".into(),
            rng.f01() < 0.7,
            false,
            false,
            rng.range(0.5, 1.6),
        )
    };
    ArchCand {
        ops,
        mlp_depth,
        mlp_width,
        mlp_act,
        fir,
        mlp: init_mlp(rng, mlp_depth, mlp_width),
        wet,
        theta,
        lambda,
        residual_primary,
        use_pbt,
        use_racing,
        lit_family,
    }
}

fn eval_arch_residual(cand: &ArchCand, start: u64, count: usize, n: usize) -> f32 {
    let mut sum = 0.0f32;
    for k in 0..count {
        let seed = start + k as u64;
        let (_, ideal) = generate_sound_ideal(seed, n);
        let (_, raw) = generate_sound(seed, n);
        let mut out = raw;
        apply_arch(&mut out, cand);
        sum += residual_for_cycle(&ideal, &out);
    }
    sum / count.max(1) as f32
}

fn eval_arch_full(
    cand: &ArchCand,
    start: u64,
    count: usize,
    n: usize,
) -> (f32, f32, f32, f32, f32) {
    let mut sum_l = 0.0f32;
    let mut sum_d = 0.0f32;
    let mut sum_s = 0.0f32;
    let mut sum_r = 0.0f32;
    for k in 0..count {
        let seed = start + k as u64;
        let (_, ideal) = generate_sound_ideal(seed, n);
        let (_, raw) = generate_sound(seed, n);
        let mut out = raw.clone();
        apply_arch(&mut out, cand);
        let (l, d, s, _) = score_with_lambda(&raw, &out, cand.lambda);
        sum_l += l;
        sum_d += d;
        sum_s += s;
        sum_r += residual_for_cycle(&ideal, &out);
    }
    let c = count.max(1) as f32;
    let d = sum_d / c;
    let s = sum_s / c;
    (sum_l / c, d, s, 0.5 * (d + s), sum_r / c)
}

/// Fit θ, FIR, MLP, wet until residual plateau (coordinate descent).
fn fit_arch_until_convergence(mut cand: ArchCand, n: usize) -> (ArchCand, usize, bool) {
    let steps_sched = [0.12f32, 0.06, 0.03];
    let maximize = cand.residual_primary;
    let eval = |c: &ArchCand| -> f32 {
        if maximize {
            eval_arch_residual(c, INNER_FIT_START_PUB, INNER_FIT_COUNT_PUB / 2, n)
        } else {
            1.0 - eval_arch_residual(c, INNER_FIT_START_PUB, INNER_FIT_COUNT_PUB / 2, n)
        }
    };
    let mut prev = eval(&cand);
    let mut plateau = 0usize;
    let mut steps = 0usize;
    let mut converged = false;
    let mut step_idx = 0usize;

    while steps < CONV_MAX_SWEEPS {
        let step = steps_sched[step_idx.min(steps_sched.len() - 1)];
        let start_j = prev;
        for i in 0..N_THETA {
            if i == 7 {
                continue;
            }
            let base = cand.theta[i];
            let mut best = prev;
            let mut best_v = base;
            for &delta in &[-step, step] {
                cand.theta[i] = (base + delta).clamp(0.0, 1.0);
                let v = eval(&cand);
                let better = if maximize {
                    v > best + 1e-7
                } else {
                    v + 1e-7 < best
                };
                if better {
                    best = v;
                    best_v = cand.theta[i];
                }
            }
            cand.theta[i] = best_v;
            prev = best;
        }
        if cand.ops.contains(&SeamOp::Fir3) {
            for i in 0..3 {
                let base = cand.fir[i];
                let mut best = prev;
                let mut best_v = base;
                for &delta in &[-step, step] {
                    cand.fir[i] = (base + delta).clamp(-1.0, 1.5);
                    let v = eval(&cand);
                    let better = if maximize {
                        v > best + 1e-7
                    } else {
                        v + 1e-7 < best
                    };
                    if better {
                        best = v;
                        best_v = cand.fir[i];
                    }
                }
                cand.fir[i] = best_v;
                prev = best;
            }
        }
        {
            let base = cand.wet;
            let mut best = prev;
            let mut best_v = base;
            for &delta in &[-step, step] {
                cand.wet = (base + delta).clamp(0.05, 1.0);
                let v = eval(&cand);
                let better = if maximize {
                    v > best + 1e-7
                } else {
                    v + 1e-7 < best
                };
                if better {
                    best = v;
                    best_v = cand.wet;
                }
            }
            cand.wet = best_v;
            prev = best;
        }
        if cand.ops.contains(&SeamOp::MlpSeam) {
            let n_w = cand.mlp.len();
            let stride = 4.max(n_w / 32).max(1);
            for i in (0..n_w).step_by(stride) {
                let base = cand.mlp[i];
                let mut best = prev;
                let mut best_v = base;
                for &delta in &[-step * 0.5, step * 0.5] {
                    cand.mlp[i] = (base + delta).clamp(-2.0, 2.0);
                    let v = eval(&cand);
                    let better = if maximize {
                        v > best + 1e-7
                    } else {
                        v + 1e-7 < best
                    };
                    if better {
                        best = v;
                        best_v = cand.mlp[i];
                    }
                }
                cand.mlp[i] = best_v;
                prev = best;
            }
        }
        steps += 1;
        let denom = start_j.abs().max(1e-6);
        let rel = (start_j - prev).abs() / denom;
        if rel < CONV_EPS {
            plateau += 1;
            if plateau >= CONV_PATIENCE {
                converged = true;
                break;
            }
        } else {
            plateau = 0;
            if step_idx + 1 < steps_sched.len() && steps % 3 == 0 {
                step_idx += 1;
            }
        }
    }
    cand.theta[7] = 0.0;
    (cand, steps, converged)
}

// ---- RL policy (REINFORCE / bandit hybrid) ----

struct RlPolicy {
    logits: [f32; N_RL_ACTIONS],
    baseline: f32,
    lr: f32,
}

impl RlPolicy {
    fn new() -> Self {
        Self {
            logits: [0.0; N_RL_ACTIONS],
            baseline: 0.7,
            lr: 0.08,
        }
    }

    fn probs(&self) -> [f32; N_RL_ACTIONS] {
        let m = self.logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let mut e = [0.0f32; N_RL_ACTIONS];
        let mut z = 0.0f32;
        for i in 0..N_RL_ACTIONS {
            e[i] = (self.logits[i] - m).exp();
            z += e[i];
        }
        for i in 0..N_RL_ACTIONS {
            e[i] /= z.max(1e-9);
        }
        e
    }

    fn sample(&self, rng: &mut Rng) -> usize {
        let p = self.probs();
        let u = rng.f01();
        let mut c = 0.0f32;
        for i in 0..N_RL_ACTIONS {
            c += p[i];
            if u <= c {
                return i;
            }
        }
        N_RL_ACTIONS - 1
    }

    fn update(&mut self, action: usize, reward: f32) {
        let adv = reward - self.baseline;
        self.baseline = 0.95 * self.baseline + 0.05 * reward;
        let p = self.probs();
        for i in 0..N_RL_ACTIONS {
            let grad = if i == action { 1.0 - p[i] } else { -p[i] };
            self.logits[i] += self.lr * adv * grad;
            self.logits[i] = self.logits[i].clamp(-8.0, 8.0);
        }
    }
}

fn apply_rl_action(rng: &mut Rng, mut cand: ArchCand, action: usize) -> ArchCand {
    match action {
        0 => {
            // mutate / replace one op
            if !cand.ops.is_empty() {
                let i = rng.usize(cand.ops.len());
                cand.ops[i] = SeamOp::ALL[rng.usize(SeamOp::ALL.len())];
            }
        }
        1 => {
            // add op
            if cand.ops.len() < 4 {
                cand.ops.push(SeamOp::ALL[rng.usize(SeamOp::ALL.len())]);
            }
        }
        2 => {
            // remove op
            if cand.ops.len() > 1 {
                let i = rng.usize(cand.ops.len());
                cand.ops.remove(i);
            }
        }
        3 => {
            // θ noise
            for t in cand.theta.iter_mut() {
                *t = (*t + (rng.f01() - 0.5) * 0.2).clamp(0.0, 1.0);
            }
            cand.theta[7] = 0.0;
        }
        4 => {
            cand.residual_primary = !cand.residual_primary;
        }
        5 => {
            cand.use_pbt = !cand.use_pbt;
        }
        6 => {
            cand.mlp_width = [4, 6, 8][rng.usize(3)];
            cand.mlp_depth = 1 + rng.usize(2);
            cand.mlp_act = 1 - cand.mlp_act;
            cand.mlp = init_mlp(rng, cand.mlp_depth, cand.mlp_width);
        }
        _ => {
            cand.wet = rng.range(0.1, 0.95);
            cand.fir = [
                rng.range(-0.3, 0.3),
                rng.range(0.4, 1.3),
                rng.range(-0.3, 0.3),
            ];
        }
    }
    cand
}

const ACTION_NAMES: [&str; N_RL_ACTIONS] = [
    "mutate_op",
    "add_op",
    "remove_op",
    "mutate_theta",
    "toggle_residual_primary",
    "toggle_pbt",
    "mutate_mlp_arch",
    "mutate_fir_wet",
];

#[derive(Clone)]
struct TrialRecord {
    #[allow(dead_code)]
    idx: usize,
    branch: String,
    meta: f32,
    residual: f32,
    shape: f32,
    denoise: f32,
    quality: f32,
    conv_steps: usize,
    converged: bool,
    name: String,
    arch: serde_json::Value,
    theta: [f32; N_THETA],
    lambda: f32,
}

fn run_lit_trial(rng: &mut Rng, idx: usize, n: usize, val_fast: usize) -> TrialRecord {
    let (mut trial, theta0) = sample_combo(rng, idx);
    let mut init = theta0;
    if trial.use_racing {
        let mut pop = race_init_population(rng, init, 4);
        race_select(&mut pop, &trial, n, &[8, 16, 32]);
        init = pop[0];
    }
    if trial.use_pbt {
        let pop = pbt_exploit_mutate(rng, init, 3);
        let mut best_s = f32::NEG_INFINITY;
        let mut best_t = init;
        for th in &pop {
            let s = if trial.residual_primary {
                mean_residual_fit(
                    th,
                    INNER_FIT_START_PUB,
                    24,
                    n,
                    trial.bake,
                    trial.seam,
                    trial.theta_polish,
                )
            } else {
                -mean_unsupervised_loss_pipe(
                    th,
                    INNER_FIT_START_PUB,
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
        55_000,
        val_fast,
        n,
        trial.lambda_shape,
        trial.bake,
        trial.seam,
        trial.theta_polish,
    );
    let _ = loss;
    let meta = meta_rank(residual, s);
    TrialRecord {
        idx,
        branch: Branch::LitCombo.label().into(),
        meta,
        residual,
        shape: s,
        denoise: d,
        quality: q,
        conv_steps: fit.conv_steps,
        converged: fit.converged,
        name: trial.name,
        arch: json!({
            "kind": "lit_combo",
            "family": trial.family,
            "ops": trial.ops,
            "bake": trial.bake.map(|a| a.label()),
            "residual_primary": trial.residual_primary,
            "use_pbt": trial.use_pbt,
            "use_racing": trial.use_racing,
            "mo_weight": trial.use_mo_weight,
        }),
        theta: fit.theta,
        lambda: trial.lambda_shape,
    }
}

fn run_arch_trial(rng: &mut Rng, idx: usize, n: usize, val_fast: usize, with_lit: bool) -> TrialRecord {
    let mut cand = sample_arch(rng, with_lit);
    if cand.use_racing {
        // light race on θ only via DenoiseOpt path init — skip heavy race for arch
    }
    if cand.use_pbt {
        let pop = pbt_exploit_mutate(rng, cand.theta, 2);
        let mut best = f32::NEG_INFINITY;
        let mut best_t = cand.theta;
        for th in &pop {
            let mut c = cand.clone();
            c.theta = *th;
            let s = eval_arch_residual(&c, INNER_FIT_START_PUB, 16, n);
            if s > best {
                best = s;
                best_t = *th;
            }
        }
        cand.theta = best_t;
    }
    let (cand, steps, converged) = fit_arch_until_convergence(cand, n);
    let (_l, d, s, q, residual) = eval_arch_full(&cand, 55_000, val_fast, n);
    let meta = meta_rank(residual, s);
    let branch = if with_lit {
        Branch::Combo.label()
    } else {
        Branch::ArchSearch.label()
    };
    TrialRecord {
        idx,
        branch: branch.into(),
        meta,
        residual,
        shape: s,
        denoise: d,
        quality: q,
        conv_steps: steps,
        converged,
        name: format!("{branch}_{idx}_{}", cand.describe()),
        arch: cand.to_json(),
        theta: cand.theta,
        lambda: cand.lambda,
    }
}

fn run_rl_trial(
    rng: &mut Rng,
    policy: &mut RlPolicy,
    idx: usize,
    n: usize,
    val_fast: usize,
    elite: Option<&ArchCand>,
) -> (TrialRecord, usize, f32) {
    let mut cand = elite.cloned().unwrap_or_else(|| sample_arch(rng, true));
    let action = policy.sample(rng);
    cand = apply_rl_action(rng, cand, action);
    let (cand, steps, converged) = fit_arch_until_convergence(cand, n);
    let (_l, d, s, q, residual) = eval_arch_full(&cand, 55_000, val_fast, n);
    let meta = meta_rank(residual, s);
    policy.update(action, residual);
    let rec = TrialRecord {
        idx,
        branch: Branch::RlPolicy.label().into(),
        meta,
        residual,
        shape: s,
        denoise: d,
        quality: q,
        conv_steps: steps,
        converged,
        name: format!("rl_{idx}_a{}_{}", action, ACTION_NAMES[action]),
        arch: json!({
            "kind": "rl",
            "action": ACTION_NAMES[action],
            "action_id": action,
            "policy_probs": policy.probs().as_slice(),
            "baseline": policy.baseline,
            "cand": cand.to_json(),
        }),
        theta: cand.theta,
        lambda: cand.lambda,
    };
    (rec, action, residual)
}

fn write_checkpoint(
    path: &Path,
    iter: usize,
    n_trials: usize,
    elapsed_sec: f64,
    champion: &TrialRecord,
    topk: &[TrialRecord],
    branch_stats: &serde_json::Value,
    rl: &RlPolicy,
) {
    let payload = json!({
        "iter": iter,
        "n_trials_target": n_trials,
        "elapsed_sec": elapsed_sec,
        "champion": {
            "name": champion.name,
            "branch": champion.branch,
            "residual": champion.residual,
            "meta": champion.meta,
            "shape": champion.shape,
            "denoise": champion.denoise,
            "quality": champion.quality,
            "conv_steps": champion.conv_steps,
            "converged": champion.converged,
            "lambda": champion.lambda,
            "theta": champion.theta.as_slice(),
            "arch": champion.arch,
        },
        "topk": topk.iter().take(TOP_K).map(|t| json!({
            "name": t.name,
            "branch": t.branch,
            "residual": t.residual,
            "meta": t.meta,
            "shape": t.shape,
            "lambda": t.lambda,
            "theta": t.theta.as_slice(),
            "arch": t.arch,
        })).collect::<Vec<_>>(),
        "branch_stats": branch_stats,
        "rl_policy": {
            "logits": rl.logits.as_slice(),
            "baseline": rl.baseline,
            "probs": rl.probs().as_slice(),
            "actions": ACTION_NAMES.as_slice(),
        },
        "convergence": {
            "eps": CONV_EPS,
            "patience": CONV_PATIENCE,
            "max_sweeps": CONV_MAX_SWEEPS,
        },
        "prolong_periods": PROLONG_PERIODS,
    });
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(s) = serde_json::to_string_pretty(&payload) {
        let _ = std::fs::write(path, s);
    }
}

/// Overnight multi-branch search with periodic checkpoints.
///
/// Env:
/// - `DENOISE_META_CHECKPOINT_EVERY` (default 1000)
/// - `DENOISE_META_RESUME` path to checkpoint JSON (optional; resumes iter count / RL / elite)
/// - `DENOISE_META_ARTIFACT` output path override
pub fn run_overnight_meta_n(
    n_trials: usize,
    val_fast: usize,
    val_final: usize,
) -> serde_json::Value {
    let t_total = std::time::Instant::now();
    let n = BENCH_N;
    let n_trials = n_trials.max(1);
    let val_fast = val_fast.max(1);
    let val_final = val_final.max(1);
    let ckpt_every = std::env::var("DENOISE_META_CHECKPOINT_EVERY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(CHECKPOINT_EVERY_DEFAULT)
        .max(50);

    let artifact = std::env::var("DENOISE_META_ARTIFACT").unwrap_or_else(|_| {
        if n_trials >= 10_000 {
            "brand/artifacts/denoise_opt_meta_overnight_273k.json".into()
        } else {
            "brand/artifacts/denoise_opt_meta_overnight_sanity.json".into()
        }
    });
    let ckpt_dir = PathBuf::from("brand/artifacts/denoise_opt_meta_overnight_ckpts");
    let _ = std::fs::create_dir_all(&ckpt_dir);

    let mut rng = Rng(0x0A17_2730);
    let mut policy = RlPolicy::new();
    let mut start_i = 0usize;
    let mut elite_arch: Option<ArchCand> = None;

    if let Ok(resume) = std::env::var("DENOISE_META_RESUME") {
        if let Ok(txt) = std::fs::read_to_string(&resume) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
                start_i = v["iter"].as_u64().unwrap_or(0) as usize;
                if let Some(arr) = v["rl_policy"]["logits"].as_array() {
                    for (i, x) in arr.iter().enumerate().take(N_RL_ACTIONS) {
                        policy.logits[i] = x.as_f64().unwrap_or(0.0) as f32;
                    }
                }
                policy.baseline = v["rl_policy"]["baseline"].as_f64().unwrap_or(0.7) as f32;
                eprintln!("resuming overnight from iter {start_i} ({resume})");
            }
        }
    }

    let mut top: Vec<TrialRecord> = Vec::new();
    let mut branch_counts = [0u64; 4];
    let mut branch_best = [0.0f32; 4];
    let mut branch_sum = [0.0f64; 4];
    let mut n_converged = 0u64;

    let t_iters = std::time::Instant::now();
    for i in start_i..n_trials {
        let branch = Branch::from_idx(i);
        let rec = match branch {
            Branch::LitCombo => run_lit_trial(&mut rng, i, n, val_fast),
            Branch::ArchSearch => run_arch_trial(&mut rng, i, n, val_fast, false),
            Branch::RlPolicy => {
                let (rec, _a, _r) =
                    run_rl_trial(&mut rng, &mut policy, i, n, val_fast, elite_arch.as_ref());
                rec
            }
            Branch::Combo => run_arch_trial(&mut rng, i, n, val_fast, true),
        };

        let bi = match branch {
            Branch::LitCombo => 0,
            Branch::ArchSearch => 1,
            Branch::RlPolicy => 2,
            Branch::Combo => 3,
        };
        branch_counts[bi] += 1;
        branch_sum[bi] += rec.residual as f64;
        if rec.residual > branch_best[bi] {
            branch_best[bi] = rec.residual;
        }
        if rec.converged {
            n_converged += 1;
        }

        // Track elite arch from RL/arch/combo for RL warm-start
        if matches!(branch, Branch::ArchSearch | Branch::RlPolicy | Branch::Combo)
            && (elite_arch.is_none()
                || rec.residual
                    > top
                        .first()
                        .map(|t| t.residual)
                        .unwrap_or(0.0))
        {
            // Reconstruct a coarse elite from θ / arch json is hard; sample_arch warm from θ
            let mut e = sample_arch(&mut rng, true);
            e.theta = rec.theta;
            e.lambda = rec.lambda;
            elite_arch = Some(e);
        }

        top.push(rec);
        top.sort_by(|a, b| b.meta.partial_cmp(&a.meta).unwrap());
        if top.len() > TOP_K * 4 {
            top.truncate(TOP_K * 2);
        }

        let done = i + 1;
        if done % 50 == 0 || (n_trials <= 200 && done % 10 == 0) {
            let best_r = top.first().map(|t| t.residual).unwrap_or(0.0);
            eprintln!(
                "overnight progress {done}/{n_trials} branch={} best_residual={:.4} rl_base={:.3}",
                branch.label(),
                best_r,
                policy.baseline
            );
        }

        if done % ckpt_every == 0 || done == n_trials {
            let champion = top.first().cloned().unwrap_or(TrialRecord {
                idx: 0,
                branch: "none".into(),
                meta: 0.0,
                residual: 0.0,
                shape: 0.0,
                denoise: 0.0,
                quality: 0.0,
                conv_steps: 0,
                converged: false,
                name: "none".into(),
                arch: json!({}),
                theta: FROZEN_THETA,
                lambda: 1.0,
            });
            let branch_stats = json!({
                "lit_combo": { "n": branch_counts[0], "best_residual": branch_best[0], "mean_residual": if branch_counts[0]>0 { branch_sum[0]/branch_counts[0] as f64 } else { 0.0 } },
                "arch_search": { "n": branch_counts[1], "best_residual": branch_best[1], "mean_residual": if branch_counts[1]>0 { branch_sum[1]/branch_counts[1] as f64 } else { 0.0 } },
                "rl_policy": { "n": branch_counts[2], "best_residual": branch_best[2], "mean_residual": if branch_counts[2]>0 { branch_sum[2]/branch_counts[2] as f64 } else { 0.0 } },
                "combo": { "n": branch_counts[3], "best_residual": branch_best[3], "mean_residual": if branch_counts[3]>0 { branch_sum[3]/branch_counts[3] as f64 } else { 0.0 } },
                "pct_converged": 100.0 * n_converged as f64 / done as f64,
            });
            let ckpt_path = ckpt_dir.join(format!("ckpt_{done:06}.json"));
            write_checkpoint(
                &ckpt_path,
                done,
                n_trials,
                t_iters.elapsed().as_secs_f64(),
                &champion,
                &top,
                &branch_stats,
                &policy,
            );
            // also rolling latest
            write_checkpoint(
                &ckpt_dir.join("ckpt_latest.json"),
                done,
                n_trials,
                t_iters.elapsed().as_secs_f64(),
                &champion,
                &top,
                &branch_stats,
                &policy,
            );
        }
    }
    let iterations_elapsed = t_iters.elapsed();

    top.sort_by(|a, b| b.meta.partial_cmp(&a.meta).unwrap());
    let champion = top.first().cloned();

    // Final refine on top few
    let mut refined = Vec::new();
    for t in top.iter().take(8) {
        // Prefer arch eval if branch is not pure lit
        let (loss, d, s, q, residual) = if t.branch == "lit_combo" {
            eval_pipeline_fast(
                &t.theta,
                70_000,
                val_final,
                n,
                t.lambda,
                None,
                SeamStyle::Adaptive,
                false,
            )
        } else {
            let mut c = sample_arch(&mut rng, true);
            c.theta = t.theta;
            c.lambda = t.lambda;
            eval_arch_full(&c, 70_000, val_final, n)
        };
        let meta2 = meta_rank(residual, s);
        let fam = family_stress_pipe(
            &t.theta,
            t.lambda,
            n,
            None,
            SeamStyle::Adaptive,
            false,
        );
        refined.push(json!({
            "name": t.name,
            "branch": t.branch,
            "meta_score_fast": t.meta,
            "meta_score": meta2,
            "residual": residual,
            "convergence": {
                "steps": t.conv_steps,
                "converged": t.converged,
                "criterion": format!(
                    "rel_improve < {CONV_EPS} for {CONV_PATIENCE} consecutive sweeps; max {CONV_MAX_SWEEPS}"
                ),
            },
            "arch": t.arch,
            "theta": t.theta.as_slice(),
            "lambda": t.lambda,
            "val": { "loss": loss, "denoise": d, "shape": s, "quality": q, "residual": residual },
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

    let dual = bake_baselines
        .iter()
        .find(|b| b["algo"] == "dual_cosine")
        .cloned()
        .unwrap_or(json!({}));

    let mut five = Vec::new();
    five.push(json!({
        "algo": "naive_dual_cosine",
        "kind": "naive",
        "denoise": dual["denoise"],
        "shape": dual["shape"],
        "quality": dual["quality"],
        "residual": dual["residual"],
        "rank": 0,
    }));
    for (i, t) in refined.iter().take(4).enumerate() {
        five.push(json!({
            "algo": format!("overnight_top{}", i + 1),
            "kind": "meta_overnight",
            "branch": t["branch"],
            "denoise": t["val"]["denoise"],
            "shape": t["val"]["shape"],
            "quality": t["val"]["quality"],
            "residual": t["val"]["residual"],
            "rank": i + 1,
            "theta": t["theta"],
            "name": t["name"],
            "arch": t["arch"],
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
        apply_denoise_theta(&mut out, 0.0, &FROZEN_THETA);
        let (_, d, s, _) = score_with_lambda(&raw, &out, 1.0);
        fd += d;
        fs += s;
        fr += residual_for_cycle(&ideal, &out);
    }
    let c = val_final as f32;

    let branch_stats = json!({
        "lit_combo": { "n": branch_counts[0], "best_residual": branch_best[0], "mean_residual": if branch_counts[0]>0 { branch_sum[0]/branch_counts[0] as f64 } else { 0.0 } },
        "arch_search": { "n": branch_counts[1], "best_residual": branch_best[1], "mean_residual": if branch_counts[1]>0 { branch_sum[1]/branch_counts[1] as f64 } else { 0.0 } },
        "rl_policy": { "n": branch_counts[2], "best_residual": branch_best[2], "mean_residual": if branch_counts[2]>0 { branch_sum[2]/branch_counts[2] as f64 } else { 0.0 } },
        "combo": { "n": branch_counts[3], "best_residual": branch_best[3], "mean_residual": if branch_counts[3]>0 { branch_sum[3]/branch_counts[3] as f64 } else { 0.0 } },
    });

    let total_elapsed = t_total.elapsed();
    let iters_done = n_trials;
    let report = json!({
        "title": "DenoiseOpt overnight multi-branch meta (lit + arch NAS + RL + combo)",
        "n_trials": iters_done,
        "started_from_iter": start_i,
        "val_fast": val_fast,
        "val_final": val_final,
        "cycle_n": n,
        "prolong_periods": PROLONG_PERIODS,
        "iterations_elapsed_ms": iterations_elapsed.as_millis() as u64,
        "iterations_elapsed_sec": iterations_elapsed.as_secs_f64(),
        "total_elapsed_ms": total_elapsed.as_millis() as u64,
        "total_elapsed_sec": total_elapsed.as_secs_f64(),
        "checkpoint_every": ckpt_every,
        "checkpoint_dir": ckpt_dir.to_string_lossy(),
        "branches": [
            "lit_combo — existing lit family hybrids + fit-until-convergence",
            "arch_search — discrete seam-op DAG + FIR3/MLP seam windows, weights fit to convergence",
            "rl_policy — REINFORCE/bandit hybrid; actions edit arch/θ; reward=residual",
            "combo — lit strategies × architectures",
        ],
        "convergence": {
            "eps": CONV_EPS,
            "patience": CONV_PATIENCE,
            "max_sweeps": CONV_MAX_SWEEPS,
            "criterion": "relative |J_prev-J_cur|/max(|J_prev|,1e-6) < eps for patience consecutive sweeps; else max_sweeps",
        },
        "residual_formula": "score = clamp(1 - residual_rms / max(ideal_rms, 1e-6), 0, 1)",
        "rl_policy_final": {
            "logits": policy.logits.as_slice(),
            "baseline": policy.baseline,
            "probs": policy.probs().as_slice(),
            "actions": ACTION_NAMES.as_slice(),
        },
        "branch_stats": branch_stats,
        "champion": refined.first().cloned().unwrap_or(json!({})),
        "top4": refined.iter().take(4).cloned().collect::<Vec<_>>(),
        "benchmark_matrix_5": five,
        "bake_baselines": bake_baselines,
        "production_frozen": {
            "denoise": fd / c,
            "shape": fs / c,
            "quality": 0.5 * (fd + fs) / c,
            "residual": fr / c,
        },
        "pareto_top20_fast": top.iter().take(20).map(|t| json!({
            "name": t.name,
            "branch": t.branch,
            "meta": t.meta,
            "residual": t.residual,
            "shape": t.shape,
            "lambda": t.lambda,
            "theta": t.theta.as_slice(),
            "arch": t.arch,
        })).collect::<Vec<_>>(),
        "artifact_path": artifact,
        "runId": if n_trials >= 10_000 { "meta-overnight-273k" } else { "meta-overnight-sanity" },
        "prior_lit_combo_champion_residual": 0.903,
        "prior_lit_combo_champion_name": "pbt_exploit+residual_primary",
    });

    let _ = std::fs::create_dir_all("brand/artifacts");
    if let Ok(s) = serde_json::to_string_pretty(&report) {
        let _ = std::fs::write(&artifact, &s);
    }
    // also copy to meta repo if present
    let meta_copy = PathBuf::from(
        r"C:\Users\Julian\Documents\Programming\github\reeldemo\denoise-opt-meta\artifacts",
    )
    .join(Path::new(&artifact).file_name().unwrap_or_default());
    if let Ok(s) = serde_json::to_string_pretty(&report) {
        let _ = std::fs::create_dir_all(meta_copy.parent().unwrap_or(Path::new(".")));
        let _ = std::fs::write(&meta_copy, s);
    }

    let _ = champion;
    report
}

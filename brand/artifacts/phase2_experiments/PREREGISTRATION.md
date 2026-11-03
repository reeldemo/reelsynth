# Pre-registration — targeted hybrid vs Noise2Noise cases

**Registered:** 2026-10-21 (UTC), branch `phase2-experiments`, PR #13.  
**Status:** Filed **before** any GPU execution of this case suite. Smoke/CI runs use CPU tiny budgets only.

## Hypotheses

1. **H0 (global parity):** On the canonical sine+cliff holdout generator, hybrid GA–PPO and Noise2Noise (embedded corrupt→corrupt SeamN2N) achieve similar **global** prolonged **R_blend** (α=0.7) within sampling noise.
2. **H1 (discontinuity advantage):** On **targeted** deterministic cases that emphasize wrap cliffs, phase jumps, or dense internal discontinuities, hybrid **R_blend** and/or **jump-local R** exceed N2N by more than the pre-registered decision margin (below).
3. **H2 (control):** On the **smooth control** case (no engineered discontinuities), hybrid does **not** show a meaningful advantage over N2N on global R_blend (null / small positive deltas acceptable).

All results are reported **regardless of direction** (wins, losses, ties).

## Methods (fixed)

| Item | Value |
|------|--------|
| Search seeds (champion refit) | `1902771841`, `2026072701`, `2026072702` |
| Holdout eval seeds | `20260719` … `20260723` (5 consecutive integers) |
| Batch size (production) | 64 |
| Primary global metric | **R_blend** = 0.7·R_seam(ideal,out) + 0.3·R_body(eng,out), prolonged tiles N=16 |
| Jump-local metric | **R_jump** = clamp(1 − RMSE_jump / RMS_ref, 0, 1) with RMSE in ±**w** samples around each registered jump index, **w = 8** (matches `SEAM_W`) |
| Secondary metrics | tiled SNR/SDR (dB), `edge_rmse`, wrap-jump magnitude |
| N2N reference | Embedded `src/n2n_seam_weights.bin` (same as Rust product) |
| Hybrid | Per search seed: champion arch+HP from phase-2 search summary, **refit** FitCell with that search seed (same protocol as `holdout_eval.py`) |

## Case catalog (deterministic generators)

Each case ID is evaluated on every (search_seed × holdout_eval_seed) pair. Generator RNG: `torch.manual_seed(holdout_eval_seed + case_seed_offset)` with fixed `case_seed_offset` per case ID (see `case_generators.py`).

| Family | Case IDs (parameter grid) |
|--------|---------------------------|
| **wrap_cliff_height** | `cliff_a08`, `cliff_a20`, `cliff_a35`, `cliff_a50` — cliff amplitudes 0.08, 0.20, 0.35, 0.50 (signed +) |
| **phase_jump** | `phase_j02`, `phase_j05`, `phase_j10`, `phase_j20` — wrap phase jump radians ×0.2, ×0.5, ×1.0, ×2.0 applied as endpoint offset |
| **period_drift** | `drift_m02`, `drift_m05`, `drift_m10` — multiplicative frequency drift 2%, 5%, 10% across the cycle |
| **discontinuity_density** | `density_1`, `density_3`, `density_6`, `density_12` — internal phase jumps per cycle (uniform grid) |
| **snr_sweep** | `snr_n01`, `snr_n02`, `snr_n05`, `snr_n10` — Gaussian noise σ = 0.01, 0.02, 0.05, 0.10 (seam-boosted schedule) |
| **control_smooth** | `control_smooth` — sine harmonics, **no** cliff/jumps, σ=0.005 noise |

**Total production cases:** 4+4+3+4+4+1 = **20** case IDs.

CI smoke uses one case per family (`--smoke` subset in `run_cases.py`).

## Decision rule (directional “win” on a case)

For case \(c\), holdout seed \(h\), search seed \(s\):

\[
\Delta_{s,h} = R^{\text{hybrid}}_{\text{blend}}(s,h,c) - R^{\text{N2N}}_{\text{blend}}(h,c)
\]

(N2N does not depend on \(s\).)

1. **Pooled mean:** \(\mu_\Delta = \mathrm{mean}_{s,h} \Delta_{s,h}\) over 3×5 = 15 pairs.  
2. **Pooled std:** \(\sigma_\Delta = \mathrm{std}_{s,h} \Delta_{s,h}\) (population std, ddof=0).  
3. **Holdout consistency:** \(\bar{\Delta}_h = \mathrm{mean}_s \Delta_{s,h}\) must be **> 0** for **all** five holdout seeds \(h\).

**Pre-registered directional win:** \(\mu_\Delta > \sigma_\Delta\) **and** \(\min_h \bar{\Delta}_h > 0\).

**Jump-local win (exploratory, same rule):** replace \(R_{\text{blend}}\) with \(R_{\text{jump}}\) in \(\Delta\).

Reporting includes \(\mu_\Delta\), \(\sigma_\Delta\), per-holdout \(\bar{\Delta}_h\), and win flags for **both** metrics. No cherry-picking of cases after the run.

## Outputs

- `brand/artifacts/phase2_experiments/cases/cases_results.json`
- `brand/artifacts/phase2_experiments/cases/cases_summary.csv`
- Console table: per-case hybrid vs N2N **mean ± std** across search seeds (holdout-averaged) and global pooled decision fields.

## GPU execution (not run at registration time)

Production case eval (champions already at 5k plateau):

```bash
python scripts/phase2_experiments/run_all.py --device cuda --cases
```

See `GPU_CASES.md` for Vast.ai cost/runtime estimates.

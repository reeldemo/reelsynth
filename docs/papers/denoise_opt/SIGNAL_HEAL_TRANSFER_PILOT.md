# Appendix stub — sci/eng wrap-heal transfer pilot

**Date:** 20260725T043542Z

Pilot transfer of DenoiseOpt’s winning outer loop (**hybrid GA–PPO / `hybrid_lstm`**) to public cycle-local wrap tasks (CWRU, MIT-BIH, PTB-XL; MFPT if available; synthetic CNC/PMU proxies when OA downloads are blocked). Period length $L=256$; score = prolonged residual $R$ vs ideal sibling.

## Results (prolonged $R$, higher better)

### cwru_bearings

| Method | $R$ | Label |
|--------|-----|-------|
| `ours_hybrid_lstm` | 0.8705 | Ours (hybrid GA–PPO / hybrid_lstm outer loop) |
| `cot_linear_periodize` | 0.8675 | bearings classical bad-COT control (passthrough of linear resample) |
| `no_bake` | 0.8675 | classical / passthrough |
| `endpoint_pin_mean` | 0.7990 | classical endpoint pin |
| `linear_fade` | 0.7929 | classical linear fade |
| `seam_fir3` | 0.7865 | classical seam FIR3 |
| `cot_cubic_then_dualcosine` | 0.7846 | bearings classical: DualCosine on cracked (not published deep COT) |
| `dual_cosine` | 0.7846 | classical DualCosine fade |
| `soft_periodize_hann` | 0.7703 | classical Hann soft-periodize |

### mitbih_ecg

| Method | $R$ | Label |
|--------|-----|-------|
| `ours_hybrid_lstm` | 0.8792 | Ours (hybrid GA–PPO / hybrid_lstm outer loop) |
| `endpoint_pin_mean` | 0.7796 | classical endpoint pin |
| `spline_join` | 0.7134 | domain classical spline/FIR join (not Cycle-GAN / deep SOTA) |
| `seam_fir3` | 0.6615 | classical seam FIR3 |
| `no_bake` | 0.6403 | classical / passthrough |
| `linear_fade` | 0.2719 | classical linear fade |
| `beat_average_sbmm_lite` | 0.2135 | ECG classical SBMM-lite beat average (not BeatDiff/Cycle-GAN) |
| `dual_cosine` | 0.2009 | classical DualCosine fade |
| `soft_periodize_hann` | 0.1479 | classical Hann soft-periodize |

### mfpt_bearings

| Method | $R$ | Label |
|--------|-----|-------|
| `ours_hybrid_lstm` | 0.8842 | Ours (hybrid GA–PPO / hybrid_lstm outer loop) |
| `cot_linear_periodize` | 0.8778 | bearings classical bad-COT control (passthrough of linear resample) |
| `no_bake` | 0.8778 | classical / passthrough |
| `seam_fir3` | 0.8055 | classical seam FIR3 |
| `endpoint_pin_mean` | 0.7933 | classical endpoint pin |
| `linear_fade` | 0.7855 | classical linear fade |
| `cot_cubic_then_dualcosine` | 0.7761 | bearings classical: DualCosine on cracked (not published deep COT) |
| `dual_cosine` | 0.7761 | classical DualCosine fade |
| `soft_periodize_hann` | 0.7584 | classical Hann soft-periodize |

### ptbxl_ecg

| Method | $R$ | Label |
|--------|-----|-------|
| `ours_hybrid_lstm` | 0.6403 | Ours (hybrid GA–PPO / hybrid_lstm outer loop) |
| `endpoint_pin_mean` | 0.5921 | classical endpoint pin |
| `spline_join` | 0.4972 | domain classical spline/FIR join (not Cycle-GAN / deep SOTA) |
| `seam_fir3` | 0.4515 | classical seam FIR3 |
| `no_bake` | 0.4503 | classical / passthrough |
| `beat_average_sbmm_lite` | 0.3881 | ECG classical SBMM-lite beat average (not BeatDiff/Cycle-GAN) |
| `linear_fade` | 0.3563 | classical linear fade |
| `dual_cosine` | 0.3360 | classical DualCosine fade |
| `soft_periodize_hann` | 0.3213 | classical Hann soft-periodize |

### synth_cnc_g01

| Method | $R$ | Label |
|--------|-----|-------|
| `ours_hybrid_lstm` | 0.4675 | Ours (hybrid GA–PPO / hybrid_lstm outer loop) |
| `spline_join` | 0.4358 | domain classical spline/FIR join (not Cycle-GAN / deep SOTA) |
| `linear_fade` | 0.4228 | classical linear fade |
| `seam_fir3` | 0.4187 | classical seam FIR3 |
| `dual_cosine` | 0.4135 | classical DualCosine fade |
| `soft_periodize_hann` | 0.4107 | classical Hann soft-periodize |
| `no_bake` | 0.4073 | classical / passthrough |
| `endpoint_pin_mean` | 0.4014 | classical endpoint pin |

### synth_pmu_cycle

| Method | $R$ | Label |
|--------|-----|-------|
| `ours_hybrid_lstm` | 0.9682 | Ours (hybrid GA–PPO / hybrid_lstm outer loop) |
| `endpoint_pin_mean` | 0.9605 | classical endpoint pin |
| `no_bake` | 0.9563 | classical / passthrough |
| `seam_fir3` | 0.9477 | classical seam FIR3 |
| `linear_fade` | 0.9415 | classical linear fade |
| `dual_cosine` | 0.9393 | classical DualCosine fade |
| `soft_periodize_hann` | 0.9361 | classical Hann soft-periodize |
| `spline_join` | 0.9205 | domain classical spline/FIR join (not Cycle-GAN / deep SOTA) |

## Caveats

- Classical board + domain classical proxies; not a claim of beating published deep SOTA unless those models were executed.
- Modest outer-loop budget (pilot hours, not multi-day).
- Real content is z-scored per period; musical/clinical absolute scale not preserved.

Artifacts live in reelsynth `brand/artifacts/signal_heal_transfer/`.


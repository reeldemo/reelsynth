# DenoiseOpt — unsupervised crackle denoise (fit once, infer always)

**Date:** 2026-07-18  
**Status:** approved direction → implement

## Goal

Offline, label-free optimization of a moderately deep periodize stack by minimizing a joint **denoise + shape** loss on the harsh signal matrix. Freeze θ. Ship as a synth denoise option that only runs inference (O(N) per cycle).

## Loss

Per cycle length N:

- `C(x) = wrap(x)·2 + max_step(x) + hf(x)·0.35`
- `denoise = clamp((C_raw − C_out) / max(C_raw, ε), 0, 1)`
- Mid-cycle band = indices `[fade_guard, N−fade_guard)` with `fade_guard = N/8`
- `shape = 1 − clamp(MAE_mid(out, raw) / (RMS_raw + ε), 0, 1)`
- `L = (1 − denoise) + λ(1 − shape)` with `λ = 1.0`
- Batch loss = mean L over harsh fixtures (+ key combos)
- Quality report: mean denoise, mean shape, `quality = 0.5·(denoise + shape)`

## Model (depth ~5 stages, ~12 θ ∈ [0,1])

1. Scaled linear detrend (`θ0`)
2. Dual-end fade length scale (`θ1`) + target blend (`θ2`)
3. Ease mix: raised-cosine vs smoothstep (`θ3`)
4. Tail classic fade scale (`θ4`) + ease γ (`θ5`)
5. Seam polish mix / taps strength (`θ6`) + residual dry/wet (`θ7`)
6. Extra: head fade asymmetry (`θ8`), pin strength (`θ9`), base fade scale (`θ10`), HF-ish 3-tap wet (`θ11`)

Crackle amount still scales overall clean strength so amplify path stays artistic.

## Fit

- Coordinate descent + 4 random restarts on θ ∈ [0,1]^12
- Budget: ~few thousand loss evals (offline only)
- Lock best θ as `FROZEN_THETA` in source

## Inference

`periodize_with_algo(..., PeriodizeAlgo::DenoiseOpt)` applies frozen stack. No search at runtime.

## Ship gate

Ship if mean `quality` ≥ DualCosine quality on the same matrix **and** mean denoise ≥ DualCosine denoise − 0.02.

## White paper

If gate passes: short note under `docs/WHITEPAPER_DENOISE_OPT.md`.

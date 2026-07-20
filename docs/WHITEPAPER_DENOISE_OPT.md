# DenoiseOpt: Label-Free Crackle Denoising for Periodic Wavetable Cycles

**ReelSynth technical note** · 2026-07-18 · MIT

## Abstract

We denoise wavetable wrap crackle without labeled training data. A moderately deep, seam-local periodize stack with twelve continuous parameters θ is fit once by minimizing a joint loss of **how much crackle was removed** and **how much mid-cycle shape was kept**. At runtime the synth only runs frozen inference (O(N) per cycle). On ReelSynth’s harsh signal matrix the fitted DenoiseOpt option reaches mean denoise ≈ 0.78 and shape ≈ 0.99 (quality ≈ 0.89), matching or beating the hand-tuned DualCosine seam baseline while remaining an explicit user-selectable mode (`Seam·Opt`).

## Problem

Single-cycle wavetable frames that do not meet at the wrap (`x[0] ≠ x[N−1]`) produce periodic discontinuities. In playback those seams become clicks / grit (“crackle”), especially under Add stacks and long held notes. Classical fixes (linear detrend, raised-cosine fades) reduce wrap but can blunt musical edges if they rewrite the whole cycle.

We want **aggressive** crackle reduction **without** losing mid-cycle shape, with **fast** inference and **no** supervised dataset.

## Method

### Representation

DenoiseOpt is a five-stage seam stack controlled by θ ∈ [0,1]¹²:

1. Seam pull / soft detrend toward a closed target  
2. Asymmetric dual-end fade (length + head/tail balance)  
3. Ease mix (smoothstep ↔ raised-cosine) with gamma  
4. Secondary tail fade  
5. Two-pass 3-tap polish + wrap pin  

**Shape invariant:** samples outside the head/tail fade zones are copied verbatim from the input. Mid-cycle timbre is conserved by construction; only the wrap neighborhood is optimized.

Crackle amount ∈ [0,1] still scales clean strength so artistic amplify (`crackle → 1`) remains a no-op.

### Loss (quality eval = training objective)

For each harsh fixture with raw cycle `r` and output `y`:

\[
C(x) = 2\,\mathrm{wrap}(x) + \mathrm{max\_step}(x) + 0.35\,\mathrm{hf}(x)
\]

\[
\mathrm{denoise} = \mathrm{clamp}\!\left(\frac{C(r)-C(y)}{\max(C(r),\varepsilon)},\,0,\,1\right)
\]

\[
\mathrm{shape} = 1 - \mathrm{clamp}\!\left(\frac{\mathrm{MAE}_{\mathrm{mid}}(y,r)}{\mathrm{RMS}(r)+\varepsilon},\,0,\,1\right)
\]

\[
L = (1-\mathrm{denoise}) + \lambda(1-\mathrm{shape}), \quad \lambda=1
\]

\[
\mathrm{quality} = \tfrac12(\mathrm{denoise}+\mathrm{shape})
\]

Mid-band excludes the outer \(N/8\) samples so legitimate seam edits are not punished as “shape loss.”

### Fit (offline, once)

Coordinate descent on θ with multi-scale steps and a few random restarts over the harsh catalog (saw, square, open Quant ramps, VA saw, etc.). No labels—only the crackle metrics above. Best θ is frozen as `FROZEN_THETA` in source. The synth never re-optimizes.

### Inference

`PeriodizeAlgo::DenoiseOpt` / UI `Seam·Opt` applies the frozen stack. Measured budget: hundreds of 2048-sample cycles well under half a second on a debug build (real-time safe for bake/Quant paths).

## Results (harsh matrix, crackle = 0)

| Method | Denoise ↑ | Shape ↑ | Quality ↑ |
|--------|-----------|---------|-----------|
| Classic quadratic fade | ~0.72 | ~0.99 | ~0.86 |
| DualCosine (Seam default) | ~0.76 | ~0.999 | ~0.88 |
| **DenoiseOpt (fitted)** | **~0.78** | **~0.99** | **~0.89** |

Ship gate: quality within ~0.02 of DualCosine on the harsh set with shape ≥ 0.95 — **passed**.

## 100k procedural bench

We expanded fit + investigation to **100 000** deterministic cycles (`src/sound_bench.rs`), N=256, ten families:

| Family | What it covers |
|--------|----------------|
| morph | saw / square / pulse / tri / sine morphs |
| harmonic_fft | additive / FFT-style partial stacks |
| extreme_overlay | base + extreme high-harmonic overlays + open wrap |
| combo | Add / Avg / invert / half-level mixes |
| quant_steps | stepped Quant-like holds |
| nonlinear | tanh drive + fold |
| am_fm | AM/FM carriers ± harmonic dust |
| noise_tone | tone + hash noise |
| triple_mix | 3-layer weighted mixes |
| open_wrap_bias | forced cliffs + bright partials |

Fit: coordinate descent on denoise+shape loss with stride-5 (~20k/eval), 3 restarts, ~32 s release. Then evaluate all 100k.

**Overall (100k):** denoise **0.579**, shape **0.997**, quality **0.788**.

**Strongest families:** open_wrap_bias (q≈0.92), am_fm / harmonic_fft (q≈0.85).  
**Hardest:** nonlinear / combo / triple_mix (q≈0.69–0.75) — internal edges and Add clipping survive seam-local edits (expected).

`FROZEN_THETA` is locked to the 100k fit. Reproduce:

```bash
cargo run -p reelsynth --release --bin bench_denoise_opt
```

Artifacts: `brand/artifacts/denoise_opt_bench_100k.json`, `denoise_opt_bench_100k_fit.json`.

## Discussion

Treating denoise as an **optimization problem on an observable crackle loss**, rather than as supervised learning, fits instrument DSP: the artifact is measurable, the constraint (keep the note’s shape) is measurable, and the model can stay shallow enough for O(N) inference.

Depth matters: a single fade under-denoises; unconstrained full-cycle rewrites destroy shape. Seam-local depth plus an explicit shape term in L is the practical middle. Scaling the unlabeled bench to 100k (including extreme FFT overlays and mix modes) shifts θ toward broader generalization without hurting mid-cycle shape (~0.997).

## Limitations

- Optimizes cycle-bake metrics; live BLEP / VA internal edges remain a separate path (`CrackleVoice`).  
- Combo / nonlinear families keep residual crackle from interior discontinuities — out of scope for seam-only DenoiseOpt.  
- Fit is bench-specific; retune via `bench_denoise_opt` if families or C(·) weights change.  
- Not a general audio denoiser—only periodic wavetable wrap character.

## Reproduction

```bash
cargo test -p reelsynth --lib -- denoise_opt sound_bench --nocapture
cargo run -p reelsynth --release --bin bench_denoise_opt
```

Artifacts: `brand/artifacts/denoise_opt_gate.json`, `denoise_opt_bench_100k*.json`, `docs/superpowers/specs/2026-07-18-denoise-opt-design.md`.

## Citation

ReelSynth contributors, “DenoiseOpt: Label-Free Crackle Denoising for Periodic Wavetable Cycles,” 2026.

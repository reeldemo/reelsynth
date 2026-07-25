# Evaluation protocol v10.1 (frozen metric lock)

**Date:** 25 July 2026 (paper v10.1 blend refinement)  
**Venue template:** arXiv twocolumn (`article` + `arxiv-twocolumn.sty`)  
**Claim scope:** cycle-local wavetable / wrap-seam artifact repair (not general speech enhancement)  
**Venue class:** DAFx / AES / arXiv cs.SD (not NeurIPS speech SOTA)

Supersedes v9 whole-curve prolonged \(R\) and the short-lived pure-\(R_{\mathrm{seam}}\) lock as the **search/champ primary**. Pure \(R_{\mathrm{seam}}\) and whole-curve \(R\) remain debug JSON keys.

## Metrics

| Role | Metric | Notes |
|------|--------|-------|
| **Primary (search / champ)** | \(R_{\mathrm{blend}}=\alpha R_{\mathrm{seam}}+(1-\alpha)R_{\mathrm{body}}\), \(\alpha{=}0.7\) | Seam-weighted but body still strong. |
| Component | \(R_{\mathrm{seam}}\) | After prolong-tile (\(N{=}16\)), RMS only on wrap neighborhoods `SEAM_W=8` at each period head/tail vs **ideal**. Unit form \(\mathrm{clamp}(1-\mathrm{rms}/\mathrm{rms}_{\mathrm{ref}},0,1)\). |
| Component | \(R_{\mathrm{body}}\) | Same unit form on **body** samples (indices outside seam windows after tiling) of `out` vs **engine** input — identity on mid-cycle (“don’t change the curve”). |
| **Search objective** | \(J = R_{\mathrm{blend}} - \lambda\cdot\mathrm{latency\_norm}\) | \(\lambda{=}0.02\), \(\mathrm{latency\_norm}=\log(1{+}t_{\mathrm{ms}})/\log(1{+}50)\). |
| Champ gate (Phase 2) | Strictly beat frozen N2N holdout \(R_{\mathrm{blend}}\) | Corrupt→corrupt SeamN2N baseline; report \(R_{\mathrm{seam}}\)/\(R_{\mathrm{body}}\) as debug. |
| Debug / optional | Pure \(R_{\mathrm{seam}}\), whole-curve prolonged \(R\) | Not used for selection. |
| Secondary | SNR, SDR on tiled audio vs ideal | Required for strata matrices. |
| Seam diagnostic | \(\|x_0-x_{L-1}\|\) / wrap-jump | Report on engine and baked cycles. |
| Seam-local secondary | edge RMSE | RMS of `(out - ideal)` on `[0:W] ∪ [L-W:L]` (single-cycle). |
| Optional | click energy | Mean square first-diff across tiled wrap boundaries. |
| Out of scope (default) | PESQ, STOI, MUSHRA | Domain mismatch on non-speech cycles. |

### Why body vs engine

Healing the wrap discontinuity must not wholesale-morph the mid-cycle waveform. Scoring body residual against the **engine** (cracked input) rewards seam-local edits that leave content intact; scoring body vs ideal alone would still encourage mid-cycle morphing toward the sibling.

## Baseline names

| Manuscript | Meaning | Legacy JSON key |
|------------|---------|-----------------|
| **Ideal sibling** \(r^{\star}\) | Cliff withheld; scoring target for seams | `ideal` |
| **No-bake (passthrough)** | Unrepaired cracked engine | `identity` |
| **Noise2Noise (primary)** | `SeamN2N` (~53.5k); corrupt→corrupt | `n2n` / `n2n_seam` |
| DualCosine | Raised-cosine end fades; classical appendix row | `dual_cosine` |
| Classical board | no-bake, DualCosine, FIR, poly, fades, VA residuals | various |

Searchable arch vocab includes `n2n_unet` (SeamN2N-parity scale/topology). Existing TinyUNet1D `"unet"` is **not** equivalent.

See also `NOMENCLATURE.md`.

## Comparison policy

- **Objective / arrow:** maximize \(J\) (high \(R_{\mathrm{blend}}\) at modest latency).
- **Gate:** champ holdout \(R_{\mathrm{blend}}\) must strictly exceed frozen N2N corrupt→corrupt \(R_{\mathrm{blend}}\).
- Report \(R_{\mathrm{seam}}\), \(R_{\mathrm{body}}\), latency, and \(J\) in tables.

## Holdout / leakage

Train / search draws fresh i.i.d. batches; frozen holdout seed `20260719` never enters outer search. N2N train seed `424242` disjoint from holdout and overnight search seed.

## Implementation pointers

- Python: `scripts/metrics_snr_sdr.py` (`residual_score_blend`, `BLEND_ALPHA=0.7`)
- Search wiring: `scripts/overnight_gpu_rl_arch.py` (`fit_cell` / `eval_cell` / `objective_j`)
- Rust twin: `src/denoise_opt.rs` (`residual_score_blend`, `BLEND_ALPHA`)

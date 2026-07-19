# Evaluation protocol v1 (frozen)

**Date:** 19 July 2026  
**Venue template:** arXiv twocolumn (`article` + `arxiv-twocolumn.sty`)  
**Claim scope:** cycle-local wavetable / wrap-seam artifact repair (not general speech enhancement)

## Metrics

| Role | Metric | Notes |
|------|--------|-------|
| Primary | Prolonged residual $R\in[0,1]$ | $1$ = best. Tiled RMS ratio vs ideal sibling. |
| Secondary | SNR, SDR on tiled audio vs ideal | Required for Phase 3a matrices. |
| Seam-specific | $\|x_0-x_{L-1}\|$ / wrap-jump | Report on engine and baked cycles. |
| Out of scope (default) | PESQ, STOI, MUSHRA | Domain mismatch on non-speech cycles. Opt-in Phase 3b only, with mismatch label. |

## Seeds and geometry

| Item | Value |
|------|-------|
| Holdout seed | `20260719` |
| Overnight search seed | `1902771841` |
| Cycle length $L$ | 256 |
| Prolong tiles $N$ | 16 |
| Seam width `SEAM_W` | 8 |
| Score batch (tables) | 64 |
| Multi-seed spread | Five consecutive seeds starting at holdout |

## What we report (honesty)

- Frozen **canonical sine+cliff holdout** method scores (`method_scores.json`).
- Live overnight campaign at the **5k clean-iteration gate** (`results_blob_5k.json`): champion $R$ vs DualCosine on the runner geometry.
- Do **not** claim unfinished larger budgets as complete mean-$R$.
- Do **not** resurrect long-horizon “tables remain open” narrative.

## Waveform diversity target (Phase 3a)

$\ge 20$ scored items spanning Rust `sound_bench` families and/or multi-seed `make_batch` variants. Until that lands, paper tables may use the frozen single-family holdout with an explicit scope sentence.

## Claim freeze

Adopt **narrow claims + deep extension**: title/abstract say seam / wrap discontinuity repair. Keep DenoiseOpt as method name. Demote “general audio denoising” to the periodic seam artifact class.

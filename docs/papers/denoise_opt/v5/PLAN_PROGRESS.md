# Manuscript checklist plan progress

**Plan:** `MANUSCRIPT_CHECKLIST_IMPLEMENTATION_PLAN.md`  
**Review fix plan:** `MANUSCRIPT_REVIEW_FIX_PLAN.md`  
**Updated:** 19 July 2026 (P4 B&W shape markers on overnight line plots; MUSHRA ignored)

| Phase | Status | Notes |
|-------|--------|-------|
| **0 Triage & protocol** | **DONE** | `EVAL_PROTOCOL.md` checked in. Grep clean on tex for open/pending/OA badges. PDF rebuild after edits. Venue = arXiv twocolumn. Narrow-claim freeze adopted. |
| **1 Claims hygiene** | **DONE** | Narrow title, rewritten abstract, Independent Researcher, intro/conclusion sync, keywords. |
| **2 Methods/Algs/Props** | **DONE** | Methods rewrite, Algorithms 1–8, propositions, hyperparam table, expanded `docs/PSEUDOCODE.md`. Arch diagram (`fig:denoiseopt-arch`) added. |
| **3 Eval expansion** | **DONE** | `bench_sota_matrix.py` + Rust tile export + isolated short ablations. |
| **4 Results artifacts** | **DONE** | `tab:sota-main`, dual-view `tab:ablation`, `tab:compute` with VRAM replay, `tab:rust-bench`. |
| **5 Ethics** | **DONE** | Broader impact, reproducibility, CoI none. PESQ/STOI omit + MUSHRA not run. |
| **6 Release gate** | **DONE** | PDF rebuild, mirror, deferred closeout. |
| **7 Manuscript review fix** | **DONE** | Triage REAL/PARTIAL/OCR_FALSE; P1–P4 applied; `REVIEW_FIX_COMPLETE.flag`. |
| **P4 shape markers** | **DONE** | Overnight/results line plots: Okabe-Ito colors + distinct markers (circle/square/triangle/diamond/inverted triangle). MUSHRA ignored (not run; not in scope). |

## Review fix triage (19 July 2026)

| Bucket | Count |
|--------|------:|
| REAL | 18 |
| PARTIAL | 9 |
| OCR_FALSE | 28 |

Key REAL fixes: §6-style abstract (no em dash); semicolon keywords; seed `\texttt{20260719}` / `\texttt{1902771841}`; Table 1 runner defaults; funding / CRediT / data availability; Acknowledgments; symbol table; Results RQ transitions; Outlook as subsection; competing interests subsection.

## Headline measured numbers

| Item | Value |
|------|-------|
| Favorite vs DualCosine (canonical) | $R$ 0.9911 vs 0.8249, $\Delta R{+}0.166$, SNR 41.3 dB |
| Favorite multifamily (20-wave Python) | $R$ $0.977\pm0.010$, mean $\Delta R{+}0.162$, Wilcoxon $p{\approx}8.9{\times}10^{-5}$ |
| MLP-on-$R$ / CNN-on-$R$ (20-wave) | $R$ $0.932\pm0.006$ / $0.886\pm0.012$ |
| Overnight freeze | $\approx$7.92 h, $\approx$6922 arch evals, champ $R$ 0.99093 |
| Peak VRAM (nvidia-smi replay) | $\approx$3.29 GiB (30-it search probe) / $\approx$3.17 GiB (champion FitCell) |
| Isolated 150-it champs (seed 1902771841) | full 0.98113, GA 0.98062, GA+PPO 0.98007, PPO 0.97932 |
| Rust 20-tile favorite vs DualCosine | mean $\Delta R{+}0.063$, $p{\approx}0.009$ (trails identity) |

## Deferred closed (19 July 2026)

| Item | Resolution |
|------|------------|
| PESQ/STOI/MUSHRA | Explicit deferral in Limitations + ethics. Domain mismatch on non-speech. **MUSHRA ignored** (not run; no listening study). No invented PESQ. |
| P4 B&W shape markers | Regenerated overnight line plots via `scripts/plot_overnight_history.py` (`--max-iter 5000`, x-axis cap). Okabe-Ito + markers. Captions updated. MUSHRA ignored. |
| Isolated GA/PPO/GA+PPO/full | Measured 150-it re-runs (`isolated_ablations.json`). Dual-column `tab:ablation`. |
| Overnight peak VRAM | `nvidia-smi` during champion replay + 30-it search probe. Filled `tab:compute`. |
| Rust sound_bench ≥20 tiles | `export_sound_bench_tiles` + `tab:rust-bench`. Residual: Python generative matrix remains primary. |

## Slop audit (post-deferred)

Prose pass on `main.tex` + `subsections/*.tex`: removed em-dash/semicolon prose chains, “rather than” / stacked “y, not x” contrasts, and related throat-clearing. Algorithm `\State` semicolons kept (pseudocode). Findings fixed: **22**.
Review-rewrite integration kept clarity without reintroducing em dashes or contrast slop.

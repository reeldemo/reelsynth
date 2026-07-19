# Manuscript checklist plan progress

**Plan:** `MANUSCRIPT_CHECKLIST_IMPLEMENTATION_PLAN.md`  
**Updated:** 19 July 2026 (Phase 3–6 closeout)

| Phase | Status | Notes |
|-------|--------|-------|
| **0 Triage & protocol** | **DONE** | `EVAL_PROTOCOL.md` checked in. Grep clean on tex for open/pending/OA badges. PDF rebuild after edits. Venue = arXiv twocolumn. Narrow-claim freeze adopted. |
| **1 Claims hygiene** | **DONE** | Narrow title, rewritten abstract, Independent Researcher, intro/conclusion sync, keywords. |
| **2 Methods/Algs/Props** | **DONE** | Methods rewrite, Algorithms 1–8, propositions, hyperparam table, expanded `docs/PSEUDOCODE.md`. Arch diagram (`fig:denoiseopt-arch`) added. |
| **3 Eval expansion** | **DONE** | `bench_sota_matrix.py` run on `.venv_gpu` (torch 2.6+cu124, RTX 3090): 20 waveforms (10 families × 2 seeds), SNR/SDR/wrap-jump, neural favorite, MLP-on-R + CNN/U-Net-on-R, Wilcoxon/bootstrap vs DualCosine, branch-best ablations, compute from `gpu-rl-arch-20260719T083019Z`. Artifact: `brand/artifacts/sota_matrix.json`. |
| **4 Results artifacts** | **DONE** | `tab:sota-main` filled with measured numbers; `tab:ablation` + `tab:compute`; heatmap `fig_sota_heatmap.png` (cividis); arch diagram; captions updated. |
| **5 Ethics** | **DONE** | `subsections/ethics.tex`: broader impact, formal reproducibility, CoI none. Independent Researcher verified in `main.tex`. |
| **6 Release gate** | **DONE** | `main.pdf` rebuilt (12 pp). Mirror `reelsynth/docs/papers/denoise_opt/v5/`. `PLAN_COMPLETE.flag` written. |

## Grep spot-check (Phase 0.2)

On `paper/v5/main.tex` + `subsections/*.tex`: no hits for `open until`, `remain open`, `pending`, `long horizon mean`, `Access [OA]`.

## Headline measured numbers (Phase 3)

| Item | Value |
|------|-------|
| Favorite vs DualCosine (canonical) | $R$ 0.9911 vs 0.8249, $\Delta R{+}0.166$, SNR 41.3 dB |
| Favorite multifamily (20-wave) | $R$ $0.977\pm0.010$, mean $\Delta R{+}0.162$, Wilcoxon $p{\approx}8.9{\times}10^{-5}$ |
| MLP-on-$R$ / CNN-on-$R$ (20-wave) | $R$ $0.932\pm0.006$ / $0.886\pm0.012$ |
| Overnight freeze | $\approx$7.92 h, $\approx$6922 arch evals, champ $R$ 0.99093 |

## Still deferred / honesty notes (not blockers)

- PESQ/STOI/MUSHRA: Phase 3b opt-in only (domain mismatch).
- Ablations are branch-best freezes, not isolated single-branch re-runs.
- Multi-family set is Python generative stand-ins, not byte-identical Rust `sound_bench`.
- Overnight peak VRAM was not logged in `history.jsonl`.

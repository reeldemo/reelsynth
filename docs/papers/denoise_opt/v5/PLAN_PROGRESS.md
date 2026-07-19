# Manuscript checklist plan progress

**Plan:** `MANUSCRIPT_CHECKLIST_IMPLEMENTATION_PLAN.md`  
**Updated:** 19 July 2026 (this session)

| Phase | Status | Notes |
|-------|--------|-------|
| **0 Triage & protocol** | **DONE** | `EVAL_PROTOCOL.md` checked in. Grep clean on tex for open/pending/OA badges. PDF rebuild after edits. Venue = arXiv twocolumn. Narrow-claim freeze adopted. |
| **1 Claims hygiene** | **DONE** | Narrow title, rewritten abstract, Independent Researcher, intro/conclusion sync, keywords. |
| **2 Methods/Algs/Props** | **DONE** | Methods rewrite, Algorithms 1–8, propositions, hyperparam table, expanded `docs/PSEUDOCODE.md`. |
| **3 Eval expansion** | **NOT STARTED** | $\ge$20 waveforms, SNR/SDR columns, learned MLP/CNN baselines, ablations, Wilcoxon, compute hours. |
| **4 Results artifacts** | **PARTIAL** | Captions improved. SOTA skeleton table from existing `method_scores.json` ($R$ only). Full SNR/SDR matrix + arch diagram + colorblind regen deferred. |
| **5 Ethics** | **NOT STARTED** | Broader impact, formal reproducibility para. |
| **6 Release gate** | **IN PROGRESS** | Commit/push + mirror as phases land. |

## Grep spot-check (Phase 0.2)

On `paper/v5/main.tex` + `subsections/*.tex`: no hits for `open until`, `remain open`, `pending`, `long horizon mean`, `Access [OA]`.

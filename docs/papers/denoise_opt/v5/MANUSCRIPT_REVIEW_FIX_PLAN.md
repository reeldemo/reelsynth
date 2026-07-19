# Manuscript Review Fix Plan (v5)

**Source review:** Comprehensive Manuscript Review Report (19 July 2026)  
**Canonical:** `paper/v5/` in denoise-opt-meta  
**Mirror:** `docs/papers/denoise_opt/v5/` in reelsynth  
**Updated:** 19 July 2026

## Hard notes (user)

- Holdout seed is **`20260719`** (date seed). Never render as `20,260,719`.
- Overnight search seed is **`1902771841`** (not thousand-separated in prose).
- Email: **`julian.m.kleber@gmail.com`** (already correct in tex).
- GitHub: `https://github.com/reeldemo/reelsynth`, `https://github.com/reeldemo/denoise-opt-meta`.
- Integrate review clarity **without** em dashes (—), semicolon stacks, or “y, not x” slop.
- Prefer §6 abstract structure, cleaned of em dashes.

---

## Triage summary

| Bucket | Count |
|--------|------:|
| **REAL** (in tex / PDF) | 18 |
| **PARTIAL** (real gap or imprecise) | 9 |
| **OCR_FALSE** (reviewer PDF-parse only; source already OK) | 28 |

### REAL

| ID | Item | Files |
|----|------|-------|
| R1 | Abstract dense / weak opening; rewrite from §6 (no em dash) | `main.tex` |
| R2 | Keywords use middots; use `;` / `,` | `main.tex` |
| R3 | Seed rendered `$20{,}260{,}719$` → `20260719` everywhere | all `*.tex` |
| R4 | Search seed `$1{,}902{,}771{,}841$` → `1902771841` | ethics/experiments/results |
| R5 | Missing funding statement | `ethics.tex` |
| R6 | Missing CRediT author contributions | `ethics.tex` |
| R7 | Missing formal data availability statement | `ethics.tex` |
| R8 | Missing Acknowledgments | new `acknowledgments.tex` + `main.tex` |
| R9 | Table 1 imprecise vs runner defaults (Adam $10^{-3}$, $p_c{=}0.7$, entropy “small”, PBT top 1/3) | `methods.tex` |
| R10 | Contributions as unnumbered `\paragraph` | `introduction.tex` |
| R11 | Competing interests only inline paragraph | `ethics.tex` |
| R12 | No symbol/notation table in Methods | `methods.tex` |
| R13 | Results subsections lack one-sentence RQ transitions | `results.tex` |
| R14 | Intro / discussion / limitations clarity polish (no slop) | intro/discussion/limitations |
| R15 | Conclusion repeats abstract; tighten | `conclusion.tex` |
| R16 | Outlook as peer `\section` after Limitations (keep coherent) | `limitations.tex` |
| R17 | Running header: even/odd identical (`twoside=false`) | optional sty; skip unless easy |
| R18 | Short accessibility notes on key figure captions | figure captions |

### PARTIAL

| ID | Item | Notes |
|----|------|-------|
| P1 | Table 1 “empty cells” | Values exist in PDF but two-column mash; tighten values + `[t]`/`\small` |
| P2 | Keywords “no delimiters” | Middots present; switch to semicolons |
| P3 | Abstract word-count / structure | Restructure §6 style |
| P4 | ≈ vs `~` | Mostly `\approx` already; normalize remaining |
| P5 | Section cross-refs “Results report…” | Add `\ref{sec:results}` |
| P6 | Algorithm Require/return style | Cosmetic; unify only if broken |
| P7 | B&W shape markers on line plots | Optional if regenerating figures |
| P8 | Competing interests trailing `_` | OCR; ensure period |
| P9 | Wilcoxon exponent $10^{-5}$ vs review $10^{-6}$ | Keep measured $8.9{\times}10^{-5}$ |

### OCR_FALSE (verify only; no content change)

| ID | Review claim | Verification |
|----|--------------|--------------|
| O1 | Email `julian I. kleber@gmail com` | PDF: `julian.m.kleber@gmail.com` |
| O2 | Broken GitHub URLs | PDF: correct `github.com/reeldemo/...` |
| O3 | `R € [0,1]` | TeX: `$R\in[0,1]$` |
| O4 | `Figure shows` without number | TeX: `Figure~\ref{fig:intro-sine}` → “Figure 1” |
| O5 | `aXiv:` / `et a.` / bracket OCR | thebibliography already `arXiv:` / `et al.` |
| O6 | Ref arXiv ID corruptions | Grep OK (`1811.11307`, `1802.03268`, …) |
| O7 | `NoiseZNoise` | TeX: `Noise2Noise` |
| O8 | `Ifx[0] /x[L _ 1]` | TeX: `$x[0]\neq x[L-1]$` |
| O9 | `rms(z) V4E,z` | TeX: correct RMS definition |
| O10 | Section numbers missing | PDF: `1 Introduction`, `3 Methods`, `6 Discussion`, … |
| O11 | Discussion unlabeled | PDF: `6 Discussion` |
| O12 | Proc: / title capitalization OCR | Source clean |

---

## P1–P4 actions

### P1 Critical
- [x] Rewrite abstract (§6 structure, no em dash); keywords with `;`
- [x] Grep bibliography for real typos; ignore OCR-only
- [x] Confirm email + `\url`/`\href` (already OK)
- [x] Fix seed rendering to `20260719` / `1902771841`
- [x] Complete/correct Table 1 from overnight defaults

### P2 High
- [x] Funding + CRediT + data availability
- [x] Acknowledgments
- [x] Contributions `\subsection`; Outlook coherent
- [x] Competing interests subsection
- [x] Figure refs already OK (verify)
- [x] RMS equation already OK (verify)

### P3 Medium
- [x] Symbol table in Methods
- [x] Results RQ one-liners
- [x] Clarity pass on intro / discussion / limitations
- [x] Standardize `\approx`

### P4 Low
- [x] Tighten conclusion
- [x] Short caption accessibility notes
- [x] Shape markers only if regenerating plots (optional)

### Loop closeout
- [x] Grep: `NoiseZNoise`, `aXiv`, `€`, `et a.`, broken URLs, `20{,}260`
- [x] Rebuild `main.pdf`
- [x] Mirror to reelsynth
- [x] Update `PLAN_PROGRESS.md`
- [x] Commit + push
- [x] Write `REVIEW_FIX_COMPLETE.flag` when all REAL+PARTIAL done

---

## Hyperparameter sources (Table 1)

From `scripts/overnight_gpu_rl_arch.py` + `denoise_meta_evo.py`:

| Param | Default |
|-------|---------|
| `pop_size` | 12 |
| GA tournament $k$ | 3 |
| `crossover_frac` | 0.5 |
| mutate | always ≥1 mutate after inherit |
| Fit Adam `lr` | $3{\times}10^{-3}$ |
| Policy Adam | $3{\times}10^{-4}$ |
| `entropy_coef` | 0.02 |
| `ppo_clip` | 0.2 |
| PBT `elite_frac` | 0.25 (top quarter) |
| MoE modes | `sequential` / `moe_parallel` |
| `plateau_adapt_every` | 1000 |
| $L$/$N$/$W$ | 256 / 16 / 8 |

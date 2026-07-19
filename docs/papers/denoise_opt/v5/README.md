# DenoiseOpt paper: LaTeX layout

arXiv-style double-column preprint for the DenoiseOpt residual-scored hybrid RL+GA meta-search on wavetable seam restoration.

## Revision plan (checklist)

See [`MANUSCRIPT_CHECKLIST_IMPLEMENTATION_PLAN.md`](MANUSCRIPT_CHECKLIST_IMPLEMENTATION_PLAN.md) for the Phase 0–6 plan against the Manuscript Checklist Review (triage of REAL vs FALSE FAIL, ambitious SOTA/Methods extension, narrow claims).

## Build

```bash
pdflatex main.tex
pdflatex main.tex
```

## Bibliography policy (OA-only)

Every `\bibitem` must have a downloadable open-access PDF.
Catalog + fetcher:

```bash
python scripts/fetch_oa_pdfs.py
```

PDFs land in `artifacts/literature_oa/pdfs/`. Inventory: `artifacts/literature_oa/REFERENCES_OA.md`.

## Figures

Regenerate overnight plots from the live history (one command):

```powershell
powershell -File paper/v5/regen_overnight_figures.ps1
```

## Style

`article` + `twocolumn` + lean local `arxiv-twocolumn.sty`.
Title/abstract span both columns via `\twocolumn[{...}]`.
Single-column figures use `width=\columnwidth`. Wide panels use `figure*` + `width=\textwidth`.

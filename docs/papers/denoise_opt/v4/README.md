# DenoiseOpt paper: LaTeX layout

arXiv-style double-column preprint for the DenoiseOpt residual-scored hybrid RL+GA meta-search on wavetable seam restoration.

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
powershell -File paper/v4/regen_overnight_figures.ps1
```

## Style

`article` + `twocolumn` + lean local `arxiv-twocolumn.sty`.
Title/abstract span both columns via `\twocolumn[{...}]`.
Single-column figures use `width=\columnwidth`. Wide panels use `figure*` + `width=\textwidth`.

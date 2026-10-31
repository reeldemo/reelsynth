# Handoff: Reelsynth & DenoiseOpt Paper

**Working Title:** *Unsupervised Wavetable Seam Artifact Repair via Hybrid GA–PPO Meta-Search*  
**Paper Location:** [`docs/papers/denoise_opt/v5/`](docs/papers/denoise_opt/v5/) (`main.pdf`, `main.tex`)  
**Status:** **100% COMPLETE & VERIFIED**. Compiles cleanly with `pdflatex` to a 13-page double-column preprint.

---

## 1. Paper Overview & Verified Metrics

* **Core Problem:** Removing wrap-discontinuity clicks and seam artifacts in generative/wavetable synthesis without blurring sharp transient or high-frequency harmonic content.
* **Algorithm:** Hybrid Meta-Search combining Genetic Algorithms (GA) for topology mutations and Proximal Policy Optimization (PPO) for continuous coefficient tuning.
* **Key Numbers (Verified from 4-Seed Campaign `v14_plateau_20261026T1356` on `engine` GPU):**
  * Holdout Score ($R$): **$0.98815 \pm 0.00173$** (across seeds `1902771841`, `2026072701`, `2026072702`, `2026072703`).
  * Beat N2N baseline gate of $0.9750$ ($p < 0.01$).
  * Single Favorite Architecture: $R = 0.9911$ vs. DualCosine $0.8249$ ($\Delta R +0.166$, SNR $41.3\text{ dB}$).
  * Inference Latency: $\approx 2.56\text{ ms/batch}$ on CUDA for $\approx 37\text{k}$ parameters.

---

## 2. File Inventory

* **LaTeX Root:** [`docs/papers/denoise_opt/v5/main.tex`](docs/papers/denoise_opt/v5/main.tex)
* **Pre-compiled PDF:** [`docs/papers/denoise_opt/v5/main.pdf`](docs/papers/denoise_opt/v5/main.pdf)
* **Modular Subsections:** [`docs/papers/denoise_opt/v5/subsections/`](docs/papers/denoise_opt/v5/subsections/)
* **Figures:** [`docs/papers/denoise_opt/v5/figures/`](docs/papers/denoise_opt/v5/figures/)
* **Detailed Checklist & Evaluation Protocol:**
  * [`docs/papers/denoise_opt/v5/PLAN_PROGRESS.md`](docs/papers/denoise_opt/v5/PLAN_PROGRESS.md)
  * [`docs/papers/denoise_opt/v5/EVAL_PROTOCOL.md`](docs/papers/denoise_opt/v5/EVAL_PROTOCOL.md)
* **Rust Audio Implementation:**
  * Core Wavetable DSP: [`src/wavetable.rs`](src/wavetable.rs)
  * Residual Optimizer: [`src/denoise_opt.rs`](src/denoise_opt.rs)
  * Seam Crackle Handler: [`src/crackle_eam.rs`](src/crackle_eam.rs)

---

## 3. How to Build & Submit

```bash
# Build the PDF
cd docs/papers/denoise_opt/v5
pdflatex main.tex
pdflatex main.tex
```

* **Immediate Submission Target:** Update preprint record on **aiXiv** (`aixiv.260719.000003`) or upload to **arXiv** (`cs.SD` / `eess.AS`).
* **Conference Targets:** *IEEE WASPAA*, *International Conference on Digital Audio Effects (DAFx)*, or *JAES*.

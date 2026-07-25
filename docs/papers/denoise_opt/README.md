# DenoiseOpt paper (local mirror)

Canonical versioned paper: **[reeldemo/denoise-opt-meta](https://github.com/reeldemo/denoise-opt-meta)** → `paper/Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v9/`.

**Application lit survey:** [SIGNAL_HEALING_APPLICATIONS_LIT.md](SIGNAL_HEALING_APPLICATIONS_LIT.md) — where wrap/seam/periodize repair could transfer (wavetable, granular, PSOLA, graphics seams, ECG, …).

## Meta objective (residual)

\[
R=\mathrm{clamp}\!\left(1-\frac{\mathrm{rms}(y_{\mathrm{engine}}-y_{\mathrm{ideal}})}{\max(\mathrm{rms}(y_{\mathrm{ideal}}),\varepsilon)},\,0,\,1\right)
\]

- Ideal: `generate_sound_ideal`, tiled $N{=}16$
- Engine: DenoiseOpt(`generate_sound`), tiled $N{=}16$
- Soft gate: $\mathcal{S}\ge 0.97$ else $\times 0.45$
- Nested inner loss opt on $L=(1-\mathcal{D})+\lambda(1-\mathcal{S})$

## Headline (1500 trials)

| Algorithm | Residual |
|-----------|----------|
| Naive DualCosine | 0.698 |
| Meta Top 1 `evo_explore_515` | **0.824** |

## Reproduce

```bash
cargo run -p reelsynth --release --bin bench_denoise_meta
python brand/artifacts/render_benchmark_matrix.py
# Full paper:
#   cd ../denoise-opt-meta/paper/Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v9 && pdflatex main.tex
```

## v9 (2026-07-25) — current

Language/slop cleanup + transfer-pilot honesty (classical-board CWRU/MIT-BIH only; no deep SOTA overclaim).
See upstream: `denoise-opt-meta/paper/Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v9`.

## v8 (2026-07-24) — archived

Review-response rewrite (W0–W5). Upstream: `denoise-opt-meta/paper/Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v8`.

## v7 (2026-07-19) — archived

Weakness elimination F1–F5 snapshot. Upstream: `denoise-opt-meta/paper/v7`.

## v5 (2026-07-19) — archived

Draft snapshot under local `v5/` if present. Canonical upstream: `denoise-opt-meta/paper/v5`.

## v4 (2026-07-18) — superseded archive

See `v4/` (canonical upstream: `denoise-opt-meta/paper/v4`).

# DenoiseOpt paper (local mirror)

Canonical versioned paper: **[reeldemo/denoise-opt-meta](https://github.com/reeldemo/denoise-opt-meta)** → `paper/v5/`.

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
#   cd ../denoise-opt-meta/paper/v5 && pdflatex main.tex
```

## v5 (2026-07-19) — current

Draft: **Unsupervised Deep Audio Denoising Algorithms via Hybrid Reinforcement Learning and Genetic Algorithm Meta-Learning**.
5k-gate residual tables, classical vs AI bench, OA bibliography.
See `v5/main.tex` (canonical upstream: `denoise-opt-meta/paper/v5`).

## v4 (2026-07-18) — superseded archive

See `v4/` (canonical upstream: `denoise-opt-meta/paper/v4`). Current sources: `v5/`.

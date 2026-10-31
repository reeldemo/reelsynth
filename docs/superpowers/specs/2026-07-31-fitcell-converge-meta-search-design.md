# FitCell converge meta-search (v14 plateau protocol)

Design note referenced from [denoiseopt-paper REPRO.md](https://github.com/reeldemo/denoiseopt-paper/blob/master/REPRO.md) §B. Reconstructed in-repo because the original launch scripts were not committed.

## Outer loop

- **Approach:** `hybrid_lstm` only (GA / PBT / NAS / combo / PPO branches unchanged from `bench_meta_approaches_5k.py`).
- **Budget:** 750 outer iterations (default when `--protocol plateau`).
- **Proposals:** 2 candidate evaluations per outer iteration (`--proposals-per-iter 2`); checkpoint `iters_done` counts outer iterations.
- **Meta plateau adapt:** disabled for this protocol (`plateau_every` effectively off) so outer depth caps do not drift from the paper run.

## Inner FitCell (per candidate)

Train until **relative improvement** stalls or **T_max** steps:

- Stop when `|R_prev − R| / max(|R_prev|, 1e−6) < ε` for `patience` consecutive fit steps.
- Hard cap `T_max` fit steps per candidate (searchable; default CLI fallback `2048` for plateau protocol).

### Searchable hyperparameters (PBT / GA genome)

| Field | Search space (implemented) | Notes |
|-------|---------------------------|--------|
| `fit_max_steps` | {512, 768, 1024, 1536, 2048} | Paper: 512–2048 |
| `fit_patience` | {2, 3, 5, 7, 10} | Paper cites patience; exact set **inferred** |
| `fit_rel_eps` | {1e−5, 3e−5, 1e−4, 3e−4, 1e−3} | Paper cites ε; set **inferred** around legacy 1e−4 |
| `batch` | {64, 80, 96, 112, 128} | Paper ≈96 |
| `lambda_latency` | {0.005, 0.01, 0.02, 0.04, 0.06} | Paper cites λ; set **inferred** around product default 0.02 |

Legacy default protocol keeps fixed inner patience `3`, ε `1e−4`, and `fit_steps` as a fixed cap (no `fit_max_steps`).

## Artifacts

- Training: `brand/artifacts/meta_approach_compare_v14_converge/`
- Checkpoints every 25 outer iters; resume without `--force-fresh`.

## Ambiguities / guesses

1. Exact discrete sets for patience, ε, and λ were not in the public repo; ranges above match REPRO wording and legacy `fit_cell` defaults.
2. Whether the headline seed `1902771841` used the same searchable sets vs fixed runner HPs is unclear; launcher defaults to that seed but phase-2 GPU seeds `2026072701–03` use the same protocol machinery.
3. `--fit-parallel` dual-FitCell threads were not implemented in Python (REPRO: off on GPU); single-stream fit only.
4. Two proposals per iteration: implemented as two hybrid branch draws per outer iter (not batched GPU parallelism).

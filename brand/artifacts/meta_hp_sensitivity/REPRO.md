# Meta HP ±50% sensitivity — reproducibility

- Honesty: **sensitivity probe** (500 iters × OAT ±50%), **not** a full 5k re-search per HP.
- Seed: `1902771841`
- FitCell protocol: sine+cliff, batch `48`, fit_steps `24` (matched to meta compare).
- Output root: `brand/artifacts/meta_hp_sensitivity/`
- **Forbidden:** do not wipe or write `brand/artifacts/meta_approach_compare/`.

## Status

- Configs complete: `11/11`
- All complete: `True`

## Resume

```bash
.venv_gpu/Scripts/python.exe scripts/bench_meta_hp_sensitivity.py
```

Checkpoints live under each `{config_id}/checkpoint.json`. Completed configs are skipped.

## Aggregate / figures only

```bash
.venv_gpu/Scripts/python.exe scripts/bench_meta_hp_sensitivity.py --aggregate-only
```

## Artifacts

| File | Role |
|------|------|
| `results.json` / `meta_hp_sensitivity.json` | Aggregate table |
| `fig_meta_hp_sensitivity.png` / `.pdf` | Bar chart |
| `tab_meta_hp_sensitivity.tex` | Paper table |
| `RESULTS_SNIPPET.md` | Q3 prose for T6 |
| `{config_id}/summary.json` | Per-config champion |

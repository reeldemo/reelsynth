# Signal-heal transfer pilot

Generated: `20260725T043542Z`

## Method under test

- **Ours:** hybrid GA–PPO outer loop (`hybrid_lstm` in `bench_meta_approaches_5k.py`).
- FitCell / SeamCell / arch search reused; period length fixed to `N=256`.
- Metric: DenoiseOpt prolonged residual $R$ (same formula as wavetable).
- Pilot budget: modest outer iters (see `config`); not full industrial overnight.

## Wrap construction

- **CWRU bearings:** DE @12 kHz; per-rev windows via RPM; **ideal** = cubic resample to $L$; **engine** = linear resample (bad-COT proxy) + DenoiseOpt-style wrap cliff + seam noise.
- **MFPT:** same protocol when zip available; fixed shaft-rate periods.
- **MIT-BIH / PTB-XL ECG:** R–R beats → $L$; **ideal** = local mean template + mild endpoint equalize (SBMM-lite classical); **engine** = single beat + wrap cliff.
- **synth_cnc_g01 / synth_pmu_cycle:** synthetic CNC / power-cycle proxies when KIT / DataPort blocked.

## Seeds

- Search / construction seed: `1902771841`
- Holdout sample seed: `20260719`

## Honesty / limits

- Baselines are a **classical board** (+ domain classical COT / SBMM-lite). We do **not** claim BeatDiff / Cycle-GAN / deep order-tracking SOTA unless those weights ran.
- See `DEEP_SOTA_NOT_EXECUTED.json` and `LISTENING_PROTOCOL_MAIN.md` (no invented MOS).
- Do not wipe `brand/artifacts/meta_approach_compare/`.
- Optional KIT CNC / IEEE PMU / Paderborn / BMRB NMR skipped if login/paywall.

### Skipped optional

- **mfpt_bearings:** MFPT zip URL returned HTML (login/wall); skipped
- **kit_cnc:** skipped — KIT CNC DOI needs browser/login flow; synthetic_cnc_wrap used as proxy
- **ieee_pmu:** skipped — IEEE DataPort free-account wall; synthetic_power_wrap used as proxy
- **bmrb_nmr:** skipped — BMRB FID deferred
- **kit_cnc_real:** skipped — KIT CNC DOI needs browser/login; ran synth_cnc_g01 instead
- **ieee_pmu_real:** skipped — IEEE DataPort free-account wall; ran synth_pmu_cycle instead
- **paderborn_kat:** skipped — Paderborn KAt registration / no anonymous OA mirror in this session
- **deep_sota_cyclegan_beatdiff:** not executed — no trained Cycle-GAN / BeatDiff weights under residual protocol

## Results table

See `results_table.json` and `fig_signal_heal_transfer.{png,pdf}`.

## Reproduce

```bash
.venv_gpu/Scripts/python scripts/download_signal_heal_data.py
.venv_gpu/Scripts/python scripts/bench_signal_heal_transfer.py --iters 250 --merge-existing
.venv_gpu/Scripts/python scripts/export_signal_heal_hear_pack.py
```

Also: `REPRO.md`, `DEEP_SOTA_NOT_EXECUTED.json`, `LISTENING_PROTOCOL_MAIN.md`.


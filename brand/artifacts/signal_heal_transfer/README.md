# Signal-heal transfer pilot

Generated: `20260726T074617Z`

## Method under test

- **Ours:** hybrid GA–PPO outer loop (`hybrid_lstm` in `bench_meta_approaches_5k.py`).
- FitCell / SeamCell / arch search reused; period length fixed to `N=256`.
- Metric: DenoiseOpt prolonged residual $R$ (same formula as wavetable).
- Pilot budget: modest outer iters (see `config`); not full industrial overnight.

## Wrap construction

- **CWRU bearings:** DE @12 kHz; per-rev windows via RPM; **ideal** = cubic resample to $L$; **engine** = linear resample (bad-COT proxy) + DenoiseOpt-style wrap cliff + seam noise.
- **MFPT:** same protocol when zip available; fixed shaft-rate periods.
- **Paderborn KAt (K001):** vibration_1 @~64 kHz; speed→angle equal-rev windows when Mech_4kHz tach available; same cubic ideal / linear+cliff engine. Classical + Ours + domain N2N are in paper Table 13 (`tab:transfer-main`); deep Paderborn models remain unwired.
- **MIT-BIH / PTB-XL ECG:** R–R beats → $L$; **ideal** = local mean template + mild endpoint equalize (SBMM-lite classical); **engine** = single beat + wrap cliff.
- **synth_cnc_g01 / synth_pmu_cycle:** synthetic CNC / power-cycle proxies when KIT / DataPort blocked.

## Seeds

- Search / construction seed: `1902771841`
- Holdout sample seed: `20260719`

## Honesty / limits

- Baselines are a **classical board** (+ domain classical COT / SBMM-lite). We do **not** claim BeatDiff / Cycle-GAN / deep order-tracking SOTA unless those weights ran.
- See `DEEP_SOTA_NOT_EXECUTED.json` and `LISTENING_PROTOCOL_MAIN.md` (no invented MOS).
- Do not wipe `brand/artifacts/meta_approach_compare/`.
- Optional KIT CNC / IEEE PMU / BMRB NMR skipped if login/paywall. Paderborn K001 is extracted; deep Paderborn models remain unwired.

### Login-walled downloads (user must open)

- KIT CNC: https://doi.org/10.35097/hvvwn1kfwf7qt48z
- IEEE 39-bus PMU: https://ieee-dataport.org/open-access/pmu-measurements-ieee-39-bus-power-system-model

### Skipped optional

- **kit_cnc_real:** awaiting user drop under raw/kit_cnc/ (kit_cnc_README.txt); synth_cnc_g01 proxy still scored
- **ieee_pmu_real:** S3 URI known but anonymous GET 403; drop IEEE-39-bus_10_generator_PMU.mat into raw/ieee_pmu/; synth_pmu_cycle proxy still scored
- **paderborn_kat:** K001 extracted; classical paderborn_kat board cached — deep SOTA still not executed
- **bmrb_nmr:** skipped — BMRB FID deferred
- **deep_sota_cyclegan_beatdiff:** not executed — no trained Cycle-GAN / BeatDiff weights under residual protocol
- **kit_cnc:** awaiting user drop under raw/kit_cnc/ (kit_cnc_README.txt); synth_cnc_g01 proxy still scored
- **ieee_pmu:** S3 URI known but anonymous GET 403; drop IEEE-39-bus_10_generator_PMU.mat into raw/ieee_pmu/; synth_pmu_cycle proxy still scored

## Results table

See `results_table.json` and `fig_signal_heal_transfer.{png,pdf}`.

## Reproduce

```bash
.venv_gpu/Scripts/python scripts/download_signal_heal_data.py
.venv_gpu/Scripts/python scripts/bench_signal_heal_transfer.py --iters 250 --merge-existing
.venv_gpu/Scripts/python scripts/export_signal_heal_hear_pack.py
```

Also: `REPRO.md`, `DEEP_SOTA_NOT_EXECUTED.json`, `LISTENING_PROTOCOL_MAIN.md`.



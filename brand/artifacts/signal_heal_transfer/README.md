# Signal-heal transfer pilot

Generated: 20260726T083000Z (IEEE PMU S3 probe + KIT drop folder)

## Method under test

- **Ours:** hybrid GA–PPO outer loop (hybrid_lstm).
- Period length fixed to N=256; metric = DenoiseOpt prolonged residual $R$.

## Wrap construction

- **CWRU / MFPT:** existing bearing boards.
- **Paderborn KAt (K001):** extracted with repo-root UnRAR.exe → raw/paderborn/K001/ (**80** .mat, ~666 MB). build_paderborn → paderborn_kat.
- **MIT-BIH / PTB-XL / synth CNC / synth PMU:** unchanged.
- **IEEE 39-bus PMU:** S3 URI `s3://ieee-dataport/open/11968/IEEE-39-bus_10_generator_PMU.mat` identified. Anonymous `--no-sign-request` / public HTTPS still **403** (OA needs free IEEE login). Drop `.mat` into `raw/ieee_pmu/` then run `scripts/fetch_and_probe_ieee_pmu.py`. Content is **phasors** → prefer TVE / window-leakage probe (not musical prolonged-$R$). Optional `build_ieee_pmu_real()` = phasor-synthesized cycles with explicit footnote.
- **KIT CNC:** awaiting manual extract into `raw/kit_cnc/` — see `raw/kit_cnc/kit_cnc_README.txt`. Stub `build_kit_cnc_real()` errors clearly until files appear.

## Honesty / limits

- UnRAR **unblocked**; K001 extract **done**.
- Deep Paderborn / Cycle-GAN / BeatDiff / MOS–MUSHRA **not executed** — see DEEP_SOTA_NOT_EXECUTED.json.
- Paderborn **Ours** smoke (iters=20) ≠ full 250-iter transfer protocol.
- Domain N2N on Paderborn is protocol-matched (4000 steps).
- IEEE PMU open S3 path **documented**; anonymous GET still blocked — manual drop / DataPort login required.
- KIT still **awaiting user drop**.

### Login / drop

- KIT CNC drop: `brand/artifacts/signal_heal_transfer/raw/kit_cnc/` — https://doi.org/10.35097/hvvwn1kfwf7qt48z
- IEEE PMU drop: `brand/artifacts/signal_heal_transfer/raw/ieee_pmu/` — https://ieee-dataport.org/open-access/pmu-measurements-ieee-39-bus-power-system-model

### Table 14 status (abbrev)

| Scope | Status |
|-------|--------|
| Domain-trained N2N | Executed (incl. Paderborn) |
| Cycle-GAN / BeatDiff | Blocked |
| Paderborn KAt deep | Blocked — extract done; deep unwired |
| Full PTB-XL | Blocked (subset) |
| IEEE PMU | S3 URI known; anonymous 403; drop/login; phasors → TVE probe when mat present |
| KIT CNC | Awaiting user drop under raw/kit_cnc/ |
| MOS / MUSHRA | Not collected |

## Paderborn paderborn_kat scores (prolonged $R$ / $R_{\mathrm{blend}}$)

| Method | Score | Note |
|--------|-------|------|
| no_bake | 0.8376 | classical |
| dual_cosine | 0.4710 | classical |
| ours_hybrid_lstm | 0.8932 | **smoke iters=20** |
| n2n_domain_trained $R$ | 0.8387 | 4000 steps |
| n2n_domain_trained $R_{\mathrm{blend}}$ | 0.8421 | 4000 steps |

Full table: results_table.json. Status: cache/paderborn_status.json · cache/ieee_pmu_status.json.

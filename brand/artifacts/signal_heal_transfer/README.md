# Signal-heal transfer pilot

Generated: 20260726T062438Z (Paderborn K001 extract + classical board + smoke Ours + domain N2N)

## Method under test

- **Ours:** hybrid GA–PPO outer loop (hybrid_lstm).
- Period length fixed to N=256; metric = DenoiseOpt prolonged residual $.

## Wrap construction

- **CWRU / MFPT:** existing bearing boards.
- **Paderborn KAt (K001):** extracted with repo-root UnRAR.exe → 
aw/paderborn/K001/ (**80** .mat, ~666 MB). uild_paderborn → paderborn_kat (vibration_1 @~64 kHz; Mech_4kHz speed → equal-angle revs; cubic ideal / linear+cliff engine).
- **MIT-BIH / PTB-XL / synth CNC / synth PMU:** unchanged.

## Honesty / limits

- UnRAR **unblocked**; K001 extract **done**.
- Deep Paderborn / Cycle-GAN / BeatDiff / MOS–MUSHRA **not executed** — see DEEP_SOTA_NOT_EXECUTED.json.
- Paderborn **Ours** row below is a **smoke** (iters=20), not the full 250-iter transfer protocol.
- Domain N2N on Paderborn is protocol-matched (4000 steps).

### Login-walled (user must open)

- KIT CNC: https://doi.org/10.35097/hvvwn1kfwf7qt48z
- IEEE 39-bus PMU: https://ieee-dataport.org/open-access/pmu-measurements-ieee-39-bus-power-system-model

### Table 14 status (abbrev)

| Scope | Status |
|-------|--------|
| Domain-trained N2N | Executed (incl. Paderborn) |
| Cycle-GAN / BeatDiff | Blocked |
| Paderborn KAt deep | Blocked — extract done; deep unwired |
| Full PTB-XL / KIT / PMU / MOS | Blocked as before |

## Paderborn paderborn_kat scores (prolonged $ / {\mathrm{blend}}$)

| Method | Score | Note |
|--------|-------|------|
| no_bake | 0.8376 | classical |
| dual_cosine | 0.4710 | classical |
| ours_hybrid_lstm | 0.8932 | **smoke iters=20** |
| n2n_domain_trained $ | 0.8387 | 4000 steps |
| n2n_domain_trained {\mathrm{blend}}$ | 0.8421 | 4000 steps |

Full table: 
esults_table.json. Status: cache/paderborn_status.json.

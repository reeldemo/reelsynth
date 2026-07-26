# Table 14 (`tab:transfer-sota-status`) — Note draft

Source of truth for blockers: `DEEP_SOTA_NOT_EXECUTED.json`.
Domain-trained N2N merged into `results_table.json` at `20260726T060508Z`.

## Suggested Scope / Note rows

| Scope | Note |
|-------|------|
| Domain-trained Noise2Noise | Scored on seven boards incl. Paderborn K001 (holdout prolonged $R$; Table 13 / `tab:transfer-main`) |
| Cycle-GAN (ECG) | Not run — no adapted Cycle-GAN weights under prolonged-$R$ wrap protocol |
| BeatDiff | Not run — no diffusion checkpoints under residual protocol |
| Paderborn KAt deep | Not run — K001 extracted; classical wrap board available; deep models unwired |
| Full PTB-XL | Subset only — `records500` lead-I, $n{=}256$ (not full corpus) |
| Real KIT CNC / IEEE PMU | Synthetic pilots only — KIT DOI / IEEE DataPort login walls |
| Formal MOS / MUSHRA | Not collected — hear assets / informal A/B only |

## Domain N2N holdout scores (prolonged $R$ / $R_{\mathrm{blend}}$)

| Domain | $R$ | $R_{\mathrm{blend}}$ |
|--------|-----|----------------------|
| cwru_bearings | 0.8686 | 0.8660 |
| mfpt_bearings | 0.8817 | 0.8662 |
| mitbih_ecg | 0.6530 | 0.7495 |
| ptbxl_ecg | 0.5605 | 0.7365 |
| synth_cnc_g01 | 0.4833 | 0.5534 |
| synth_pmu_cycle | 0.9789 | 0.9589 |
| paderborn_kat | 0.8387 | 0.8421 |

Protocol: corrupt→corrupt SeamN2N, 4000 Adam steps, train seed `424242`, holdout seed `20260719`, $n{=}256$ / holdout 64.
Artifacts: `domain_n2n/summary.json`, `results_table.json` → `table[*].n2n_domain_trained`.

Paderborn classical board extract: UnRAR unblocked; deep Paderborn pipelines still not executed.
KIT / IEEE PMU login URLs: `https://doi.org/10.35097/hvvwn1kfwf7qt48z` · `https://ieee-dataport.org/open-access/pmu-measurements-ieee-39-bus-power-system-model`.

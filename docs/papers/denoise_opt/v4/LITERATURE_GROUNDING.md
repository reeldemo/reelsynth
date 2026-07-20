# Literature grounding notes

Generated from OA PDFs under `artifacts/literature_oa/pdfs/` (catalog: `artifacts/literature_oa/oa_catalog.json`).
Policy: **OA-only**. Do not invent citations. Do not cite paywalled-only works.

## Used (method / design anchors)

| cite_key | Paper | Relevance |
|----------|-------|-----------|
| stilson1996 | Alias-free classic analog waveforms | Wrap / BLIT discontinuity |
| esqueda2016blamp | BLAMP | Corner / derivative discontinuity |
| nam2009polyblep | PolyBLEP / frac delay | Practical antialiasing oscillators |
| lehtinen2018 | Noise2Noise | Label-free restoration philosophy |
| kashyap2021n2n | Speech Noise2Noise | Audio unsupervised denoise |
| wisdom2020 | MixIT | Unsupervised separation ranking |
| engel2020 | DDSP | Modular/differentiable DSP context |
| jaderberg2017 | PBT | Exploit–mutate population schedules |
| real2017large / real2019 | Evolution NAS | GA/evolutionary NAS prior |
| khadka2018 | ERL | Interleave GA population with RL |
| schulman2017 | PPO | Clipped surrogate for discrete arch mutations |
| zoph2017nas / pham2018 | RL-NAS / ENAS | RL controller ancestors |
| elsken2019 | NAS survey | Search space / strategy / evaluation taxonomy |
| shazeer2017 | MoE | Soft gates over heterogeneous blocks |
| stoller2018 / macartney2018 / luo2019 / luo2020dprnn | Waveform cells | Tiny U-Net / TCN / dual-path priors |
| snoek2012 / hansen2016cmaes | BO / CMA-ES | Local Bayesian and continuous evolutionary priors |

## Screened (contrast only)

| cite_key | Why screened |
|----------|----------------|
| finn2017 | MAML: no task-gradient through a deep net |
| liu2019 | DARTS: continuous relaxation. We keep discrete ops |
| dfossez2019 / dfossez2020 / dfossez2021 | Demucs / realtime SE: too heavy per trial |
| kong2021 | DiffWave: generative sampling stack, not seam bake |
| pascual2017 | SEGAN: full GAN loops screened for cost |

## Dropped non-OA (replaced)
- `valimaki2006va` (CMJ paywall) → stilson1996, nam2009polyblep, esqueda2016blamp
- `esqueda2016aliasing` (IEEE TSP paywall) → esqueda2016blamp, nam2009polyblep, stilson1996
- `valimaki2010dpw` (IEEE TASLP author-hosted; not strict OA) → stilson1996, nam2009polyblep, esqueda2016blamp
- `zhang2007` (IEEE TEC paywall) → hansen2016cmaes, snoek2012

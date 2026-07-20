# Paper plan: DenoiseOpt Overnight: Residual-Scored RL/NAS for Wavetable Seam Bake Operators

_Source: **llm** · created 2026-07-18T11:04:47.512627+00:00_

## User needs
DenoiseOpt paper v5 (CS/DSP). Follow PAPER_WRITING_QUALITY.md. Excellent Introduction: wavetable wrap crackle phenomenon, prolonged residual vs seam-local proxies, landscape of VA/BLEP, unsupervised restoration, HPO/PBT, NAS/RL. Related Work thematic with used vs screened; ground paraphrases on PDF analyses. Discussion will compare final 1M champ R vs DualCosine and RL/NAS/combo branches — NUMERIC SLOTS ONLY until final ingest. Author Julian M. Kleber ORCID 0000-0001-5518-0932. Repos reelsynth + denoise-opt-meta. Honest numbers only.

## Abstract sketch
Open wrap seams inject audible crackle under cyclic wavetable playback. We run a dense 1,000,000-step CUDA overnight search over RL, NAS, and literature-combo branches, scoring candidates by prolonged residual R in [0,1] (1=best) against an ideal tiled reference, and compare to a DualCosine baseline. Honest numbers only after 1M completion.

## Writing mode hint
Write full academic paragraphs (IMRaD / CS–DSP). Prefer precise claims tied to ingested metrics and PDF analyses. Cite with \cite{cite_key} only for attached refs. Distinguish used vs screened literature. Do not invent numbers or references. Mark template-mode output if no LLM is used.

## Literature count: 90

## Open data gaps
- `baseline_comparison`
- `baselines`
- `hyperparameters`
- `method_definition`
- `metrics`
- `primary_metric`
- `table_rows`
- `trial_budget`

## Sections

### `abstract` — Abstract
- **IMRaD role:** abstract
- **Target words:** ~180
- **Goal:** Concise problem–method–result–limitation summary with the key numeric headline (primary metric and baseline comparison).
- **Claims:**
  - State the scientific problem in one sentence.
  - Name the method and evaluation protocol.
  - Report the primary quantitative finding vs baseline.
- **Citation slots:** elsken2019nas, engel2020ddsp, jaderberg2017pbt
- **Data still needed:** `primary_metric`, `baseline_comparison`

### `introduction` — Introduction
- **IMRaD role:** introduction
- **Target words:** ~450
- **Goal:** Motivate the problem, situate it in prior practice, state contributions as falsifiable claims, and preview the paper.
- **Claims:**
  - Why the phenomenon matters scientifically or practically.
  - Gap in existing proxies / methods.
  - Explicit numbered contributions.
- **Citation slots:** elsken2019nas, engel2020ddsp, jaderberg2017pbt, lehtinen2018n2n

### `related_work` — Related Work
- **IMRaD role:** related_work
- **Target words:** ~500
- **Goal:** Organize prior art by theme; distinguish used vs screened literature; end with how this work differs.
- **Claims:**
  - Group citations thematically (not as a dump).
  - State what was screened out and why.
- **Citation slots:** elsken2019nas, engel2020ddsp, jaderberg2017pbt, lehtinen2018n2n, finn2017maml

### `methods` — Methods
- **IMRaD role:** methods
- **Target words:** ~650
- **Goal:** Define notation, model/operator, objective(s), and search procedure so a reader could reimplement the essentials.
- **Claims:**
  - Formal definition of the primary score / loss.
  - Algorithmic procedure (outer/inner loops if any).
  - Implementation constraints that affect validity.
- **Citation slots:** elsken2019nas, engel2020ddsp, jaderberg2017pbt
- **Data still needed:** `method_definition`, `hyperparameters`

### `experiments` — Experiments
- **IMRaD role:** experiments
- **Target words:** ~350
- **Goal:** Describe protocol: datasets/seeds, trial budget, validation sizes, baselines, and selection rules before revealing outcomes.
- **Claims:**
  - Protocol is fixed before reporting winners.
  - Baselines and ablations are named.
- **Data still needed:** `trial_budget`, `baselines`

### `results` — Results
- **IMRaD role:** results
- **Target words:** ~450
- **Goal:** Present primary metrics with table(s) and figure(s); report effect sizes vs baselines; avoid overclaiming.
- **Claims:**
  - Primary metric ranking with uncertainty or held-out N.
  - Secondary metrics that check for failure modes.
- **Figures:** primary_comparison
- **Tables:** benchmark_matrix
- **Data still needed:** `metrics`, `table_rows`

### `discussion` — Discussion
- **IMRaD role:** discussion
- **Target words:** ~400
- **Goal:** Interpret results relative to hypotheses; reconcile surprising outcomes (e.g. which prior won); connect to related work.
- **Claims:**
  - What the evidence supports.
  - What it does not support.
- **Citation slots:** elsken2019nas, engel2020ddsp, jaderberg2017pbt, lehtinen2018n2n

### `limitations` — Limitations
- **IMRaD role:** limitations
- **Target words:** ~220
- **Goal:** State validity threats, metric proxies, and compute budgets honestly.
- **Claims:**
  - At least three concrete limitations.

### `conclusion` — Conclusion
- **IMRaD role:** conclusion
- **Target words:** ~180
- **Goal:** Restate contributions and headline numbers; point to artifacts.
- **Claims:**
  - One-paragraph takeaway with primary metric.

# HP ±50% sensitivity — Results snippet (Q3)

> Honesty: **sensitivity probe** at 500 outer iterations (OAT ±50%), seed `1902771841`.
> This does **not** re-prove the 5k meta-approach ranking.

- Protocol: sine+cliff FitCell, batch 48, fit_steps 24, hybrid GA–PPO loop.
- Configs complete: 11/11.
- Default champ $R$: `0.98882`.
- Largest |ΔR| vs default among completed OAT arms: `0.01228` (most negative: `lr_m50` ΔR=`-0.01228`; most positive: `entropy_p50` ΔR=`+0.00234`).
- Interpretation for Q3: Table-2 defaults are **locally robust** under ±50% one-at-a-time shifts at this 500-iter budget when |ΔR| stays small relative to the DualCosine gap; remaining gaps are sensitivity, not a claim that the defaults are globally optimal.

## Paper pointer

- Figure: `fig_meta_hp_sensitivity.png` / `.pdf`
- Table: `tab_meta_hp_sensitivity.tex` (`tab:hp-sensitivity`)
- Aggregate JSON: `meta_hp_sensitivity.json`

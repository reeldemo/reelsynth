# Table 14 (`tab:transfer-sota-status`) — Note draft (no invented scores)

Source of truth for blockers: `DEEP_SOTA_NOT_EXECUTED.json`.
Prefer leaving `results_transfer.tex` untouched until domain-trained N2N merges into `results_table.json`.

## Suggested Scope / Note rows (wording only)

| Scope | Note (draft) |
|-------|----------------|
| Domain-trained Noise2Noise | In progress — train+holdout prolonged $R$ via `scripts/train_n2n_transfer_domains.py`; cite numbers only after `domain_n2n/summary.json` + `results_table.json` merge |
| Cycle-GAN (ECG) | Not run — no adapted Cycle-GAN weights under prolonged-$R$ wrap protocol |
| BeatDiff | Not run — no diffusion checkpoints under residual protocol |
| Paderborn KAt deep | Not run — `K001.rar` present but CLI UnRAR blocked; deep models unwired |
| Full PTB-XL | Subset only — `records500` lead-I, $n{=}256$ (not full corpus) |
| Real KIT CNC / IEEE PMU | Synthetic pilots only — KIT DOI / IEEE DataPort login walls |
| Formal MOS / MUSHRA | Not collected — hear assets / informal A/B only |

## Optional LaTeX fragment (do not paste until N2N lands)

```latex
% Draft only — update Domain-trained Noise2Noise row after merge.
Domain-trained Noise2Noise & In progress (holdout $R$; merge pending) \\
Cycle-GAN (ECG) & Not run (no adapted weights under $R$) \\
BeatDiff & Not run (no diffusion checkpoints) \\
Paderborn KAt deep models & Not run ($K001$.rar UnRAR blocked; deep unwired) \\
Full PTB-XL & Subset only (\texttt{records500} lead-I, $n{=}256$) \\
Real KIT CNC / IEEE PMU & Synthetic pilots only (download walls) \\
Formal MOS / MUSHRA & Not collected \\
```

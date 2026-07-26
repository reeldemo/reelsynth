# Table 14 (	ab:transfer-sota-status) — Note draft

Source of truth: DEEP_SOTA_NOT_EXECUTED.json + 
esults_table.json.

| Scope | Note |
|-------|------|
| Domain-trained Noise2Noise | Scored on seven boards (Paderborn N2N R=0.8387; expanded PTB-XL N2N R=0.5702) |
| Cycle-GAN (ECG) | OOD wrap-R scored (MIT-BIH 0.0700; PTB-XL 0.1480) — clinical restore ≠ wrap-R |
| BeatDiff | Not run — HF `lbedin/BeatDiff` 401 without auth; Drive `beatdiff_prior` missing `.hydra/config.yaml` (Orbax shards unusable) |
| Paderborn KAt deep | Deep unwired; classical + full Ours R=0.9270 + N2N |
| Expanded PTB-XL | records500 lead-I expanded (≈2137 records / ≈23094 beat pool); board n=256 |
| Real KIT CNC / IEEE PMU | Synthetic pilots only (download walls) |
| Formal MOS / MUSHRA | Not collected |

## Deltas vs prior smoke

| Item | Before | After |
|------|--------|-------|
| Paderborn Ours | 0.8932 (iters=20 smoke) | **0.9270** (iters=250) |
| PTB-XL N2N | 0.5605 | **0.5702** (expanded board) |
| PTB-XL records | ~94–200 | **≈2137** *_hr in pool build |
| Cycle-GAN | not run | MIT-BIH R=0.0700; PTB-XL R=0.1480 |
| BeatDiff | not run | still blocked (HF 401; Drive prior incomplete) |

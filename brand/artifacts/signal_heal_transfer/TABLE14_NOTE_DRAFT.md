# Table 14 (tab:transfer-sota-status) — Note draft

Source of truth: DEEP_SOTA_NOT_EXECUTED.json + deep_sota_adapters/

| Scope | Status |
|-------|--------|
| SeamN2N | Executed (ours, N2N-style) |
| Cycle-GAN (author) | OOD wrap-R scored |
| BeatDiff (Bedin) | OOD wrap-R scored — Drive Orbax prior (not HF) |
| Paderborn Al Firdausi | Native classifier + wrap bake |

## BeatDiff download (no HF)

```text
# Folder (README): https://drive.google.com/drive/folders/1m2OvyYebvnirh1CraCrnSOyjihSkSkLG
# Prior subfolder: 1QN6mZXnBpYJFxwUNYV5PXbkhd4HYw3Xh
# Scripts: scripts/download_beatdiff_curl.py , scripts/score_beatdiff_wrap_r.py
# Scores: MIT-BIH R=0.3693; PTB-XL R=0.3259 (one-step σ=0.5; clinical ≠ wrap-R)
```

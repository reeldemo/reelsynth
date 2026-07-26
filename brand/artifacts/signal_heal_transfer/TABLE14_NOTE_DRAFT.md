# Table 14 (tab:transfer-sota-status) — Note draft

Source of truth: DEEP_SOTA_NOT_EXECUTED.json + deep_sota_adapters/

| Scope | Status |
|-------|--------|
| Domain-trained Noise2Noise (SeamN2N) | Scored on seven boards (Paderborn N2N R=0.8387; expanded PTB-XL N2N R=0.5702). Our SeamN2N, not Lehtinen code. |
| Cycle-GAN (ECG) author | OOD wrap-R scored (MIT-BIH R=0.0700; PTB-XL R=0.1555) — clinical restore ≠ wrap-R |
| BeatDiff (Bedin) | Not run — HF huggingface-cli login + download OR browser Drive folder required |
| Paderborn KAt deep (Al Firdausi CNN) | Author weights loaded; K001 healthy→Normal rate=1.0 (n=256); wrap-R refused (classifier ≠ denoise). Wrap board: Ours 0.9270 / SeamN2N 0.8387 / DualCosine 0.4710 |

## BeatDiff user action
`
huggingface-cli login
huggingface-cli download lbedin/BeatDiff --local-dir brand/artifacts/signal_heal_transfer/external/weights/beatdiff_hf
`
Or browser: https://drive.google.com/drive/folders/1m2OvyYebvnirh1CraCrnSOyjihSkSkLG

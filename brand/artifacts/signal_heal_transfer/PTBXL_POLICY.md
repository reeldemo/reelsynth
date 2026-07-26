# PTB-XL wrap-board policy (explicit)

Source of truth: `scripts/signal_heal/datasets.py` → `PTBXL_POLICY` / `build_ptbxl`.

## Download

Prefer AWS Open Data (unsigned):

```text
python scripts/sync_ptbxl_records500.py --flatten
```

This syncs `s3://physionet-open/ptb-xl/1.0.3/records500/` (~2–3 GB) into
`brand/artifacts/signal_heal_transfer/raw/ptbxl_aws/records500/` and flattens
`*_hr.{dat,hea}` into `raw/ptbxl/`.

HTTP fallback (when AWS is unavailable): `scripts/download_signal_heal_data.py`
with `PTBXL_HR_N=5000` (records `00001_hr` … `05000_hr`).

## Board construction

| Field | Value |
|-------|-------|
| Corpus | PhysioNet PTB-XL 1.0.3 |
| Preferred rate | records500 (`*_hr`, 500 Hz) |
| Lead | index 0 = lead I only |
| Period length | L = 256 |
| Board size | n = 256 periods (protocol-matched to other transfer boards) |
| Pool | **all** available downloaded `*_hr` records (no early stop at n×4) |
| Beat filter | peak thr 0.55·max; min peak distance 0.28 s; RR ∈ [0.22, 1.8] s |
| Ideal | local mean template (±8 neighbors) + mild endpoint equalize (w=8) |
| Engine | single beat + DenoiseOpt wrap cliff |
| Seed | 1902771841 (+23 for beat sample) |

## Honesty

- This is a **wrap residual R** board, not a clinical multi-lead diagnosis restore.
- Cycle-GAN / BeatDiff clinical metrics ≠ wrap-R (see `DEEP_SOTA_NOT_EXECUTED.json`).

KIT multimodal CNC — drop folder
================================

DOI: https://doi.org/10.35097/hvvwn1kfwf7qt48z
RADAR4KIT: https://radar.kit.edu/radar/en/dataset/hvvwn1kfwf7qt48z
License: CC BY 4.0 (Ströbel et al., 2025 / Data in Brief)

What to do
----------
1. Download the archive via the DOI / RADAR4KIT UI (browser login as required).
2. Extract **here** (this folder):
   brand/artifacts/signal_heal_transfer/raw/kit_cnc/

Keep the published tree layout if possible (raw_data / descriptive / …).
Flat dumps are OK as long as sensor `.mat` files are reachable by recursive search.

Expected filenames / formats (from Data in Brief / RADAR docs)
-------------------------------------------------------------
- Controller (SINUMERIK Edge, ~500 Hz): `*.json` (HFData / axis positions, torque, current)
  — CSV exports may also appear.
- External sensors (force + acceleration, ~10 kHz): MATLAB `*.mat` under each trial’s
  `raw_data/` (or equivalent).
- Per-trial **synced** MATLAB timetable: one `*.mat` with aligned controller + sensor streams.
- NC programs: `*.nc` (G-code) under part / descriptive folders.
- CAD: `*.stp` / `*.STEP` (geometry; optional for wrap board).
- Metadata: DoE / tool lists under `descriptive/` (JSON/CSV/PDF).

Rough scale: ~33 milling trials, ~6 h process data, three component types, anomaly cases.

How the builder uses this
-------------------------
`scripts/signal_heal/datasets.py` → `build_kit_cnc_real()`:

- Recursively looks for `*.mat` / `*.json` / `*.nc`.
- Prefer long 1-D numeric channels in synced sensor `.mat` (accel/force proxy).
- Builds L=256 periods: ideal = z-scored segment; engine = + DenoiseOpt wrap cliff.
- If this folder is empty, the builder raises a clear FileNotFoundError and the transfer
  pilot keeps scoring **synth_cnc_g01** as the CNC proxy.

Do not commit bulky raw extracts — `brand/artifacts/signal_heal_transfer/raw/` is gitignored.

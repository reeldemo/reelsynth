# Meta-compare hear presets

Playable **ReelSynth** presets for the wrap-seam heal tiles (holdout seed **20260719**),
from the **Ours (hybrid GA–PPO)** / `hybrid_lstm` champion in
`brand/artifacts/meta_approach_compare/`.

Each preset is a real Design patch (filter, ADSR, mod matrix, chorus + delay) pointing at a
sidecar `.reelwt` bank built from the healed / cracked / DualCosine period.

## Open in reelsynth-app

1. Build & launch:
   ```bash
   cargo run -p reelsynth-app --bin reelsynth-app
   ```
2. **File → Open Preset…** (or the app’s Open Preset control) and choose a
   `*.reelpreset` in this folder — keep the matching `*.reelwt` **next to it**
   (same directory; same stem).
3. Play notes from the keyboard / MIDI. Compare:
   - `*_ours.*` — healed FitCell (hybrid_lstm champion)
   - `*_nobake.*` — cracked engine wrap (A/B)
   - `*_dualcosine.*` — DualCosine classical bake (A/B)

Bank geometry: **16 frames × 2048 samples** (cycle upsampled from L=256).
Preset Design: **factory_wt_lead**-style WT lead + **Factory Lead** FX (chorus/delay on, reverb bypassed).

## Samples

| # | Tile | Ours | Nobake | DualCosine |
|---|------|------|--------|------------|
| 1 | 46 (paper heal) | `01_tile46_ours.reelpreset` + `01_tile46_ours.reelwt` | `01_tile46_nobake.reelpreset` + `01_tile46_nobake.reelwt` | `01_tile46_dualcosine.reelpreset` + `01_tile46_dualcosine.reelwt` |
| 2 | 21 | `02_tile21_ours.reelpreset` + `02_tile21_ours.reelwt` | `02_tile21_nobake.reelpreset` + `02_tile21_nobake.reelwt` | `02_tile21_dualcosine.reelpreset` + `02_tile21_dualcosine.reelwt` |
| 3 | 4 | `03_tile4_ours.reelpreset` + `03_tile4_ours.reelwt` | `03_tile4_nobake.reelpreset` + `03_tile4_nobake.reelwt` | `03_tile4_dualcosine.reelpreset` + `03_tile4_dualcosine.reelwt` |
| 4 | 49 | `04_tile49_ours.reelpreset` + `04_tile49_ours.reelwt` | `04_tile49_nobake.reelpreset` + `04_tile49_nobake.reelwt` | `04_tile49_dualcosine.reelpreset` + `04_tile49_dualcosine.reelwt` |
| 5 | 48 | `05_tile48_ours.reelpreset` + `05_tile48_ours.reelwt` | `05_tile48_nobake.reelpreset` + `05_tile48_nobake.reelwt` | `05_tile48_dualcosine.reelpreset` + `05_tile48_dualcosine.reelwt` |

See `manifest.json` for absolute R scores and wrap magnitudes.

Absolute paths (this machine at generation time) are listed in `manifest.json` → `absolute_paths`.

Rebuild:

```bash
.venv_gpu/Scripts/python.exe scripts/export_meta_hear_presets.py --approach hybrid_lstm
```

Skip engine load check (faster):

```bash
.venv_gpu/Scripts/python.exe scripts/export_meta_hear_presets.py --skip-rust-verify
```

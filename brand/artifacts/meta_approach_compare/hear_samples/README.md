# Meta-compare hear samples

Audible demos of wrap-seam heal using the **Ours (hybrid GA–PPO)** / `hybrid_lstm` champion
from `brand/artifacts/meta_approach_compare/`.

## Playback

- Sample rate: **44100 Hz**, mono PCM16
- Pitch: **440.0 Hz (A4)** via linear wavetable interpolation of each 256-sample cycle
- Duration: **3.0 s** per clip
- Holdout seed: **20260719** (paper heal figure); search/refit seed: **1902771841**

Open the `*_nobake.wav` vs `*_dualcosine.wav` vs `*_ours_healed.wav` files in any audio player.
Cracked (nobake) clips should click/buzz at the wrap; healed Ours should sound smoother.

## Samples

| # | Tile | Files |
|---|------|-------|
| 1 | 46 (paper heal tile) | `01_tile46_nobake.wav, 01_tile46_dualcosine.wav, 01_tile46_ours_healed.wav` |
| 2 | 21 | `02_tile21_nobake.wav, 02_tile21_dualcosine.wav, 02_tile21_ours_healed.wav` |
| 3 | 4 | `03_tile4_nobake.wav, 03_tile4_dualcosine.wav, 03_tile4_ours_healed.wav` |
| 4 | 49 | `04_tile49_nobake.wav, 04_tile49_dualcosine.wav, 04_tile49_ours_healed.wav` |
| 5 | 48 | `05_tile48_nobake.wav, 05_tile48_dualcosine.wav, 05_tile48_ours_healed.wav` |

See `manifest.json` for absolute R scores and wrap magnitudes.

Rebuild:

```bash
.venv_gpu/Scripts/python.exe scripts/export_meta_hear_samples.py --approach hybrid_lstm
```

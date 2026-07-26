# Vibrato / dynamic-pitch spectrogram

Slow vibrato playback of cracked / DualCosine / Ours on holdout tile **46**.

- Sample rate: **44100 Hz**
- Base pitch: **220.0 Hz**
- Vibrato: **5.0 Hz** rate, **+/-3.0%** depth
- Duration: **2.0 s**
- Seeds: eval **20260719**, search/refit **1902771841**

Figures: `fig_vibrato_spectrogram.{png,pdf}` (mirrored to paper v11/v10 `figures/`).

Rebuild:

```bash
.venv_gpu/Scripts/python.exe scripts/bench_vibrato_spectrogram.py --approach hybrid_lstm
```

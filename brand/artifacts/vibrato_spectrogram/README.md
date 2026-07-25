# Vibrato / dynamic-pitch spectrogram (paper v8 W2)

Slow vibrato playback of cracked / DualCosine / Ours on holdout tile **46**.

- Sample rate: **44100 Hz**
- Base pitch: **220.0 Hz**
- Vibrato: **5.0 Hz** rate, **±3.0%** depth
- Duration: **2.0 s**
- Seeds: eval **20260719**, search/refit **1902771841**

Figures: `fig_vibrato_spectrogram.{png,pdf}` (mirrored to `paper/Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v8/figures/`).

Rebuild:

```bash
.venv_gpu/Scripts/python.exe scripts/bench_vibrato_spectrogram.py --approach hybrid_lstm
```

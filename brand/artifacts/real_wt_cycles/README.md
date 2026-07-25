# ReelSynth-exported WT cycles (Factory + FX)

- Source: true factory bank frames via `export_reelsynth_wt_cycles`.
- Banks: saw_morph, square_morph, sine, formant, metallic.
- Morphs per bank: 128 (dense).
- FX: Factory Lead–style `FxChain` (chorus + delay on, reverb bypassed) offline;
mid-buffer L=256 period extracted after 32 tiled cycles.
- Export geometry: source frame_size → linear resample → L=256, peak-normalized.
- Count: 1280 periods (640 dry + 640 FX).
- Not procedural Python stand-ins; not LibriSpeech/MUSDB.

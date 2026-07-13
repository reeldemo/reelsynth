# ReelSynth SDK and API reference

Integration surfaces for embedding ReelSynth in applications, scripts, and future plugin hosts.

## Quick start by language

| Surface | Build | Best for |
|---------|-------|----------|
| **Rust crate** | `cargo build` | Native apps, tests, custom hosts |
| **Python (PyO3)** | `maturin develop --features python` | Offline render, batch export |
| **CLI** | `cargo run --bin reelsynth-export` | Shell scripts, CI |
| **C FFI** | `cdylib` feature | Future VST/CLAP bridges |

Generate Rust docs locally:

```bash
cargo doc --no-deps --open
```

---

## Python API (PyO3)

Build:

```bash
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin develop --features python
```

Module: `reelsynth` (alias `grok_dsp` for migration)

### Constants

| Name | Value | Meaning |
|------|-------|---------|
| `DEFAULT_NUM_FRAMES` | 256 | Factory bank frame count |
| `DEFAULT_FRAME_SIZE` | 2048 | Samples per frame |

### `render_note_py`

Render one note offline to a numpy float32 array.

```python
import reelsynth

audio = reelsynth.render_note_py(
    bank_path="data/wavetables/saw_morph.reelwt",
    freq=440.0,
    duration=1.0,
    patch_json='{"oscillators":[{"level":1.0,"position":0.0}],"filter":{"cutoff":1200}}',
    sample_rate=44100,
)
```

### `import_wavetable`

Import foreign format → `.reelwt`.

```python
reelsynth.import_wavetable("vital", "table.vitaltable", "out/table.reelwt")
reelsynth.import_wavetable("wav", "/path/to/cycles/", "out/table.reelwt")
reelsynth.import_wavetable("serum", "patch.fxp", "out/table.reelwt")
```

### `bank_info`

```python
frames, frame_size = reelsynth.bank_info("table.reelwt")
```

### `write_factory_wavetables`

Write built-in banks to a directory.

```python
paths = reelsynth.write_factory_wavetables("data/wavetables/")
# saw_morph, square_morph, sine, formant, metallic
```

### Export functions

Return JSON `ExportReport` dict.

```python
reelsynth.export_wavetable_py("vital", "table.reelwt", "table.vitaltable")
reelsynth.export_preset_py("ableton", "patch.reelpreset", "wavetable_map.json")
reelsynth.export_reelpack_py(
    "patch.reelpreset",
    "out/",
    '["vital","wav","midi","audio"]',  # optional JSON list
)
```

### `modulated_one_pole_lowpass`

Utility DSP helper — lowpass with per-sample cutoff modulation.

---

## CLI (`reelsynth-export`)

```bash
cargo run --bin reelsynth-export -- <target> <input> -o <output> [options]
```

### Targets

| Target | Input | Output |
|--------|-------|--------|
| `vital` | `.reelwt` | `.vitaltable` |
| `wav` | `.reelwt` | folder of frame WAVs |
| `serum` | `.reelpreset` + bank | `.fxp` (WT subset) |
| `ableton` | `.reelpreset` | `wavetable_map.json` |
| `sfz` | `.reelpreset` + bank | `.sfz` + sample WAV |
| `midi` | `.reelpreset` | `.mid` (single note) |
| `audio` | `.reelpreset` + bank | 24-bit WAV stem |
| `reelpack` | `.reelpreset` | full bundle |

### Options

| Flag | Meaning |
|------|---------|
| `-o`, `--output` | Output path or directory |
| `--targets` | Comma list for `reelpack` |
| `--name` | Table name for exports (default `reelsynth`) |

### Examples

```bash
cargo run --bin reelsynth-export -- vital table.reelwt -o table.vitaltable
cargo run --bin reelsynth-export -- reelpack patch.reelpreset -o out/ \
  --targets vital,wav,serum,ableton,sfz,midi,audio
```

### Default `ExportOptions`

| Field | Default | Notes |
|-------|---------|-------|
| `freq` | 440.0 Hz | Offline render pitch |
| `duration` | 2.0 s | Stem length |
| `midi_note` | 69 (A4) | Demo MIDI export |
| `sample_rate` | 44100 | |
| `table_name` | `reelsynth` | Vital/Serum naming |

These defaults apply to `midi` and `audio` exports — not live performance capture.

---

## C FFI

Header-less C ABI in `src/ffi/mod.rs`. Link `libreelsynth` as `cdylib`.

| Function | Signature | Description |
|----------|-----------|-------------|
| `reelsynth_create` | `(bank_path, sample_rate) → *mut Handle` | Load bank, create engine |
| `reelsynth_process` | `(handle, out, frames)` | Render mono samples |
| `reelsynth_note_on` | `(handle, note, velocity)` | Note on ch 0 |
| `reelsynth_note_off` | `(handle, note)` | Note off ch 0 |
| `reelsynth_destroy` | `(handle)` | Free instance |

**Status:** Minimal stub for future plugin hosts. No preset load, stereo, or MIDI event stream yet.

---

## Rust crate — public API

Crate root re-exports (`src/lib.rs`):

```rust
pub use wavetable::WavetableBank;
pub use patch::{Patch, Envelope, Macro, ModSlot};
pub use voice::{render_note, render_note_single_bank};
pub use engine::{SynthEngine, MidiEvent, VoiceMpe, BankSet};
pub use export::{ExportOptions, ExportTarget, ExportReport, export_reelpack, ...};
pub use scope::{ScopeMonitor, ScopePreviews, ...};
pub use fx::{FxChain, EffectSlot, EffectType, FxBypass};
```

### Module: `wavetable`

**`WavetableBank`**

| Method | Description |
|--------|-------------|
| `new(num_frames, frame_size)` | Empty bank |
| `from_flat(...)` | From flat f32 slice |
| `read_file` / `write_file` | `.reelwt` I/O |
| `frame` / `frame_mut` | Access frame slice |
| `sample(position, phase)` | Interpolated lookup |
| `sample_warped(...)` | Warped phase sampling |
| `set_frame_from_cycle` | Write single cycle into frame |
| `factory_saw_morph` etc. | Built-in banks |

Constants: `DEFAULT_NUM_FRAMES` (256), `DEFAULT_FRAME_SIZE` (2048), `REELWT_MAGIC`, `REELWT_VERSION`.

### Module: `patch`

**`Patch`** — schema `reelsynth-preset-v2`

| Method | Description |
|--------|-------------|
| `from_json` / `to_json` | Parse/serialize with v1 migration |
| `default_mono` | Single-osc default patch |

**Structs:** `Oscillator`, `Filter`, `Envelope`, `Lfo`, `Macro`, `ModSlot`

### Module: `engine`

**`SynthEngine`** — realtime voice pool + FX

| Method | Description |
|--------|-------------|
| `new(bank, patch, sample_rate)` | Construct |
| `process` / `process_stereo` | Render audio block |
| `handle_event(MidiEvent)` | Route MIDI (+ MPE) |
| `note_on` / `note_off` | Direct note API |
| `load_preset(bank, patch)` | Hot-swap |
| `set_*` | Param setters (filter, osc, LFO, mod matrix, FX, …) |
| `render_offline(freq, duration)` | Single-note offline buffer |
| `scope_monitor()` | Live scope taps |

**`MidiEvent`** — `NoteOn`, `NoteOff`, `PitchBend`, `ChannelPressure`, `PolyAftertouch`, `ControlChange`

**`MpeState`**, **`VoiceMpe`**, **`BankSet`** — multi-timbral / MPE support

Constants: `BLOCK_SIZE` = 64

### Module: `voice`

| Function | Description |
|----------|-------------|
| `render_note(banks, freq, duration, sr, patch)` | Multi-bank offline |
| `render_note_single_bank(bank, ...)` | Single bank offline |

### Module: `export`

| Type / fn | Description |
|-----------|-------------|
| `ExportTarget` | `Vital`, `Wav`, `Serum`, `Ableton`, `Sfz`, `Midi`, `Audio`, `Reelpack` |
| `ExportOptions` | Render/export parameters |
| `ExportReport` | Success, paths, dropped params, errors |
| `export_wavetable` | Single-target WT export |
| `export_preset` | Patch + bank → target |
| `export_reelpack` | Full bundle |
| `load_preset` | Read `.reelpreset` |
| `resolve_bank_for_preset` | Find sibling `.reelwt` |
| `parse_targets` | Parse comma target list |

Submodules: `export_vital`, `export_serum_wt`, `export_ableton_map`, `export_sfz`, `export_midi`, `export_audio_wav`.

### Module: `import`

| Function | Description |
|----------|-------------|
| `import_to_reelwt(source, path, out)` | `vital` / `wav` / `serum` → `.reelwt` |
| `import_vital` | Vital table parser |
| `import_wav_folder` | Sorted cycles |
| `import_serum_fxp` | Serum WT scan |

### Module: `fx`

**`EffectType`**, **`EffectSlot`**, **`FxChain`**, **`FxBypass`**, **`default_effects`**, **`effects_from_bypass`**

### Module: `scope`

**`ScopeMonitor`**, **`ScopeRingBuffer`**, **`ScopePreviews`**, **`render_scope_previews`**, **`spectrum_magnitudes`**

Constants: `SCOPE_RING_LEN`, `SCOPE_DISPLAY_LEN`, `PREVIEW_ROOT_NOTE`, `PREVIEW_FIFTH_NOTE`

### Module: `modulation`

**`ModSources`**, **`compute_mods`**, **`compute_macro_mods`**, **`merge_mods`**

### Module: `lfo`

**`LfoShape`**, **`LfoRuntime`**, **`lfo_wave_unit`**, **`lfo_value`**

### Module: `fm`

**`FmSource`**, **`fm_mod_signal`**, **`sample_carrier_with_fm`**, …

### Module: `osc`

**`WtWarpMode`**, **`warp_phase`**, **`VaWaveform`**, **`sample_va`**

### Module: `oversample`

**`OS_FACTOR`**, **`process_os`**, upsample/downsample helpers

---

## Workspace crates

| Crate | Role |
|-------|------|
| `reelsynth` | Core DSP + export |
| `reelsynth-app` | Standalone egui + cpal + midir |
| `reelsynth-ui` | Shared editor UI |
| `reelsynth-ui-theme` | Design tokens |
| `reelsynth-plugin` | CLAP stub + editor spike (S7: real host) |

---

## Export report

Every export produces or merges `export_report.json`:

```json
{
  "version": 1,
  "target": "serum",
  "success": true,
  "output_path": "synth/serum/patch_export.fxp",
  "dropped": [
    {"path": "mod_matrix[4]", "reason": "Serum v1 supports 4 mod slots"}
  ],
  "warnings": [],
  "errors": []
}
```

See [INTEROP.md](INTEROP.md) for the full loss matrix.

---

## Related docs

- [FORMAT.md](FORMAT.md) — `.reelwt`, `.reelpreset`, `reelpack/` layout
- [WORKFLOW.md](WORKFLOW.md) — user-facing DAW handoff
- [REELDEMO_INTEGRATION.md](REELDEMO_INTEGRATION.md) — commercial Python wrappers

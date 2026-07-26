# Ableton Live — ReelSynth integration

Musician-facing Ableton path. Studio agent handoff comes later.

## Recommended setup (plugin + external editor)

Ableton’s tiny plugin pane is **not** where you design sounds. Live only hosts the instrument (MIDI in → audio out). The **full ReelSynth UI** runs in a separate connected window.

```text
Ableton Live                    External window
┌─────────────────┐            ┌──────────────────────────┐
│ ReelSynth VST3  │◄── IPC ──►│ reelsynth-plugin-editor  │
│ (sound + MIDI)  │  localhost │ (full Design UI)         │
└─────────────────┘            └──────────────────────────┘
```

### 1. Install the VST3

```powershell
cargo build -p reelsynth-plugin --release
```

Copy the staged bundle:

`target\release\ReelSynth.vst3`  
→ `C:\Program Files\Common Files\VST3\ReelSynth.vst3`  
(or rebuild the bundle from `target\release\reelsynth_plugin.dll` as in earlier notes)

Live → Preferences → Plug-ins → enable VST3 system folders → **Rescan**.

### 2. Load the instrument in Live

MIDI track → **ReelSynth**. Play notes — you should hear sound. DAW knobs are only a tiny subset (WT / filter / amp).

### 3. Open the full editor

With Live running and ReelSynth on a track:

```powershell
cargo run -p reelsynth-plugin --release --bin reelsynth-plugin-editor
```

Or after a release build:

```powershell
.\target\release\reelsynth-plugin-editor.exe
```

The editor reads `%LOCALAPPDATA%\ReelSynth\plugin_ipc.json` (written by the plugin) and pushes patch/wavetable edits into Live over localhost.

**Optional auto-launch:** set env `REELSYNTH_AUTO_EDITOR=1` before starting Live (and ensure `reelsynth-plugin-editor.exe` is findable, or set `REELSYNTH_ROOT` to this repo for `cargo run`).

### What works vs not

| Feature | Status |
|---------|--------|
| MIDI notes → sound in Live | Yes |
| Full Design UI in external window | Yes (connected) |
| Automate 5 params from Live | Yes |
| Save/reload Live set | Yes (patch + table blob) |
| Full UI *inside* the Ableton pane | Intentionally not — too small |
| Compose in the editor-for-Live | Use Design; melody stays in Live |

## Path A — Send (no plugin)

Fallback if you are not using the VST3:

1. Standalone app → header **Ableton**
2. Drag `table_multicycle.wav` onto Ableton Wavetable
3. See inbox README / `wavetable_map.json`

## Live QA checklist

- [ ] Rescan finds ReelSynth VST3
- [ ] MIDI note produces audio
- [ ] External editor connects (status line)
- [ ] Change filter in editor → hear change in Live
- [ ] Save Live set, reopen, sound still plays
- [ ] Editor reconnects after closing/reopening

## Studio (later)

Commercial session handoff stays after this path is solid. See [REELDEMO_INTEGRATION.md](REELDEMO_INTEGRATION.md).

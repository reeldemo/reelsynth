# Ableton Live — ReelSynth integration

Musician-facing Ableton path. **Studio** handoff comes later.

## OS support

| OS | Ableton Live | Installer |
|----|--------------|-----------|
| **Windows** | Yes | [`scripts/install-ableton.ps1`](../scripts/install-ableton.ps1) |
| **macOS** | Yes | [`scripts/install-ableton.sh`](../scripts/install-ableton.sh) |
| **Linux** | No (Ableton does not support Linux) | — use another DAW / CLAP elsewhere |

## Recommended setup (plugin + external editor)

Ableton’s plug-in pane stays slim. Live hosts sound + MIDI; the **full Design UI** opens in a connected window.

```text
Ableton Live                    External window
┌─────────────────┐            ┌──────────────────────────┐
│ ReelSynth VST3  │◄── IPC ──►│ reelsynth-plugin-editor  │
│ (sound + MIDI)  │  localhost │ (full Design UI)         │
└─────────────────┘            └──────────────────────────┘
```

### One-shot install

**Windows** (PowerShell; Admin recommended for system VST3 folder):

```powershell
cd C:\Users\Julian\Documents\Programming\github\reeldemo\reelsynth
.\scripts\install-ableton.ps1
# If Program Files is locked, use:
# .\scripts\install-ableton.ps1 -UserVst3
```

**macOS**:

```bash
cd /path/to/reelsynth
chmod +x scripts/install-ableton.sh
./scripts/install-ableton.sh
```

What the installer does:

1. `cargo build -p reelsynth-plugin --release` (unless `-SkipBuild` / `--skip-build`)
2. Installs VST3 where Live looks
3. Installs `reelsynth-plugin-editor` under user App Support / LocalAppData
4. Writes `config.json` with `auto_editor: true` so loading the plug-in opens the full UI

Then in Live: **Preferences → Plug-ins → Rescan** → put **ReelSynth** on a MIDI track.

### Manual editor

If auto-open fails:

- Windows: `%LOCALAPPDATA%\ReelSynth\bin\reelsynth-plugin-editor.exe`
- macOS: `~/Library/Application Support/ReelSynth/bin/reelsynth-plugin-editor`

Config: `%LOCALAPPDATA%\ReelSynth\config.json` (Win) or `~/Library/Application Support/ReelSynth/config.json` (Mac). Set `"auto_editor": false` to disable auto-launch.

### What works vs not

| Feature | Status |
|---------|--------|
| MIDI notes → sound in Live | Yes |
| Full Design UI in external window | Yes (connected) |
| Auto-open editor after install | Yes (`auto_editor`) |
| Automate 5 params from Live | Yes |
| Save/reload Live set | Yes |
| Full UI *inside* the Ableton pane | Intentionally not |
| Ableton on Linux | Not supported |

## Path A — Send (no plugin)

Standalone app → header **Ableton** → drag `table_multicycle.wav` onto Wavetable. See inbox README.

## Live QA checklist

- [ ] Installer completes without error
- [ ] Rescan finds ReelSynth VST3
- [ ] Load track → editor opens (or launch manually)
- [ ] Edit filter in editor → hear change in Live
- [ ] Save/reopen Live set

## Studio (later)

See [REELDEMO_INTEGRATION.md](REELDEMO_INTEGRATION.md).

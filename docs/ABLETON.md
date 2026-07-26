# Ableton Live

How to get ReelSynth into Live. Studio handoff is separate — [REELDEMO_INTEGRATION.md](REELDEMO_INTEGRATION.md).

## OS

| OS | Live | Installer |
|----|------|-----------|
| Windows | Yes | [`scripts/install-ableton.ps1`](../scripts/install-ableton.ps1) |
| macOS | Yes | [`scripts/install-ableton.sh`](../scripts/install-ableton.sh) |
| Linux | No — Ableton isn’t on Linux | Use another host / CLAP elsewhere |

## Plugin + external editor

Live’s plug-in pane stays small on purpose. Sound and MIDI stay in Live; the full Design UI opens in another window.

```text
Ableton Live                    External window
┌─────────────────┐            ┌──────────────────────────┐
│ ReelSynth VST3  │◄── IPC ──►│ reelsynth-plugin-editor  │
│ (sound + MIDI)  │  localhost │ (Design UI)              │
└─────────────────┘            └──────────────────────────┘
```

### Install

**Windows** (PowerShell; Admin if you want the system VST3 folder):

```powershell
cd path\to\reelsynth
.\scripts\install-ableton.ps1
# If Program Files is locked / Live has the DLL open:
# .\scripts\install-ableton.ps1 -UserVst3
```

Quit Live before overwriting an installed VST3.

**macOS:**

```bash
cd path/to/reelsynth
chmod +x scripts/install-ableton.sh
./scripts/install-ableton.sh
```

The script:

1. Builds `reelsynth-plugin` release (unless `-SkipBuild` / `--skip-build`)
2. Drops the VST3 where Live looks
3. Installs `reelsynth-plugin-editor` under LocalAppData / Application Support
4. Writes `config.json` with `auto_editor: true`

Then: **Preferences → Plug-ins → Rescan**, add **ReelSynth** on a MIDI track. Editor should open with the plug-in.

### Editor won’t open

Open the plug-in window in Live and click **Open Editor**, or flip the **Open Editor** switch on the device itself (no floating window needed). Manual launch:

- Windows: `%LOCALAPPDATA%\ReelSynth\bin\reelsynth-plugin-editor.exe`
- macOS: `~/Library/Application Support/ReelSynth/bin/reelsynth-plugin-editor`

Config: same `ReelSynth` folder → `config.json`. Set `"auto_editor": false` to stop auto-launch (the Live **Open Editor** button still works).

### What works

| Feature | Status |
|---------|--------|
| MIDI → sound in Live | Yes |
| Slim Live pane + **Open Editor** | Yes (plug-in window) |
| **Open Editor** switch on device | Yes (Live device rack — toggle on) |
| Full Design UI in external window | Yes |
| Piano / Z–M keys in external editor | Yes (notes → Live VST via IPC) |
| Auto-open after install | Yes (`auto_editor`) |
| Automate a handful of params from Live | Yes |
| Save / reload Live set | Yes |
| Full UI inside Live’s pane | No (by design) |
| Ableton on Linux | No |

## Path A — Send (no plugin)

Standalone → header **Ableton** → drag `table_multicycle.wav` onto Wavetable. Inbox README has the rest.

## Quick check

- [ ] Installer finishes clean
- [ ] Rescan finds ReelSynth
- [ ] Load track → editor opens, or click **Open Editor** in the Live pane
- [ ] Filter move in editor → hear it in Live
- [ ] Save and reopen the set

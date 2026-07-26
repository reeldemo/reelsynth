# Ableton Live — ReelSynth integration

Musician-facing Ableton path. Studio agent handoff is separate and comes later.

## Path A — Send (works today, one drag)

1. Run the standalone app: `cargo run -p reelsynth-app --bin reelsynth-app`
2. Design your sound, then click header **Ableton**.
3. A folder opens under Ableton User Library (`ReelSynth/inbox/…`). Override with env `REELSYNTH_ABLETON_INBOX`.
4. In Live, load **Wavetable** on a MIDI track (Send may create one if [AbletonOSC](https://github.com/ideoforms/AbletonOSC) / Luftbahn fork is enabled).
5. Drag `synth/ableton/table_multicycle.wav` onto Wavetable’s waveform sprite.
6. Tweaks: see `wavetable_map.json` (`reelsynth-ableton-wt-v2`).

Custom tables cannot be injected by API — that drag is required for this path.

## Path B — VST3 plugin (developer preview)

ReelSynth as a real Live instrument (no Wavetable drag). License: plugin crate is **GPL-3.0-or-later**; core engine stays MIT.

### Build

```bash
cargo build -p reelsynth-plugin --release
```

Output (Windows): `target/release/reelsynth_plugin.dll`  
(macOS/Linux): `target/release/libreelsynth_plugin.dylib` / `.so`

### Install into Live (Windows)

1. Create folder: `C:\Program Files\Common Files\VST3\ReelSynth.vst3\Contents\x86_64-win\`
2. Copy `reelsynth_plugin.dll` there and rename to `ReelSynth.vst3` **or** keep as `ReelSynth.dll` inside that Contents path per [VST3 bundle layout](https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical+Documentation/Locations+Format/Plugin+Format.html).
3. In Live: Preferences → Plug-ins → rescan. Enable VST3 system folders.
4. Place **ReelSynth** on a MIDI track and play notes.

Until a bundler/`xtask` lands, a flat copy of the cdylib into your VST3 folder is enough for smoke tests on many hosts; Live prefers a proper `.vst3` bundle.

### What works vs not

| Feature | Status |
|---------|--------|
| MIDI notes → sound | Yes |
| Automate WT / filter / amp attack-release | Yes (5 params) |
| Save/reload set (patch + wavetable blob) | Yes (`reelsynth-plugin-state-v1`) |
| Full ReelSynth egui editor in Live | Not yet (egui version bump) |
| Compose mode in plugin | Disabled / not shown |

## Live QA checklist

- [ ] Rescan finds ReelSynth VST3
- [ ] MIDI note produces audio
- [ ] Move Filter Cutoff while holding a note
- [ ] Save Live set, reopen, sound still plays with same timbre
- [ ] Compare Send+Wavetable path vs VST3 by ear (expect VST3 closer to standalone)

## Studio (later)

Commercial Reeldemo Studio session handoff stays **after** this Ableton path is solid. See [REELDEMO_INTEGRATION.md](REELDEMO_INTEGRATION.md).

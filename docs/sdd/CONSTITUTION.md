# ReelSynth SDD Constitution

Non-negotiable principles for Spec-Driven Development. Later artifacts (`requirements.md`, `design.md`, `tasks.md`) must not contradict this document.

1. **Canonical state** — `.reelpreset` + `.reelwt` are source of truth; foreign formats (Vital, Serum, Ableton Wavetable, SFZ) are lossy targets.
2. **Honest capability claims** — never imply a loadable DAW plugin, full SMF performance export, or automatic custom Wavetable frame import until those exist and are verified.
3. **Compose vs DAW** — Compose mode is in-app sketching; Live/arrangement ownership stays with the DAW until sequence SMF export ships. `daw/midi/melody.mid` remains a demo note until then.
4. **OSS vs Studio** — ReelSynth stays MIT and usable alone; Reeldemo Studio is optional commercial agent/handoff. Do not require Studio for core sound design.
5. **Ableton formats** — Live integration that claims “works in Ableton” must ship **VST3** (AU on macOS). CLAP is complementary for other hosts, not a Live substitute.
6. **Plugin licensing wall** — DSP/core crates stay MIT. A VST3 crate that links GPLv3 `vst3-sys` / nih-plug VST3 path is **GPL-3.0** and documented as such; do not silently relicense the whole tree.
7. **Interop honesty** — every export path records dropped/non-mapped params in `export_report.json`; docs cite [INTEROP.md](../INTEROP.md).
8. **Musician vs developer docs** — keep those tracks separate; user-visible behavior/doc changes update CHANGELOG.
9. **RT safety** — host/plugin audio callbacks stay real-time safe; UI→DSP via lock-free/command channels, not blocking locks on the audio thread.
10. **Cross-repo contracts** — Ableton param maps and handover schemas shared with `reeldemo-ableton` stay versioned (`reelsynth-ableton-wt-v*`); breaking changes bump the schema id.

Accepted: 2026-07-26.

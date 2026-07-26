# Requirements: Ableton Live Integration

## Problem

Musicians can design sounds in ReelSynth but cannot use them in Ableton Live without a clunky manual export (frame folders + param JSON). Studio can stage sessions for Live but does not fully apply ReelSynth maps or custom tables. Live has no API to auto-load custom Wavetable sprites. Users need a friendly interim bridge and a real VST3 instrument for seamless Live play.

Constitution: [docs/sdd/CONSTITUTION.md](../../CONSTITUTION.md). Roadmap context: Cursor plan *Ableton Live Integration*.

## User stories

### US-1 — Versioned Ableton export contract

As a developer integrating ReelSynth with Live or Studio, I want a versioned Ableton export payload (params + frame references + multi-cycle WAV) so that OSS and Studio share one contract.

**Acceptance criteria**

- AC-1.1: `reelpack` / `ableton` export emits schema id `reelsynth-ableton-wt-v2` (or newer) in `synth/ableton/wavetable_map.json`.
- AC-1.2: The map includes semantic `parameters` for at least: osc position, filter cutoff, filter resonance, amp attack, amp release.
- AC-1.3: The map includes Live-facing alias keys (or a documented alias table) that Studio OSC can apply without a separate ad-hoc recipe blob.
- AC-1.4: Export writes a single mono multi-cycle WAV at a path declared in the map (e.g. `synth/ableton/table_multicycle.wav`) suitable for Ableton User Wavetable drag-drop.
- AC-1.5: `export_report.json` lists dropped params (mod slots beyond hints, sub/noise, unmapped FX) — no silent omission.
- AC-1.6: Automated tests assert schema id, required keys, and multi-cycle WAV presence/frame count consistency with the bank.

### US-2 — Send to Ableton from standalone

As a sound designer using only the MIT app, I want a **Send to Ableton** action so that my patch lands in a known inbox and, when Live+AbletonOSC is available, a MIDI track with Wavetable and mapped params is created without Studio.

**Acceptance criteria**

- AC-2.1: UI exposes **Send to Ableton** (header/menu) that exports the current patch + table into a documented inbox under Ableton User Library (path overridable by env).
- AC-2.2: Inbox contains canonical preset/table, `wavetable_map.json`, multi-cycle WAV, and a short README instructing the one required drag onto the Wavetable sprite.
- AC-2.3: When AbletonOSC is reachable (documented probe), Send creates a MIDI track, inserts stock Wavetable, and applies mapped params from the v2 map.
- AC-2.4: When AbletonOSC is not reachable, Send still writes the inbox and shows a clear status/toast with how to install/configure OSC — it does not fail silently.
- AC-2.5: UI/docs never claim that custom wavetable frames were loaded automatically (constitution: honest claims).
- AC-2.6: Studio is not required for Send to succeed (inbox write works offline).

### US-3 — Studio session handoff consumes ReelSynth exports

As a Reeldemo Studio user, I want agent handover to Live to consume `synth_exports` / Ableton maps so that programmatic ReelSynth layers get MIDI + device params consistently.

**Acceptance criteria**

- AC-3.1: For layers with `engine: reelsynth` and reelpack attached, handover prefers `wavetable_map.json` parameters (with aliases) over unrelated default recipe knobs when both exist.
- AC-3.2: OSC and/or Extension import creates tracks and applies device params for `midi_device` layers; multi-cycle WAV is staged in the bundle inbox.
- AC-3.3: Extension (or documented interim OSC path) no longer no-ops on `importHandoverBundle` for the supported layer modes — tracks/clips/params land or a visible error is reported.
- AC-3.4: Docs in both repos state whether frame import still requires one user drag.
- AC-3.5: After VST3 ships (US-4), handover can target ReelSynth VST3 when installed (`midi_reelsynth_vst3` or equivalent) without breaking Wavetable fallback.

### US-4 — ReelSynth VST3 instrument in Ableton

As an Ableton Live user, I want ReelSynth as a VST3 instrument on a MIDI track so that I play and edit the real engine inside Live without Wavetable approximation.

**Acceptance criteria**

- AC-4.1: A loadable **VST3** bundle builds for Windows (and macOS when CI/host available); Live 12 can rescan and load it on a MIDI track.
- AC-4.2: Host MIDI note on/off produces audible output from `SynthEngine`; sample-rate changes do not crash or stay silent without recovery.
- AC-4.3: Shared egui editor embeds in the host window (not only the standalone spike); plugin mode hides cpal/midir device pickers.
- AC-4.4: At least a documented subset of params (e.g. filter cutoff, WT position, amp ADSR or attack/release) is automatable from Live.
- AC-4.5: Saving and reloading a Live set restores patch+wavetable state (round-trip of canonical blob or equivalent).
- AC-4.6: `reelsynth-plugin` is documented as **GPL-3.0** where VST3 bindings require it; core `reelsynth` remains MIT.
- AC-4.7: CLAP export may ship for other DAWs but is not marketed as Ableton support.
- AC-4.8: Compose-in-plugin is instrument-local or disabled in v1; docs say melody recording/arrangement stays in Live.

### US-5 — Docs and product honesty

As a musician reading docs, I want the Ableton path described accurately so that I know which steps are automatic vs one-drag vs install-plugin.

**Acceptance criteria**

- AC-5.1: [WORKFLOW.md](../../../WORKFLOW.md) and [INTEROP.md](../../../INTEROP.md) describe bridge (Send + one drag) vs VST3 (seamless) vs Studio agent handoff.
- AC-5.2: [REELDEMO_INTEGRATION.md](../../../REELDEMO_INTEGRATION.md) matches implemented handover behavior after US-3.
- AC-5.3: CHANGELOG entries for user-visible Send and VST3 milestones.
- AC-5.4: Capability matrix no longer claims “plugin in DAW: No” after US-4 ships; until then it stays No.

## Out of scope

- CLAP-only Ableton support (Live does not load CLAP).
- Bit-exact Ableton Wavetable round-trip of full mod matrix, multi-osc, sub/noise, and FX.
- Programmatic injection of custom Wavetable sprites via LOM (API does not exist).
- Full Compose → SMF performance export (tracked separately).
- Replacing Studio agent compose with the OSS standalone app.
- Max for Live as the primary integration (optional helper only if Extension/OSC gaps remain after US-3/US-4).

## Open questions

Resolved for MVP unless user reopens:

1. **Frame load** — Accepted limitation: one user drag for Wavetable bridge; VST3 removes the need.
2. **Plugin license** — Dual-license plugin GPL-3.0; core MIT.
3. **Compose in plugin** — Disabled or local-only in VST3 v1.
4. **Studio repo** — US-3 work lands in `reeldemo-ableton`; US-1/2/4/5 primarily in `reelsynth`.

Resolved in [spec.md](spec.md):

- OQ-1: Inbox defaults Win `%USERPROFILE%\Documents\Ableton\User Library\ReelSynth\inbox\`, macOS `~/Music/Ableton/User Library/ReelSynth/inbox/`; override `REELSYNTH_ABLETON_INBOX`.
- OQ-2: Send/OSC = Live 12 + AbletonOSC; Extension = Live 12.4.5+ when implemented; OSC remains default until Extension verified; VST3 QA on Live 12.

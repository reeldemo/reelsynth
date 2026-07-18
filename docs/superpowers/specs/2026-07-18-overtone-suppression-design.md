# Overtone suppression (master anti-crackle)

**Date:** 2026-07-18  
**Status:** Approved  
**Placement:** C — global voice output (master bus after voices sum)  
**Control:** 3 — continuous 0–100% strength  
**Adaptive:** A — `strength` is ceiling; `effective = strength × curveHarshness`  
**Algorithms:** all three as selectable modes

## Motivation

Wavetable frames with wrap cliffs or bright upper partials can still crackle or feel harsh after voices sum, even when Quant Seam has softened the table. Quant Seam lives on the WT toolbar and edits the frame; musicians also need a **navbar master** control that tames what they hear on the summed bus without replacing seam processing or Analyze tools.

## Goals

1. Master-bus overtone / crackle suppression after all voices sum (option C).
2. Navbar mode + continuous strength (0–100%), with strength hidden/disabled when Off.
3. Adaptive amount: user strength is the maximum; effective amount scales with measured curve harshness (option A).
4. Three processing modes: Lowpass, Harmonic, Slew (plus Off).
5. Deterministic harshness metric so tests can assert adaptive behavior.
6. Optional AgentSession / MCP surface for mode + strength (same pattern as Quant Seam).

## Non-goals

- Replacing or merging with **Quant Seam** (toolbar; edits WT frame wrap).
- Mipmapped wavetable bandlimiting / per-note anti-alias tables.
- Changing Analyze FFT tools or their display.
- Per-voice insertion (rejected; would diverge polyphony and fight existing voice slew).
- FX-rack slot UI (this is navbar master anti-crackle, not a rack effect).

## UI

**Location:** navbar / header — `ui/src/shell/header.rs` (near existing header controls; not the WT Quant toolbar).

| Control | Behavior |
|---------|----------|
| Mode combo | `Off` \| `Lowpass` \| `Harmonic` \| `Slew` |
| Strength knob | Continuous **0–100%**; **hidden or disabled when mode is Off** |

Labels should match the combo strings above (no `None`-style placeholders).

Quant Seam remains on the Design WT toolbar (`ui/src/wt/toolbar.rs`) and stays independent.

## DSP

### Shared adaptive amount

```
strength ∈ [0, 1]          // UI 0–100%
curveHarshness ∈ [0, 1]    // from active WT frame (below)
effective = strength × curveHarshness
```

When the active frame is already smooth (`curveHarshness ≈ 0`), every mode approaches identity even at strength = 100%. When the frame is discontinuous / HF-rich, `effective` approaches `strength`.

### Mode behaviors

| Mode | Behavior |
|------|----------|
| **Off** | Identity: output sample equals input sample (within float noise). |
| **Lowpass** | One-pole (or equivalent) **master lowpass** on the summed bus. Cutoff **falls** as `effective` rises (brighter harshness + higher strength → lower cutoff). At `effective = 0`, cutoff is high enough to be audibly transparent. |
| **Harmonic** | **Upper-partial / high shelf attenuation** scaled by `effective` (more attenuation as `effective` rises). Prefer a shelf or partial-band gain above a fixed fraction of Nyquist rather than a full brickwall. |
| **Slew** | **Soft-clip + slew limit** on the master bus, aggressiveness scaled by `effective`. Reuse concepts from `src/fx/processors.rs` (`soft_clip`) and voice slew (`slew_limit` in `src/voice/process.rs`), but as a **bus** stage with its own state (not per-voice). |

Stereo: apply L/R with shared coefficients / shared slew state policy (same `effective`, same cutoff/shelf/slew budget) so imaging does not wander.

### Harshness metric (concrete, testable)

Compute `curveHarshness` from the **active wavetable frame** currently driving the primary audible oscillator path (see Open questions for multi-osc). Frame samples `x[0..N)` in approximately `[-1, 1]`, `N = bank.frame_size`.

**1. Wrap discontinuity**

```
seam = |x[N - 1] - x[0]|
wrapHarsh = clamp(seam / 2.0, 0.0, 1.0)
```

Matches the seam scale already used by Quant Adaptive fade (`ui/src/wt/quant_handles.rs`).

**2. High-frequency energy ratio**

Let `X[k]` be the magnitude of a real DFT of `x` for bins `k = 0 .. N/2`.

```
E_total = Σ_{k=1}^{N/2} X[k]²
E_hi    = Σ_{k=k0}^{N/2} X[k]²    where k0 = max(1, floor(N / 4))
hfHarsh = E_hi / (E_total + ε)     // ε = 1e-12
```

Bins from `k0` cover the upper half of the positive spectrum (above ~Nyquist/2 of the frame).

**3. Combine**

```
curveHarshness = clamp(max(wrapHarsh, hfHarsh), 0.0, 1.0)
```

**Fixtures for tests**

| Fixture | Expectation |
|---------|-------------|
| Pure sine period (`sin(2π i / N)`) | `wrapHarsh ≈ 0`, `hfHarsh` low → `curveHarshness` near 0 |
| Unit step / saw-like wrap (`x[i] = 2*(i/N)-1` without seam close) | `wrapHarsh` near 1 and/or high `hfHarsh` → `curveHarshness` clearly higher than sine |

Update rate: recompute when the active frame contents or frame index change; may cache per block. Do not require per-sample DFT.

### Parameter mapping sketches (implementation may tune constants)

- **Lowpass:** `cutoff_hz = mix(cutoff_max, cutoff_min, effective)` with e.g. `cutoff_max ≈ 0.45 * sample_rate`, `cutoff_min ≈ 800 Hz` (exact constants chosen in implementation plan; must be monotonic in `effective`).
- **Harmonic:** shelf gain `g_db = mix(0, g_min_db, effective)` with `g_min_db < 0` on frequencies above ~`0.25 * sample_rate`.
- **Slew:** `max_delta = mix(max_delta_open, max_delta_tight, effective)`; soft-clip drive increases with `effective`.

## Signal chain

Current engine path (`src/engine/mod.rs` — `process_block_mono` / `process_block_stereo`):

```
per voice → process_sample_stages
  → sum L/R (and osc tap)
  → voice_headroom
  → × master_gain
  → FxChain
  → sanitize_sample → out
```

**Intended insertion (option C):** after voice sum + headroom + `master_gain`, **before** `FxChain`:

```
… → × master_gain
  → OvertoneSuppressor(mode, effective)   // NEW master bus stage
  → FxChain
  → sanitize → out
```

Rationale: navbar anti-crackle shapes the synth bus feeding the rack; existing FX end soft-clip remains. Scope taps: keep osc/filt taps pre-suppressor; post-FX “out” tap remains after FX (suppressed signal then FX).

Do **not** insert inside the per-voice loop.

## State / API

### UI / session state

Suggested fields on `UiState` (`ui/src/state.rs`):

```rust
overtone_mode: OvertoneSuppressMode,  // Off | Lowpass | Harmonic | Slew
overtone_strength: f32,               // 0.0 ..= 1.0
```

Defaults: mode `Off`, strength **`1.0` (100%)**. Adaptive math already scales by `curveHarshness`, so full strength on a smooth frame stays gentle; enabling a mode does not require hunting for the knob. Strength is ignored while mode is Off.

Engine receives mode + strength plus either the active frame samples or a precomputed `curveHarshness` each block. Prefer computing harshness on the audio side from the current bank frame (or caching when frame bytes/index unchanged) so the suppressor stays a pure DSP object.

### Persistence

Session-only for v1 (not written into `.reelpreset`). A later FORMAT bump can add fields; document in `docs/FORMAT.md` only when persisted.

### AgentSession / MCP (optional extension)

Mirror Quant Seam:

- `AgentSession::set_overtone_mode` / `set_overtone_strength`
- Snapshot fields: `overtone_mode`, `overtone_strength`
- MCP tools e.g. `reelsynth_set_overtone_mode`, `reelsynth_set_overtone_strength` in `mcp/src/main.rs`
- Document in `docs/AGENT_API.md`

Not blocking for first audio+UI slice.

## Tests

Required automated coverage (engine-level unit/QA preferred):

1. **Off ≈ identity** — bit-identical or max abs error `< 1e-6` vs bypass for sine and harsh fixtures.
2. **Mode sensitivity** — for each of Lowpass, Harmonic, Slew at `strength > 0` and a harsh fixture with high `curveHarshness`, a measurable spectral or time-domain harshness proxy (e.g. spectral centroid, HF energy, or peak `|Δsample|`) moves **more** than the same settings on a smooth sine fixture.
3. **Adaptive scaling** — fixed `strength` and mode; harsher curve (higher `curveHarshness`) produces **stronger** effect than smoother curve (compare same proxy as above).
4. **Stability** — no NaNs/Infs; finite output at strength `0%` and `100%` for all modes; Off ignores strength.

Harshness unit tests: sine fixture → `curveHarshness < 0.15`; discontinuous saw/step wrap → `curveHarshness > 0.5` (thresholds adjustable if N or DFT window changes, but ordering must hold).

## Open questions

1. **Which frame when multiple oscs / morph?** Recommendation for v1: osc 0’s current morph frame; alternative is `max` harshness across audible oscillators if dual-osc crackle remains.
2. **Persist in `.reelpreset` in v1** or session-only until a FORMAT bump? Recommendation: session-only for first ship.

## Success criteria

- Musician can switch Off / Lowpass / Harmonic / Slew from the header and hear master-bus taming without touching Quant Seam.
- Smooth WT + high strength stays gentle; harsh WT + same strength is clearly stronger.
- Tests above pass; docs stay honest that Quant Seam and this control are separate.

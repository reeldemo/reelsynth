# Overtone suppression (master anti-crackle chain)

**Date:** 2026-07-18  
**Status:** Approved (revised — chainable filters)  
**Placement:** C — global voice output (master bus after voices sum, before `FxChain`)  
**Control:** chainable filter slots (FxChain-style add / reorder / remove) + per-slot strength 0–100%  
**Adaptive:** A — each slot’s `strength` is a ceiling; `effective = strength × curveHarshness`  
**Algorithms:** Lowpass, Harmonic, Slew as **insertable filter types** (not mutually exclusive modes)

## Motivation

Wavetable frames with wrap cliffs or bright upper partials can still crackle or feel harsh after voices sum, even when Quant Seam has softened the table. Quant Seam lives on the WT toolbar and edits the frame; musicians also need a **master-bus** control that tames what they hear on the summed bus without replacing seam processing or Analyze tools.

A single mode radio was too limiting: users want to **stack** Lowpass, Harmonic, and/or Slew (e.g. mild shelf then slew) the same way they build an FX rack. Off is simply an **empty chain**.

## Goals

1. Master-bus overtone / crackle suppression after all voices sum (option C).
2. **Chainable** overtone filters with the **same add / reorder / remove UX** as the effects rack (`FxChain` / FX rack UI).
3. Per-slot continuous strength (0–100%); empty chain = Off (identity).
4. Adaptive amount: user strength is the maximum; effective amount scales with measured curve harshness (option A), shared across slots.
5. Three **filter types** insertable in any order / multiplicity: Lowpass, Harmonic, Slew.
6. Deterministic harshness metric so tests can assert adaptive behavior.
7. Optional AgentSession / MCP surface for the chain (same pattern as Quant Seam / FX slots).

## Non-goals

- Replacing or merging with **Quant Seam** (toolbar; edits WT frame wrap).
- Merging overtone filters into the musical **Effects** rack (`Chorus` / `Delay` / `Reverb` / …). This is a **separate** anti-crackle chain that **reuses the same interaction pattern**, not new `EffectType` variants.
- Mipmapped wavetable bandlimiting / per-note anti-alias tables.
- Changing Analyze FFT tools or their display.
- Per-voice insertion (rejected; would diverge polyphony and fight existing voice slew).
- Mutually exclusive single-mode radio (superseded by this revision).

## Mirrored FxChain pattern (explore summary)

| Layer | Path | Mechanic |
|-------|------|----------|
| Types / slots | `src/fx/types.rs` | `EffectType` + `EffectSlot` (`bypassed`, `mix`, type-specific params); `default_effects()` seeds a `Vec` |
| Engine chain | `src/fx/chain.rs` | `FxChain { slots, processors }` — `set_effects`, process slots **in vector order**, skip bypassed / near-zero mix, end soft-clip |
| Processors | `src/fx/processors.rs` | Per-type DSP; soft_clip shared at chain end |
| UI rack | `ui/src/fx_rack.rs` | Slot cards: type combo, On/Off bypass, params; **◀ / ▶** reorder; **✕** remove (if `len > 1`); **“+ Add effect”** appends a default slot |
| Shell placement | `ui/src/shell/mod.rs` (horizontal strip), `ui/src/shell/header.rs` (osc-column sidebar via `draw_effect_rack_sidebar`) | Same `state.fx_slots` / `EffectRackState` |
| Sync | `ui/src/state_sync.rs` | Patch ↔ UI slot vec |

**Overtone chain mirrors this:** ordered `Vec` of filter slots, add / swap / remove in UI, process in order on the master bus. It does **not** share `EffectSlot` / `EffectType` or live inside `FxChain`.

## UI

**Product pattern:** match the FX rack — do **not** invent a second paradigm (no lone mode combo + one knob as the primary model).

**Recommended layout (pick one at implementation; both are valid product patterns already in-tree):**

1. **Compact panel / collapsible section** (preferred for navbar discoverability) — header or center chrome entry labeled e.g. “Overtone” / “Anti-crackle” that opens a small chain UI built like `draw_effect_rack` / `draw_effect_rack_sidebar` (`ui/src/fx_rack.rs`): slot cards + “+ Add filter”.
2. **Sidebar / strip sibling** — if layout budget allows, a dedicated strip next to Effects using the same slot-card chrome (vertical chain or horizontal), not a different control language.

Navbar / header (`ui/src/shell/header.rs`) should either host the **entry point** to that chain UI or host the **compact chain controls** themselves — same relationship Effects already has to the osc column / FX strip.

| Control | Behavior |
|---------|----------|
| **+ Add filter** | Append a default slot (recommend default type **Lowpass**, strength `1.0`) |
| Slot type combo | `Lowpass` \| `Harmonic` \| `Slew` (change type in place; keep strength) |
| ◀ / ▶ | Reorder adjacent slots (same as FX) |
| ✕ | Remove slot (allow empty chain; unlike FX’s `len > 1` guard if product prefers always-removable — **allow empty** so Off = no slots) |
| Strength | Per slot, continuous **0–100%** |
| Optional bypass | Per-slot On/Off (mirror FX `bypassed`) if useful; empty chain still means full Off |

**Empty chain = Off:** identity on the bus; no strength UI required when there are zero slots (or show only the Add control).

Quant Seam remains on the Design WT toolbar (`ui/src/wt/toolbar.rs`) and stays independent.

## DSP

### Shared adaptive amount

One `curveHarshness` from the active WT frame feeds **every** slot:

```
strength_i ∈ [0, 1]        // UI 0–100% for slot i
curveHarshness ∈ [0, 1]    // from active WT frame (below)
effective_i = strength_i × curveHarshness
```

When the active frame is already smooth (`curveHarshness ≈ 0`), every slot approaches identity even at strength = 100%. When the frame is discontinuous / HF-rich, each `effective_i` approaches that slot’s `strength_i`.

### Filter types (insertable, chainable)

| Type | Behavior |
|------|----------|
| **Lowpass** | One-pole (or equivalent) **master lowpass**. Cutoff **falls** as `effective` rises. At `effective = 0`, cutoff high enough to be audibly transparent. |
| **Harmonic** | **Upper-partial / high shelf attenuation** scaled by `effective`. Prefer a shelf or partial-band gain above a fixed fraction of Nyquist rather than a full brickwall. |
| **Slew** | **Soft-clip + slew limit** on the bus, aggressiveness scaled by `effective`. Reuse concepts from `src/fx/processors.rs` (`soft_clip`) and voice slew (`slew_limit` in `src/voice/process.rs`), but as a **bus** stage with **per-slot** state (not per-voice). |

Slots run **in chain order** (serial). Duplicate types are allowed (e.g. two Lowpasses). Stereo: apply L/R with shared coefficients / shared slew state policy per slot (same `effective`, same cutoff/shelf/slew budget) so imaging does not wander.

Empty chain: output sample equals input sample (within float noise).

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

Per slot, using that slot’s `effective_i`:

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

**Intended insertion (option C):** after voice sum + headroom + `master_gain`, **before** musical `FxChain`:

```
… → × master_gain
  → OvertoneFilterChain(slots, curveHarshness)   // NEW; serial slots
  → FxChain                                       // existing musical FX
  → sanitize → out
```

Rationale: navbar / master anti-crackle shapes the synth bus feeding the rack; existing FX end soft-clip remains. Scope taps: keep osc/filt taps pre-overtone chain; post-FX “out” tap remains after FX (suppressed signal then FX).

Do **not** insert inside the per-voice loop. Do **not** fold these types into `FxChain`’s `EffectType` list.

## State / API

### Suggested types (engine)

Mirror `EffectType` / `EffectSlot` / `FxChain` naming without sharing those types:

```rust
enum OvertoneFilterType { Lowpass, Harmonic, Slew }

struct OvertoneFilterSlot {
    filter_type: OvertoneFilterType,
    strength: f32,      // 0.0 ..= 1.0
    bypassed: bool,     // optional; default false
}

struct OvertoneFilterChain {
    slots: Vec<OvertoneFilterSlot>,
    // processors aligned 1:1 with slots, rebuilt on set_slots
}
```

### UI / session state

Suggested fields on `UiState` (`ui/src/state.rs`):

```rust
overtone_slots: Vec<OvertoneFilterSlotUi>,  // empty = Off
```

Defaults: **empty chain** (Off). When adding a slot, default strength **`1.0` (100%)**. Adaptive math already scales by `curveHarshness`, so full strength on a smooth frame stays gentle.

Engine receives the slot list plus either the active frame samples or a precomputed `curveHarshness` each block. Prefer computing harshness on the audio side from the current bank frame (or caching when frame bytes/index unchanged) so the chain stays a pure DSP object.

### Persistence

Session-only for v1 (not written into `.reelpreset`). A later FORMAT bump can add an `overtone_filters` (or similar) array; document in `docs/FORMAT.md` only when persisted.

### AgentSession / MCP (optional extension)

Mirror Quant Seam / FX slot APIs at chain granularity:

- Set / replace chain, add slot, remove / reorder, set per-slot type + strength
- Snapshot: `overtone_slots` (or equivalent)
- MCP tools e.g. `reelsynth_set_overtone_chain` (and/or finer mutators) in `mcp/src/main.rs`
- Document in `docs/AGENT_API.md`

Not blocking for first audio+UI slice.

## Tests

Required automated coverage (engine-level unit/QA preferred):

1. **Empty chain ≈ identity** — bit-identical or max abs error `< 1e-6` vs bypass for sine and harsh fixtures.
2. **Type sensitivity** — for each of Lowpass, Harmonic, Slew as a **single** active slot at `strength > 0` and a harsh fixture with high `curveHarshness`, a measurable spectral or time-domain harshness proxy (e.g. spectral centroid, HF energy, or peak `|Δsample|`) moves **more** than the same settings on a smooth sine fixture.
3. **Chain order** — two different types in order A→B vs B→A produce distinguishable outputs under the same strengths (or document if intentionally commutative for a given pair; prefer measurable order dependence for LP then Slew vs Slew then LP).
4. **Adaptive scaling** — fixed slot strength and type; harsher curve (higher `curveHarshness`) produces **stronger** effect than smoother curve (compare same proxy as above).
5. **Stability** — no NaNs/Infs; finite output at strength `0%` and `100%` for all types; empty chain ignores leftover UI strength if any.

Harshness unit tests: sine fixture → `curveHarshness < 0.15`; discontinuous saw/step wrap → `curveHarshness > 0.5` (thresholds adjustable if N or DFT window changes, but ordering must hold).

## Open questions

1. **Which frame when multiple oscs / morph?** Recommendation for v1: osc 0’s current morph frame; alternative is `max` harshness across audible oscillators if dual-osc crackle remains.
2. **Persist in `.reelpreset` in v1** or session-only until a FORMAT bump? Recommendation: session-only for first ship.
3. **UI home:** collapsible header entry vs dedicated strip beside Effects — implementer chooses based on layout budget; both must keep FxChain-style slot mechanics.
4. **FX remove guard:** FX rack refuses remove when only one slot remains; overtone chain **should allow empty** so Off needs no special “Off” type.

## Success criteria

- Musician can **add / reorder / remove** Lowpass, Harmonic, and Slew filters on the master anti-crackle chain using the same interaction pattern as Effects, and hear bus taming without touching Quant Seam.
- Empty chain is transparent; stacked slots apply in order with per-slot strength.
- Smooth WT + high strength stays gentle; harsh WT + same strength is clearly stronger (shared `curveHarshness`).
- Tests above pass; docs stay honest that Quant Seam, this chain, and the musical `FxChain` are separate.

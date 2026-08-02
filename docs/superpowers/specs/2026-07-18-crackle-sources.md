# Crackle / click sources (2026-07-18)

Ranked remaining discontinuities after Quant seam periodize, polyBLEP widen, overtone chain, and voice slew. Evidence from code paths + unit tests (`held_sine_period_step_bounded`, Factory Lead sustain step bounds).

| Rank | Source | Likelihood | Where | Notes / next fix |
|------|--------|------------|-------|------------------|
| 1 | **WT wrap / Quant Seam Off** | High when Seam=Off or harsh frames | `ui/src/wt/*` periodize; `src/osc` BLEP; bank frames | Adaptive/Soft already periodize; Off leaves raw `|x[N-1]-x[0]|`. Keep Seam≠Off for edited curves; overtone Lowpass/Slew as safety net. |
| 2 | **Stack Add clip (mitigated 0.4.6)** | Was high on loud overlays | `src/osc/stack.rs` `finish_add_sum` | Peak-safe Add + default **avg**; residual risk only if a single layer itself exceeds ±1 (BLEP overshoot → soft limit). |
| 3 | **Filter cutoff jumps while editing** | Medium–high while dragging | `src/engine/params.rs` smoother (10 ms) only on legacy `filter.cutoff`; chain slots 1+ update instantly via `SetPatch` | Smooth all active chain slot cutoffs (or one-pole per slot) when UI edits. |
| 4 | **Note-on / HP cold start** | Medium on HP/Notch | `src/voice/process.rs` `filter_fade` (~8 ms) + `slew_limit` | Already mitigated; lengthen fade for HP-first chains or zero SVF state with longer crossfade. |
| 5 | **Voice steal / retrigger** | Medium under polyphony | `src/engine/voice_*` | Retrigger-while-releasing keeps fade (`retrigger_while_releasing_keeps_filter_fade`); steal of another note can still click — soft-steal crossfade. |
| 6 | **Amp / filt envelope edges** | Low–medium | `src/voice/envelope.rs` | Very short attack (≤1 ms) + loud velocity → click. Soft-knee attack floor. |
| 7 | **Compose / MIDI scheduling** | Low–medium | `app` compose transport → note on/off | Sample-accurate scheduling vs block edges; verify no double note-on. |
| 8 | **Device buffer underruns** | Unknown from code | cpal callback | Not provable in unit tests; if crackle correlates with CPU spikes / buffer size, raise buffer. |
| 9 | **Morph / cubic vs spectral** | Low for clicks, high for brightness | WT morph path | Brightness ≠ click; wrap discontinuity is the click path. |

**Diagnostics added:** `held_sine_period_step_bounded` (empty filter chain, sine sustain step &lt; 0.12); chain empty=bypass + series/drive-order tests in `voice::process_tests`.

**Musical filter chain** (this change) helps crackling when users stack LP/Notch, but is separate from header **Overtone**.

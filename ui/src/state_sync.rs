//! Patch ↔ `UiState` synchronization (extracted for Q&A roundtrip tests).

use reelsynth::patch::{Envelope, Oscillator, Patch};
use crate::{
    effect_slots_from_patch, effect_slots_to_patch, factory_label, filter_slots_from_patch,
    filter_slots_to_patch, fm_source_from_index, mod_slots_from_patch, mod_slots_to_patch,
    osc_type_from_index, OscillatorUi, UiState, warp_mode_from_index,
};
use crate::oscillator_ui::{ensure_wave_layers, WaveLayerUi};
use crate::wt::position_from_osc_ui;
use crate::osc_column::osc_type_index;

pub fn lfo_shape_from_index(idx: usize) -> &'static str {
    match idx {
        1 => "tri",
        2 => "saw",
        3 => "sh",
        _ => "sine",
    }
}

pub fn lfo_shape_index(shape: &str) -> usize {
    match shape.to_ascii_lowercase().as_str() {
        "tri" | "triangle" => 1,
        "saw" => 2,
        "sh" | "s&h" => 3,
        _ => 0,
    }
}

pub fn filter_mode_from_type(filter_type: &str) -> usize {
    match filter_type.to_ascii_lowercase().as_str() {
        "highpass" | "hp" => 1,
        "bandpass" | "bp" => 2,
        "notch" => 3,
        _ => 0,
    }
}

pub fn filter_type_from_mode(mode: usize) -> &'static str {
    match mode {
        1 => "highpass",
        2 => "bandpass",
        3 => "notch",
        _ => "lowpass",
    }
}

fn sync_flat_filters_from_slots(state: &mut UiState) {
    if let Some(s0) = state.filter_slots.first() {
        state.filter_cutoff = s0.cutoff;
        state.filter_resonance = s0.resonance;
        state.filter_key_tracking = s0.key_tracking;
        state.filter_drive = s0.drive;
        state.filter_mode = filter_mode_from_type(&s0.filter_type);
    }
    if let Some(s1) = state.filter_slots.get(1) {
        state.filter2_cutoff = s1.cutoff;
        state.filter2_resonance = s1.resonance;
        state.filter2_drive = s1.drive;
        state.filter2_mode = filter_mode_from_type(&s1.filter_type);
    }
}

fn preset_category_label(patch: &Patch) -> String {
    let wt = patch
        .wavetable_id
        .as_deref()
        .unwrap_or("wavetable")
        .replace('_', " ");
    format!("Preset · Wavetable · {wt}")
}

fn osc_ui_to_patch(osc: &OscillatorUi) -> Oscillator {
    let layer_wt_id = osc
        .wave_layers
        .iter()
        .find(|l| l.is_wavetable())
        .and_then(|l| l.wavetable_id.clone());
    let mut out = Oscillator {
        osc_type: osc_type_from_index(osc.osc_type).into(),
        level: osc.level,
        pan: osc.pan,
        detune: osc.coarse,
        unison: osc.unison,
        position: osc.position,
        pulse_width: osc.pulse_width,
        morph_a: osc.morph_a,
        morph_b: osc.morph_b,
        morph_amount: osc.morph_amount,
        warp_mode: warp_mode_from_index(osc.warp_mode).into(),
        warp_amount: osc.warp_amount,
        fm_source: fm_source_from_index(osc.fm_source).into(),
        fm_ratio: osc.fm_ratio,
        fm_index: osc.fm_index,
        wave_quant: osc.wave_quant,
        wave_slot: osc.wave_slot,
        wave_slot_fine: osc.wave_slot_fine,
        wave_slots: osc.wave_slots.clone(),
        wave_layers: osc.wave_layers.iter().map(WaveLayerUi::to_patch).collect(),
        stack_mode: osc.stack_mode.clone(),
        wavetable_id: layer_wt_id,
        ..Oscillator::default_va()
    };
    if osc.morph_amount > 0.0 {
        out.position =
            osc.morph_a + (osc.morph_b - osc.morph_a) * osc.morph_amount.clamp(0.0, 1.0);
    }
    out
}

pub fn sync_state_from_patch(state: &mut UiState, patch: &Patch) {
    state.preset_name = patch.name.clone();
    state.preset_category = preset_category_label(patch);
    state.wt_bank_name = patch
        .wavetable_id
        .as_deref()
        .and_then(factory_label)
        .map(str::to_string)
        .unwrap_or_else(|| {
            patch
                .wavetable_id
                .as_deref()
                .unwrap_or("wavetable")
                .replace('_', " ")
        });
    state.wt_position = patch
        .oscillators
        .first()
        .map(|o| {
            let ui = OscillatorUi::from_patch(o);
            position_from_osc_ui(&ui, 256)
        })
        .unwrap_or(0.0);

    if patch.oscillators.is_empty() {
        state.oscillators = vec![OscillatorUi::new_active()];
    } else {
        state.oscillators = patch
            .oscillators
            .iter()
            .map(OscillatorUi::from_patch)
            .collect();
    }
    for osc in &mut state.oscillators {
        ensure_wave_layers(osc);
    }
    if state.selected_layer_idx.is_none() {
        state.selected_layer_idx = Some(0);
    }
    let layer_count = state
        .oscillators
        .get(state.active_osc_index())
        .map(|o| o.wave_layers.len())
        .unwrap_or(0);
    if let Some(idx) = state.selected_layer_idx {
        if idx >= layer_count {
            state.selected_layer_idx = Some(layer_count.saturating_sub(1));
        }
    }
    state.osc_tab = state.osc_tab.min(state.oscillators.len().saturating_sub(1));

    state.unison_stereo_spread = patch.unison_stereo_spread;
    state.filter_slots = filter_slots_from_patch(&patch.filter, &patch.filter2, &patch.filters);
    sync_flat_filters_from_slots(state);

    let idx = state.active_osc_index();
    let active = &state.oscillators[idx];
    state.wt_morph_a = active.morph_a;
    state.wt_morph_b = active.morph_b;
    state.wt_morph_amount = active.morph_amount;

    state.sub_level = patch.sub_level;
    state.noise_level = patch.noise_level;
    state.env_attack = patch.envelope.attack;
    state.env_decay = patch.envelope.decay;
    state.env_sustain = patch.envelope.sustain;
    state.env_release = patch.envelope.release;
    state.filt_env_attack = patch.filter_envelope.attack;
    state.filt_env_decay = patch.filter_envelope.decay;
    state.filt_env_sustain = patch.filter_envelope.sustain;
    state.filt_env_release = patch.filter_envelope.release;
    state.lfo_rate = patch.lfo.rate;
    state.lfo_depth = patch.lfo.depth;
    state.lfo_shape = lfo_shape_index(&patch.lfo.shape);
    state.lfo2_rate = patch.lfo2.rate;
    state.lfo2_depth = patch.lfo2.depth;
    state.lfo2_shape = lfo_shape_index(&patch.lfo2.shape);
    for (i, mac) in patch.macros.iter().enumerate().take(4) {
        state.macro_values[i] = mac.value;
    }
    state.mod_routes = mod_slots_from_patch(&patch.mod_matrix);
    state.mod_route_total = state.mod_routes.len().max(24);
    state.fx_slots = effect_slots_from_patch(&patch.effects);
    state.performance = crate::performance::PerformanceUi::from_settings(&patch.performance);
    state.patch_crackle = patch.crackle.clamp(0.0, 1.0);
    sync_compose_from_patch(state, patch);
}

fn sync_compose_from_patch(state: &mut UiState, patch: &Patch) {
    state.compose.project = patch.sequence.clone();
    state.compose.transport.loop_enabled = patch.sequence.loop_region.enabled;
    state.compose.snap_division = patch.sequence.quantize.division;
}

pub fn compose_to_patch_sequence(compose: &crate::compose::ComposeUi) -> reelsynth::SequenceProject {
    let mut seq = compose.project.clone();
    seq.loop_region.enabled = compose.transport.loop_enabled;
    seq.quantize.division = compose.snap_division;
    seq
}

/// After loading a wavetable bank (factory / file / import), make Design audio match the editor.
///
/// Design tone is driven by `wave_layers`. VA layers ignore the bank, so a bare bank swap
/// only updates previews. This promotes a wavetable layer to the audible primary, ducks
/// sibling VA layers, and selects that layer for Quant / Shape editing.
pub fn apply_loaded_bank_to_design(
    state: &mut UiState,
    bank_id: Option<&str>,
    num_frames: usize,
) -> usize {
    let osc_idx = state.active_osc_index();
    let osc = &mut state.oscillators[osc_idx];
    ensure_wave_layers(osc);

    let max_pos = (num_frames.saturating_sub(1)).max(1) as f32;
    let preferred = state
        .selected_layer_idx
        .unwrap_or(0)
        .min(osc.wave_layers.len().saturating_sub(1));

    let wt_idx = osc
        .wave_layers
        .iter()
        .position(|l| l.is_wavetable())
        .unwrap_or(preferred);

    if !osc.wave_layers[wt_idx].is_wavetable() {
        osc.wave_layers[wt_idx].source_type = "wavetable".into();
    }

    for (i, layer) in osc.wave_layers.iter_mut().enumerate() {
        if i == wt_idx {
            layer.source_type = "wavetable".into();
            layer.enabled = true;
            layer.level = layer.level.max(0.9);
            layer.wt_position = layer.wt_position.clamp(0.0, max_pos);
            layer.wavetable_id = bank_id.map(str::to_string);
        } else if layer.is_va() && layer.enabled {
            // Keep VA colour in the stack, but let the loaded bank dominate the tone.
            layer.level = (layer.level * 0.4).min(0.22);
        }
    }

    osc.osc_type = osc_type_index("wavetable");
    state.selected_layer_idx = Some(wt_idx);
    state.wt_position = osc.wave_layers[wt_idx].wt_position;
    wt_idx
}

pub fn patch_from_state(state: &UiState, base: &Patch) -> Patch {
    let mut patch = base.clone();
    patch.name = state.preset_name.clone();
    patch.oscillators = state.oscillators.iter().map(osc_ui_to_patch).collect();
    if patch.oscillators.is_empty() {
        patch.ensure_oscillators(1);
    }
    patch.filters = filter_slots_to_patch(&state.filter_slots);
    patch.sync_legacy_filters_from_chain();
    // Mirror slot0 into flat fields used by smoother / footer when chain is non-empty.
    if let Some(s0) = state.filter_slots.first() {
        patch.filter = s0.to_slot().to_filter();
    }
    if let Some(s1) = state.filter_slots.get(1) {
        patch.filter2 = s1.to_slot().to_filter();
    }
    patch.unison_stereo_spread = state.unison_stereo_spread;
    patch.envelope = Envelope {
        attack: state.env_attack,
        decay: state.env_decay,
        sustain: state.env_sustain,
        release: state.env_release,
    };
    patch.filter_envelope = Envelope {
        attack: state.filt_env_attack,
        decay: state.filt_env_decay,
        sustain: state.filt_env_sustain,
        release: state.filt_env_release,
    };
    patch.lfo.rate = state.lfo_rate;
    patch.lfo.depth = state.lfo_depth;
    patch.lfo.shape = lfo_shape_from_index(state.lfo_shape).into();
    patch.lfo2.rate = state.lfo2_rate;
    patch.lfo2.depth = state.lfo2_depth;
    patch.lfo2.shape = lfo_shape_from_index(state.lfo2_shape).into();
    for (i, mac) in patch.macros.iter_mut().enumerate().take(4) {
        mac.value = state.macro_values[i];
    }
    patch.sub_level = state.sub_level;
    patch.noise_level = state.noise_level;
    patch.mod_matrix = mod_slots_to_patch(&state.mod_routes);
    patch.effects = effect_slots_to_patch(&state.fx_slots);
    patch.performance = state.performance.to_settings();
    patch.sequence = compose_to_patch_sequence(&state.compose);
    patch.crackle = state.patch_crackle.clamp(0.0, 1.0);
    patch
}

#[cfg(test)]
mod tests {
    use super::*;
    use reelsynth::patch::Patch;

    #[test]
    fn empty_wave_layers_seeded_on_sync() {
        let mut patch = Patch::default_mono();
        patch.oscillators[0].wave_layers.clear();
        let mut state = UiState::default();
        sync_state_from_patch(&mut state, &patch);
        assert_eq!(state.oscillators[0].wave_layers.len(), 3);
        assert_eq!(state.oscillators[0].wave_layers[0].source_type, "saw");
        assert_eq!(state.oscillators[0].wave_layers[1].source_type, "sine");
        assert_eq!(state.oscillators[0].wave_layers[2].source_type, "square");
        assert_eq!(state.selected_layer_idx, Some(0));
    }

    #[test]
    fn crackle_amount_roundtrip() {
        let mut original = Patch::factory_lead();
        original.crackle = 0.42;
        let mut state = UiState::default();
        sync_state_from_patch(&mut state, &original);
        assert!((state.patch_crackle - 0.42).abs() < 1e-6);
        state.patch_crackle = 0.75;
        let patch = patch_from_state(&state, &original);
        assert!((patch.crackle - 0.75).abs() < 1e-6);
    }

    #[test]
    fn performance_settings_roundtrip() {
        let mut original = Patch::factory_lead();
        original.performance.root = 9;
        original.performance.scale = reelsynth::Scale::Mixolydian;
        original.performance.layout = reelsynth::PerformanceLayout::Scale;
        let mut state = UiState::default();
        sync_state_from_patch(&mut state, &original);
        assert_eq!(state.performance.root, 9);
        assert_eq!(state.performance.scale, 8);
        let restored = patch_from_state(&state, &Patch::default_mono());
        assert_eq!(restored.performance.root, 9);
        assert_eq!(restored.performance.scale, reelsynth::Scale::Mixolydian);
        assert_eq!(restored.performance.layout, reelsynth::PerformanceLayout::Scale);
    }

    #[test]
    fn arp_settings_preset_roundtrip() {
        let mut original = Patch::factory_lead();
        original.performance.arp = reelsynth::ArpSettings {
            enabled: true,
            input_mode: reelsynth::ArpInputMode::HeldChord,
            direction: reelsynth::ArpDirection::UpDown,
            rate: reelsynth::ArpRate::Eighth,
            gate: 0.6,
            octave_spread: 3,
            latch: true,
        };
        let mut state = UiState::default();
        sync_state_from_patch(&mut state, &original);
        assert!(state.performance.arp.enabled);
        assert_eq!(state.performance.arp.octave_spread, 3);
        let restored = patch_from_state(&state, &Patch::default_mono());
        assert_eq!(restored.performance.arp, original.performance.arp);
    }

    #[test]
    fn wave_layer_invert_roundtrip() {
        let mut original = Patch::factory_lead();
        original.oscillators[0].wave_layers[0].invert = true;
        original.oscillators[0].stack_mode = "avg_equal".into();
        let mut state = UiState::default();
        sync_state_from_patch(&mut state, &original);
        assert!(state.oscillators[0].wave_layers[0].invert);
        assert_eq!(state.oscillators[0].stack_mode, "avg_equal");
        let restored = patch_from_state(&state, &Patch::default_mono());
        assert!(restored.oscillators[0].wave_layers[0].invert);
        assert_eq!(restored.oscillators[0].stack_mode, "avg_equal");
    }

    #[test]
    fn factory_lead_wave_stack_roundtrip() {
        let original = Patch::factory_lead();
        let mut state = UiState::default();
        sync_state_from_patch(&mut state, &original);
        assert_eq!(state.oscillators[0].wave_layers.len(), 3);
        assert_eq!(state.oscillators[0].stack_mode, "avg");
        assert!((state.oscillators[0].wave_layers[0].level - 0.55).abs() < 1e-4);
        assert_eq!(state.oscillators[0].wave_layers[0].source_type, "saw");
        let restored = patch_from_state(&state, &Patch::default_mono());
        assert_eq!(restored.oscillators[0].wave_layers.len(), 3);
        assert_eq!(restored.oscillators[0].stack_mode, "avg");
        assert!((restored.oscillators[0].wave_layers[2].wt_position - 108.0).abs() < 0.01);
        assert_eq!(
            restored.oscillators[0].wave_layers[2].wavetable_id.as_deref(),
            Some("saw_morph")
        );
    }

    #[test]
    fn apply_loaded_bank_promotes_wavetable_layer() {
        let mut state = UiState::default();
        state.oscillators[0].wave_layers = vec![
            WaveLayerUi {
                source_type: "saw".into(),
                level: 0.55,
                enabled: true,
                ..WaveLayerUi::default()
            },
            WaveLayerUi {
                source_type: "sine".into(),
                level: 0.30,
                enabled: true,
                ..WaveLayerUi::default()
            },
        ];
        state.selected_layer_idx = Some(0);

        let wt_idx = apply_loaded_bank_to_design(&mut state, Some("metallic"), 256);
        assert_eq!(wt_idx, 0);
        assert_eq!(state.selected_layer_idx, Some(0));
        let osc = &state.oscillators[0];
        assert!(osc.wave_layers[0].is_wavetable());
        assert!((osc.wave_layers[0].level - 0.9).abs() < 1e-4);
        assert_eq!(osc.wave_layers[0].wavetable_id.as_deref(), Some("metallic"));
        assert!(osc.wave_layers[1].level <= 0.22);
        assert_eq!(osc_type_from_index(osc.osc_type), "wavetable");

        let patch = patch_from_state(&state, &Patch::default_mono());
        assert_eq!(
            patch.oscillators[0].wave_layers[0].wavetable_id.as_deref(),
            Some("metallic")
        );
        assert_eq!(patch.oscillators[0].wavetable_id.as_deref(), Some("metallic"));
    }

    #[test]
    fn apply_loaded_bank_selects_existing_wt_layer() {
        let mut state = UiState::default();
        state.oscillators[0].wave_layers = vec![
            WaveLayerUi {
                source_type: "saw".into(),
                level: 0.55,
                enabled: true,
                ..WaveLayerUi::default()
            },
            WaveLayerUi {
                source_type: "wavetable".into(),
                level: 0.18,
                enabled: true,
                wt_position: 108.0,
                ..WaveLayerUi::default()
            },
        ];
        state.selected_layer_idx = Some(0);
        let wt_idx = apply_loaded_bank_to_design(&mut state, Some("formant"), 256);
        assert_eq!(wt_idx, 1);
        assert_eq!(state.selected_layer_idx, Some(1));
        assert!((state.oscillators[0].wave_layers[1].level - 0.9).abs() < 1e-4);
        assert!(state.oscillators[0].wave_layers[0].level <= 0.22);
    }

    #[test]
    fn factory_lead_wave_slot_roundtrip() {
        let original = Patch::factory_lead();
        let mut state = UiState::default();
        sync_state_from_patch(&mut state, &original);
        assert_eq!(state.oscillators[0].wave_quant, 16);
        assert_eq!(state.oscillators[0].wave_slot, 7);
        assert_eq!(state.oscillators[0].wave_slots.len(), 16);
        assert_eq!(state.oscillators[0].wave_slots[7].label, "Lead");
        let restored = patch_from_state(&state, &Patch::default_mono());
        assert_eq!(restored.oscillators[0].wave_slot, 7);
        assert_eq!(restored.oscillators[0].wave_slots.len(), 16);
        assert!((restored.oscillators[0].wave_slots[7].frame - 108.0).abs() < 0.01);
    }

    #[test]
    fn factory_va_bass_roundtrip() {
        let original = Patch::factory_va_bass();
        let mut state = UiState::default();
        sync_state_from_patch(&mut state, &original);
        let restored = patch_from_state(&state, &Patch::default_mono());
        assert_eq!(restored.name, original.name);
        assert!((restored.filter.cutoff - original.filter.cutoff).abs() < 1e-3);
        assert_eq!(restored.oscillators[0].osc_type, original.oscillators[0].osc_type);
        assert!((restored.sub_level - original.sub_level).abs() < 1e-4);
    }

    #[test]
    fn factory_fm_bell_roundtrip() {
        let original = Patch::factory_fm_bell();
        let mut state = UiState::default();
        sync_state_from_patch(&mut state, &original);
        let restored = patch_from_state(&state, &Patch::default_mono());
        assert_eq!(restored.oscillators[0].fm_source, original.oscillators[0].fm_source);
        assert!((restored.oscillators[0].fm_index - original.oscillators[0].fm_index).abs() < 1e-3);
    }

    #[test]
    fn add_oscillator_roundtrip() {
        let mut state = UiState::default();
        let before = state.oscillators.len();
        state.add_oscillator();
        state.oscillators.last_mut().unwrap().level = 0.5;
        let patch = patch_from_state(&state, &Patch::default_mono());
        assert_eq!(patch.oscillators.len(), before + 1);
        assert!((patch.oscillators.last().unwrap().level - 0.5).abs() < 1e-4);

        let mut restored = UiState::default();
        sync_state_from_patch(&mut restored, &patch);
        assert_eq!(restored.oscillators.len(), before + 1);
    }

    #[test]
    fn remove_oscillator_roundtrip() {
        let mut state = UiState::default();
        state.add_oscillator();
        state.add_oscillator();
        let count = state.oscillators.len();
        state.remove_oscillator(1);
        let patch = patch_from_state(&state, &Patch::default_mono());
        assert_eq!(patch.oscillators.len(), count - 1);
    }

    #[test]
    fn lfo_shape_roundtrip() {
        assert_eq!(lfo_shape_from_index(lfo_shape_index("triangle")), "tri");
        assert_eq!(lfo_shape_index(lfo_shape_from_index(3)), 3);
    }

    #[test]
    fn fx_bypass_roundtrip_through_patch() {
        let mut state = UiState::default();
        assert!(
            !state.fx_slots.is_empty(),
            "default UI should include FX slots"
        );
        assert!(!state.fx_slots[0].bypassed, "default chorus should be active");
        state.fx_slots[0].bypassed = true;
        let patch = patch_from_state(&state, &Patch::default_mono());
        assert_eq!(patch.effects.len(), state.fx_slots.len());
        assert!(patch.effects[0].bypassed);
    }

    #[test]
    fn fx_delay_params_roundtrip_through_patch() {
        let mut state = UiState::default();
        let delay_idx = state
            .fx_slots
            .iter()
            .position(|s| s.effect_type == reelsynth::EffectType::Delay)
            .expect("default delay slot");
        state.fx_slots[delay_idx].time_ms = 512.0;
        state.fx_slots[delay_idx].feedback = 0.4;
        state.fx_slots[delay_idx].mix = 0.75;
        let patch = patch_from_state(&state, &Patch::default_mono());
        let delay = patch
            .effects
            .iter()
            .find(|s| s.effect_type == reelsynth::EffectType::Delay)
            .expect("delay in patch");
        assert!((delay.time_ms - 512.0).abs() < 1e-3);
        assert!((delay.feedback - 0.4).abs() < 1e-4);
        assert!((delay.mix - 0.75).abs() < 1e-4);
    }

    #[test]
    fn fx_ui_patch_changes_engine_output() {
        use reelsynth::{SynthEngine, WavetableBank};

        let bank = WavetableBank::factory_saw_morph();
        let base = Patch::default_mono();

        let mut dry_state = UiState::default();
        for slot in &mut dry_state.fx_slots {
            slot.bypassed = true;
        }
        let dry_patch = patch_from_state(&dry_state, &base);
        let mut engine =
            SynthEngine::new(bank.clone(), dry_patch.clone(), 44100);
        let dry = engine.render_offline(440.0, 0.15);

        let mut wet_state = UiState::default();
        wet_state.fx_slots[0].bypassed = false;
        wet_state.fx_slots[0].mix = 1.0;
        for slot in wet_state.fx_slots.iter_mut().skip(1) {
            slot.bypassed = true;
        }
        let wet_patch = patch_from_state(&wet_state, &base);
        engine.apply_patch_hot(wet_patch);
        let wet = engine.render_offline(440.0, 0.15);

        assert_eq!(dry.len(), wet.len());
        let max_delta = dry
            .iter()
            .zip(wet.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0_f32, f32::max);
        assert!(
            max_delta > 0.001,
            "enabling chorus via UI patch path should change rendered audio (delta={max_delta})"
        );
    }

    #[test]
    fn sequence_bpm_roundtrip() {
        let mut original = Patch::default_mono();
        original.sequence.bpm = 140.0;
        let mut state = UiState::default();
        sync_state_from_patch(&mut state, &original);
        assert!((state.compose.project.bpm - 140.0).abs() < 1e-3);
        let restored = patch_from_state(&state, &Patch::default_mono());
        assert!((restored.sequence.bpm - 140.0).abs() < 1e-3);
    }
}

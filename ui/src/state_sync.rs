//! Patch ↔ `UiState` synchronization (extracted for Q&A roundtrip tests).

use reelsynth::patch::{Envelope, Oscillator, Patch};
use crate::{
    effect_slots_from_patch, effect_slots_to_patch, factory_label, fm_source_from_index,
    mod_slots_from_patch, mod_slots_to_patch, osc_type_from_index, OscillatorUi, UiState,
    warp_mode_from_index,
};
use crate::oscillator_ui::WaveLayerUi;
use crate::wt::position_from_osc_ui;

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

fn preset_category_label(patch: &Patch) -> String {
    let wt = patch
        .wavetable_id
        .as_deref()
        .unwrap_or("wavetable")
        .replace('_', " ");
    format!("Preset · Wavetable · {wt}")
}

fn osc_ui_to_patch(osc: &OscillatorUi) -> Oscillator {
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
    state.osc_tab = state.osc_tab.min(state.oscillators.len().saturating_sub(1));

    state.unison_stereo_spread = patch.unison_stereo_spread;
    state.filter_drive = patch.filter.drive;
    state.filter2_cutoff = patch.filter2.cutoff;
    state.filter2_resonance = patch.filter2.resonance;
    state.filter2_mode = filter_mode_from_type(&patch.filter2.filter_type);
    state.filter2_drive = patch.filter2.drive;

    let idx = state.active_osc_index();
    let active = &state.oscillators[idx];
    state.wt_morph_a = active.morph_a;
    state.wt_morph_b = active.morph_b;
    state.wt_morph_amount = active.morph_amount;

    state.sub_level = patch.sub_level;
    state.noise_level = patch.noise_level;
    state.filter_cutoff = patch.filter.cutoff;
    state.filter_resonance = patch.filter.resonance;
    state.filter_key_tracking = patch.filter.key_tracking;
    state.filter_mode = filter_mode_from_type(&patch.filter.filter_type);
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

pub fn patch_from_state(state: &UiState, base: &Patch) -> Patch {
    let mut patch = base.clone();
    patch.name = state.preset_name.clone();
    patch.oscillators = state.oscillators.iter().map(osc_ui_to_patch).collect();
    if patch.oscillators.is_empty() {
        patch.ensure_oscillators(1);
    }
    patch.filter.cutoff = state.filter_cutoff;
    patch.filter.resonance = state.filter_resonance;
    patch.filter.key_tracking = state.filter_key_tracking;
    patch.filter.drive = state.filter_drive;
    patch.filter.filter_type = filter_type_from_mode(state.filter_mode).into();
    patch.filter2.cutoff = state.filter2_cutoff;
    patch.filter2.resonance = state.filter2_resonance;
    patch.filter2.drive = state.filter2_drive;
    patch.filter2.filter_type = filter_type_from_mode(state.filter2_mode).into();
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
    patch
}

#[cfg(test)]
mod tests {
    use super::*;
    use reelsynth::patch::Patch;

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
    fn factory_lead_wave_stack_roundtrip() {
        let original = Patch::factory_lead();
        let mut state = UiState::default();
        sync_state_from_patch(&mut state, &original);
        assert_eq!(state.oscillators[0].wave_layers.len(), 3);
        assert_eq!(state.oscillators[0].stack_mode, "add");
        assert!((state.oscillators[0].wave_layers[0].level - 0.65).abs() < 1e-4);
        assert_eq!(state.oscillators[0].wave_layers[0].source_type, "saw");
        let restored = patch_from_state(&state, &Patch::default_mono());
        assert_eq!(restored.oscillators[0].wave_layers.len(), 3);
        assert_eq!(restored.oscillators[0].stack_mode, "add");
        assert!((restored.oscillators[0].wave_layers[2].wt_position - 108.0).abs() < 0.01);
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
        engine.set_patch(wet_patch);
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

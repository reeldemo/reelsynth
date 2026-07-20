//! Patch parsing and factory preset tests.

use super::*;

#[test]
fn parse_minimal_patch() {
    let p = Patch::from_json(r#"{"filter":{"type":"lowpass","cutoff":800}}"#).unwrap();
    assert_eq!(p.filter.cutoff, 800.0);
    assert_eq!(p.schema, SCHEMA_V2);
}

#[test]
fn v1_migration_adds_v2_fields() {
    let json = r#"{"schema":"reelsynth-preset-v1","oscillators":[{"type":"wavetable","level":1.0}]}"#;
    let p = Patch::from_json(json).unwrap();
    assert_eq!(p.schema, SCHEMA_V2);
    assert_eq!(p.filter_envelope.attack, 0.005);
    assert_eq!(p.oscillators[0].pan, 0.0);
    assert_eq!(p.filter.key_tracking, 0.5);
}

#[test]
fn wavetable_ids_dedupes() {
    let mut p = Patch::default_mono();
    p.ensure_oscillators(3);
    p.oscillators[0].wavetable_id = Some("saw_morph".into());
    p.oscillators[1].wavetable_id = Some("sine".into());
    p.oscillators[2].wavetable_id = Some("saw_morph".into());
    let ids = p.wavetable_ids();
    assert_eq!(ids, vec!["saw_morph", "sine"]);
}

#[test]
fn factory_lead_parses() {
    let p = Patch::factory_lead();
    assert_eq!(p.name, "Factory Lead");
    assert_eq!(p.wavetable_id.as_deref(), Some("saw_morph"));
    assert_eq!(p.oscillators[0].position, 108.0);
    assert_eq!(p.oscillators[0].unison, 3);
    assert!((p.oscillators[0].detune - 10.0).abs() < 0.01);
    assert_eq!(p.oscillators[0].warp_mode, "none");
    assert!((p.sub_level - 0.12).abs() < 0.01);
    assert!((p.unison_stereo_spread - 0.75).abs() < 0.01);
}

#[test]
fn factory_lead_fast_attack() {
    let p = Patch::factory_lead();
    assert!(p.envelope.attack < 0.01, "amp attack was {}", p.envelope.attack);
    assert!(
        p.filter_envelope.attack < 0.01,
        "filt attack was {}",
        p.filter_envelope.attack
    );
}

#[test]
fn factory_lead_mod_matrix_curated() {
    let p = Patch::factory_lead();
    assert_eq!(p.mod_matrix.len(), 3);
    assert!(p.mod_matrix.iter().all(|s| s.source != "step" && s.source != "rand"));
    assert!(p
        .mod_matrix
        .iter()
        .any(|s| s.source == "filt_env" && s.target == "filter_cutoff"));
}

#[test]
fn factory_lead_fx_chorus_delay_on() {
    let p = Patch::factory_lead();
    assert_eq!(p.effects.len(), 3);
    assert!(!p.effects[0].bypassed);
    assert!((p.effects[0].mix - 0.22).abs() < 0.01);
    assert!(!p.effects[1].bypassed);
    assert!((p.effects[1].time_ms - 120.0).abs() < 1.0);
    assert!((p.effects[1].mix - 0.18).abs() < 0.01);
    assert!(p.effects[2].bypassed);
}

#[test]
fn factory_lead_wave_stack() {
    let p = Patch::factory_lead();
    let layers = &p.oscillators[0].wave_layers;
    assert_eq!(layers.len(), 3);
    assert_eq!(layers[0].source_type, "saw");
    assert_eq!(layers[1].source_type, "sine");
    assert_eq!(layers[2].source_type, "wavetable");
    assert!((layers[0].level - 0.55).abs() < 0.01);
    assert!((layers[2].wt_position - 108.0).abs() < 0.01);
    assert_eq!(p.oscillators[0].stack_mode, "avg");
    assert_eq!(p.filter2.filter_type, "lowpass");
}

#[test]
fn factory_va_bass_parses() {
    let p = Patch::factory_va_bass();
    assert_eq!(p.oscillators[0].osc_type, "saw");
    assert!(p.filter.drive > 0.0);
    assert!(p.filter2.cutoff > p.filter.cutoff);
}

#[test]
fn factory_wt_lead_has_morph() {
    let p = Patch::factory_wt_lead();
    assert_eq!(p.oscillators[0].warp_mode, "sync");
    assert!(p.oscillators[0].morph_amount > 0.0);
    assert_eq!(p.oscillators[0].unison, 4);
}

#[test]
fn factory_fm_bell_has_routing() {
    let p = Patch::factory_fm_bell();
    assert_eq!(p.oscillators[0].fm_source, "osc2");
    assert!(p.oscillators[0].fm_index > 0.0);
    assert_eq!(p.oscillators[1].osc_type, "sine");
}

#[test]
fn factory_fm_pluck_has_dual_mod() {
    let p = Patch::factory_fm_pluck();
    assert_eq!(p.oscillators[0].fm_source, "osc2_osc3");
    assert!(p.envelope.sustain < 0.01);
}

#[test]
fn effects_default_has_three_slots() {
    let p = Patch::default_mono();
    assert_eq!(p.effects.len(), 3);
    assert_eq!(p.effects[0].effect_type, crate::fx::EffectType::Chorus);
    assert_eq!(p.effects[1].effect_type, crate::fx::EffectType::Delay);
    assert!(p.effects[2].bypassed);
}

#[test]
fn legacy_fx_bypass_migrates_to_effects() {
    let json = r#"{"fx_bypass":{"chorus_bypassed":true,"delay_bypassed":false,"reverb_bypassed":true}}"#;
    let p = Patch::from_json(json).unwrap();
    assert_eq!(p.effects.len(), 3);
    assert!(p.effects[0].bypassed);
    assert!(!p.effects[1].bypassed);
}

#[test]
fn v1_migration_adds_fm_fields() {
    let json = r#"{"schema":"reelsynth-preset-v1","oscillators":[{"type":"wavetable","level":1.0}]}"#;
    let p = Patch::from_json(json).unwrap();
    assert_eq!(p.oscillators[0].fm_source, "none");
    assert_eq!(p.oscillators[0].fm_ratio, 1.0);
}

#[test]
fn oscillator_wave_slot_defaults() {
    let osc = Oscillator::default_va();
    assert_eq!(osc.wave_quant, 16);
    assert_eq!(osc.wave_slot, 7);
    assert!((osc.wave_slot_fine - 0.0).abs() < f32::EPSILON);
    assert!(osc.wave_slots.is_empty());
}

#[test]
fn empty_wave_slots_auto_generate_evenly_spaced() {
    let osc = Oscillator::default_va();
    let slots = crate::wt_quant::resolved_wave_slots(&osc, 256);
    assert_eq!(slots.len(), 16);
    assert!((slots[0].frame - 0.0).abs() < 0.01);
    assert!((slots[15].frame - 255.0).abs() < 0.01);
}

#[test]
fn v1_migration_adds_wave_slot_fields() {
    let json = r#"{"schema":"reelsynth-preset-v1","oscillators":[{"type":"wavetable","level":1.0}]}"#;
    let p = Patch::from_json(json).unwrap();
    assert_eq!(p.oscillators[0].wave_quant, 16);
    assert_eq!(p.oscillators[0].wave_slot, 7);
}

#[test]
fn wave_layer_quant_interp_defaults_and_roundtrips() {
    let old: WaveLayer = serde_json::from_str(r#"{"type":"saw","level":0.5}"#).unwrap();
    assert_eq!(old.quant_interp, "hold");
    assert!(old.quant_segment_interps.is_empty());

    let layer = WaveLayer {
        source_type: "wavetable".into(),
        quant_interp: "expo".into(),
        quant_segment_interps: vec![
            "hold".to_string(),
            "linear".to_string(),
            "spline".to_string(),
        ],
        ..WaveLayer::default()
    };
    let json = serde_json::to_string(&layer).unwrap();
    let back: WaveLayer = serde_json::from_str(&json).unwrap();
    assert_eq!(back.quant_interp, "expo");
    assert_eq!(back.quant_segment_interps.len(), 3);
}

#[test]
fn missing_filters_key_uses_legacy_dual() {
    let p = Patch::from_json(r#"{"filter":{"type":"lowpass","cutoff":800}}"#).unwrap();
    assert!(p.filters.is_none());
    let slots = p.effective_filter_slots();
    assert_eq!(slots.len(), 2);
    assert_eq!(slots[0].cutoff, 800.0);
}

#[test]
fn empty_filters_array_is_bypass() {
    let p = Patch::from_json(r#"{"filters":[]}"#).unwrap();
    assert_eq!(p.filters.as_ref().map(|s| s.len()), Some(0));
    assert!(p.effective_filter_slots().is_empty());
}

#[test]
fn filter_chain_roundtrip_and_legacy_mirror() {
    let json = r#"{
        "filters":[
            {"type":"highpass","cutoff":200,"resonance":0.2},
            {"type":"lowpass","cutoff":4000,"resonance":0.3,"drive":0.1}
        ]
    }"#;
    let mut p = Patch::from_json(json).unwrap();
    assert_eq!(p.filters.as_ref().unwrap().len(), 2);
    assert_eq!(p.filter.filter_type, "highpass");
    assert_eq!(p.filter.cutoff, 200.0);
    assert_eq!(p.filter2.filter_type, "lowpass");
    assert_eq!(p.filter2.cutoff, 4000.0);
    p.filters.as_mut().unwrap().swap(0, 1);
    p.sync_legacy_filters_from_chain();
    assert_eq!(p.filter.filter_type, "lowpass");
    assert_eq!(p.filter2.filter_type, "highpass");
}

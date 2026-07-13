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

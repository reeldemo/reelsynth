//! Patch schema parsing (reelsynth-preset-v2 with v1 migration).

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SCHEMA_V1: &str = "reelsynth-preset-v1";
pub const SCHEMA_V2: &str = "reelsynth-preset-v2";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Oscillator {
    #[serde(default = "default_wt_type", rename = "type")]
    pub osc_type: String,
    #[serde(default = "one")]
    pub level: f32,
    #[serde(default)]
    pub position: f32,
    #[serde(default)]
    pub detune: f32,
    #[serde(default = "default_unison")]
    pub unison: u32,
    #[serde(default)]
    pub pan: f32,
    #[serde(default)]
    pub wavetable_id: Option<String>,
    /// Square/pulse duty cycle (0.05..0.95).
    #[serde(default = "default_pulse_width")]
    pub pulse_width: f32,
    /// Morph endpoint A (frame index).
    #[serde(default)]
    pub morph_a: f32,
    /// Morph endpoint B (frame index).
    #[serde(default = "default_morph_b")]
    pub morph_b: f32,
    /// Morph blend 0..1 between A and B.
    #[serde(default)]
    pub morph_amount: f32,
    /// Wavetable warp: none | sync | bend.
    #[serde(default = "default_warp_none")]
    pub warp_mode: String,
    #[serde(default)]
    pub warp_amount: f32,
    /// FM modulator source: none | osc2 | osc3 | osc2_osc3 | feedback.
    #[serde(default = "default_fm_none")]
    pub fm_source: String,
    /// Modulator frequency ratio relative to carrier (0.5..16).
    #[serde(default = "default_fm_ratio")]
    pub fm_ratio: f32,
    /// FM modulation depth (0..10).
    #[serde(default)]
    pub fm_index: f32,
}

impl Oscillator {
    pub fn default_va() -> Self {
        Self {
            osc_type: "saw".into(),
            level: 0.0,
            position: 0.0,
            detune: 0.0,
            unison: 1,
            pan: 0.0,
            wavetable_id: None,
            pulse_width: default_pulse_width(),
            morph_a: 0.0,
            morph_b: default_morph_b(),
            morph_amount: 0.0,
            warp_mode: default_warp_none(),
            warp_amount: 0.0,
            fm_source: default_fm_none(),
            fm_ratio: default_fm_ratio(),
            fm_index: 0.0,
        }
    }
}

fn default_pulse_width() -> f32 {
    0.5
}
fn default_morph_b() -> f32 {
    255.0
}
fn default_warp_none() -> String {
    "none".into()
}
fn default_fm_none() -> String {
    "none".into()
}
fn default_fm_ratio() -> f32 {
    1.0
}

fn default_wt_type() -> String {
    "wavetable".into()
}
fn one() -> f32 {
    1.0
}
fn default_unison() -> u32 {
    1
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Filter {
    #[serde(default = "default_lp", rename = "type")]
    pub filter_type: String,
    #[serde(default = "default_cutoff")]
    pub cutoff: f32,
    #[serde(default)]
    pub resonance: f32,
    /// 0 = no tracking, 1 = cutoff follows pitch 1:1 in semitones.
    #[serde(default = "default_key_tracking")]
    pub key_tracking: f32,
    /// Soft tanh drive before the SVF (0..1).
    #[serde(default)]
    pub drive: f32,
}

fn default_lp() -> String {
    "lowpass".into()
}
fn default_cutoff() -> f32 {
    1200.0
}
fn default_key_tracking() -> f32 {
    0.5
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Envelope {
    #[serde(default = "default_attack")]
    pub attack: f32,
    #[serde(default = "default_decay")]
    pub decay: f32,
    #[serde(default = "default_sustain")]
    pub sustain: f32,
    #[serde(default = "default_release")]
    pub release: f32,
}

fn default_attack() -> f32 {
    0.01
}
fn default_decay() -> f32 {
    0.2
}
fn default_sustain() -> f32 {
    0.6
}
fn default_release() -> f32 {
    0.4
}

impl Default for Envelope {
    fn default() -> Self {
        Self {
            attack: default_attack(),
            decay: default_decay(),
            sustain: default_sustain(),
            release: default_release(),
        }
    }
}

fn default_filter_envelope() -> Envelope {
    Envelope {
        attack: 0.005,
        decay: 0.35,
        sustain: 0.2,
        release: 0.5,
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Lfo {
    #[serde(default = "default_lfo_rate")]
    pub rate: f32,
    #[serde(default)]
    pub depth: f32,
    #[serde(default = "default_lfo_target")]
    pub target: String,
    /// Waveform: sine | tri | saw | sh
    #[serde(default = "default_lfo_shape")]
    pub shape: String,
}

fn default_lfo_shape() -> String {
    "sine".into()
}

fn default_lfo_rate() -> f32 {
    0.5
}
fn default_lfo_target() -> String {
    "wt_position".into()
}

impl Default for Lfo {
    fn default() -> Self {
        Self {
            rate: default_lfo_rate(),
            depth: 0.0,
            target: default_lfo_target(),
            shape: default_lfo_shape(),
        }
    }
}

/// Macro knob with direct destination routing (S6).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Macro {
    #[serde(default = "default_macro_value")]
    pub value: f32,
    #[serde(default = "default_macro_target")]
    pub target: String,
    #[serde(default = "default_macro_amount")]
    pub amount: f32,
}

fn default_macro_value() -> f32 {
    0.5
}
fn default_macro_target() -> String {
    "filter_cutoff".into()
}
fn default_macro_amount() -> f32 {
    0.5
}

impl Default for Macro {
    fn default() -> Self {
        Self {
            value: default_macro_value(),
            target: default_macro_target(),
            amount: default_macro_amount(),
        }
    }
}

pub fn default_macros() -> Vec<Macro> {
    vec![
        Macro {
            target: "filter_cutoff".into(),
            amount: 0.6,
            ..Macro::default()
        },
        Macro {
            target: "osc1_position".into(),
            amount: 0.5,
            ..Macro::default()
        },
        Macro {
            target: "osc1_fm_index".into(),
            amount: 0.4,
            ..Macro::default()
        },
        Macro {
            target: "filter_resonance".into(),
            amount: 0.35,
            ..Macro::default()
        },
    ]
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModSlot {
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub amount: f32,
    /// When false the route is ignored by the engine (S6 UI On/Off).
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Patch {
    #[serde(default)]
    pub schema: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub wavetable_id: Option<String>,
    #[serde(default)]
    pub oscillators: Vec<Oscillator>,
    #[serde(default)]
    pub filter: Filter,
    /// Second parallel filter for stereo sculpting.
    #[serde(default = "default_filter2")]
    pub filter2: Filter,
    #[serde(default)]
    pub envelope: Envelope,
    #[serde(default = "default_filter_envelope")]
    pub filter_envelope: Envelope,
    #[serde(default)]
    pub lfo: Lfo,
    #[serde(default)]
    pub lfo2: Lfo,
    #[serde(default = "default_macros")]
    pub macros: Vec<Macro>,
    #[serde(default)]
    pub mod_matrix: Vec<ModSlot>,
    #[serde(default)]
    pub effects: Vec<crate::fx::EffectSlot>,
    /// Legacy field — migrated into `effects` on load.
    #[serde(default, skip_serializing)]
    pub fx_bypass: crate::fx::FxBypass,
    #[serde(default)]
    pub sub_level: f32,
    #[serde(default)]
    pub noise_level: f32,
    /// Unison voice pan spread (0 = mono, 1 = full L/R).
    #[serde(default = "default_unison_spread")]
    pub unison_stereo_spread: f32,
}

fn default_filter2() -> Filter {
    Filter {
        filter_type: default_lp(),
        cutoff: 2400.0,
        resonance: 0.25,
        key_tracking: default_key_tracking(),
        drive: 0.0,
    }
}

fn default_unison_spread() -> f32 {
    0.7
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            filter_type: default_lp(),
            cutoff: default_cutoff(),
            resonance: 0.3,
            key_tracking: default_key_tracking(),
            drive: 0.0,
        }
    }
}

impl Patch {
    pub fn from_json(json: &str) -> Result<Self, String> {
        let mut v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        migrate_v1_to_v2(&mut v);
        // Accept "type" alias for oscillator/filter
        if let Some(arr) = v.get_mut("oscillators").and_then(|a| a.as_array_mut()) {
            for osc in arr {
                let ty = osc.get("type").and_then(|t| t.as_str()).map(str::to_string);
                if let Some(t) = ty {
                    osc.as_object_mut()
                        .unwrap()
                        .entry("osc_type")
                        .or_insert(Value::String(t));
                }
            }
        }
        if let Some(f) = v.get_mut("filter") {
            let ty = f.get("type").and_then(|t| t.as_str()).map(str::to_string);
            if let Some(t) = ty {
                f.as_object_mut()
                    .unwrap()
                    .entry("filter_type")
                    .or_insert(Value::String(t));
            }
        }
        let mut patch: Patch = serde_json::from_value(v).map_err(|e| e.to_string())?;
        if patch.schema.is_empty() || patch.schema == SCHEMA_V1 {
            patch.schema = SCHEMA_V2.into();
        }
        migrate_fx_bypass(&mut patch);
        Ok(patch)
    }

    pub fn to_json(&self) -> Result<String, String> {
        let mut patch = self.clone();
        patch.schema = SCHEMA_V2.into();
        serde_json::to_string_pretty(&patch).map_err(|e| e.to_string())
    }

    pub fn default_mono() -> Self {
        Self {
            schema: SCHEMA_V2.into(),
            name: "default".into(),
            wavetable_id: Some("saw_morph".into()),
            oscillators: vec![Oscillator {
                osc_type: "wavetable".into(),
                level: 1.0,
                position: 0.0,
                detune: 0.0,
                unison: 1,
                pan: 0.0,
                wavetable_id: None,
                pulse_width: default_pulse_width(),
                morph_a: 0.0,
                morph_b: default_morph_b(),
                morph_amount: 0.0,
                warp_mode: default_warp_none(),
                warp_amount: 0.0,
                fm_source: default_fm_none(),
                fm_ratio: default_fm_ratio(),
                fm_index: 0.0,
            }],
            filter: Filter::default(),
            filter2: default_filter2(),
            envelope: Envelope::default(),
            filter_envelope: default_filter_envelope(),
            lfo: Lfo::default(),
            lfo2: Lfo::default(),
            macros: default_macros(),
            mod_matrix: vec![],
            effects: crate::fx::default_effects(),
            fx_bypass: crate::fx::FxBypass::default(),
            sub_level: 0.0,
            noise_level: 0.0,
            unison_stereo_spread: default_unison_spread(),
        }
    }

    /// Warm subtractive bass: dual VA saws + filter envelope.
    pub fn factory_va_bass() -> Self {
        Self {
            schema: SCHEMA_V2.into(),
            name: "VA Bass".into(),
            wavetable_id: Some("saw_morph".into()),
            oscillators: vec![
                Oscillator {
                    osc_type: "saw".into(),
                    level: 0.85,
                    detune: -7.0,
                    unison: 2,
                    ..Oscillator::default_va()
                },
                Oscillator {
                    osc_type: "saw".into(),
                    level: 0.55,
                    detune: 7.0,
                    unison: 1,
                    ..Oscillator::default_va()
                },
                Oscillator {
                    osc_type: "square".into(),
                    level: 0.15,
                    pulse_width: 0.25,
                    unison: 1,
                    ..Oscillator::default_va()
                },
            ],
            filter: Filter {
                cutoff: 180.0,
                resonance: 0.45,
                key_tracking: 0.35,
                drive: 0.35,
                ..Filter::default()
            },
            filter2: Filter {
                cutoff: 420.0,
                resonance: 0.3,
                filter_type: "lowpass".into(),
                key_tracking: 0.2,
                drive: 0.2,
            },
            filter_envelope: Envelope {
                attack: 0.003,
                decay: 0.45,
                sustain: 0.15,
                release: 0.35,
            },
            envelope: Envelope {
                attack: 0.005,
                decay: 0.25,
                sustain: 0.75,
                release: 0.2,
            },
            sub_level: 0.35,
            unison_stereo_spread: 0.85,
            ..Self::default_mono()
        }
    }

    /// WT lead with morph + unison spread.
    pub fn factory_wt_lead() -> Self {
        Self {
            schema: SCHEMA_V2.into(),
            name: "WT Lead".into(),
            wavetable_id: Some("saw_morph".into()),
            oscillators: vec![Oscillator {
                osc_type: "wavetable".into(),
                level: 0.9,
                position: 64.0,
                morph_a: 0.0,
                morph_b: 180.0,
                morph_amount: 0.55,
                warp_mode: "sync".into(),
                warp_amount: 0.35,
                unison: 4,
                detune: 12.0,
                pan: 0.0,
                wavetable_id: Some("saw_morph".into()),
                pulse_width: default_pulse_width(),
                fm_source: default_fm_none(),
                fm_ratio: default_fm_ratio(),
                fm_index: 0.0,
            }],
            filter: Filter {
                cutoff: 2800.0,
                resonance: 0.55,
                key_tracking: 0.65,
                drive: 0.15,
                ..Filter::default()
            },
            filter2: Filter {
                cutoff: 5200.0,
                resonance: 0.35,
                filter_type: "highpass".into(),
                key_tracking: 0.4,
                drive: 0.0,
            },
            filter_envelope: Envelope {
                attack: 0.08,
                decay: 0.5,
                sustain: 0.35,
                release: 0.6,
            },
            envelope: Envelope {
                attack: 0.02,
                decay: 0.35,
                sustain: 0.65,
                release: 0.45,
            },
            unison_stereo_spread: 1.0,
            ..Self::default_mono()
        }
    }

    /// FM bell: WT carrier + VA sine modulator (2→1).
    pub fn factory_fm_bell() -> Self {
        Self {
            schema: SCHEMA_V2.into(),
            name: "FM Bell".into(),
            wavetable_id: Some("sine".into()),
            oscillators: vec![
                Oscillator {
                    osc_type: "wavetable".into(),
                    level: 0.9,
                    position: 32.0,
                    fm_source: "osc2".into(),
                    fm_ratio: 3.5,
                    fm_index: 4.5,
                    wavetable_id: Some("sine".into()),
                    ..Oscillator::default_va()
                },
                Oscillator {
                    osc_type: "sine".into(),
                    level: 0.0,
                    fm_ratio: 1.0,
                    ..Oscillator::default_va()
                },
                Oscillator {
                    level: 0.0,
                    ..Oscillator::default_va()
                },
            ],
            filter: Filter {
                cutoff: 4200.0,
                resonance: 0.35,
                key_tracking: 0.75,
                ..Filter::default()
            },
            filter2: Filter {
                cutoff: 6800.0,
                resonance: 0.2,
                filter_type: "highpass".into(),
                key_tracking: 0.5,
                ..Filter::default()
            },
            envelope: Envelope {
                attack: 0.002,
                decay: 1.2,
                sustain: 0.05,
                release: 1.8,
            },
            filter_envelope: Envelope {
                attack: 0.001,
                decay: 0.8,
                sustain: 0.1,
                release: 1.5,
            },
            lfo: Lfo {
                rate: 0.35,
                depth: 0.15,
                target: "osc1_fm_index".into(),
                shape: default_lfo_shape(),
            },
            mod_matrix: vec![ModSlot {
                source: "lfo1".into(),
                target: "osc1_fm_index".into(),
                amount: 0.35,
                enabled: true,
            }],
            ..Self::default_mono()
        }
    }

    /// FM pluck: WT carrier with dual-mod algorithm preset (2+3→1).
    pub fn factory_fm_pluck() -> Self {
        Self {
            schema: SCHEMA_V2.into(),
            name: "FM Pluck".into(),
            wavetable_id: Some("metallic".into()),
            oscillators: vec![
                Oscillator {
                    osc_type: "wavetable".into(),
                    level: 0.85,
                    position: 48.0,
                    fm_source: "osc2_osc3".into(),
                    fm_ratio: 2.0,
                    fm_index: 3.2,
                    wavetable_id: Some("metallic".into()),
                    ..Oscillator::default_va()
                },
                Oscillator {
                    osc_type: "sine".into(),
                    level: 0.0,
                    fm_ratio: 1.0,
                    ..Oscillator::default_va()
                },
                Oscillator {
                    osc_type: "triangle".into(),
                    level: 0.0,
                    detune: 7.0,
                    ..Oscillator::default_va()
                },
            ],
            filter: Filter {
                cutoff: 3200.0,
                resonance: 0.5,
                key_tracking: 0.85,
                ..Filter::default()
            },
            filter2: Filter {
                cutoff: 5200.0,
                resonance: 0.3,
                filter_type: "bandpass".into(),
                key_tracking: 0.6,
                ..Filter::default()
            },
            envelope: Envelope {
                attack: 0.001,
                decay: 0.35,
                sustain: 0.0,
                release: 0.45,
            },
            filter_envelope: Envelope {
                attack: 0.001,
                decay: 0.25,
                sustain: 0.0,
                release: 0.35,
            },
            ..Self::default_mono()
        }
    }

    /// Ensure at least `count` wavetable oscillators (S3 tri-osc UI).
    pub fn ensure_oscillators(&mut self, count: usize) {
        while self.oscillators.len() < count {
            self.oscillators.push(Oscillator {
                osc_type: "wavetable".into(),
                level: 0.0,
                position: 0.0,
                detune: 0.0,
                unison: 1,
                pan: 0.0,
                wavetable_id: None,
                pulse_width: default_pulse_width(),
                morph_a: 0.0,
                morph_b: default_morph_b(),
                morph_amount: 0.0,
                warp_mode: default_warp_none(),
                warp_amount: 0.0,
                fm_source: default_fm_none(),
                fm_ratio: default_fm_ratio(),
                fm_index: 0.0,
            });
        }
        if let Some(first) = self.oscillators.first_mut() {
            if first.level <= 0.0 {
                first.level = 1.0;
            }
        }
    }

    /// Unique wavetable IDs referenced by oscillators (deduped, order preserved).
    pub fn wavetable_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        for osc in &self.oscillators {
            if let Some(id) = osc.wavetable_id.as_deref() {
                if !ids.iter().any(|existing: &String| existing == id) {
                    ids.push(id.to_string());
                }
            }
        }
        if ids.is_empty() {
            if let Some(id) = &self.wavetable_id {
                ids.push(id.clone());
            }
        }
        ids
    }
}

fn migrate_fx_bypass(patch: &mut Patch) {
    if patch.effects.is_empty() {
        patch.effects = crate::fx::effects_from_bypass(&patch.fx_bypass);
    }
}

fn migrate_v1_to_v2(v: &mut Value) {
    let obj = match v.as_object_mut() {
        Some(o) => o,
        None => return,
    };

    let schema = obj
        .get("schema")
        .and_then(|s| s.as_str())
        .unwrap_or(SCHEMA_V1);
    let is_v1 = schema.is_empty() || schema == SCHEMA_V1;

    if is_v1 {
        obj.insert("schema".into(), Value::String(SCHEMA_V2.into()));
    }

    if !obj.contains_key("filter_envelope") {
        obj.insert(
            "filter_envelope".into(),
            serde_json::to_value(default_filter_envelope()).unwrap(),
        );
    }

    if let Some(arr) = obj.get_mut("oscillators").and_then(|a| a.as_array_mut()) {
        for osc in arr {
            if let Some(o) = osc.as_object_mut() {
                o.entry("pan").or_insert(Value::Number(0.into()));
            }
        }
    }

    if let Some(f) = obj.get_mut("filter").and_then(|f| f.as_object_mut()) {
        f.entry("key_tracking")
            .or_insert(Value::Number(serde_json::Number::from_f64(0.5).unwrap()));
        f.entry("drive").or_insert(Value::Number(0.into()));
    }

    if !obj.contains_key("filter2") {
        obj.insert(
            "filter2".into(),
            serde_json::to_value(default_filter2()).unwrap(),
        );
    }

    obj.entry("unison_stereo_spread")
        .or_insert(Value::Number(serde_json::Number::from_f64(0.7).unwrap()));

    if !obj.contains_key("lfo2") {
        obj.insert("lfo2".into(), serde_json::to_value(Lfo::default()).unwrap());
    }
    if !obj.contains_key("macros") {
        obj.insert(
            "macros".into(),
            serde_json::to_value(default_macros()).unwrap(),
        );
    }

    if let Some(lfo) = obj.get_mut("lfo").and_then(|l| l.as_object_mut()) {
        lfo.entry("shape")
            .or_insert(Value::String("sine".into()));
    }
    if let Some(lfo) = obj.get_mut("lfo2").and_then(|l| l.as_object_mut()) {
        lfo.entry("shape")
            .or_insert(Value::String("sine".into()));
    }

    if let Some(arr) = obj.get_mut("oscillators").and_then(|a| a.as_array_mut()) {
        for osc in arr {
            if let Some(o) = osc.as_object_mut() {
                o.entry("pulse_width")
                    .or_insert(Value::Number(serde_json::Number::from_f64(0.5).unwrap()));
                o.entry("morph_a").or_insert(Value::Number(0.into()));
                o.entry("morph_b")
                    .or_insert(Value::Number(serde_json::Number::from_f64(255.0).unwrap()));
                o.entry("morph_amount").or_insert(Value::Number(0.into()));
                o.entry("warp_mode")
                    .or_insert(Value::String("none".into()));
                o.entry("warp_amount").or_insert(Value::Number(0.into()));
                o.entry("fm_source")
                    .or_insert(Value::String("none".into()));
                o.entry("fm_ratio")
                    .or_insert(Value::Number(serde_json::Number::from_f64(1.0).unwrap()));
                o.entry("fm_index").or_insert(Value::Number(0.into()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
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
}

//! Patch schema parsing (reelsynth-preset-v2 with v1 migration).

mod factory;
mod migrate;
mod schema;

pub use schema::*;

use crate::performance::PerformanceSettings;
use crate::SequenceProject;
use serde_json::Value;

use migrate::{migrate_fx_bypass, migrate_v1_to_v2};

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
                wave_quant: default_wave_quant(),
                wave_slot: default_wave_slot(),
                wave_slot_fine: 0.0,
                wave_slots: Vec::new(),
                wave_layers: Vec::new(),
                stack_mode: default_stack_mode(),
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
            performance: PerformanceSettings::default(),
            sequence: SequenceProject::default_template(),
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
                wave_quant: default_wave_quant(),
                wave_slot: default_wave_slot(),
                wave_slot_fine: 0.0,
                wave_slots: Vec::new(),
                wave_layers: Vec::new(),
                stack_mode: default_stack_mode(),
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
            for layer in &osc.wave_layers {
                if let Some(id) = layer.wavetable_id.as_deref() {
                    if !ids.iter().any(|existing: &String| existing == id) {
                        ids.push(id.to_string());
                    }
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

#[cfg(test)]
mod tests;

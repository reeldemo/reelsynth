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
        if let Some(f) = v.get_mut("filter2") {
            let ty = f.get("type").and_then(|t| t.as_str()).map(str::to_string);
            if let Some(t) = ty {
                f.as_object_mut()
                    .unwrap()
                    .entry("filter_type")
                    .or_insert(Value::String(t));
            }
        }
        if let Some(arr) = v.get_mut("filters").and_then(|a| a.as_array_mut()) {
            for slot in arr {
                let ty = slot.get("type").and_then(|t| t.as_str()).map(str::to_string);
                if let Some(t) = ty {
                    if let Some(obj) = slot.as_object_mut() {
                        obj.entry("filter_type").or_insert(Value::String(t));
                    }
                }
            }
        }
        let mut patch: Patch = serde_json::from_value(v).map_err(|e| e.to_string())?;
        if patch.schema.is_empty() || patch.schema == SCHEMA_V1 {
            patch.schema = SCHEMA_V2.into();
        }
        migrate_fx_bypass(&mut patch);
        patch.sync_legacy_filters_from_chain();
        Ok(patch)
    }

    pub fn to_json(&self) -> Result<String, String> {
        let mut patch = self.clone();
        patch.schema = SCHEMA_V2.into();
        patch.sync_legacy_filters_from_chain();
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
            filters: None,
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
            crackle: 0.0,
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

    /// Effective musical filter chain for DSP.
    /// `None` → legacy dual (`filter` then `filter2`). `Some([])` → bypass.
    pub fn effective_filter_slots(&self) -> std::borrow::Cow<'_, [FilterSlot]> {
        match &self.filters {
            Some(slots) => std::borrow::Cow::Borrowed(slots.as_slice()),
            None => std::borrow::Cow::Owned(legacy_filter_slots(&self.filter, &self.filter2)),
        }
    }

    /// Mirror chain slots 0/1 into legacy `filter` / `filter2` for modulators and older tools.
    pub fn sync_legacy_filters_from_chain(&mut self) {
        let Some(slots) = &self.filters else {
            return;
        };
        if let Some(s0) = slots.first() {
            self.filter = s0.to_filter();
        }
        if let Some(s1) = slots.get(1) {
            self.filter2 = s1.to_filter();
        } else if slots.is_empty() {
            // Keep last legacy values; chain bypass does not wipe them.
        } else {
            // Single-slot chain: keep filter2 as a mild default twin of slot 0.
            self.filter2 = self.filter.clone();
            self.filter2.cutoff = (self.filter.cutoff * 1.5).min(12000.0);
        }
    }

    /// Keep `filters[0]` in sync when legacy cutoff/res setters or smoothers run.
    pub fn sync_chain_slot0_from_legacy(&mut self) {
        if let Some(slots) = &mut self.filters {
            if let Some(slot) = slots.first_mut() {
                slot.cutoff = self.filter.cutoff;
                slot.resonance = self.filter.resonance;
                slot.key_tracking = self.filter.key_tracking;
                slot.drive = self.filter.drive;
                slot.filter_type = self.filter.filter_type.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests;

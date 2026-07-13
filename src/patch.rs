//! Patch schema parsing (reelsynth-preset-v1).

use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    pub wavetable_id: Option<String>,
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
}

fn default_lp() -> String {
    "lowpass".into()
}
fn default_cutoff() -> f32 {
    1200.0
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Lfo {
    #[serde(default = "default_lfo_rate")]
    pub rate: f32,
    #[serde(default)]
    pub depth: f32,
    #[serde(default = "default_lfo_target")]
    pub target: String,
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
        }
    }
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
    #[serde(default)]
    pub envelope: Envelope,
    #[serde(default)]
    pub lfo: Lfo,
    #[serde(default)]
    pub mod_matrix: Vec<ModSlot>,
    #[serde(default)]
    pub fx_bypass: crate::fx::FxBypass,
    #[serde(default)]
    pub sub_level: f32,
    #[serde(default)]
    pub noise_level: f32,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            filter_type: default_lp(),
            cutoff: default_cutoff(),
            resonance: 0.3,
        }
    }
}

impl Patch {
    pub fn from_json(json: &str) -> Result<Self, String> {
        let mut v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
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
        serde_json::from_value(v).map_err(|e| e.to_string())
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    pub fn default_mono() -> Self {
        Self {
            schema: "reelsynth-preset-v1".into(),
            name: "default".into(),
            wavetable_id: Some("saw_morph".into()),
            oscillators: vec![Oscillator {
                osc_type: "wavetable".into(),
                level: 1.0,
                position: 0.0,
                detune: 0.0,
                unison: 1,
                wavetable_id: None,
            }],
            filter: Filter::default(),
            envelope: Envelope::default(),
            lfo: Lfo::default(),
            mod_matrix: vec![],
            fx_bypass: crate::fx::FxBypass::default(),
            sub_level: 0.0,
            noise_level: 0.0,
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
                wavetable_id: None,
            });
        }
        if let Some(first) = self.oscillators.first_mut() {
            if first.level <= 0.0 {
                first.level = 1.0;
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
    }
}

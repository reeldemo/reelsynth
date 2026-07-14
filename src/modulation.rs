//! Mod matrix evaluation — sources → target offsets.

use crate::patch::{Macro, ModSlot};
use std::collections::HashMap;

/// Runtime modulation sources for one voice sample.
#[derive(Clone, Copy, Debug, Default)]
pub struct ModSources {
    pub lfo1: f32,
    pub lfo2: f32,
    pub amp_env: f32,
    pub filt_env: f32,
    pub velocity: f32,
    pub modwheel: f32,
    pub aftertouch: f32,
    pub pressure: f32,
    pub timbre: f32,
    pub pitch_bend: f32,
    pub step: f32,
    pub rand: f32,
    pub macros: [f32; 4],
}

impl ModSources {
    pub fn source_value(&self, name: &str) -> f32 {
        match name {
            "lfo1" | "lfo" => self.lfo1,
            "lfo2" => self.lfo2,
            "env1" | "env" => self.amp_env,
            "env2" | "filt_env" => self.filt_env,
            "velocity" | "vel" => self.velocity,
            "modwheel" => self.modwheel,
            "aftertouch" | "after" => self.aftertouch.max(self.pressure),
            "pressure" => self.pressure,
            "timbre" | "cc74" => self.timbre,
            "pitch_bend" | "pitch" => self.pitch_bend,
            "step" => self.step,
            "rand" => self.rand,
            "macro1" | "m1" => self.macros[0],
            "macro2" | "m2" => self.macros[1],
            "macro3" | "m3" => self.macros[2],
            "macro4" | "m4" => self.macros[3],
            other if other.starts_with("macro") => {
                if let Some(idx) = other.strip_prefix("macro").and_then(|s| s.parse::<usize>().ok()) {
                    self.macros.get(idx.saturating_sub(1)).copied().unwrap_or(0.0)
                } else {
                    0.0
                }
            }
            _ => 0.0,
        }
    }
}

/// Target scaling: amount is normalized -1..1 from UI, scaled per target type.
pub fn apply_target_scale(target: &str, src: f32, amount: f32) -> f32 {
    let raw = src * amount;
    if target.ends_with("_wave_slot") {
        raw * 4.0
    } else if target.ends_with("_position") {
        raw * 64.0
    } else if target.ends_with("_fm_index") {
        raw * 5.0
    } else if target.ends_with("_detune") {
        raw * 1200.0
    } else if target.ends_with("_level") || target == "amp" {
        raw * 0.5
    } else if target.ends_with("_pan") {
        raw * 0.5
    } else if target == "filter_cutoff" {
        raw * 4000.0
    } else if target == "filter_resonance" {
        raw * 0.5
    } else {
        raw
    }
}

/// Sum enabled mod-matrix routes into per-target offsets.
pub fn compute_mods(slots: &[ModSlot], sources: &ModSources) -> HashMap<String, f32> {
    let mut out = HashMap::new();
    for slot in slots {
        if !slot.enabled {
            continue;
        }
        let src = sources.source_value(&slot.source);
        let delta = apply_target_scale(&slot.target, src, slot.amount);
        *out.entry(slot.target.clone()).or_insert(0.0) += delta;
    }
    out
}

/// Direct macro knob routing (target + amount stored per macro).
pub fn compute_macro_mods(macros: &[Macro]) -> HashMap<String, f32> {
    let mut out = HashMap::new();
    for mac in macros {
        if mac.target.is_empty() || mac.amount.abs() < 1e-6 {
            continue;
        }
        let centered = (mac.value - 0.5) * 2.0;
        let delta = apply_target_scale(&mac.target, centered, mac.amount);
        *out.entry(mac.target.clone()).or_insert(0.0) += delta;
    }
    out
}

/// Merge matrix and macro modulation maps.
pub fn merge_mods(
    matrix: HashMap<String, f32>,
    macros: HashMap<String, f32>,
) -> HashMap<String, f32> {
    let mut out = matrix;
    for (k, v) in macros {
        *out.entry(k).or_insert(0.0) += v;
    }
    out
}

/// Apply modulation offsets to a patch (automation + runtime overrides).
pub fn apply_mods_to_patch(patch: &mut crate::patch::Patch, mods: &HashMap<String, f32>) {
    for (target, delta) in mods {
        if target == "filter_cutoff" {
            patch.filter.cutoff = (patch.filter.cutoff + delta).max(25.0);
        } else if target == "filter_resonance" {
            patch.filter.resonance = (patch.filter.resonance + delta).clamp(0.0, 0.95);
        } else if let Some(rest) = target.strip_prefix("osc") {
            if let Some((idx_str, param)) = rest.split_once('_') {
                if let Ok(idx) = idx_str.parse::<usize>() {
                    if let Some(osc) = patch.oscillators.get_mut(idx.saturating_sub(1)) {
                        match param {
                            "position" => osc.position = (osc.position + delta).clamp(0.0, 255.0),
                            "level" => osc.level = (osc.level + delta).clamp(0.0, 1.0),
                            "fm_index" => osc.fm_index = (osc.fm_index + delta).clamp(0.0, 10.0),
                            "detune" => osc.detune += delta,
                            "pan" => osc.pan = (osc.pan + delta).clamp(-1.0, 1.0),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::{Macro, ModSlot};

    #[test]
    fn lfo2_routes_to_cutoff() {
        let sources = ModSources {
            lfo2: 1.0,
            ..Default::default()
        };
        let slots = vec![ModSlot {
            source: "lfo2".into(),
            target: "filter_cutoff".into(),
            amount: 0.5,
            enabled: true,
        }];
        let mods = compute_mods(&slots, &sources);
        assert!(mods.get("filter_cutoff").copied().unwrap_or(0.0) > 100.0);
    }

    #[test]
    fn macro_routes_to_position() {
        let macros = vec![Macro {
            value: 1.0,
            target: "osc1_position".into(),
            amount: 0.5,
        }];
        let mods = compute_macro_mods(&macros);
        assert!(mods.get("osc1_position").copied().unwrap_or(0.0) > 0.0);
    }

    #[test]
    fn wave_slot_mod_scales_by_four() {
        let sources = ModSources {
            lfo1: 1.0,
            ..Default::default()
        };
        let slots = vec![ModSlot {
            source: "lfo1".into(),
            target: "osc1_wave_slot".into(),
            amount: 0.5,
            enabled: true,
        }];
        let mods = compute_mods(&slots, &sources);
        assert!((mods.get("osc1_wave_slot").copied().unwrap_or(0.0) - 2.0).abs() < 0.01);
    }

    #[test]
    fn position_mod_still_fine_frames() {
        let sources = ModSources {
            lfo1: 1.0,
            ..Default::default()
        };
        let slots = vec![ModSlot {
            source: "lfo1".into(),
            target: "osc1_position".into(),
            amount: 0.5,
            enabled: true,
        }];
        let mods = compute_mods(&slots, &sources);
        assert!((mods.get("osc1_position").copied().unwrap_or(0.0) - 32.0).abs() < 0.01);
    }
}

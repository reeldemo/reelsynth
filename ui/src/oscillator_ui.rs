//! Per-oscillator UI state (unlimited count).

use reelsynth::patch::{Oscillator, WaveLayer, WaveSlot};

use crate::osc_column::{fm_algorithm_index, fm_source_index, osc_type_index, warp_mode_index};
use crate::quant_interp::{
    fill_segment_interps, resize_segment_interps, segment_count, WtQuantInterp,
};

pub const MIN_OSCILLATORS: usize = 1;

#[derive(Debug, Clone)]
pub struct WaveLayerUi {
    pub source_type: String,
    pub level: f32,
    pub detune: f32,
    pub wt_position: f32,
    pub pulse_width: f32,
    pub phase: f32,
    pub enabled: bool,
    pub invert: bool,
    /// Optional bank id for this layer (falls back to patch/osc primary bank).
    pub wavetable_id: Option<String>,
    /// UI-only: auto-created layer that absorbs Result quant edits.
    pub residual: bool,
    /// Curve-wide default interp; All·… writes this into every segment.
    pub quant_interp: WtQuantInterp,
    /// Per-segment modes (`len = max(0, slot_count−1)`). Segment `i` is knob `i → i+1`.
    pub quant_segment_interps: Vec<WtQuantInterp>,
}

impl Default for WaveLayerUi {
    fn default() -> Self {
        Self {
            source_type: "saw".into(),
            level: 1.0,
            detune: 0.0,
            wt_position: 0.0,
            pulse_width: 0.5,
            phase: 0.0,
            enabled: true,
            invert: false,
            wavetable_id: None,
            residual: false,
            quant_interp: WtQuantInterp::default(),
            quant_segment_interps: Vec::new(),
        }
    }
}

/// Default additive stack for empty oscillators (saw + sine + square).
pub fn default_wave_layers() -> Vec<WaveLayerUi> {
    vec![
        WaveLayerUi {
            source_type: "saw".into(),
            level: 0.5,
            ..WaveLayerUi::default()
        },
        WaveLayerUi {
            source_type: "sine".into(),
            level: 0.35,
            ..WaveLayerUi::default()
        },
        WaveLayerUi {
            source_type: "square".into(),
            level: 0.25,
            ..WaveLayerUi::default()
        },
    ]
}

/// Ensure at least three stack layers so Design home is never blank.
pub fn ensure_wave_layers(osc: &mut OscillatorUi) {
    if osc.wave_layers.is_empty() {
        osc.wave_layers = default_wave_layers();
    }
}

impl WaveLayerUi {
    pub fn is_wavetable(&self) -> bool {
        self.source_type.eq_ignore_ascii_case("wavetable")
    }

    pub fn is_va(&self) -> bool {
        !self.is_wavetable()
    }

    pub fn from_patch(layer: &WaveLayer) -> Self {
        Self {
            source_type: layer.source_type.clone(),
            level: layer.level,
            detune: layer.detune,
            wt_position: layer.wt_position,
            pulse_width: layer.pulse_width,
            phase: layer.phase,
            enabled: layer.level > 0.0,
            invert: layer.invert,
            wavetable_id: layer.wavetable_id.clone(),
            residual: false,
            quant_interp: WtQuantInterp::from_patch_str(&layer.quant_interp),
            quant_segment_interps: layer
                .quant_segment_interps
                .iter()
                .map(|s| WtQuantInterp::from_patch_str(s))
                .collect(),
        }
    }

    pub fn to_patch(&self) -> WaveLayer {
        WaveLayer {
            source_type: self.source_type.clone(),
            level: if self.enabled { self.level } else { 0.0 },
            detune: self.detune,
            wt_position: self.wt_position,
            pulse_width: self.pulse_width,
            phase: self.phase,
            wavetable_id: self.wavetable_id.clone(),
            invert: self.invert,
            quant_interp: self.quant_interp.to_patch_str().into(),
            quant_segment_interps: self
                .quant_segment_interps
                .iter()
                .map(|m| m.to_patch_str().into())
                .collect(),
        }
    }

    /// Ensure `quant_segment_interps.len() == max(0, slot_count−1)`.
    pub fn ensure_segment_interps(&mut self, slot_count: usize) {
        if self.quant_segment_interps.is_empty() && segment_count(slot_count) > 0 {
            fill_segment_interps(
                &mut self.quant_segment_interps,
                slot_count,
                self.quant_interp,
            );
        } else {
            resize_segment_interps(
                &mut self.quant_segment_interps,
                slot_count,
                self.quant_interp,
            );
        }
    }

    /// Set curve default and write it onto every segment.
    pub fn apply_curve_interp_to_segments(&mut self, slot_count: usize, mode: WtQuantInterp) {
        self.quant_interp = mode;
        fill_segment_interps(&mut self.quant_segment_interps, slot_count, mode);
    }
}

#[derive(Debug, Clone)]
pub struct OscillatorUi {
    pub osc_type: usize,
    pub level: f32,
    pub pan: f32,
    pub coarse: f32,
    pub unison: u32,
    pub position: f32,
    pub pulse_width: f32,
    pub warp_mode: usize,
    pub warp_amount: f32,
    pub fm_source: usize,
    pub fm_algorithm: usize,
    pub fm_ratio: f32,
    pub fm_index: f32,
    pub morph_a: f32,
    pub morph_b: f32,
    pub morph_amount: f32,
    pub wave_quant: u8,
    pub wave_slot: u8,
    pub wave_slot_fine: f32,
    pub wave_slots: Vec<WaveSlot>,
    pub wave_layers: Vec<WaveLayerUi>,
    pub stack_mode: String,
}

impl Default for OscillatorUi {
    fn default() -> Self {
        Self::new_silent()
    }
}

impl OscillatorUi {
    pub fn new_silent() -> Self {
        Self {
            osc_type: 0,
            level: 0.0,
            pan: 0.0,
            coarse: 0.0,
            unison: 1,
            position: 0.0,
            pulse_width: 0.5,
            warp_mode: 0,
            warp_amount: 0.0,
            fm_source: 0,
            fm_algorithm: 0,
            fm_ratio: 1.0,
            fm_index: 0.0,
            morph_a: 0.0,
            morph_b: 255.0,
            morph_amount: 0.0,
            wave_quant: 16,
            wave_slot: 7,
            wave_slot_fine: 0.0,
            wave_slots: Vec::new(),
            wave_layers: Vec::new(),
            stack_mode: "add".into(),
        }
    }

    pub fn new_active() -> Self {
        Self {
            level: 0.85,
            unison: 3,
            position: 108.0,
            wave_quant: 16,
            wave_slot: 7,
            wave_layers: default_wave_layers(),
            ..Self::new_silent()
        }
    }

    pub fn effective_wave_quant(&self) -> u8 {
        if self.wave_quant == 255 {
            return 255;
        }
        if self.wave_quant > 0 {
            self.wave_quant
        } else if !self.wave_slots.is_empty() {
            self.wave_slots.len().min(255) as u8
        } else {
            16
        }
    }

    pub fn from_patch(osc: &Oscillator) -> Self {
        let mut out = Self {
            osc_type: osc_type_index(&osc.osc_type),
            level: osc.level,
            pan: osc.pan,
            coarse: osc.detune,
            unison: osc.unison,
            position: osc.position,
            pulse_width: osc.pulse_width,
            warp_mode: warp_mode_index(&osc.warp_mode),
            warp_amount: osc.warp_amount,
            fm_source: fm_source_index(&osc.fm_source),
            fm_algorithm: fm_algorithm_index(&osc.fm_source),
            fm_ratio: osc.fm_ratio,
            fm_index: osc.fm_index,
            morph_a: osc.morph_a,
            morph_b: osc.morph_b,
            morph_amount: osc.morph_amount,
            wave_quant: osc.wave_quant,
            wave_slot: osc.wave_slot,
            wave_slot_fine: osc.wave_slot_fine,
            wave_slots: osc.wave_slots.clone(),
            wave_layers: osc.wave_layers.iter().map(WaveLayerUi::from_patch).collect(),
            stack_mode: osc.stack_mode.clone(),
        };
        out.ensure_layer_segment_interps();
        out
    }

    /// Resize every layer's segment interp vec to match current Quant count.
    pub fn ensure_layer_segment_interps(&mut self) {
        let slots = if self.wave_quant == 255 {
            256
        } else if self.wave_quant > 0 {
            self.wave_quant as usize
        } else {
            0
        };
        for layer in &mut self.wave_layers {
            layer.ensure_segment_interps(slots);
        }
    }
}

#[cfg(test)]
mod layer_interp_tests {
    use super::*;

    #[test]
    fn ui_layer_interp_roundtrips_through_patch() {
        let mut layer = WaveLayerUi {
            quant_interp: WtQuantInterp::Exponential,
            quant_segment_interps: vec![
                WtQuantInterp::Hold,
                WtQuantInterp::Linear,
                WtQuantInterp::Spline,
            ],
            ..WaveLayerUi::default()
        };
        layer.ensure_segment_interps(4);
        assert_eq!(layer.quant_segment_interps.len(), 3);
        let patch = layer.to_patch();
        let back = WaveLayerUi::from_patch(&patch);
        assert_eq!(back.quant_interp, WtQuantInterp::Exponential);
        assert_eq!(back.quant_segment_interps.len(), 3);
        assert_eq!(back.quant_segment_interps[1], WtQuantInterp::Linear);
    }

    #[test]
    fn apply_curve_default_fills_segments() {
        let mut layer = WaveLayerUi::default();
        layer.apply_curve_interp_to_segments(8, WtQuantInterp::Polynomial);
        assert_eq!(layer.quant_interp, WtQuantInterp::Polynomial);
        assert_eq!(layer.quant_segment_interps.len(), 7);
        assert!(layer
            .quant_segment_interps
            .iter()
            .all(|&m| m == WtQuantInterp::Polynomial));
    }
}

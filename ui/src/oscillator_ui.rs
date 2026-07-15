//! Per-oscillator UI state (unlimited count).

use reelsynth::patch::{Oscillator, WaveLayer, WaveSlot};

use crate::osc_column::{fm_algorithm_index, fm_source_index, osc_type_index, warp_mode_index};

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
        }
    }
}

impl WaveLayerUi {
    pub fn from_patch(layer: &WaveLayer) -> Self {
        Self {
            source_type: layer.source_type.clone(),
            level: layer.level,
            detune: layer.detune,
            wt_position: layer.wt_position,
            pulse_width: layer.pulse_width,
            phase: layer.phase,
            enabled: layer.level > 0.0,
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
            wavetable_id: None,
        }
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
        Self {
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
        }
    }
}

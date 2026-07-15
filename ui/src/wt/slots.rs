//! Wave-slot helpers — bridge UI oscillator state and engine slot resolution.

use reelsynth::patch::{Oscillator, WaveSlot};
use reelsynth::{resolve_wt_position, resolved_wave_slots};

use crate::oscillator_ui::OscillatorUi;

pub const WAVE_QUANT_LABELS: [&str; 6] = ["8", "16", "32", "64", "256", "Smooth"];

pub fn wave_quant_index(quant: u8) -> usize {
    match quant {
        8 => 0,
        16 => 1,
        32 => 2,
        64 => 3,
        255 => 4, // wire encoding for 256 quant
        _ => 5,
    }
}

pub fn wave_quant_from_index(idx: usize) -> u8 {
    match idx {
        0 => 8,
        1 => 16,
        2 => 32,
        3 => 64,
        4 => 255, // 256 quant (u8 wire value)
        _ => 0,
    }
}

/// Resolve UI/engine quant count (255 wire value → 256 slots).
pub fn effective_quant_count(quant: u8) -> usize {
    if quant == 255 {
        256
    } else if quant > 0 {
        quant as usize
    } else {
        0
    }
}

pub fn osc_ui_to_slot_osc(osc: &OscillatorUi) -> Oscillator {
    Oscillator {
        osc_type: "wavetable".into(),
        position: osc.position,
        morph_a: osc.morph_a,
        morph_b: osc.morph_b,
        morph_amount: osc.morph_amount,
        wave_quant: osc.wave_quant,
        wave_slot: osc.wave_slot,
        wave_slot_fine: osc.wave_slot_fine,
        wave_slots: osc.wave_slots.clone(),
        ..Oscillator::default_va()
    }
}

pub fn resolved_slots_for_ui(osc: &OscillatorUi, num_frames: usize) -> Vec<WaveSlot> {
    resolved_wave_slots(&osc_ui_to_slot_osc(osc), num_frames)
}

pub fn position_from_osc_ui(osc: &OscillatorUi, num_frames: usize) -> f32 {
    resolve_wt_position(&osc_ui_to_slot_osc(osc), 0.0, 0.0, num_frames)
}

pub fn apply_slot_selection(osc: &mut OscillatorUi, slot: u8, num_frames: usize) {
    let max_slot = effective_quant_count(osc.effective_wave_quant()).saturating_sub(1) as u8;
    osc.wave_slot = slot.min(max_slot);
    osc.wave_slot_fine = 0.0;
    osc.position = position_from_osc_ui(osc, num_frames);
}

pub fn sync_slot_from_position(osc: &mut OscillatorUi, num_frames: usize) {
    if osc.wave_quant == 0 {
        return;
    }
    let slots = resolved_slots_for_ui(osc, num_frames);
    if slots.is_empty() {
        return;
    }
    let pos = osc.position;
    let mut best_idx = 0usize;
    let mut best_dist = f32::MAX;
    for (i, slot) in slots.iter().enumerate() {
        let d = (slot.frame - pos).abs();
        if d < best_dist {
            best_dist = d;
            best_idx = i;
        }
    }
    osc.wave_slot = best_idx as u8;
    if best_idx + 1 < slots.len() {
        let a = slots[best_idx].frame;
        let b = slots[best_idx + 1].frame;
        if (b - a).abs() > f32::EPSILON {
            osc.wave_slot_fine = ((pos - a) / (b - a)).clamp(0.0, 1.0);
        } else {
            osc.wave_slot_fine = 0.0;
        }
    } else {
        osc.wave_slot_fine = 0.0;
    }
}

pub fn frame_to_slot_coord(slots: &[WaveSlot], frame: f32) -> f32 {
    if slots.is_empty() {
        return 0.0;
    }
    let mut best_idx = 0usize;
    let mut best_dist = f32::MAX;
    for (i, slot) in slots.iter().enumerate() {
        let d = (slot.frame - frame).abs();
        if d < best_dist {
            best_dist = d;
            best_idx = i;
        }
    }
    best_idx as f32
}

pub fn slot_coord_to_frame(slots: &[WaveSlot], slot_coord: f32) -> f32 {
    if slots.is_empty() {
        return 0.0;
    }
    let max_idx = (slots.len().saturating_sub(1)) as f32;
    let clamped = slot_coord.clamp(0.0, max_idx);
    let idx = clamped.floor() as usize;
    let frac = clamped - idx as f32;
    if idx >= slots.len().saturating_sub(1) {
        slots[idx.min(slots.len() - 1)].frame
    } else {
        let a = slots[idx].frame;
        let b = slots[idx + 1].frame;
        a + (b - a) * frac
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quant_index_roundtrip() {
        for (q, idx) in [(8, 0), (16, 1), (32, 2), (64, 3), (255, 4), (0, 5)] {
            assert_eq!(wave_quant_index(q), idx);
            assert_eq!(wave_quant_from_index(idx), q);
        }
    }

    #[test]
    fn slot_selection_sets_position() {
        let mut osc = OscillatorUi::new_active();
        osc.wave_quant = 16;
        apply_slot_selection(&mut osc, 0, 256);
        assert_eq!(osc.wave_slot, 0);
        assert!((osc.position - 0.0).abs() < 0.01);
        apply_slot_selection(&mut osc, 7, 256);
        assert_eq!(osc.wave_slot, 7);
        assert!(osc.position > 100.0);
    }
}

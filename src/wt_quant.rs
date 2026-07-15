//! Wavetable slot quantization — resolve position from slot map + modulation.

use crate::patch::{Oscillator, WaveSlot};

/// Evenly spaced slot frames across `0..max_frame`.
pub fn generate_even_wave_slots(quant: usize, num_frames: usize) -> Vec<WaveSlot> {
    let quant = quant.max(1);
    let max_frame = (num_frames.saturating_sub(1)).max(1) as f32;
    (0..quant)
        .map(|i| {
            let frame = if quant <= 1 {
                0.0
            } else {
                i as f32 * max_frame / (quant as f32 - 1.0)
            };
            WaveSlot {
                frame,
                label: format!("{i}"),
            }
        })
        .collect()
}

/// Map wire `wave_quant` to slot count (255 → 256).
pub fn quant_slot_count(wave_quant: u8) -> usize {
    if wave_quant == 255 {
        256
    } else {
        wave_quant as usize
    }
}

/// Resolve slot map, auto-generating evenly spaced entries when empty.
pub fn resolved_wave_slots(osc: &Oscillator, num_frames: usize) -> Vec<WaveSlot> {
    if !osc.wave_slots.is_empty() {
        osc.wave_slots.clone()
    } else {
        generate_even_wave_slots(quant_slot_count(osc.effective_wave_quant()), num_frames)
    }
}

/// Interpolate frame index from a continuous slot coordinate.
fn slot_to_frame(slots: &[WaveSlot], slot_pos: f32) -> f32 {
    if slots.is_empty() {
        return 0.0;
    }
    let max_idx = (slots.len().saturating_sub(1)) as f32;
    let clamped = slot_pos.clamp(0.0, max_idx);
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

/// Resolve WT frame position from oscillator slots, fine frame mod, and slot mod.
///
/// When `wave_quant == 0` (smooth / legacy), uses `position` or morph endpoints
/// with frame-level modulation only. Otherwise maps `wave_slot` + fine + slot mod
/// through the slot table and adds fine frame mod (`oscN_position`).
pub fn resolve_wt_position(
    osc: &Oscillator,
    pos_mod: f32,
    slot_mod: f32,
    num_frames: usize,
) -> f32 {
    let max_pos = (num_frames.saturating_sub(1)).max(1) as f32;

    if osc.morph_amount > 0.0 {
        let morph_pos =
            osc.morph_a + (osc.morph_b - osc.morph_a) * osc.morph_amount.clamp(0.0, 1.0);
        return (morph_pos + pos_mod).clamp(0.0, max_pos);
    }

    if osc.wave_quant == 0 {
        return (osc.position + pos_mod + slot_mod).clamp(0.0, max_pos);
    }

    let slots = resolved_wave_slots(osc, num_frames);
    let slot_pos = osc.wave_slot as f32 + slot_mod + osc.wave_slot_fine.clamp(0.0, 1.0);
    let base = slot_to_frame(&slots, slot_pos);
    (base + pos_mod).clamp(0.0, max_pos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::Oscillator;

    fn test_osc() -> Oscillator {
        Oscillator {
            osc_type: "wavetable".into(),
            level: 1.0,
            position: 108.0,
            ..Oscillator::default_va()
        }
    }

    #[test]
    fn even_slots_256_span_bank() {
        let slots = generate_even_wave_slots(256, 256);
        assert_eq!(slots.len(), 256);
        assert!((slots[0].frame - 0.0).abs() < 0.01);
        assert!((slots[255].frame - 255.0).abs() < 0.01);
    }

    #[test]
    fn even_slots_span_bank() {
        let slots = generate_even_wave_slots(16, 256);
        assert_eq!(slots.len(), 16);
        assert!((slots[0].frame - 0.0).abs() < 0.01);
        assert!((slots[15].frame - 255.0).abs() < 0.01);
        assert!((slots[7].frame - 119.0).abs() < 0.5);
    }

    #[test]
    fn auto_generate_when_slots_empty() {
        let osc = test_osc();
        let slots = resolved_wave_slots(&osc, 256);
        assert_eq!(slots.len(), 16);
    }

    #[test]
    fn resolve_slot_zero_is_first_frame() {
        let mut osc = test_osc();
        osc.wave_slot = 0;
        osc.wave_slot_fine = 0.0;
        let pos = resolve_wt_position(&osc, 0.0, 0.0, 256);
        assert!((pos - 0.0).abs() < 0.01);
    }

    #[test]
    fn resolve_slot_mod_steps() {
        let mut osc = test_osc();
        osc.wave_slot = 0;
        osc.wave_slot_fine = 0.0;
        let pos = resolve_wt_position(&osc, 0.0, 1.0, 256);
        let slots = generate_even_wave_slots(16, 256);
        assert!((pos - slots[1].frame).abs() < 0.01);
    }

    #[test]
    fn resolve_fine_lerp_between_slots() {
        let mut osc = test_osc();
        osc.wave_slot = 0;
        osc.wave_slot_fine = 0.5;
        let pos = resolve_wt_position(&osc, 0.0, 0.0, 256);
        let slots = generate_even_wave_slots(16, 256);
        let expected = slots[0].frame + (slots[1].frame - slots[0].frame) * 0.5;
        assert!((pos - expected).abs() < 0.01);
    }

    #[test]
    fn resolve_pos_mod_adds_fine_frames() {
        let mut osc = test_osc();
        osc.wave_slot = 0;
        osc.wave_slot_fine = 0.0;
        let pos = resolve_wt_position(&osc, 4.0, 0.0, 256);
        assert!((pos - 4.0).abs() < 0.01);
    }

    #[test]
    fn smooth_mode_uses_position() {
        let mut osc = test_osc();
        osc.wave_quant = 0;
        osc.position = 108.0;
        let pos = resolve_wt_position(&osc, 0.0, 0.0, 256);
        assert!((pos - 108.0).abs() < 0.01);
    }

    #[test]
    fn morph_overrides_slots() {
        let mut osc = test_osc();
        osc.morph_a = 10.0;
        osc.morph_b = 200.0;
        osc.morph_amount = 0.5;
        let pos = resolve_wt_position(&osc, 0.0, 5.0, 256);
        assert!((pos - 105.0).abs() < 0.01);
    }
}

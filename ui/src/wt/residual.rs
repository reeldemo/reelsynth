//! Residual wavetable layer — solves stack sum toward a desired Result curve.

use reelsynth::WavetableBank;

use crate::oscillator_ui::WaveLayerUi;

use super::quant_handles::quant_control_points;
use super::slots::effective_quant_count;
use super::view_3d_stack::{composite_stack_sample, layer_quant_display_scale};

/// Signed stack sum of all layers except `skip_idx` at `phase`.
pub fn others_signed_sum_at_phase(
    layers: &[WaveLayerUi],
    bank: &WavetableBank,
    stack_mode: &str,
    phase: f32,
    skip_idx: usize,
) -> f32 {
    let mut partial: Vec<WaveLayerUi> = Vec::new();
    for (i, layer) in layers.iter().enumerate() {
        if i == skip_idx {
            continue;
        }
        if layer.enabled && layer.level > 0.0 {
            partial.push(layer.clone());
        }
    }
    composite_stack_sample(&partial, bank, stack_mode, phase, 0.0)
}

/// Result composite amplitude at each quant slot (phase knots).
pub fn composite_quant_points(
    layers: &[WaveLayerUi],
    bank: &WavetableBank,
    stack_mode: &str,
    slot_count: usize,
) -> Vec<f32> {
    (0..slot_count)
        .map(|slot| {
            let phase = quant_phase(slot, slot_count);
            composite_stack_sample(layers, bank, stack_mode, phase, 0.0)
        })
        .collect()
}

/// `residual_sample[i] = (desired[i] - others[i]) / residual_scale`, clamped.
pub fn residual_samples_from_desired(
    desired: &[f32],
    others_sum: &[f32],
    residual_scale: f32,
) -> Vec<f32> {
    let scale = if residual_scale.abs() < 1e-6 {
        1.0
    } else {
        residual_scale
    };
    let n = desired.len().min(others_sum.len());
    (0..n)
        .map(|i| ((desired[i] - others_sum[i]) / scale).clamp(-1.0, 1.0))
        .collect()
}

/// User-facing layer type — maps empty / legacy `none` to a sensible default.
pub fn layer_type_display(source_type: &str) -> String {
    let s = source_type.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("none") {
        "layer".to_string()
    } else {
        s.to_string()
    }
}

/// Label for a layer curve in the Layers pane.
pub fn layer_curve_label(i: usize, layer: &WaveLayerUi) -> String {
    let ty = if layer.residual {
        "residual".to_string()
    } else {
        layer_type_display(&layer.source_type)
    };
    format!("L{} · {}", i + 1, ty)
}

pub fn find_residual_layer_idx(layers: &[WaveLayerUi]) -> Option<usize> {
    layers.iter().position(|l| l.residual)
}

/// Append a residual wavetable layer once; force `stack_mode = add` when needed.
/// Returns `(layer_index, stack_mode_changed)`.
pub fn ensure_residual_layer(
    layers: &mut Vec<WaveLayerUi>,
    stack_mode: &mut String,
    wavetable_id: Option<String>,
) -> (usize, bool) {
    let mut mode_changed = false;
    if stack_mode != "add" {
        *stack_mode = "add".into();
        mode_changed = true;
    }
    if let Some(idx) = find_residual_layer_idx(layers) {
        return (idx, mode_changed);
    }
    let idx = layers.len();
    layers.push(WaveLayerUi {
        source_type: "wavetable".into(),
        level: 1.0,
        enabled: true,
        residual: true,
        wavetable_id,
        ..WaveLayerUi::default()
    });
    (idx, mode_changed)
}

/// Others sum at quant phases excluding the residual layer.
pub fn others_sum_at_quant_phases(
    layers: &[WaveLayerUi],
    bank: &WavetableBank,
    stack_mode: &str,
    residual_idx: usize,
    slot_count: usize,
) -> Vec<f32> {
    (0..slot_count)
        .map(|slot| {
            let phase = quant_phase(slot, slot_count);
            others_signed_sum_at_phase(layers, bank, stack_mode, phase, residual_idx)
        })
        .collect()
}

/// Residual raw frame samples from desired Result quant knobs.
pub fn residual_frame_from_desired(
    desired: &[f32],
    layers: &[WaveLayerUi],
    bank: &WavetableBank,
    stack_mode: &str,
    residual_idx: usize,
) -> Vec<f32> {
    let layer = &layers[residual_idx];
    let scale = layer_quant_display_scale(layer);
    let others = others_sum_at_quant_phases(layers, bank, stack_mode, residual_idx, desired.len());
    residual_samples_from_desired(desired, &others, scale)
}

/// Residual frame quant amplitudes (scaled for Result knob Y).
#[allow(dead_code)]
pub fn residual_quant_display_points(
    layers: &[WaveLayerUi],
    bank: &WavetableBank,
    residual_idx: usize,
    frame_idx: usize,
    wave_quant: u8,
) -> Vec<f32> {
    let slot_count = effective_quant_count(wave_quant).max(1);
    let frame = bank.frame(frame_idx.min(bank.num_frames.saturating_sub(1)));
    let scale = layer_quant_display_scale(&layers[residual_idx]);
    quant_control_points(frame, slot_count)
        .into_iter()
        .map(|raw| raw * scale)
        .collect()
}

fn quant_phase(slot: usize, slot_count: usize) -> f32 {
    if slot_count <= 1 {
        0.0
    } else {
        slot as f32 / (slot_count - 1) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reelsynth::WavetableBank;

    #[test]
    fn residual_identity_when_desired_equals_others() {
        let desired = vec![0.5, 0.0, -0.5];
        let others = vec![0.5, 0.0, -0.5];
        let out = residual_samples_from_desired(&desired, &others, 1.0);
        assert_eq!(out.len(), 3);
        for v in out {
            assert!(v.abs() < 1e-5, "expected ~0 got {v}");
        }
    }

    #[test]
    fn residual_simple_offset() {
        let desired = vec![1.0, 1.0];
        let others = vec![0.25, 0.25];
        let out = residual_samples_from_desired(&desired, &others, 1.0);
        assert!((out[0] - 0.75).abs() < 1e-5);
        assert!((out[1] - 0.75).abs() < 1e-5);
    }

    #[test]
    fn residual_clamps_to_unit() {
        let desired = vec![2.0];
        let others = vec![0.0];
        let out = residual_samples_from_desired(&desired, &others, 0.5);
        assert!((out[0] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn residual_respects_scale_and_invert() {
        let desired = vec![0.0];
        let others = vec![0.4];
        let out = residual_samples_from_desired(&desired, &others, -0.5);
        assert!((out[0] - 0.8).abs() < 1e-4, "got {}", out[0]);
    }

    #[test]
    fn ensure_residual_layer_creates_once() {
        let mut layers = vec![WaveLayerUi {
            source_type: "saw".into(),
            level: 0.5,
            enabled: true,
            ..WaveLayerUi::default()
        }];
        let mut mode = "avg".into();
        let (a, changed) = ensure_residual_layer(&mut layers, &mut mode, Some("bank".into()));
        assert!(changed);
        assert_eq!(mode, "add");
        assert_eq!(layers.len(), 2);
        assert!(layers[a].residual);
        assert!(layers[a].is_wavetable());

        let (b, changed2) = ensure_residual_layer(&mut layers, &mut mode, None);
        assert!(!changed2);
        assert_eq!(a, b);
        assert_eq!(layers.len(), 2);
    }

    #[test]
    fn layer_curve_label_formats_type() {
        let layer = WaveLayerUi {
            source_type: "saw".into(),
            ..WaveLayerUi::default()
        };
        assert_eq!(layer_curve_label(0, &layer), "L1 · saw");
        let residual = WaveLayerUi {
            source_type: "wavetable".into(),
            residual: true,
            ..WaveLayerUi::default()
        };
        assert_eq!(layer_curve_label(2, &residual), "L3 · residual");
        let unset = WaveLayerUi {
            source_type: "none".into(),
            ..WaveLayerUi::default()
        };
        assert_eq!(layer_curve_label(0, &unset), "L1 · layer");
    }

    #[test]
    fn others_sum_excludes_residual() {
        let bank = WavetableBank::factory_saw_morph();
        let layers = vec![
            WaveLayerUi {
                source_type: "sine".into(),
                level: 0.5,
                enabled: true,
                ..WaveLayerUi::default()
            },
            WaveLayerUi {
                source_type: "wavetable".into(),
                level: 1.0,
                enabled: true,
                residual: true,
                ..WaveLayerUi::default()
            },
        ];
        let sum = others_signed_sum_at_phase(&layers, &bank, "add", 0.25, 1);
        let sine_only = composite_stack_sample(
            std::slice::from_ref(&layers[0]),
            &bank,
            "add",
            0.25,
            0.0,
        );
        assert!(
            (sum - sine_only).abs() < 1e-4,
            "others should match non-residual layers only: {sum} vs {sine_only}"
        );
    }
}

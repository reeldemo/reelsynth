//! UI preview of mod-matrix position modulation for WT views.

use reelsynth::patch::Patch;
use reelsynth::{compute_macro_mods, compute_mods, lfo_value, merge_mods, LfoRuntime, ModSources};

const POSITION_TARGET: &str = "osc1_position";

/// Build runtime mod sources for animated WT preview (no active voice).
pub fn preview_mod_sources(patch: &Patch, time: f32, macro_values: &[f32; 4]) -> ModSources {
    let mut lfo1_rt = LfoRuntime::default();
    let mut lfo2_rt = LfoRuntime::default();
    ModSources {
        lfo1: lfo_value(&patch.lfo, time, &mut lfo1_rt),
        lfo2: lfo_value(&patch.lfo2, time, &mut lfo2_rt),
        velocity: 1.0,
        modwheel: 0.0,
        macros: *macro_values,
        ..Default::default()
    }
}

/// Matrix + macro offset for `osc1_position` (same scaling as the voice engine).
pub fn preview_position_mod(
    patch: &Patch,
    sources: &ModSources,
    macro_values: &[f32; 4],
) -> f32 {
    let matrix = compute_mods(&patch.mod_matrix, sources);
    let mut macros = patch.macros.clone();
    for (i, value) in macro_values.iter().enumerate().take(4) {
        if let Some(mac) = macros.get_mut(i) {
            mac.value = *value;
        }
    }
    let macro_mods = compute_macro_mods(&macros);
    merge_mods(matrix, macro_mods)
        .get(POSITION_TARGET)
        .copied()
        .unwrap_or(0.0)
}

/// Whether the patch has active routes that modulate WT position (for repaint).
#[cfg_attr(not(test), allow(dead_code))]
pub fn has_position_mod_routes(patch: &Patch) -> bool {
    patch.mod_matrix.iter().any(|slot| {
        slot.enabled
            && slot.target == POSITION_TARGET
            && slot.amount.abs() > 1e-6
    }) || patch.macros.iter().any(|mac| {
        mac.target == POSITION_TARGET && mac.amount.abs() > 1e-6
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use reelsynth::patch::{Macro, ModSlot, Patch};

    #[test]
    fn lfo1_routes_to_position_preview() {
        let patch = Patch {
            lfo: reelsynth::patch::Lfo {
                rate: 1.0,
                depth: 1.0,
                ..Default::default()
            },
            mod_matrix: vec![ModSlot {
                source: "lfo1".into(),
                target: POSITION_TARGET.into(),
                amount: 0.5,
                enabled: true,
            }],
            macros: vec![],
            ..Patch::default_mono()
        };
        let sources = preview_mod_sources(&patch, 0.25, &[0.5; 4]);
        let pos_mod = preview_position_mod(&patch, &sources, &[0.5; 4]);
        assert!(pos_mod.abs() > 0.0);
    }

    #[test]
    fn macro_knob_routes_to_position_preview() {
        let patch = Patch {
            macros: vec![Macro {
                value: 1.0,
                target: POSITION_TARGET.into(),
                amount: 0.5,
            }],
            ..Patch::default_mono()
        };
        let sources = preview_mod_sources(&patch, 0.0, &[1.0, 0.5, 0.5, 0.5]);
        let pos_mod = preview_position_mod(&patch, &sources, &[1.0, 0.5, 0.5, 0.5]);
        assert!(pos_mod > 0.0);
    }

    #[test]
    fn has_position_mod_routes_detects_matrix_and_macro() {
        let matrix_patch = Patch {
            mod_matrix: vec![ModSlot {
                source: "lfo2".into(),
                target: POSITION_TARGET.into(),
                amount: 0.25,
                enabled: true,
            }],
            ..Patch::default_mono()
        };
        assert!(has_position_mod_routes(&matrix_patch));

        let macro_patch = Patch {
            macros: vec![Macro {
                value: 0.5,
                target: POSITION_TARGET.into(),
                amount: 0.3,
            }],
            ..Patch::default_mono()
        };
        assert!(has_position_mod_routes(&macro_patch));

        let no_routes = Patch {
            mod_matrix: vec![],
            macros: vec![Macro {
                target: "filter_cutoff".into(),
                amount: 0.0,
                ..Macro::default()
            }],
            ..Patch::default_mono()
        };
        assert!(!has_position_mod_routes(&no_routes));
    }
}

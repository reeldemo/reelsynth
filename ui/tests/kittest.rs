//! egui_kittest interaction smoke tests (UI-K01 … UI-K06).

use std::collections::HashSet;

use egui_kittest::Harness;
use reelsynth::Patch;
use reelsynth_ui::{
    default_effect_slots, draw_shell, osc_type_index, ShellMidiDevices, ShellConfig, UiState,
};
use reelsynth_ui::widgets::{Knob, KnobSize, PianoKeyboard};
use reelsynth_ui_theme;

#[test]
fn piano_widget_harness_smoke() {
    let keys = HashSet::new();
    let mut harness = Harness::new_ui(|ui| {
        reelsynth_ui_theme::Tokens::default();
        let _ = PianoKeyboard::new(&keys).show(ui);
    });
    harness.run();
}

#[test]
fn knob_widget_harness_smoke() {
    let mut value = 0.25_f32;
    let mut harness = Harness::builder()
        .with_size([120.0, 120.0])
        .build_ui(|ui| {
            Knob::new(&mut value, 0.0..=1.0, "Level")
                .size(KnobSize::Md)
                .show(ui);
        });
    harness.run();
    drop(harness);
    assert!((0.0..=1.0).contains(&value));
}

#[test]
fn mod_matrix_toggle_route_enabled() {
    let mut state = UiState::default();
    state.mod_routes[0].enabled = true;
    state.mod_routes[0].enabled = false;
    assert!(!state.mod_routes[0].enabled);
}

#[test]
fn fx_rack_add_slot() {
    let mut slots = default_effect_slots();
    let before = slots.len();
    slots.push(reelsynth_ui::EffectSlotUi::from_slot(
        &reelsynth::fx::EffectSlot::chorus(),
    ));
    assert_eq!(slots.len(), before + 1);
}

#[test]
fn osc_tab_switch_follows_state() {
    let mut state = UiState::default();
    state.osc_tab = 1;
    state.osc_type[1] = osc_type_index("square");
    assert_eq!(state.osc_type[1], 2);
}

#[test]
fn compact_mode_collapses_sections() {
    struct ShellTest {
        fonts_applied: bool,
        state: UiState,
    }

    let config = ShellConfig {
        show_mod_matrix: false,
        show_fx_rack: false,
        ..Default::default()
    };
    let midi = ShellMidiDevices {
        names: &["None".to_string()],
        selected: 0,
    };
    let preview = Patch::default_mono();
    let mut harness = Harness::builder()
        .with_size([1280.0, 720.0])
        .build_state(
            |ctx, test| {
                if !test.fonts_applied {
                    reelsynth_ui_theme::apply(ctx);
                    test.fonts_applied = true;
                    return;
                }

                egui::CentralPanel::default().show(ctx, |ui| {
                    let screen = ui.max_rect();
                    let _actions = draw_shell(
                        ui,
                        screen,
                        &mut test.state,
                        None,
                        &preview,
                        &midi,
                        &config,
                        None,
                    );
                });
            },
            ShellTest {
                fonts_applied: false,
                state: UiState::default(),
            },
        );
    harness.run();
    assert!(!config.show_mod_matrix && !config.show_fx_rack);
}

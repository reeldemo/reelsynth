//! ReelSynth standalone app entry point.

mod app;
mod audio_commands;
mod audio_host;
mod midi_host;
mod midi_input;

use app::ReelSynthApp;
use audio_host::start_audio;
use crossbeam_channel;
use eframe::egui;
use midi_host::MidiDevices;
use reelsynth::engine::MidiEvent;
use reelsynth_ui::{APP_HEIGHT_FULL, APP_MIN_HEIGHT, APP_MIN_WIDTH};
use std::sync::Arc;

fn main() -> eframe::Result<()> {
    let midi_devices = MidiDevices::enumerate();
    let (midi_event_tx, midi_event_rx) = crossbeam_channel::unbounded::<MidiEvent>();

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([APP_MIN_WIDTH, APP_HEIGHT_FULL])
        .with_min_inner_size([APP_MIN_WIDTH, APP_MIN_HEIGHT])
        .with_title("ReelSynth");

    // cpal (WASAPI) and winit both touch COM on Windows; disable OLE drag-and-drop
    // so audio init after the window is created stays compatible with cpal.
    #[cfg(windows)]
    {
        viewport = viewport.with_drag_and_drop(false);
    }

    eframe::run_native(
        "ReelSynth",
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(move |cc| {
            reelsynth_ui_theme::apply(&cc.egui_ctx);

            let audio = match start_audio(44100) {
                Ok(a) => Some(Arc::new(a)),
                Err(e) => {
                    eprintln!("audio init failed: {e}");
                    None
                }
            };

            Ok(Box::new(ReelSynthApp::new(
                audio,
                midi_devices,
                midi_event_tx,
                midi_event_rx,
            )))
        }),
    )
}

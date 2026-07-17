//! ReelSynth standalone app entry point.

mod app;
mod app_settings;
mod audio_commands;
mod audio_devices;
mod audio_host;
mod keyboard_layout;
mod midi_host;
mod midi_input;

use app::ReelSynthApp;
use app_settings::AppSettings;
use audio_devices::AudioOutputDevices;
use audio_host::start_audio_on_device;
use crossbeam_channel;
use eframe::egui;
use midi_host::MidiDevices;
use reelsynth::engine::MidiEvent;
use reelsynth_ui::{set_gpu_renderer_active, APP_HEIGHT_FULL, APP_MIN_HEIGHT, APP_MIN_WIDTH};
use std::sync::Arc;

fn main() -> eframe::Result<()> {
    let settings = AppSettings::load();
    let midi_devices = MidiDevices::enumerate();
    let audio_devices = AudioOutputDevices::enumerate();
    let (midi_event_tx, midi_event_rx) = crossbeam_channel::unbounded::<MidiEvent>();

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([APP_MIN_WIDTH, APP_HEIGHT_FULL])
        .with_min_inner_size([APP_MIN_WIDTH, APP_MIN_HEIGHT])
        .with_title("ReelSynth");

    #[cfg(windows)]
    {
        viewport = viewport.with_drag_and_drop(false);
    }

    let renderer = settings.graphics_backend.to_renderer();
    let gpu_waveforms = settings.gpu_waveforms;
    let preferred_audio = settings.audio_output_device.clone();

    eframe::run_native(
        "ReelSynth",
        eframe::NativeOptions {
            viewport,
            renderer,
            ..Default::default()
        },
        Box::new(move |cc| {
            reelsynth_ui_theme::apply(&cc.egui_ctx);
            set_gpu_renderer_active(&cc.egui_ctx, gpu_waveforms);

            let preferred = preferred_audio.as_deref().and_then(|name| {
                if audio_devices.index_of_name(name).is_some() {
                    Some(name)
                } else {
                    None
                }
            });

            let audio = match start_audio_on_device(44100, preferred, None, None) {
                Ok(a) => Some(Arc::new(a)),
                Err(e) => {
                    eprintln!("audio init failed: {e}");
                    None
                }
            };

            Ok(Box::new(ReelSynthApp::new(
                audio,
                audio_devices,
                midi_devices,
                midi_event_tx,
                midi_event_rx,
                settings,
            )))
        }),
    )
}

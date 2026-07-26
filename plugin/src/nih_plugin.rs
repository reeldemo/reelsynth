//! ReelSynth nih-plug instrument — slim DAW surface + IPC to external full editor.

use crate::ipc::{launch_external_editor, IpcBridge, PendingNote};
use crate::plugin_state::{PluginStateV1, PLUGIN_STATE_SCHEMA};
use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, widgets, EguiState};
use parking_lot::Mutex;
use reelsynth::{Patch, SynthEngine, WavetableBank};
use std::sync::Arc;

struct ReelSynthPlugin {
    params: Arc<ReelSynthParams>,
    engine: Option<SynthEngine>,
    patch: Patch,
    bank: WavetableBank,
    sample_rate: u32,
    ipc: Option<IpcBridge>,
    stereo_scratch: Vec<f32>,
}

#[derive(Params)]
struct ReelSynthParams {
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,

    #[id = "open_editor"]
    pub open_editor: BoolParam,

    #[id = "wt_position"]
    pub wt_position: FloatParam,
    #[id = "filter_cutoff"]
    pub filter_cutoff: FloatParam,
    #[id = "filter_res"]
    pub filter_res: FloatParam,
    #[id = "amp_attack"]
    pub amp_attack: FloatParam,
    #[id = "amp_release"]
    pub amp_release: FloatParam,

    #[persist = "canonical"]
    pub canonical: Arc<Mutex<PluginStateV1>>,
}

struct HostPanelState {
    status: String,
}

impl Default for ReelSynthPlugin {
    fn default() -> Self {
        let patch = Patch::default_mono();
        let bank = WavetableBank::factory_saw_morph();
        let params = ReelSynthParams {
            canonical: Arc::new(Mutex::new(PluginStateV1::from_patch_bank(&patch, &bank))),
            ..ReelSynthParams::default_params_only()
        };
        let ipc = IpcBridge::start(patch.clone(), bank.clone()).ok();
        Self {
            params: Arc::new(params),
            engine: None,
            patch,
            bank,
            sample_rate: 44100,
            ipc,
            stereo_scratch: Vec::new(),
        }
    }
}

impl ReelSynthParams {
    fn default_params_only() -> Self {
        Self {
            editor_state: EguiState::from_size(360, 160),
            open_editor: BoolParam::new("Open Editor", false)
                .with_callback(Arc::new(|on| {
                    if on {
                        std::thread::spawn(|| {
                            let _ = launch_external_editor();
                        });
                    }
                }))
                .non_automatable()
                .with_value_to_string(Arc::new(|on| {
                    if on {
                        "launched".into()
                    } else {
                        "off".into()
                    }
                })),
            wt_position: FloatParam::new(
                "WT Position",
                0.25,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(50.0)),
            filter_cutoff: FloatParam::new(
                "Filter Cutoff",
                0.7,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(50.0)),
            filter_res: FloatParam::new(
                "Filter Res",
                0.2,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(50.0)),
            amp_attack: FloatParam::new(
                "Amp Attack",
                0.01,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(50.0)),
            amp_release: FloatParam::new(
                "Amp Release",
                0.2,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(50.0)),
            canonical: Arc::new(Mutex::new(PluginStateV1::from_patch_bank(
                &Patch::default_mono(),
                &WavetableBank::factory_saw_morph(),
            ))),
        }
    }
}

impl Default for ReelSynthParams {
    fn default() -> Self {
        Self::default_params_only()
    }
}

impl Plugin for ReelSynthPlugin {
    const NAME: &'static str = "ReelSynth";
    const VENDOR: &'static str = "Reeldemo";
    const URL: &'static str = "https://github.com/reeldemo/reelsynth";
    const EMAIL: &'static str = "hello@reeldemo.xyz";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: None,
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: None,
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        create_egui_editor(
            self.params.editor_state.clone(),
            HostPanelState {
                status: String::new(),
            },
            |_, _| {},
            move |egui_ctx, setter, state| {
                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    ui.heading("ReelSynth");
                    ui.label("Full Design UI opens in a separate window.");
                    ui.add_space(8.0);

                    let open = ui.add_sized(
                        egui::vec2(ui.available_width().max(220.0), 36.0),
                        egui::Button::new("Open Editor"),
                    );
                    if open.clicked() {
                        state.status = match launch_external_editor() {
                            Ok(()) => "Launched external editor.".into(),
                            Err(e) => e,
                        };
                    }

                    ui.add_space(6.0);
                    if !state.status.is_empty() {
                        ui.label(&state.status);
                    } else {
                        ui.small(
                            "In Live's device rack: toggle the Open Editor switch (no plug-in window needed).",
                        );
                    }

                    ui.add_space(10.0);
                    ui.collapsing("Host params", |ui| {
                        ui.add(widgets::ParamSlider::for_param(&params.open_editor, setter));
                        ui.add(widgets::ParamSlider::for_param(&params.wt_position, setter));
                        ui.add(widgets::ParamSlider::for_param(&params.filter_cutoff, setter));
                        ui.add(widgets::ParamSlider::for_param(&params.filter_res, setter));
                        ui.add(widgets::ParamSlider::for_param(&params.amp_attack, setter));
                        ui.add(widgets::ParamSlider::for_param(&params.amp_release, setter));
                    });
                });
            },
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate as u32;
        self.hydrate_from_persisted();
        if let Some(ipc) = self.ipc.as_ref() {
            if let Some(mut g) = ipc.state.try_lock() {
                g.patch = Arc::new(self.patch.clone());
                g.bank = Arc::new(self.bank.clone());
                g.dirty = false;
            }
        }
        self.rebuild_engine();
        true
    }

    fn reset(&mut self) {
        self.rebuild_engine();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // Never block the audio thread on IPC. Skip editor sync this buffer if busy.
        if let Some(ipc) = self.ipc.as_ref() {
            if let Some((patch, bank)) = ipc.try_take_dirty() {
                self.patch = (*patch).clone();
                self.bank = (*bank).clone();
                if let Some(engine) = self.engine.as_mut() {
                    engine.update_bank((*bank).clone());
                    engine.apply_patch_hot((*patch).clone());
                } else {
                    self.rebuild_engine();
                }
            }
        }

        if self.engine.is_none() {
            self.rebuild_engine();
        }

        self.apply_params_to_engine();
        let Some(engine) = self.engine.as_mut() else {
            silence(buffer);
            return ProcessStatus::Normal;
        };

        if let Some(ipc) = self.ipc.as_ref() {
            while let Some(n) = ipc.notes.pop() {
                match n {
                    PendingNote::On { note, velocity } => {
                        engine.note_on(0, note, velocity);
                    }
                    PendingNote::Off { note } => {
                        engine.note_off(0, note);
                    }
                }
            }
        }

        while let Some(event) = context.next_event() {
            match event {
                NoteEvent::NoteOn {
                    note, velocity, ..
                } => {
                    engine.note_on(0, note, velocity.clamp(0.0, 1.0));
                }
                NoteEvent::NoteOff { note, .. } => {
                    engine.note_off(0, note);
                }
                _ => {}
            }
        }

        let frames = buffer.samples();
        let need = frames * 2;
        if self.stereo_scratch.len() < need {
            self.stereo_scratch.resize(need, 0.0);
        }
        let scratch = &mut self.stereo_scratch[..need];
        scratch.fill(0.0);
        engine.process_stereo(scratch);

        let channels = buffer.channels();
        for (frame_i, channel_samples) in buffer.iter_samples().enumerate() {
            let l = scratch[frame_i * 2];
            let r = scratch[frame_i * 2 + 1];
            let mut ch = channel_samples.into_iter();
            if let Some(s) = ch.next() {
                *s = l;
            }
            if let Some(s) = ch.next() {
                *s = if channels >= 2 { r } else { l };
            }
            for s in ch {
                *s = 0.0;
            }
        }

        // Normal (not KeepAlive): let Live suspend when idle so transport stays snappy.
        ProcessStatus::Normal
    }
}

fn silence(buffer: &mut Buffer) {
    for mut channel_samples in buffer.iter_samples() {
        for s in channel_samples.iter_mut() {
            *s = 0.0;
        }
    }
}

impl ReelSynthPlugin {
    fn hydrate_from_persisted(&mut self) {
        let guard = self.params.canonical.lock();
        if guard.schema != PLUGIN_STATE_SCHEMA {
            return;
        }
        if let Ok(json) = guard.to_json() {
            drop(guard);
            if let Ok((patch, bank)) = PluginStateV1::from_json(&json) {
                self.patch = patch;
                self.bank = bank;
            }
        }
    }

    fn rebuild_engine(&mut self) {
        self.engine = Some(SynthEngine::new(
            self.bank.clone(),
            self.patch.clone(),
            self.sample_rate,
        ));
    }

    fn apply_params_to_engine(&mut self) {
        let Some(engine) = self.engine.as_mut() else {
            return;
        };
        let pos = self.params.wt_position.smoothed.next();
        engine.set_wt_position(pos * 255.0);
        let t = self.params.filter_cutoff.smoothed.next().clamp(0.0, 1.0);
        let min = 20.0f32.ln();
        let max = 20000.0f32.ln();
        let cutoff = (min + t * (max - min)).exp();
        engine.set_filter_cutoff(cutoff);
        let res = self.params.filter_res.smoothed.next();
        engine.set_filter_resonance(res);
        let mut env = self.patch.envelope.clone();
        env.attack = self.params.amp_attack.smoothed.next() * 5.0;
        env.release = self.params.amp_release.smoothed.next() * 8.0;
        self.patch.envelope = env.clone();
        if let Some(osc) = self.patch.oscillators.get_mut(0) {
            osc.position = pos;
        }
        self.patch.filter.cutoff = cutoff;
        self.patch.filter.resonance = res;
        engine.set_envelope(env);
    }
}

impl ClapPlugin for ReelSynthPlugin {
    const CLAP_ID: &'static str = "xyz.reelsynth";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Wavetable synth — Open Editor for the full Design UI");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
        ClapFeature::Mono,
    ];
}

impl Vst3Plugin for ReelSynthPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"ReelSynthPlugin!";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

nih_export_clap!(ReelSynthPlugin);
nih_export_vst3!(ReelSynthPlugin);

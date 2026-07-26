//! ReelSynth nih-plug instrument (VST3 + CLAP) — audio/MIDI + DAW state; shared egui later.

use crate::plugin_state::{PluginStateV1, PLUGIN_STATE_SCHEMA};
use nih_plug::prelude::*;
use parking_lot::Mutex;
use reelsynth::{Patch, SynthEngine, WavetableBank};
use std::sync::Arc;

struct ReelSynthPlugin {
    params: Arc<ReelSynthParams>,
    engine: Option<SynthEngine>,
    patch: Patch,
    bank: WavetableBank,
    sample_rate: u32,
}

#[derive(Params)]
struct ReelSynthParams {
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

    /// Full canonical patch + wavetable for Ableton set save/reload.
    #[persist = "canonical"]
    pub canonical: Arc<Mutex<PluginStateV1>>,
}

impl Default for ReelSynthPlugin {
    fn default() -> Self {
        let patch = Patch::default_mono();
        let bank = WavetableBank::factory_saw_morph();
        let params = ReelSynthParams {
            canonical: Arc::new(Mutex::new(PluginStateV1::from_patch_bank(&patch, &bank))),
            ..ReelSynthParams::default_params_only()
        };
        Self {
            params: Arc::new(params),
            engine: None,
            patch,
            bank,
            sample_rate: 44100,
        }
    }
}

impl ReelSynthParams {
    fn default_params_only() -> Self {
        Self {
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

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(0),
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate as u32;
        self.hydrate_from_persisted();
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
        self.apply_params_to_engine();
        let Some(engine) = self.engine.as_mut() else {
            return ProcessStatus::Normal;
        };

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
        let mut tmp = vec![0.0f32; frames * 2];
        engine.process_stereo(&mut tmp);
        for (frame_i, channel_samples) in buffer.iter_samples().enumerate() {
            let l = tmp[frame_i * 2];
            let r = tmp[frame_i * 2 + 1];
            let mut ch = channel_samples.into_iter();
            if let Some(s) = ch.next() {
                *s = l;
            }
            if let Some(s) = ch.next() {
                *s = r;
            }
        }

        // Keep persisted blob in sync for the next set save.
        *self.params.canonical.lock() = PluginStateV1::from_patch_bank(&self.patch, &self.bank);

        ProcessStatus::Normal
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
        engine.set_filter_cutoff((min + t * (max - min)).exp());
        engine.set_filter_resonance(self.params.filter_res.smoothed.next());
        let mut env = self.patch.envelope.clone();
        env.attack = self.params.amp_attack.smoothed.next() * 5.0;
        env.release = self.params.amp_release.smoothed.next() * 8.0;
        self.patch.envelope = env.clone();
        if let Some(osc) = self.patch.oscillators.get_mut(0) {
            osc.position = pos;
        }
        self.patch.filter.cutoff = (min + t * (max - min)).exp();
        self.patch.filter.resonance = self.params.filter_res.smoothed.next();
        engine.set_envelope(env);
    }
}

impl ClapPlugin for ReelSynthPlugin {
    const CLAP_ID: &'static str = "xyz.reelsynth";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Open-source wavetable synthesizer");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for ReelSynthPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"ReelSynthPlugin!";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

nih_export_clap!(ReelSynthPlugin);
nih_export_vst3!(ReelSynthPlugin);

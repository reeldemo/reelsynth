//! Block-based realtime synthesizer engine (S0).

mod midi;
mod params;
mod voice_pool;
mod voice_rt;

pub use midi::{note_to_freq, MidiEvent};
pub use params::{EngineParams, Smoother};
pub use voice_pool::{VoicePool, MAX_VOICES};
pub use voice_rt::RtVoice;

use crate::fx::FxChain;
use crate::patch::Patch;
use crate::voice::render_note;
use crate::wavetable::WavetableBank;

/// Polyphonic wavetable synth engine with shared offline/realtime DSP.
pub struct SynthEngine {
    bank: WavetableBank,
    patch: Patch,
    pool: VoicePool,
    params: EngineParams,
    fx: FxChain,
    sample_rate: u32,
    global_time: f32,
}

impl SynthEngine {
    pub fn new(bank: WavetableBank, patch: Patch, sample_rate: u32) -> Self {
        let params = EngineParams::new(&patch, sample_rate as f32);
        let pool = VoicePool::new(&patch);
        let fx = FxChain::new(sample_rate);
        Self {
            bank,
            patch,
            pool,
            params,
            fx,
            sample_rate,
            global_time: 0.0,
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn patch(&self) -> &Patch {
        &self.patch
    }

    pub fn set_patch(&mut self, patch: Patch) {
        self.params.sync_from_patch(&patch);
        self.pool.reset_patch(&patch);
        self.fx.set_bypass(patch.fx_bypass.clone());
        self.patch = patch;
    }

    pub fn bank(&self) -> &WavetableBank {
        &self.bank
    }

    /// Hot-swap wavetable bank and patch (preset load).
    pub fn load_preset(&mut self, bank: WavetableBank, patch: Patch) {
        self.bank = bank;
        self.set_patch(patch);
    }

    pub fn set_wt_position(&mut self, position: f32) {
        if let Some(osc) = self.patch.oscillators.get_mut(0) {
            osc.position = position.clamp(0.0, 255.0);
        }
    }

    pub fn set_filter_cutoff(&mut self, cutoff: f32) {
        self.patch.filter.cutoff = cutoff;
        self.params.filter_cutoff.set_target(cutoff);
    }

    pub fn set_filter_resonance(&mut self, resonance: f32) {
        self.patch.filter.resonance = resonance.clamp(0.0, 0.95);
    }

    pub fn set_filter_type(&mut self, filter_type: &str) {
        self.patch.filter.filter_type = filter_type.to_string();
    }

    pub fn set_envelope(&mut self, envelope: crate::patch::Envelope) {
        self.patch.envelope = envelope;
    }

    pub fn set_lfo_rate(&mut self, rate: f32) {
        self.patch.lfo.rate = rate.max(0.0);
    }

    pub fn set_lfo_depth(&mut self, depth: f32) {
        self.patch.lfo.depth = depth.clamp(0.0, 1.0);
    }

    pub fn set_osc_level(&mut self, index: usize, level: f32) {
        self.patch.ensure_oscillators(index + 1);
        if let Some(osc) = self.patch.oscillators.get_mut(index) {
            osc.level = level.clamp(0.0, 1.0);
        }
    }

    pub fn set_osc_detune(&mut self, index: usize, detune: f32) {
        self.patch.ensure_oscillators(index + 1);
        if let Some(osc) = self.patch.oscillators.get_mut(index) {
            osc.detune = detune.clamp(-2400.0, 2400.0);
        }
    }

    pub fn set_osc_unison(&mut self, index: usize, unison: u32) {
        self.patch.ensure_oscillators(index + 1);
        if let Some(osc) = self.patch.oscillators.get_mut(index) {
            osc.unison = unison.clamp(1, 8);
        }
    }

    pub fn set_osc_position(&mut self, index: usize, position: f32) {
        self.patch.ensure_oscillators(index + 1);
        if let Some(osc) = self.patch.oscillators.get_mut(index) {
            osc.position = position.clamp(0.0, 255.0);
        }
    }

    pub fn set_sub_level(&mut self, level: f32) {
        self.patch.sub_level = level.clamp(0.0, 1.0);
    }

    pub fn set_noise_level(&mut self, level: f32) {
        self.patch.noise_level = level.clamp(0.0, 1.0);
    }

    pub fn set_mod_matrix(&mut self, slots: Vec<crate::patch::ModSlot>) {
        self.patch.mod_matrix = slots;
    }

    pub fn set_fx_bypass(&mut self, bypass: crate::fx::FxBypass) {
        self.patch.fx_bypass = bypass.clone();
        self.fx.set_bypass(bypass);
    }

    pub fn note_on(&mut self, note: u8, velocity: f32) {
        let freq = note_to_freq(note);
        self.pool
            .note_on(&self.patch, note, freq, velocity, self.global_time);
    }

    pub fn note_off(&mut self, note: u8) {
        self.pool.note_off(note);
    }

    pub fn handle_event(&mut self, event: MidiEvent) {
        match event {
            MidiEvent::NoteOn { note, velocity } => self.note_on(note, velocity),
            MidiEvent::NoteOff { note } => self.note_off(note),
        }
    }

    /// Render one block of mono audio into `out`.
    pub fn process(&mut self, out: &mut [f32]) {
        let sr = self.sample_rate as f32;
        let dt = 1.0 / sr;
        let mut patch = self.patch.clone();
        patch.filter.cutoff = self.params.filter_cutoff.current();

        for sample in out.iter_mut() {
            self.params.filter_cutoff.process();
            self.params.master_gain.process();
            patch.filter.cutoff = self.params.filter_cutoff.current();

            let mut acc = 0.0f32;
            for voice in self.pool.voices_mut() {
                acc += voice.process_sample(&self.bank, &patch, self.global_time, dt, sr);
            }
            *sample = self.fx.process_sample(acc * self.params.master_gain.current());
            self.global_time += dt;
        }
    }

    /// Offline reference render using the same patch/bank (for golden tests).
    pub fn render_offline(&self, freq: f32, duration: f32) -> Vec<f32> {
        let mut audio = render_note(
            &self.bank,
            freq,
            duration,
            self.sample_rate,
            &self.patch,
        );
        let mut fx = FxChain::new(self.sample_rate);
        fx.set_bypass(self.patch.fx_bypass.clone());
        for sample in audio.iter_mut() {
            *sample = fx.process_sample(*sample);
        }
        audio
    }
}

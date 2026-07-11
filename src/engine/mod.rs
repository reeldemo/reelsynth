//! Block-based realtime synthesizer engine (S0).

mod midi;
mod params;
mod voice_pool;
mod voice_rt;

pub use midi::{note_to_freq, MidiEvent};
pub use params::{EngineParams, Smoother};
pub use voice_pool::{VoicePool, MAX_VOICES};
pub use voice_rt::RtVoice;

use crate::patch::Patch;
use crate::voice::render_note;
use crate::wavetable::WavetableBank;

/// Polyphonic wavetable synth engine with shared offline/realtime DSP.
pub struct SynthEngine {
    bank: WavetableBank,
    patch: Patch,
    pool: VoicePool,
    params: EngineParams,
    sample_rate: u32,
    global_time: f32,
}

impl SynthEngine {
    pub fn new(bank: WavetableBank, patch: Patch, sample_rate: u32) -> Self {
        let params = EngineParams::new(&patch, sample_rate as f32);
        let pool = VoicePool::new(&patch);
        Self {
            bank,
            patch,
            pool,
            params,
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
        self.patch = patch;
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
            *sample = acc * self.params.master_gain.current();
            self.global_time += dt;
        }
    }

    /// Offline reference render using the same patch/bank (for golden tests).
    pub fn render_offline(&self, freq: f32, duration: f32) -> Vec<f32> {
        render_note(
            &self.bank,
            freq,
            duration,
            self.sample_rate,
            &self.patch,
        )
    }
}

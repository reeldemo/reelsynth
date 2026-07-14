//! Sample-accurate beat clock derived from transport BPM.

use super::transport::TransportState;

/// Advances transport playhead in beat time per audio sample.
#[derive(Clone, Debug, Default)]
pub struct SampleClock;

impl SampleClock {
    /// Beat delta for one sample at `sample_rate` and `bpm`.
    pub fn beats_per_sample(sample_rate: f32, bpm: f32) -> f32 {
        (bpm / 60.0) / sample_rate
    }

    /// Advance transport by `frames` samples; returns beat range covered.
    pub fn tick(&self, transport: &mut TransportState, frames: usize, sample_rate: f32) -> BeatRange {
        let dt = Self::beats_per_sample(sample_rate, transport.bpm);
        let start = transport.playhead_beats;
        if transport.playing {
            transport.playhead_beats += dt * frames as f32;
            transport.wrap_loop();
        }
        BeatRange {
            start_beats: start,
            end_beats: transport.playhead_beats,
            beats_per_sample: dt,
        }
    }

    /// Advance one sample (for sample-accurate scheduling).
    pub fn tick_one(&self, transport: &mut TransportState, sample_rate: f32) -> f32 {
        let dt = Self::beats_per_sample(sample_rate, transport.bpm);
        if transport.playing {
            transport.playhead_beats += dt;
            transport.wrap_loop();
        }
        transport.playhead_beats
    }
}

/// Beat-time window covered by one audio buffer.
#[derive(Clone, Copy, Debug)]
pub struct BeatRange {
    pub start_beats: f32,
    pub end_beats: f32,
    pub beats_per_sample: f32,
}

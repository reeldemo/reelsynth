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
    ///
    /// When the playhead would cross a loop boundary mid-buffer, the returned
    /// range is truncated at `loop_end` and the playhead wraps to `loop_start`
    /// so `end_beats >= start_beats` always holds (scheduler assumes a forward window).
    pub fn tick(&self, transport: &mut TransportState, frames: usize, sample_rate: f32) -> BeatRange {
        let dt = Self::beats_per_sample(sample_rate, transport.bpm);
        let start = transport.playhead_beats;
        if !transport.playing {
            return BeatRange {
                start_beats: start,
                end_beats: start,
                beats_per_sample: dt,
            };
        }

        let raw_end = start + dt * frames as f32;
        let end = if transport.loop_enabled
            && transport.loop_end > transport.loop_start
            && start < transport.loop_end
            && raw_end > transport.loop_end
        {
            transport.playhead_beats = transport.loop_start;
            transport.loop_end
        } else {
            transport.playhead_beats = raw_end;
            transport.wrap_loop();
            // If wrap moved playhead behind start (edge case), keep range forward.
            if transport.playhead_beats < start {
                transport.loop_end.max(start)
            } else {
                transport.playhead_beats
            }
        };

        BeatRange {
            start_beats: start,
            end_beats: end.max(start),
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

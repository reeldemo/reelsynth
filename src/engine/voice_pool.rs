//! 16-voice polyphonic pool with steal-on-overflow.

use super::voice_rt::RtVoice;
use crate::patch::Patch;

pub const MAX_VOICES: usize = 16;

pub struct VoicePool {
    voices: Vec<RtVoice>,
    steal_cursor: usize,
    global_age: u64,
}

impl VoicePool {
    pub fn new(patch: &Patch) -> Self {
        Self {
            voices: (0..MAX_VOICES).map(|_| RtVoice::new(patch)).collect(),
            steal_cursor: 0,
            global_age: 0,
        }
    }

    pub fn reset_patch(&mut self, patch: &Patch) {
        for voice in &mut self.voices {
            *voice = RtVoice::new(patch);
        }
    }

    pub fn note_on(
        &mut self,
        patch: &Patch,
        channel: u8,
        note: u8,
        freq: f32,
        velocity: f32,
        start_time: f32,
        mpe: crate::engine::mpe::VoiceMpe,
    ) {
        if let Some(idx) = self
            .voices
            .iter()
            .position(|v| v.active && v.note == note && v.channel == channel && v.gate)
        {
            self.global_age += 1;
            let age = self.global_age;
            self.voices[idx].trigger(patch, channel, note, freq, velocity, start_time, mpe);
            self.voices[idx].age = age;
            return;
        }

        let idx = self.allocate_index();
        let voice = &mut self.voices[idx];
        voice.trigger(patch, channel, note, freq, velocity, start_time, mpe);
        self.global_age += 1;
        voice.age = self.global_age;
    }

    pub fn note_off(&mut self, channel: u8, note: u8) {
        for voice in &mut self.voices {
            if voice.active && voice.note == note && voice.channel == channel && voice.gate {
                voice.release();
            }
        }
    }

    pub fn update_channel_mpe(&mut self, channel: u8, mpe: crate::engine::mpe::VoiceMpe) {
        for voice in &mut self.voices {
            if voice.active && voice.channel == channel {
                voice.mpe = mpe;
            }
        }
    }

    pub fn voices_mut(&mut self) -> &mut [RtVoice] {
        &mut self.voices
    }

    fn allocate_index(&mut self) -> usize {
        if let Some((idx, _)) = self
            .voices
            .iter()
            .enumerate()
            .find(|(_, v)| !v.active)
        {
            return idx;
        }

        if let Some((idx, _)) = self
            .voices
            .iter()
            .enumerate()
            .find(|(_, v)| !v.gate)
        {
            return idx;
        }

        let idx = self
            .voices
            .iter()
            .enumerate()
            .min_by_key(|(_, v)| v.age)
            .map(|(i, _)| i)
            .unwrap_or(self.steal_cursor);

        self.steal_cursor = (idx + 1) % MAX_VOICES;
        idx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::Patch;

    #[test]
    fn steals_when_full() {
        let patch = Patch::default_mono();
        let mut pool = VoicePool::new(&patch);

        for n in 0..MAX_VOICES as u8 {
            pool.note_on(&patch, 0, 60 + n, 440.0, 1.0, 0.0, Default::default());
        }
        assert!(pool.voices.iter().all(|v| v.active));

        pool.note_on(&patch, 0, 127, 880.0, 1.0, 0.0, Default::default());
        assert!(pool.voices.iter().any(|v| v.active && v.note == 127));
    }
}

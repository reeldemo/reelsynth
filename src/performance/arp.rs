//! Arpeggiator engine — live performance and clip pattern generation.

use serde::{Deserialize, Serialize};

use super::{scale::scale_degree_to_midi, PerformanceSettings};
use crate::sequence::MidiNote;

/// What notes feed the arpeggiator pool.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArpInputMode {
    #[default]
    SingleNote,
    HeldChord,
    ScaleDegrees,
}

/// Step direction through the note pool.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArpDirection {
    #[default]
    Up,
    Down,
    UpDown,
    DownUp,
    Random,
    AsPlayed,
    Converge,
}

/// Arp rate synced to project BPM (steps per beat).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArpRate {
    #[default]
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
    EighthTriplet,
    SixteenthTriplet,
}

impl ArpRate {
    pub fn steps_per_beat(self) -> f32 {
        match self {
            Self::Quarter => 1.0,
            Self::Eighth => 2.0,
            Self::Sixteenth => 4.0,
            Self::ThirtySecond => 8.0,
            Self::EighthTriplet => 3.0,
            Self::SixteenthTriplet => 6.0,
        }
    }

    pub fn step_beats(self) -> f32 {
        1.0 / self.steps_per_beat()
    }
}

fn default_gate() -> f32 {
    0.85
}

fn default_octave_spread() -> u8 {
    1
}

/// Persisted arpeggiator settings (preset + performance layer).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArpSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub input_mode: ArpInputMode,
    #[serde(default)]
    pub direction: ArpDirection,
    #[serde(default)]
    pub rate: ArpRate,
    #[serde(default = "default_gate")]
    pub gate: f32,
    #[serde(default = "default_octave_spread")]
    pub octave_spread: u8,
    #[serde(default)]
    pub latch: bool,
}

impl Default for ArpSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            input_mode: ArpInputMode::default(),
            direction: ArpDirection::default(),
            rate: ArpRate::default(),
            gate: default_gate(),
            octave_spread: default_octave_spread(),
            latch: false,
        }
    }
}

/// One emitted arp step (live tick).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArpStep {
    pub note: u8,
    pub velocity: f32,
    pub gate_beats: f32,
}

/// Live arp events for the audio/UI layer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArpEvent {
    NoteOn { note: u8, velocity: f32 },
    NoteOff { note: u8 },
}

/// Runtime arpeggiator state.
#[derive(Clone, Debug, Default)]
pub struct ArpEngine {
    pool: Vec<u8>,
    as_played: Vec<u8>,
    held: Vec<u8>,
    velocity: f32,
    step_index: usize,
    ping_pong_dir: i8,
    phase_beats: f32,
    gate_remaining: f32,
    current_note: Option<u8>,
    latched: bool,
    root_degree: usize,
}

impl ArpEngine {
    pub fn reset(&mut self) {
        self.pool.clear();
        self.as_played.clear();
        self.held.clear();
        self.velocity = 0.0;
        self.step_index = 0;
        self.ping_pong_dir = 1;
        self.phase_beats = 0.0;
        self.gate_remaining = 0.0;
        self.current_note = None;
        self.latched = false;
        self.root_degree = 0;
    }

    pub fn note_on(&mut self, note: u8, velocity: f32, arp: &ArpSettings, perf: &PerformanceSettings) {
        if !self.held.contains(&note) {
            self.held.push(note);
            self.as_played.push(note);
        }
        self.velocity = velocity.max(self.velocity);
        self.rebuild_pool(arp, perf);
        if self.pool.is_empty() {
            return;
        }
        if self.current_note.is_none() {
            self.step_index = 0;
            self.ping_pong_dir = 1;
            self.phase_beats = 0.0;
        }
    }

    pub fn note_off(&mut self, note: u8, arp: &ArpSettings, perf: &PerformanceSettings) {
        if arp.latch {
            return;
        }
        self.held.retain(|&n| n != note);
        self.as_played.retain(|&n| n != note);
        if self.held.is_empty() {
            self.clear_after_release();
        } else {
            self.rebuild_pool(arp, perf);
        }
    }

    pub fn set_chord_pool(&mut self, notes: Vec<u8>, velocity: f32, arp: &ArpSettings, perf: &PerformanceSettings) {
        self.held = notes;
        self.as_played = self.held.clone();
        self.velocity = velocity;
        self.rebuild_pool(arp, perf);
        self.step_index = 0;
        self.ping_pong_dir = 1;
        self.phase_beats = 0.0;
    }

    pub fn all_notes_off(&mut self, arp: &ArpSettings) {
        if arp.latch && self.latched {
            return;
        }
        self.clear_after_release();
    }

    fn clear_after_release(&mut self) {
        self.pool.clear();
        self.held.clear();
        self.as_played.clear();
        self.latched = false;
        self.phase_beats = 0.0;
        self.gate_remaining = 0.0;
        self.step_index = 0;
    }

    fn rebuild_pool(&mut self, arp: &ArpSettings, perf: &PerformanceSettings) {
        self.pool = build_pool(&self.held, arp, perf);
        if let Some(&lowest) = self.held.iter().min() {
            self.root_degree = anchor_degree(lowest, perf);
        }
    }

    pub fn tick(
        &mut self,
        dt_beats: f32,
        arp: &ArpSettings,
        perf: &PerformanceSettings,
    ) -> Vec<ArpEvent> {
        if !arp.enabled || self.pool.is_empty() {
            return Vec::new();
        }

        let step_beats = arp.rate.step_beats();
        let gate_beats = step_beats * arp.gate.clamp(0.05, 1.0);
        let mut events = Vec::new();

        if dt_beats <= 0.0 {
            return events;
        }

        if let Some(note) = self.current_note {
            self.gate_remaining -= dt_beats;
            if self.gate_remaining <= 0.0 {
                events.push(ArpEvent::NoteOff { note });
                self.current_note = None;
            }
        }

        self.phase_beats += dt_beats;
        while self.phase_beats >= step_beats {
            self.phase_beats -= step_beats;

            if let Some(note) = self.current_note.take() {
                events.push(ArpEvent::NoteOff { note });
                self.gate_remaining = 0.0;
            }

            let pool_idx = self.next_pool_index(arp);
            let note = self.pool[pool_idx];
            events.push(ArpEvent::NoteOn {
                note,
                velocity: self.velocity,
            });
            self.current_note = Some(note);
            self.gate_remaining = gate_beats;
            self.advance_step(arp);
        }

        let _ = perf;
        events
    }

    pub fn pending_note_offs(&mut self) -> Vec<u8> {
        let note = self.current_note.take();
        self.gate_remaining = 0.0;
        note.into_iter().collect()
    }

    pub fn pool_is_empty(&self) -> bool {
        self.pool.is_empty()
    }

    fn next_pool_index(&self, arp: &ArpSettings) -> usize {
        let len = self.pool.len();
        if len == 0 {
            return 0;
        }
        match arp.direction {
            ArpDirection::Up => self.step_index % len,
            ArpDirection::Down => len - 1 - (self.step_index % len),
            ArpDirection::UpDown | ArpDirection::DownUp => {
                let idx = self.step_index % len;
                idx
            }
            ArpDirection::Random => pseudo_random_index(self.step_index, len),
            ArpDirection::AsPlayed => self.step_index % len,
            ArpDirection::Converge => converge_index(self.step_index, len),
        }
    }

    fn advance_step(&mut self, arp: &ArpSettings) {
        let len = self.pool.len().max(1);
        match arp.direction {
            ArpDirection::Up | ArpDirection::Down | ArpDirection::Random => {
                self.step_index = self.step_index.wrapping_add(1);
            }
            ArpDirection::AsPlayed => {
                let cap = self.as_played.len().max(len);
                self.step_index = (self.step_index + 1) % cap.max(1);
            }
            ArpDirection::UpDown => {
                if len <= 1 {
                    return;
                }
                let last = len - 1;
                let mut idx = self.step_index % len;
                if self.ping_pong_dir > 0 {
                    if idx >= last {
                        self.ping_pong_dir = -1;
                        idx = last.saturating_sub(1);
                    } else {
                        idx += 1;
                    }
                } else if idx == 0 {
                    self.ping_pong_dir = 1;
                    idx = 1.min(last);
                } else {
                    idx -= 1;
                }
                self.step_index = idx;
            }
            ArpDirection::DownUp => {
                if len <= 1 {
                    return;
                }
                let last = len - 1;
                let mut idx = if self.step_index == 0 {
                    last
                } else {
                    self.step_index % len
                };
                if self.ping_pong_dir < 0 {
                    if idx == 0 {
                        self.ping_pong_dir = 1;
                        idx = 1.min(last);
                    } else {
                        idx -= 1;
                    }
                } else if idx >= last {
                    self.ping_pong_dir = -1;
                    idx = last.saturating_sub(1);
                } else {
                    idx += 1;
                }
                self.step_index = idx;
            }
            ArpDirection::Converge => {
                self.step_index = self.step_index.wrapping_add(1);
            }
        }
    }

    /// Resolve the note for the current step (used by tests and bakers).
    pub fn current_step_note(&self, arp: &ArpSettings) -> Option<u8> {
        if self.pool.is_empty() {
            return None;
        }
        Some(self.pool[self.next_pool_index(arp)])
    }

    /// Bake a MIDI pattern for the piano roll.
    pub fn build_pattern_notes(
        pool: &[u8],
        arp: &ArpSettings,
        length_beats: f32,
        velocity: f32,
    ) -> Vec<MidiNote> {
        if pool.is_empty() || length_beats <= 0.0 {
            return Vec::new();
        }

        let step_beats = arp.rate.step_beats();
        let gate_beats = step_beats * arp.gate.clamp(0.05, 1.0);
        let mut notes = Vec::new();
        let mut engine = ArpEngine {
            pool: pool.to_vec(),
            as_played: pool.to_vec(),
            held: pool.to_vec(),
            velocity,
            ..Default::default()
        };
        engine.step_index = 0;
        engine.ping_pong_dir = if matches!(arp.direction, ArpDirection::DownUp) {
            -1
        } else {
            1
        };

        let mut t = 0.0_f32;
        while t < length_beats - 1e-6 {
            let idx = engine.next_pool_index(arp);
            let note = engine.pool[idx % engine.pool.len()];
            let dur = gate_beats.min(length_beats - t);
            notes.push(MidiNote {
                pitch: note,
                start_beats: t,
                duration_beats: dur,
                velocity,
            });
            t += step_beats;
            engine.advance_step(arp);
        }
        notes
    }
}

pub fn build_pool(held: &[u8], arp: &ArpSettings, perf: &PerformanceSettings) -> Vec<u8> {
    if held.is_empty() {
        return Vec::new();
    }
    let octaves = arp.octave_spread.clamp(1, 4);
    match arp.input_mode {
        ArpInputMode::SingleNote => {
            let base = *held.iter().min().unwrap_or(&60);
            spread_octaves(base, octaves)
        }
        ArpInputMode::HeldChord => {
            if arp.direction == ArpDirection::AsPlayed {
                held.to_vec()
            } else {
                let mut notes = held.to_vec();
                notes.sort_unstable();
                notes.dedup();
                notes
            }
        }
        ArpInputMode::ScaleDegrees => build_scale_pool(held, perf, octaves),
    }
}

fn spread_octaves(base: u8, octaves: u8) -> Vec<u8> {
    (0..octaves)
        .map(|o| (base as i16 + o as i16 * 12).clamp(0, 127) as u8)
        .collect()
}

fn anchor_degree(note: u8, perf: &PerformanceSettings) -> usize {
    if perf.scale.is_chromatic() {
        return (note % 12) as usize;
    }
    let root_pc = perf.root % 12;
    let rel = (note % 12 + 12 - root_pc) % 12;
    let intervals = perf.scale.intervals();
    intervals
        .iter()
        .position(|&iv| iv == rel)
        .unwrap_or(0)
}

fn build_scale_pool(held: &[u8], perf: &PerformanceSettings, octave_spread: u8) -> Vec<u8> {
    let anchor = held.iter().min().copied().unwrap_or(60);
    let root_degree = anchor_degree(anchor, perf);
    let degrees = perf.scale.degree_count();
    let mut pool = Vec::new();
    for oct in 0..octave_spread {
        for d in 0..degrees {
            let degree = root_degree + d + oct as usize * degrees;
            pool.push(scale_degree_to_midi(
                perf.root,
                perf.scale,
                degree,
                perf.base_octave,
            ));
        }
    }
    pool.sort_unstable();
    pool.dedup();
    pool
}

fn pseudo_random_index(step: usize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    ((step.wrapping_mul(7)).wrapping_add(3)) % len
}

fn converge_index(step: usize, len: usize) -> usize {
    if len <= 1 {
        return 0;
    }
    let cycle = len * 2 - 2;
    let pos = step % cycle;
    if pos < len {
        pos
    } else {
        cycle - pos
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::performance::{PerformanceLayout, Scale, ScaleBehavior};

    fn perf_settings(scale: Scale) -> PerformanceSettings {
        PerformanceSettings {
            root: 0,
            scale,
            scale_behavior: ScaleBehavior::Snap,
            layout: PerformanceLayout::Piano,
            ..PerformanceSettings::default()
        }
    }

    #[test]
    fn single_note_up_two_octaves() {
        let arp = ArpSettings {
            input_mode: ArpInputMode::SingleNote,
            direction: ArpDirection::Up,
            octave_spread: 2,
            rate: ArpRate::Quarter,
            ..ArpSettings::default()
        };
        let perf = perf_settings(Scale::Major);
        let pool = build_pool(&[60], &arp, &perf);
        assert_eq!(pool, vec![60, 72]);

        let pattern = ArpEngine::build_pattern_notes(&pool, &arp, 4.0, 0.9);
        assert_eq!(pattern.len(), 4);
        assert_eq!(pattern[0].pitch, 60);
        assert_eq!(pattern[1].pitch, 72);
        assert_eq!(pattern[2].pitch, 60);
        assert_eq!(pattern[3].pitch, 72);
    }

    #[test]
    fn held_chord_up_pattern() {
        let arp = ArpSettings {
            input_mode: ArpInputMode::HeldChord,
            direction: ArpDirection::Up,
            rate: ArpRate::Quarter,
            ..ArpSettings::default()
        };
        let perf = perf_settings(Scale::Major);
        let pool = build_pool(&[60, 64, 67], &arp, &perf);
        assert_eq!(pool, vec![60, 64, 67]);

        let pattern = ArpEngine::build_pattern_notes(&pool, &arp, 3.0, 0.8);
        assert_eq!(pattern.len(), 3);
        assert_eq!(pattern[0].pitch, 60);
        assert_eq!(pattern[1].pitch, 64);
        assert_eq!(pattern[2].pitch, 67);
    }

    #[test]
    fn major_pentatonic_scale_pool() {
        let arp = ArpSettings {
            input_mode: ArpInputMode::ScaleDegrees,
            direction: ArpDirection::Random,
            octave_spread: 2,
            ..ArpSettings::default()
        };
        let perf = perf_settings(Scale::MajorPent);
        let pool = build_pool(&[60], &arp, &perf);
        assert_eq!(pool.len(), 10);
        for &note in &pool {
            assert!(note_in_pentatonic(note, 0));
        }

        let pattern = ArpEngine::build_pattern_notes(&pool, &arp, 2.0, 0.7);
        for n in pattern {
            assert!(note_in_pentatonic(n.pitch, 0));
        }
    }

    fn note_in_pentatonic(note: u8, root: u8) -> bool {
        let rel = (note % 12 + 12 - root) % 12;
        [0, 2, 4, 7, 9].contains(&rel)
    }

    #[test]
    fn gate_halves_step_duration() {
        let arp = ArpSettings {
            gate: 0.5,
            rate: ArpRate::Quarter,
            ..ArpSettings::default()
        };
        let pool = vec![60];
        let pattern = ArpEngine::build_pattern_notes(&pool, &arp, 1.0, 1.0);
        assert_eq!(pattern.len(), 1);
        assert!((pattern[0].duration_beats - 0.5).abs() < 1e-4);
    }

    #[test]
    fn build_pattern_sixteenth_two_bars() {
        let arp = ArpSettings {
            rate: ArpRate::Sixteenth,
            ..ArpSettings::default()
        };
        let pool = vec![60, 64, 67];
        let pattern = ArpEngine::build_pattern_notes(&pool, &arp, 8.0, 0.8);
        assert_eq!(pattern.len(), 32);
        assert!((pattern[1].start_beats - 0.25).abs() < 1e-4);
        assert!((pattern[31].start_beats - 7.75).abs() < 1e-4);
    }

    #[test]
    fn live_tick_emits_note_on() {
        let mut engine = ArpEngine::default();
        let arp = ArpSettings {
            enabled: true,
            rate: ArpRate::Quarter,
            ..ArpSettings::default()
        };
        let perf = perf_settings(Scale::Major);
        engine.note_on(60, 0.9, &arp, &perf);
        let events = engine.tick(1.0, &arp, &perf);
        assert!(events.iter().any(|e| matches!(e, ArpEvent::NoteOn { note: 60, .. })));
    }
}

//! Per-effect DSP processors.

use super::types::{EffectSlot, EffectType};

pub(crate) fn soft_clip(sample: f32) -> f32 {
    (sample * 1.15).tanh()
}

#[derive(Clone, Debug)]
pub(crate) enum EffectProcessor {
    Chorus(ChorusProc),
    Delay(DelayProc),
    Reverb(ReverbProc),
    Distortion(DistortionProc),
    Compressor(CompressorProc),
}

impl EffectProcessor {
    pub(crate) fn new(slot: &EffectSlot, sr: f32) -> Self {
        match slot.effect_type {
            EffectType::Chorus => Self::Chorus(ChorusProc::new(sr)),
            EffectType::Delay => Self::Delay(DelayProc::new(sr)),
            EffectType::Reverb => Self::Reverb(ReverbProc::new(sr)),
            EffectType::Distortion => Self::Distortion(DistortionProc::new(sr)),
            EffectType::Compressor => Self::Compressor(CompressorProc::new(sr)),
        }
    }

    pub(crate) fn process_stereo(&mut self, l: f32, r: f32, slot: &EffectSlot) -> [f32; 2] {
        match self {
            Self::Chorus(p) => p.process(l, r, slot),
            Self::Delay(p) => p.process(l, r, slot),
            Self::Reverb(p) => p.process(l, r, slot),
            Self::Distortion(p) => p.process(l, r, slot),
            Self::Compressor(p) => p.process(l, r, slot),
        }
    }
}

// ── Chorus ───────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub(crate) struct ChorusProc {
    sr: f32,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    pos: usize,
    phase: f32,
}

impl ChorusProc {
    fn new(sr: f32) -> Self {
        let len = (sr * 0.05).max(64.0) as usize;
        Self {
            sr,
            buf_l: vec![0.0; len],
            buf_r: vec![0.0; len],
            pos: 0,
            phase: 0.0,
        }
    }

    fn process(&mut self, l: f32, r: f32, slot: &EffectSlot) -> [f32; 2] {
        let rate = slot.rate.clamp(0.05, 8.0);
        let depth = slot.depth.clamp(0.0, 1.0);
        let base_delay = 0.012 * self.sr;
        let mod_depth = depth * 0.004 * self.sr;

        self.phase += rate / self.sr;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        let mod_l = (self.phase * std::f32::consts::TAU).sin();
        let mod_r = ((self.phase + 0.25) % 1.0 * std::f32::consts::TAU).sin();

        self.buf_l[self.pos] = l;
        self.buf_r[self.pos] = r;

        let dl = (base_delay + mod_l * mod_depth).clamp(1.0, self.buf_l.len() as f32 - 2.0);
        let dr = (base_delay + mod_r * mod_depth).clamp(1.0, self.buf_r.len() as f32 - 2.0);

        let out_l = read_interp(&self.buf_l, self.pos, dl);
        let out_r = read_interp(&self.buf_r, self.pos, dr);

        self.pos = (self.pos + 1) % self.buf_l.len();
        [out_l, out_r]
    }
}

// ── Delay (stereo ping-pong) ─────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub(crate) struct DelayProc {
    sr: f32,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    pos: usize,
}

impl DelayProc {
    fn new(sr: f32) -> Self {
        let len = (sr * 2.0).max(128.0) as usize;
        Self {
            sr,
            buf_l: vec![0.0; len],
            buf_r: vec![0.0; len],
            pos: 0,
        }
    }

    fn process(&mut self, l: f32, r: f32, slot: &EffectSlot) -> [f32; 2] {
        let time_ms = slot.time_ms.clamp(1.0, 2000.0);
        let fb = slot.feedback.clamp(0.0, 0.92);
        let delay_samp = (time_ms * 0.001 * self.sr)
            .clamp(1.0, self.buf_l.len() as f32 - 2.0);

        let delayed_l = read_interp(&self.buf_l, self.pos, delay_samp);
        let delayed_r = read_interp(&self.buf_r, self.pos, delay_samp * 1.07);

        // Ping-pong feedback cross-feed.
        self.buf_l[self.pos] = l + delayed_r * fb;
        self.buf_r[self.pos] = r + delayed_l * fb;

        self.pos = (self.pos + 1) % self.buf_l.len();
        [delayed_l, delayed_r]
    }
}

// ── Reverb (Schroeder-style) ─────────────────────────────────────────────────

const NUM_COMBS: usize = 4;
const NUM_ALLPASS: usize = 2;

#[derive(Clone, Debug)]
struct CombFilter {
    buf: Vec<f32>,
    pos: usize,
    delay: usize,
    feedback: f32,
    damp: f32,
    store: f32,
}

impl CombFilter {
    fn new(delay: usize) -> Self {
        Self {
            buf: vec![0.0; delay.max(1)],
            pos: 0,
            delay: delay.max(1),
            feedback: 0.84,
            damp: 0.2,
            store: 0.0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.buf[self.pos];
        self.store = delayed * (1.0 - self.damp) + self.store * self.damp;
        self.buf[self.pos] = input + self.store * self.feedback;
        self.pos = (self.pos + 1) % self.delay;
        delayed
    }
}

#[derive(Clone, Debug)]
struct AllpassFilter {
    buf: Vec<f32>,
    pos: usize,
    delay: usize,
}

impl AllpassFilter {
    fn new(delay: usize) -> Self {
        Self {
            buf: vec![0.0; delay.max(1)],
            pos: 0,
            delay: delay.max(1),
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let buf_out = self.buf[self.pos];
        let out = -input + buf_out;
        self.buf[self.pos] = input + buf_out * 0.5;
        self.pos = (self.pos + 1) % self.delay;
        out
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ReverbProc {
    combs_l: [CombFilter; NUM_COMBS],
    combs_r: [CombFilter; NUM_COMBS],
    allpass_l: [AllpassFilter; NUM_ALLPASS],
    allpass_r: [AllpassFilter; NUM_ALLPASS],
}

impl ReverbProc {
    fn new(sr: f32) -> Self {
        let scale = (sr / 44100.0).max(0.5);
        let comb_delays = [
            (1116.0 * scale) as usize,
            (1188.0 * scale) as usize,
            (1277.0 * scale) as usize,
            (1356.0 * scale) as usize,
        ];
        let ap_delays = [(556.0 * scale) as usize, (441.0 * scale) as usize];
        Self {
            combs_l: [
                CombFilter::new(comb_delays[0]),
                CombFilter::new(comb_delays[1]),
                CombFilter::new(comb_delays[2]),
                CombFilter::new(comb_delays[3]),
            ],
            combs_r: [
                CombFilter::new(comb_delays[0] + 23),
                CombFilter::new(comb_delays[1] + 23),
                CombFilter::new(comb_delays[2] + 23),
                CombFilter::new(comb_delays[3] + 23),
            ],
            allpass_l: [
                AllpassFilter::new(ap_delays[0]),
                AllpassFilter::new(ap_delays[1]),
            ],
            allpass_r: [
                AllpassFilter::new(ap_delays[0] + 13),
                AllpassFilter::new(ap_delays[1] + 13),
            ],
        }
    }

    fn process(&mut self, l: f32, r: f32, slot: &EffectSlot) -> [f32; 2] {
        let size = slot.size.clamp(0.0, 1.0);
        let damping = slot.damping.clamp(0.0, 1.0);
        let fb = 0.7 + size * 0.25;

        for comb in self.combs_l.iter_mut().chain(self.combs_r.iter_mut()) {
            comb.feedback = fb;
            comb.damp = damping * 0.9;
        }

        let mut out_l = 0.0f32;
        let mut out_r = 0.0f32;
        for c in &mut self.combs_l {
            out_l += c.process(l);
        }
        for c in &mut self.combs_r {
            out_r += c.process(r);
        }
        out_l *= 0.25;
        out_r *= 0.25;

        for ap in &mut self.allpass_l {
            out_l = ap.process(out_l);
        }
        for ap in &mut self.allpass_r {
            out_r = ap.process(out_r);
        }

        [out_l, out_r]
    }
}

// ── Distortion ─────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub(crate) struct DistortionProc {
    tone_lp: f32,
    tone_state_l: f32,
    tone_state_r: f32,
}

impl DistortionProc {
    fn new(_sr: f32) -> Self {
        Self {
            tone_lp: 0.3,
            tone_state_l: 0.0,
            tone_state_r: 0.0,
        }
    }

    fn process(&mut self, l: f32, r: f32, slot: &EffectSlot) -> [f32; 2] {
        let drive = 1.0 + slot.drive.clamp(0.0, 1.0) * 12.0;
        let tone = slot.tone.clamp(0.0, 1.0);
        let lp = 0.05 + tone * 0.85;
        self.tone_lp = lp;

        let shaped_l = (l * drive).tanh();
        let shaped_r = (r * drive).tanh();

        self.tone_state_l += lp * (shaped_l - self.tone_state_l);
        self.tone_state_r += lp * (shaped_r - self.tone_state_r);
        [self.tone_state_l, self.tone_state_r]
    }
}

// ── Compressor (feed-forward RMS) ────────────────────────────────────────────

#[derive(Clone, Debug)]
pub(crate) struct CompressorProc {
    sr: f32,
    env: f32,
}

impl CompressorProc {
    fn new(sr: f32) -> Self {
        Self { sr, env: 0.0 }
    }

    fn process(&mut self, l: f32, r: f32, slot: &EffectSlot) -> [f32; 2] {
        let threshold_db = slot.threshold.clamp(-60.0, 0.0);
        let ratio = slot.ratio.clamp(1.0, 20.0);
        let attack = slot.attack.clamp(0.001, 0.5);
        let release = slot.release.clamp(0.01, 2.0);

        let mono = (l.abs() + r.abs()) * 0.5;
        let target = mono;

        let coeff = if target > self.env {
            (-1.0 / (attack * self.sr)).exp()
        } else {
            (-1.0 / (release * self.sr)).exp()
        };
        self.env = target + coeff * (self.env - target);

        let level_db = 20.0 * (self.env.max(1e-8)).log10();
        let over_db = level_db - threshold_db;
        let gain_db = if over_db > 0.0 {
            -over_db * (1.0 - 1.0 / ratio)
        } else {
            0.0
        };
        let gain = 10.0_f32.powf(gain_db / 20.0);
        [l * gain, r * gain]
    }
}

fn read_interp(buf: &[f32], write_pos: usize, delay: f32) -> f32 {
    let len = buf.len();
    if len < 2 {
        return 0.0;
    }
    let read_pos = (write_pos as f32 - delay).rem_euclid(len as f32);
    let idx = read_pos.floor() as usize % len;
    let frac = read_pos - read_pos.floor();
    let next = (idx + 1) % len;
    buf[idx] * (1.0 - frac) + buf[next] * frac
}

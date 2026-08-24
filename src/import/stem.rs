//! Stem-to-wavetable: turn an arbitrary audio stem into a playable
//! `WavetableBank` (the "translate a song stem into a synth sound" feature).
//!
//! Two extraction modes mirror Serum/Vital:
//!
//! - **Pitched** ("pitch-splice"): YIN period detection, then single-cycle
//!   slices advanced through the buffer become consecutive wavetable frames —
//!   the morph tracks the stem's time evolution.
//! - **Vocode**: STFT magnitude frames with randomized phase (`FFT` → keep
//!   magnitudes → random phase → IFFT). Independent of pitch, ideal for
//!   dynamic-pitch or unpitched stems (vocals, fx).
//!
//! `Auto` picks pitched when a voiced period is found, else vocode.

use crate::analysis::pitch::{detect_period, hz_to_midi};
use crate::wavetable::{WavetableBank, DEFAULT_FRAME_SIZE, DEFAULT_NUM_FRAMES};
use rustfft::{num_complex::Complex, FftPlanner};

/// Extraction mode selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StemMode {
    /// Detect: pitched if voiced, otherwise vocode.
    Auto,
    /// Single-cycle slicing from a detected period (pitch-splice).
    Pitched,
    /// FFT-magnitude + random phase frames (vocode).
    Vocode,
}

impl StemMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "pitched" | "pitch" => Some(Self::Pitched),
            "vocode" | "vocoded" => Some(Self::Vocode),
            _ => None,
        }
    }
}

/// Modest raised-cosine trim applied at the seam of single cycles so
/// extracted frames loop without a click.
const SEAM_FADE_T: f32 = 0.03;
/// Peak normalization target for each frame.
const FRAME_PEAK: f32 = 0.9;
/// Frames used for period-synchronous averaging (noise reduction).
const AVG_CYCLES: usize = 3;

/// Tuning knobs for stem extraction.
#[derive(Clone, Debug)]
pub struct StemOptions {
    pub sample_rate: u32,
    pub mode: StemMode,
    pub num_frames: usize,
    pub frame_size: usize,
    /// Zero any bin below `gate * max_bin` in vocode mode (kills noise,
    /// avoids the "came out empty/aliased" failure class). `None` disables.
    pub harmonic_gate: Option<f32>,
    pub min_freq: f32,
    pub max_freq: f32,
}

impl Default for StemOptions {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            mode: StemMode::Auto,
            num_frames: DEFAULT_NUM_FRAMES,
            frame_size: DEFAULT_FRAME_SIZE,
            harmonic_gate: None,
            min_freq: 50.0,
            max_freq: 2000.0,
        }
    }
}

/// What actually happened during extraction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StemModeUsed {
    Pitched,
    Vocode,
}

/// Result of a stem extraction.
#[derive(Clone, Debug)]
pub struct StemResult {
    pub bank: WavetableBank,
    pub root_note: u8,
    pub f0_hz: f32,
    pub period_samples: f32,
    pub mode_used: StemModeUsed,
    pub warnings: Vec<String>,
}

/// Read a WAV file and convert it to a wavetable (engine-level CLI entry).
///
/// The file is decoded to mono at whatever rate `opts.sample_rate` claims
/// the render target is (decode is format-agnostic; callers are expected to
/// normalize to the target rate beforehand, e.g. 44.1 kHz).
pub fn import_stem_file(path: &str, opts: &StemOptions) -> Result<StemResult, String> {
    let samples = crate::import::wav::decode_wav_mono(std::path::Path::new(path))?;
    stem_to_wavetable(&samples, opts)
}

/// Convert raw mono samples into a wavetable bank.
pub fn stem_to_wavetable(samples: &[f32], opts: &StemOptions) -> Result<StemResult, String> {
    let sr = opts.sample_rate.max(1);
    if samples.len() < 512 {
        return Err(format!("stem too short ({} samples)", samples.len()));
    }
    if opts.frame_size < 64 || opts.frame_size & (opts.frame_size - 1) != 0 {
        return Err("frame_size must be a power of two >= 64".into());
    }
    let num_frames = opts.num_frames.max(2);

    // 1) Detect pitch once (used by Pitched, Auto, and vocode window sizing).
    let pitch = detect_period(samples, sr, opts.min_freq, opts.max_freq);
    let voiced = pitch.map(|p| p.level_is_voiced()).unwrap_or(false);

    let mode_used = match opts.mode {
        StemMode::Pitched => StemModeUsed::Pitched,
        StemMode::Vocode => StemModeUsed::Vocode,
        StemMode::Auto => {
            if voiced {
                StemModeUsed::Pitched
            } else {
                StemModeUsed::Vocode
            }
        }
    };

    let mut warnings = Vec::new();
    let bank = match mode_used {
        StemModeUsed::Pitched => {
            let p = pitch.ok_or("pitched mode requires a voiced detection")?;
            extract_pitched(
                samples,
                p.period,
                num_frames,
                opts.frame_size,
                &mut warnings,
            )
        }
        StemModeUsed::Vocode => extract_vocode(
            samples,
            pitch.map(|p| p.period),
            num_frames,
            opts.frame_size,
            opts.harmonic_gate,
            &mut warnings,
        )?,
    };

    let root_note = match pitch {
        Some(p) if voiced => hz_to_midi(p.f0),
        _ => 0,
    };
    let f0 = pitch.map(|p| p.f0).unwrap_or(0.0);
    let period = pitch.map(|p| p.period).unwrap_or(0.0);

    validate_frames(&bank, &mut warnings)?;

    Ok(StemResult {
        bank,
        root_note,
        f0_hz: f0,
        period_samples: period,
        mode_used,
        warnings,
    })
}

/// Single-cycle slicing: frame `i` starts at `start_i` (swept through the
/// buffer), reads one detected period with a seamless loop fade.
fn extract_pitched(
    samples: &[f32],
    period: f32,
    num_frames: usize,
    frame_size: usize,
    warnings: &mut Vec<String>,
) -> WavetableBank {
    let mut bank = WavetableBank::new(num_frames, frame_size);
    let len = samples.len() as f32;
    let step = (len - period).max(period) / (num_frames - 1) as f32;

    for fi in 0..num_frames {
        let start = if num_frames == 1 {
            0.0
        } else {
            fi as f32 * step
        };
        let cycle = sample_cycle_averaged(samples, start, period, AVG_CYCLES);
        let seamless = loop_fade(&cycle, SEAM_FADE_T);
        bank.set_frame_from_cycle(fi, &seamless);
        normalize_frame(&mut bank, fi);
    }
    let _ = warnings;
    bank
}

/// Period-synchronous averaging: average up to `avg` consecutive cycles to
/// suppress transient noise while keeping the harmonic structure.
fn sample_cycle_averaged(samples: &[f32], start: f32, period: f32, avg: usize) -> Vec<f32> {
    let mut acc = vec![0.0f32; DEFAULT_FRAME_SIZE];
    let n = avg.max(1);
    for k in 0..n {
        let cycle = sample_cycle(samples, start + k as f32 * period, period);
        for (a, c) in acc.iter_mut().zip(cycle.iter()) {
            *a += c;
        }
    }
    for a in &mut acc {
        *a /= n as f32;
    }
    acc
}

/// Read one period of `samples` into a `frame_size`-long buffer with linear
/// interpolation, wrapped circularly so short stems still produce material.
fn sample_cycle(samples: &[f32], start: f32, period: f32) -> Vec<f32> {
    let n = samples.len() as f32;
    let period = period.max(2.0);
    (0..DEFAULT_FRAME_SIZE)
        .map(|i| {
            let t = i as f32 / DEFAULT_FRAME_SIZE as f32;
            let pos = start + t * period;
            let clamped = pos % n;
            let i0 = clamped.floor() as usize % samples.len();
            let i1 = (i0 + 1) % samples.len();
            let f = clamped - clamped.floor();
            samples[i0] * (1.0 - f) + samples[i1] * f
        })
        .collect()
}

/// Fade the tail of a cycle back into its head so it loops click-free.
fn loop_fade(cycle: &[f32], width: f32) -> Vec<f32> {
    if cycle.is_empty() {
        return Vec::new();
    }
    let fade = ((cycle.len() as f32 * width).floor() as usize).clamp(8, cycle.len() / 4);
    let mut out = cycle.to_vec();
    for i in 0..fade {
        let t = i as f32 / (fade - 1) as f32;
        let g = 0.5 - 0.5 * (std::f32::consts::PI * t).cos(); // raised cosine 0..1
        let blend = 1.0 - g;
        out[i] = out[i] * blend + out[cycle.len() - 1 - i] * g;
    }
    out
}

/// Vocode mode: FFT magnitude frames with randomized phase.
fn extract_vocode(
    samples: &[f32],
    period: Option<f32>,
    num_frames: usize,
    frame_size: usize,
    harmonic_gate: Option<f32>,
    warnings: &mut Vec<String>,
) -> Result<WavetableBank, String> {
    let len = samples.len();
    let window_len = match period {
        Some(p) if p >= 64.0 && p <= frame_size as f32 * 2.0 => p.floor() as usize,
        _ => (frame_size / 2).max(256),
    };
    let window_len = window_len.min(len);
    let step = (len - window_len) as f32 / (num_frames - 1) as f32;

    let mut planner = FftPlanner::<f32>::new();
    let fwd = planner.plan_fft_forward(frame_size);
    let inv = planner.plan_fft_inverse(frame_size);

    let mut bank = WavetableBank::new(num_frames, frame_size);
    let mut rng = XorShift::new(0x5EED_2026);

    let mut max_energy = 0.0f32;
    for fi in 0..num_frames {
        let start = if num_frames == 1 {
            0
        } else {
            (fi as f32 * step) as usize
        };
        let start = start.min(len - window_len);

        let mut buf: Vec<Complex<f32>> = vec![Complex::new(0.0, 0.0); frame_size];
        for (i, s) in samples[start..start + window_len].iter().enumerate() {
            buf[i] = Complex::new(*s, 0.0);
        }
        hann_window(&mut buf, window_len);

        fwd.process(&mut buf);

        // Keep magnitudes only, apply optional gate, override phases.
        let mut max_bin = 0.0f32;
        for b in buf.iter().take(frame_size / 2) {
            max_bin = max_bin.max(b.norm());
        }
        if max_bin <= 1e-9 {
            warnings.push(format!("frame {fi}: near-silent after FFT"));
        }
        let gate = harmonic_gate.unwrap_or(0.0);
        for (k, b) in buf.iter_mut().enumerate() {
            let mag = b.norm();
            if mag * ((frame_size / 2) as f32) < 1e-9 || (gate > 0.0 && mag < max_bin * gate) {
                b.re = 0.0;
                b.im = 0.0;
            } else {
                let phase = rng.next_f32() * std::f32::consts::TAU;
                b.re = mag * phase.cos();
                b.im = mag * phase.sin();
            }
            if k == 0 {
                b.re = 0.0;
                b.im = 0.0;
            }
        }

        inv.process(&mut buf);
        let scale = 1.0 / frame_size as f32;
        let frame_out = bank.frame_mut(fi);
        for (i, b) in buf.into_iter().enumerate() {
            frame_out[i] = b.re * scale;
        }
        let e = frame_out.iter().fold(0.0f32, |acc, s| acc + s * s);
        max_energy = max_energy.max(e);
        normalize_frame(&mut bank, fi);
    }
    if max_energy < 1e-6 {
        return Err("vocode extraction produced only silence".into());
    }
    Ok(bank)
}

/// Raised-cosine window applied to the FFT input so frame slices are smooth.
fn hann_window(buf: &mut [Complex<f32>], window_len: usize) {
    let n = window_len.max(2);
    for (i, sample) in buf.iter_mut().take(n).enumerate() {
        let t = i as f32 / (n - 1) as f32;
        let w = 0.5 - 0.5 * (std::f32::consts::TAU * t).cos();
        sample.re *= w;
    }
}

/// Normalize one frame to `FRAME_PEAK`, guarding divisions by silence.
fn normalize_frame(bank: &mut WavetableBank, fi: usize) {
    let frame = bank.frame_mut(fi);
    let peak = frame.iter().fold(0.0f32, |acc, s| acc.max(s.abs()));
    if peak > 1e-6 {
        let g = FRAME_PEAK / peak;
        for s in frame {
            *s *= g;
        }
    }
}

/// Per-frame energy check (REE-11 guard): any frame that collapsed to
/// silence is reported so callers never get an empty-looking wavetable.
fn validate_frames(bank: &WavetableBank, warnings: &mut Vec<String>) -> Result<(), String> {
    let mut empty = 0;
    for fi in 0..bank.num_frames {
        let e = bank.frame(fi).iter().fold(0.0f32, |a, s| a + s * s);
        if e < 1e-8 {
            empty += 1;
        }
    }
    if empty == bank.num_frames {
        return Err("all wavetable frames are silent".into());
    }
    if empty > 0 {
        warnings.push(format!(
            "{empty}/{bank} frames are silent",
            bank = bank.num_frames
        ));
    }
    Ok(())
}

/// Deterministic tiny PRNG for vocode phases (reproducible builds).
struct XorShift {
    state: u64,
}

impl XorShift {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(freq: f32, sr: u32, seconds: f32, harmonics: u32) -> Vec<f32> {
        let n = (sr as f32 * seconds) as usize;
        (0..n)
            .map(|i| {
                let t = i as f32 / sr as f32;
                let mut s = 0.0f32;
                for h in 1..=harmonics {
                    s += (std::f32::consts::TAU * freq * h as f32 * t).sin() / h as f32;
                }
                s
            })
            .collect()
    }

    #[test]
    fn auto_mode_detects_pitched() {
        let samples = tone(220.0, 44100, 1.0, 8);
        let opts = StemOptions::default();
        let res = stem_to_wavetable(&samples, &opts).unwrap();
        assert_eq!(res.mode_used, StemModeUsed::Pitched);
        assert_eq!(res.root_note, 57); // A3
        assert_eq!(res.bank.num_frames, 256);
        assert_eq!(res.bank.frame_size, 2048);
        assert!(res.warnings.is_empty());
    }

    #[test]
    fn pitched_frames_have_energy_and_normalized() {
        let samples = tone(110.0, 44100, 2.0, 12);
        let opts = StemOptions {
            mode: StemMode::Pitched,
            ..StemOptions::default()
        };
        let res = stem_to_wavetable(&samples, &opts).unwrap();
        for fi in 0..res.bank.num_frames {
            let frame = res.bank.frame(fi);
            let peak = frame.iter().fold(0.0f32, |a, s| a.max(s.abs()));
            assert!(peak > 0.5, "frame {fi} peak {peak}");
            assert!(peak <= 1.0);
        }
    }

    #[test]
    fn vocode_mode_preserves_energy_on_noise() {
        // Unpitched material: white-ish noise (seeded, deterministic).
        let mut rng = XorShift::new(42);
        let samples: Vec<f32> = (0..44100).map(|_| rng.next_f32() * 2.0 - 1.0).collect();
        let opts = StemOptions {
            mode: StemMode::Vocode,
            ..StemOptions::default()
        };
        let res = stem_to_wavetable(&samples, &opts).unwrap();
        assert_eq!(res.mode_used, StemModeUsed::Vocode);
        let total: f32 = (0..res.bank.num_frames)
            .map(|fi| res.bank.frame(fi).iter().fold(0.0f32, |a, s| a + s * s))
            .sum();
        assert!(total > 1.0, "vocode frames must carry energy");
    }

    #[test]
    fn auto_falls_back_to_vocode() {
        let mut rng = XorShift::new(7);
        let samples: Vec<f32> = (0..44100).map(|_| rng.next_f32() * 0.4 - 0.2).collect();
        let res = stem_to_wavetable(&samples, &StemOptions::default()).unwrap();
        assert_eq!(res.mode_used, StemModeUsed::Vocode);
    }

    #[test]
    fn rejects_tiny_stems() {
        assert!(stem_to_wavetable(&[0.0f32; 100], &StemOptions::default()).is_err());
    }

    #[test]
    fn harmonic_gate_keeps_bright_peaks() {
        let samples = tone(440.0, 44100, 1.0, 20);
        let opts = StemOptions {
            mode: StemMode::Vocode,
            harmonic_gate: Some(0.05),
            ..StemOptions::default()
        };
        let res = stem_to_wavetable(&samples, &opts).unwrap();
        assert_eq!(res.bank.num_frames, 256);
        // with gate the brightest bins survive -> frames must not be silent
        for fi in (0..res.bank.num_frames).step_by(32) {
            let e: f32 = res.bank.frame(fi).iter().fold(0.0, |a, s| a + s * s);
            assert!(e > 1e-6);
        }
    }

    #[test]
    fn roundtrip_write_read() {
        let samples = tone(220.0, 44100, 1.0, 8);
        let res = stem_to_wavetable(&samples, &StemOptions::default()).unwrap();
        let dir = std::env::temp_dir().join("reelsynth_stem_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("stem_out.reelwt");
        res.bank.write_file(path.to_str().unwrap()).expect("write");
        let re = WavetableBank::read_file(path.to_str().unwrap()).unwrap();
        assert_eq!(re.num_frames, res.bank.num_frames);
        assert_eq!(re.frame_size, res.bank.frame_size);
    }

    #[test]
    fn hann_window_sums_smoothly() {
        let mut buf: Vec<Complex<f32>> = (0..256)
            .map(|i| Complex::new(if i % 2 == 0 { 1.0 } else { -1.0 }, 0.0))
            .collect();
        hann_window(&mut buf, 256);
        let e: f32 = buf.iter().map(|b| b.re * b.re).sum();
        // Hann: sum(w^2) = 3/8 * n for n samples of unit magnitude.
        assert!((e - 96.0).abs() < 8.0, "e={e}");
    }
}

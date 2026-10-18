//! Pitch/period detection for stem-to-wavetable analysis.
//!
//! YIN-lite (difference function + cumulative-mean-normalization) period
//! detection, used to slice single-cycle wavetable frames from pitched stems.

/// Detected fundamental period and frequency.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PitchDetect {
    /// Period in samples (fractional, sub-sample resolved).
    pub period: f32,
    /// Fundamental frequency in Hz.
    pub f0: f32,
    /// Voicing confidence in 0..1 (1 = confident). Callers below
    /// `level_is_voiced()` should treat material as unpitched (vocode mode).
    pub confidence: f32,
}

/// YIN threshold: the first lag whose CMND value falls under this wins.
const YIN_THRESHOLD: f32 = 0.20;
/// Denominator floor for CMND (guards near-silent frames).
const CMND_EPSILON: f32 = 1e-4;

/// Estimate the dominant period of `samples`.
///
/// Windows are swept across the buffer and the best-voiced candidate is
/// returned (so a voiced section of a dynamic stem is still found).
/// `min_freq`/`max_freq` clamp the search range (typical 50..2000 Hz).
pub fn detect_period(
    samples: &[f32],
    sample_rate: u32,
    min_freq: f32,
    max_freq: f32,
) -> Option<PitchDetect> {
    let sr = sample_rate.max(1) as f32;
    let min_freq = min_freq.clamp(20.0, sr * 0.4);
    let max_freq = max_freq.clamp(min_freq, sr * 0.5);
    let max_lag = (sr / min_freq).floor() as usize;
    let min_lag = (sr / max_freq).ceil() as usize;
    if max_lag < 4 || samples.len() < max_lag * 2 {
        return None;
    }

    let mut best: Option<PitchDetect> = None;
    let mut best_threshold = f32::INFINITY;
    let mut best_energy = 0.0f32;

    // Sweep up to 4 sliding windows so a voiced section is found even when
    // the stem has dynamic pitch (sung lines, phrasing).
    for win_index in 0..4 {
        let win_len = (max_lag * 2).min((samples.len() / 2) * 2).max(min_lag + 8);
        if win_len > samples.len() {
            continue;
        }
        let avail = samples.len() - win_len;
        // Avoid a carpet of silence at the very edges of the file.
        let span = if avail > 0 { avail + 1 } else { 1 };
        let start = ((span * win_index / 4) / 2) * 2;
        let x = &samples[start..start + win_len];

        let energy = x.iter().fold(0.0f32, |acc, s| acc + s * s) / win_len as f32;
        if energy < CMND_EPSILON * 10.0 {
            continue;
        }

        if let Some((lag, threshold)) = yin_period(x, min_lag, max_lag) {
            if threshold >= best_threshold {
                continue;
            }
            let period = parabolic_refine(x, lag);
            let f0 = sr / period;
            let confidence = (YIN_THRESHOLD - threshold) / YIN_THRESHOLD;
            let cand = PitchDetect {
                period,
                f0,
                confidence: confidence.clamp(0.0, 1.0),
            };
            if cand.level_is_voiced() && (energy >= best_energy * 0.25 || best.is_none()) {
                best_threshold = threshold;
                best_energy = energy;
                best = Some(cand);
            }
        }
    }

    best
}

impl PitchDetect {
    /// Conservative voiced gate: period sane + above-noise confidence.
    pub fn level_is_voiced(&self) -> bool {
        self.confidence >= 0.6 && self.period.is_finite() && self.period > 2.0
    }
}

/// YIN difference + CMND; returns `(lag, cmnd_value)` of the winning lag.
///
/// Plain "first dip under threshold" YIN is brittle on harmonic-rich stems:
/// a weak early dip (octave-spoiled) can swallow the true period. Instead we
/// pick the *strongest* dip (the lag with the best CMND value) and then
/// prefer an earlier nearly-as-good lag as the ground pitch. A fractional
/// fundamental period P can be beaten several dozen times by its integer
/// double (2P), so the penalty must be generous (e.g. saw at 220 Hz: 0.0005
/// at P=200.5 vs 0.00002 at 2P=401). Non-voiced victims are already kept
/// out by the threshold check, so this cannot surface spurious octaves.
fn yin_period(x: &[f32], min_lag: usize, max_lag: usize) -> Option<(usize, f32)> {
    /// Prefer the earlier lag if it is within this factor of the best dip.
    const DOUBLE_PENALTY: f32 = 64.0;

    let n = x.len();
    if n <= max_lag + 2 {
        return None;
    }

    let mut diff = vec![0.0f32; max_lag + 1];
    for lag in min_lag..=max_lag {
        let mut sum = 0.0f32;
        for i in 0..(n - max_lag) {
            let d = x[i] - x[i + lag];
            sum += d * d;
        }
        diff[lag] = sum;
    }

    // CMND normalization and strongest-dip search.
    let mut running = 0.0f32;
    let mut norm = vec![0.0f32; max_lag + 1];
    let mut best_lag = min_lag;
    let mut best_value = f32::INFINITY;
    for lag in min_lag..=max_lag {
        running += diff[lag];
        let mean = (running / (lag - min_lag + 1) as f32).max(CMND_EPSILON);
        let v = diff[lag] / mean;
        norm[lag] = v;
        if v < best_value {
            best_value = v;
            best_lag = lag;
        }
    }

    if best_value >= YIN_THRESHOLD {
        return None; // nothing periodic enough to trust
    }

    // Prefer the earliest lag that is a local minimum and almost as strong
    // as the global best (rejects weak early dips and period doubling).
    for lag in min_lag.saturating_add(1)..=max_lag.saturating_sub(1) {
        let v = norm[lag];
        if v >= YIN_THRESHOLD {
            continue;
        }
        if v > best_value * DOUBLE_PENALTY {
            continue;
        }
        let local_min = v <= norm[lag - 1] + 1e-9 && v <= norm[lag + 1] + 1e-9;
        if local_min {
            return Some((lag, v));
        }
        if lag == best_lag {
            return Some((best_lag, best_value));
        }
    }
    Some((best_lag, best_value))
}

/// Parabolic interpolation around the integer lag for sub-sample accuracy.
fn parabolic_refine(x: &[f32], lag: usize) -> f32 {
    if lag < 2 {
        return lag as f32;
    }
    let d0 = rolloff_sq(x, lag - 1);
    let d1 = rolloff_sq(x, lag);
    let d2 = rolloff_sq(x, lag + 1);
    let denom = (d0 - 2.0 * d1 + d2).abs();
    if denom < 1e-9 {
        lag as f32
    } else {
        let shift = 0.5 * (d0 - d2) / denom;
        (lag as f32 + shift.clamp(-1.0, 1.0)).max(1.0)
    }
}

/// Sum of squared first differences at a given lag (small window).
fn rolloff_sq(x: &[f32], lag: usize) -> f32 {
    let n = x.len();
    if lag >= n {
        return f32::INFINITY;
    }
    let m = (n - lag).min(2048);
    let mut sum = 0.0f32;
    for i in 0..m {
        let d = x[i] - x[i + lag];
        sum += d * d;
    }
    sum
}

/// Convert a frequency to the nearest MIDI note number (69 = A4 = 440 Hz).
pub fn hz_to_midi(f0: f32) -> u8 {
    if !f0.is_finite() || f0 <= 0.0 {
        return 0;
    }
    let midi = 69.0 + 12.0 * (f0 / 440.0).log2();
    (midi.round() as i32).clamp(0, 127) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(freq: f32, sr: u32, seconds: f32) -> Vec<f32> {
        let n = (sr as f32 * seconds) as usize;
        (0..n)
            .map(|i| {
                let t = i as f32 / sr as f32;
                (std::f32::consts::TAU * freq * t).sin()
                    + 0.4 * (std::f32::consts::TAU * freq * 2.0 * t).sin()
            })
            .collect()
    }

    #[test]
    fn detects_440hz() {
        let samples = tone(440.0, 44100, 0.5);
        let p = detect_period(&samples, 44100, 100.0, 800.0).expect("pitch");
        assert!((p.f0 - 440.0).abs() < 4.0, "f0={}", p.f0);
        assert!(p.level_is_voiced());
    }

    #[test]
    fn detects_110hz_bass() {
        let samples = tone(110.0, 44100, 0.5);
        let p = detect_period(&samples, 44100, 50.0, 300.0).expect("pitch");
        assert!((p.f0 - 110.0).abs() < 3.0, "f0={}", p.f0);
    }

    #[test]
    fn rejects_silence() {
        let samples = vec![0.0f32; 44100];
        assert!(detect_period(&samples, 44100, 100.0, 800.0).is_none());
    }

    #[test]
    fn hz_to_midi_440_is_69() {
        assert_eq!(hz_to_midi(440.0), 69);
        assert_eq!(hz_to_midi(880.0), 81);
        assert_eq!(hz_to_midi(220.0), 57);
    }
}

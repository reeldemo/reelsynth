//! Virtual-analog oscillators with band-limited (MinBLEP-style) anti-aliasing.

/// VA waveform type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VaWaveform {
    Saw,
    Square,
    Triangle,
    Pulse,
    Sine,
}

impl VaWaveform {
    pub fn from_osc_type(osc_type: &str) -> Option<Self> {
        match osc_type.to_ascii_lowercase().as_str() {
            "saw" => Some(Self::Saw),
            "square" => Some(Self::Square),
            "triangle" => Some(Self::Triangle),
            "pulse" => Some(Self::Pulse),
            "sine" => Some(Self::Sine),
            _ => None,
        }
    }
}

/// Sample a VA oscillator at `phase` (0..1), advancing by `phase_inc` (= freq/sr).
pub fn sample_va(
    waveform: VaWaveform,
    phase: f32,
    phase_inc: f32,
    pulse_width: f32,
) -> f32 {
    let pw = pulse_width.clamp(0.05, 0.95);
    match waveform {
        VaWaveform::Saw => saw_blep(phase, phase_inc),
        VaWaveform::Square => square_blep(phase, phase_inc, 0.5),
        VaWaveform::Pulse => square_blep(phase, phase_inc, pw),
        VaWaveform::Triangle => triangle_blep(phase, phase_inc),
        VaWaveform::Sine => (phase.fract() * std::f32::consts::TAU).sin(),
    }
}

/// Polynomial BLEP — minimum-phase band-limited step correction.
pub(crate) fn poly_blep(t: f32, dt: f32) -> f32 {
    if dt <= 0.0 {
        return 0.0;
    }
    if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}

/// Widen the BLEP window so wrap cliffs stay gentle at musical pitches.
///
/// A classical 1-sample polyBLEP still leaves ~0.98 adjacent jumps at A4; those
/// residual edges fight any post-filter slew and read as held-note crackle.
/// Stretching the correction to ~10 samples keeps brightness while capping wrap.
pub(crate) fn blep_dt(phase_inc: f32) -> f32 {
    const MIN_SAMPLES: f32 = 10.0;
    if phase_inc <= 0.0 {
        return 0.0;
    }
    (phase_inc * MIN_SAMPLES).clamp(phase_inc, 0.35)
}

fn blep(t: f32, phase_inc: f32) -> f32 {
    poly_blep(t, blep_dt(phase_inc))
}

fn saw_blep(phase: f32, dt: f32) -> f32 {
    let t = phase.fract();
    2.0 * t - 1.0 - blep(t, dt)
}

fn square_blep(phase: f32, dt: f32, pw: f32) -> f32 {
    let t = phase.fract();
    let mut out = if t < pw { 1.0 } else { -1.0 };
    out += blep(t, dt);
    out -= blep((t - pw).fract(), dt);
    out
}

fn triangle_blep(phase: f32, dt: f32) -> f32 {
    let t = phase.fract();
    let mut out = if t < 0.5 {
        4.0 * t - 1.0
    } else {
        3.0 - 4.0 * t
    };
    let slope = 4.0;
    out -= slope * blep(t, dt);
    out += slope * blep((t - 0.5).fract(), dt);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saw_has_harmonic_energy() {
        let sr = 48_000.0f32;
        let freq = 110.0;
        let dt = freq / sr;
        let mut phase = 0.0;
        let mut peak = 0.0f32;
        for _ in 0..(sr as usize) {
            let s = saw_blep(phase, dt);
            peak = peak.max(s.abs());
            phase += dt;
            if phase >= 1.0 {
                phase -= 1.0;
            }
        }
        assert!(peak > 0.5, "saw peak={peak}");
    }

    #[test]
    fn va_waveforms_parse() {
        assert_eq!(VaWaveform::from_osc_type("saw"), Some(VaWaveform::Saw));
        assert_eq!(VaWaveform::from_osc_type("wavetable"), None);
    }

    /// Classical 1-sample polyBLEP left ~0.98 adjacent jumps at A4 — audible wrap crackle.
    /// Widened BLEP must keep the cliff gentle so slew does not fight every cycle.
    #[test]
    fn saw_wrap_jump_bounded_at_a4() {
        let dt = 440.0 / 44_100.0;
        let mut phase = 1.0 - 8.0 * dt;
        let mut prev = sample_va(VaWaveform::Saw, phase, dt, 0.5);
        let mut max_jump = 0.0f32;
        for _ in 0..16 {
            phase = (phase + dt).fract();
            let cur = sample_va(VaWaveform::Saw, phase, dt, 0.5);
            max_jump = max_jump.max((cur - prev).abs());
            prev = cur;
        }
        assert!(
            max_jump < 0.28,
            "saw wrap jump too steep: {max_jump} (causes held-note crackle)"
        );
    }
}

//! Curve harshness metric from an active wavetable frame.

use rustfft::{num_complex::Complex, FftPlanner};

/// Compute `curveHarshness ∈ [0, 1]` from frame samples in approximately `[-1, 1]`.
///
/// Combines wrap discontinuity and high-frequency energy ratio (upper half of spectrum).
pub fn curve_harshness(frame: &[f32]) -> f32 {
    let n = frame.len();
    if n < 2 {
        return 0.0;
    }
    let wrap = wrap_harshness(frame);
    let hf = hf_harshness(frame);
    wrap.max(hf).clamp(0.0, 1.0)
}

/// Wrap seam discontinuity: `|x[N-1] - x[0]| / 2`.
pub fn wrap_harshness(frame: &[f32]) -> f32 {
    let n = frame.len();
    if n < 2 {
        return 0.0;
    }
    let seam = (frame[n - 1] - frame[0]).abs();
    (seam / 2.0).clamp(0.0, 1.0)
}

/// High-frequency energy ratio via real DFT magnitudes.
pub fn hf_harshness(frame: &[f32]) -> f32 {
    let n = frame.len();
    if n < 4 {
        return 0.0;
    }

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(n);
    let mut buffer: Vec<Complex<f32>> = frame.iter().map(|&s| Complex::new(s, 0.0)).collect();
    fft.process(&mut buffer);

    let half = n / 2;
    let k0 = (n / 4).max(1);
    let mut e_total = 0.0f32;
    let mut e_hi = 0.0f32;
    for k in 1..=half {
        let c = buffer[k];
        let p = c.re * c.re + c.im * c.im;
        e_total += p;
        if k >= k0 {
            e_hi += p;
        }
    }
    e_hi / (e_total + 1e-12)
}

/// Pure sine period fixture: `sin(2π i / N)`.
pub fn fixture_sine(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| (i as f32 / n as f32 * std::f32::consts::TAU).sin())
        .collect()
}

/// Saw-like wrap fixture: `2*(i/N)-1` (discontinuous at wrap).
pub fn fixture_saw_wrap(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| 2.0 * (i as f32 / n as f32) - 1.0)
        .collect()
}

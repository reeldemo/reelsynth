//! Phase-aware harmonic decomposition of a single-cycle frame.

use crate::patch::WaveLayer;
use rustfft::{num_complex::Complex, FftPlanner};

const FRAME_SIZE: usize = 2048;

/// Decompose a single-cycle frame into sine `WaveLayer` entries.
pub fn decompose_frame(frame: &[f32; FRAME_SIZE], max_harmonics: usize, min_mag: f32) -> Vec<WaveLayer> {
    let max_h = max_harmonics.clamp(1, FRAME_SIZE / 2);
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FRAME_SIZE);

    let mut buffer: Vec<Complex<f32>> = frame
        .iter()
        .map(|&s| Complex::new(s, 0.0))
        .collect();

    fft.process(&mut buffer);

    let mut layers = Vec::new();
    for h in 1..=max_h {
        let bin = buffer[h];
        let raw_mag = (bin.re * bin.re + bin.im * bin.im).sqrt();
        // Single-sided amplitude estimate from real FFT bin.
        let level = 2.0 * raw_mag / FRAME_SIZE as f32;
        if level < min_mag {
            continue;
        }
        let detune = 1200.0 * (h as f32).log2();
        // Real FFT phase → sine phase (cos/sin basis offset).
        let phase = bin.im.atan2(bin.re) + std::f32::consts::FRAC_PI_2;
        layers.push(WaveLayer {
            source_type: "sine".into(),
            level,
            detune,
            phase,
            ..WaveLayer::default()
        });
    }

    layers
}

/// Resynthesize a frame from sine layers (for A/B comparison).
pub fn resynthesize_frame(layers: &[WaveLayer], out: &mut [f32; FRAME_SIZE]) {
    out.fill(0.0);
    for layer in layers {
        if layer.level <= 0.0 {
            continue;
        }
        let h = 2.0_f32.powf(layer.detune / 1200.0);
        for (i, sample) in out.iter_mut().enumerate() {
            let phase =
                i as f32 / FRAME_SIZE as f32 * std::f32::consts::TAU * h + layer.phase;
            *sample += layer.level * phase.sin();
        }
    }
}

/// RMS error and Pearson correlation between original and resynthesized frames.
pub fn resynthesis_error(original: &[f32; FRAME_SIZE], resynth: &[f32; FRAME_SIZE]) -> (f32, f32) {
    let mut sum_sq = 0.0f32;
    let mut mean_a = 0.0f32;
    let mut mean_b = 0.0f32;
    for i in 0..FRAME_SIZE {
        let d = original[i] - resynth[i];
        sum_sq += d * d;
        mean_a += original[i];
        mean_b += resynth[i];
    }
    mean_a /= FRAME_SIZE as f32;
    mean_b /= FRAME_SIZE as f32;

    let rms = (sum_sq / FRAME_SIZE as f32).sqrt();

    let mut cov = 0.0f32;
    let mut var_a = 0.0f32;
    let mut var_b = 0.0f32;
    for i in 0..FRAME_SIZE {
        let da = original[i] - mean_a;
        let db = resynth[i] - mean_b;
        cov += da * db;
        var_a += da * da;
        var_b += db * db;
    }
    let denom = (var_a * var_b).sqrt();
    let corr = if denom > 1e-9 { cov / denom } else { 0.0 };

    (rms, corr)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sine_frame(phase_offset: f32) -> [f32; FRAME_SIZE] {
        let mut frame = [0.0f32; FRAME_SIZE];
        for (i, s) in frame.iter_mut().enumerate() {
            let p = i as f32 / FRAME_SIZE as f32 * std::f32::consts::TAU + phase_offset;
            *s = p.sin();
        }
        frame
    }

    fn make_saw_frame() -> [f32; FRAME_SIZE] {
        let mut frame = [0.0f32; FRAME_SIZE];
        for (i, s) in frame.iter_mut().enumerate() {
            let p = i as f32 / FRAME_SIZE as f32;
            *s = 2.0 * p - 1.0;
        }
        frame
    }

    #[test]
    fn pure_sine_one_dominant_layer() {
        let frame = make_sine_frame(0.0);
        let layers = decompose_frame(&frame, 16, 0.001);
        assert!(!layers.is_empty());
        let dominant = layers
            .iter()
            .max_by(|a, b| a.level.partial_cmp(&b.level).unwrap())
            .unwrap();
        assert!(dominant.level > 0.85, "dominant level was {}", dominant.level);
        assert!(dominant.detune.abs() < 1.0);
    }

    #[test]
    fn saw_harmonics_descending() {
        let frame = make_saw_frame();
        let layers = decompose_frame(&frame, 16, 0.001);
        assert!(layers.len() >= 4);
        let mut sorted = layers.clone();
        sorted.sort_by(|a, b| a.detune.partial_cmp(&b.detune).unwrap());
        for w in sorted.windows(2) {
            assert!(w[0].level >= w[1].level * 0.4);
        }
    }

    #[test]
    fn ab_resynthesis_sine_close() {
        let frame = make_sine_frame(0.3);
        let layers = decompose_frame(&frame, 8, 0.001);
        let mut resynth = [0.0f32; FRAME_SIZE];
        resynthesize_frame(&layers, &mut resynth);
        let (rms, corr) = resynthesis_error(&frame, &resynth);
        assert!(rms < 0.12, "rms was {rms}");
        assert!(corr > 0.95, "corr was {corr}");
    }

    #[test]
    fn ab_resynthesis_saw_reasonable() {
        let frame = make_saw_frame();
        let layers = decompose_frame(&frame, 32, 0.001);
        let mut resynth = [0.0f32; FRAME_SIZE];
        resynthesize_frame(&layers, &mut resynth);
        let (rms, corr) = resynthesis_error(&frame, &resynth);
        assert!(rms < 0.35, "rms was {rms}");
        assert!(corr > 0.85, "corr was {corr}");
    }
}

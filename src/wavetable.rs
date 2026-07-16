//! Wavetable bank: N frames × M samples, linear + spectral crossfade.

pub const DEFAULT_NUM_FRAMES: usize = 256;
pub const DEFAULT_FRAME_SIZE: usize = 2048;
pub const REELWT_MAGIC: &[u8; 6] = b"REELWT";
pub const REELWT_VERSION: u16 = 1;

/// Map normalized view coordinates to a frame sample value.
#[inline]
pub fn sample_from_view_coords(y_norm: f32) -> f32 {
    (0.5 - y_norm).clamp(-1.0, 1.0) * 2.0
}

/// Map normalized view x (0..1) to frame sample index.
#[inline]
pub fn sample_index_from_phase(frame_size: usize, phase: f32) -> usize {
    if frame_size == 0 {
        return 0;
    }
    let idx = (phase.clamp(0.0, 1.0) * frame_size as f32).floor() as usize;
    idx.min(frame_size.saturating_sub(1))
}

#[derive(Clone, Debug)]
pub struct WavetableBank {
    pub num_frames: usize,
    pub frame_size: usize,
    pub frames: Vec<f32>,
}

impl WavetableBank {
    pub fn new(num_frames: usize, frame_size: usize) -> Self {
        Self {
            num_frames,
            frame_size,
            frames: vec![0.0; num_frames * frame_size],
        }
    }

    pub fn from_flat(num_frames: usize, frame_size: usize, frames: Vec<f32>) -> Result<Self, String> {
        if frames.len() != num_frames * frame_size {
            return Err(format!(
                "expected {} samples, got {}",
                num_frames * frame_size,
                frames.len()
            ));
        }
        Ok(Self {
            num_frames,
            frame_size,
            frames,
        })
    }

    pub fn frame(&self, index: usize) -> &[f32] {
        let start = index * self.frame_size;
        &self.frames[start..start + self.frame_size]
    }

    pub fn frame_mut(&mut self, index: usize) -> &mut [f32] {
        let start = index * self.frame_size;
        &mut self.frames[start..start + self.frame_size]
    }

    /// Sample at wavetable position (0..num_frames-1) and phase (0..1).
    pub fn sample(&self, position: f32, phase: f32) -> f32 {
        self.sample_with_inc(position, phase, 0.0)
    }

    /// Sample with phase increment for band-limited wrap correction.
    ///
    /// When `phase_inc` is 0, wrap BLEP is skipped (legacy / static sampling).
    pub fn sample_with_inc(&self, position: f32, phase: f32, phase_inc: f32) -> f32 {
        self.sample_warped_inc(position, phase, crate::osc::WtWarpMode::None, 0.0, phase_inc)
    }

    /// Sample with optional phase warp (sync / bend).
    pub fn sample_warped(
        &self,
        position: f32,
        phase: f32,
        warp: crate::osc::WtWarpMode,
        warp_amount: f32,
    ) -> f32 {
        self.sample_warped_inc(position, phase, warp, warp_amount, 0.0)
    }

    /// Sample with warp and optional polyBLEP wrap correction when `phase_inc` > 0.
    pub fn sample_warped_inc(
        &self,
        position: f32,
        phase: f32,
        warp: crate::osc::WtWarpMode,
        warp_amount: f32,
        phase_inc: f32,
    ) -> f32 {
        if self.num_frames == 0 || self.frame_size == 0 {
            return 0.0;
        }
        let warped_phase = crate::osc::warp_phase(phase, warp, warp_amount);
        let pos = position.clamp(0.0, (self.num_frames - 1) as f32);
        let idx0 = pos.floor() as usize;
        let idx1 = (idx0 + 1).min(self.num_frames - 1);
        let frac = pos - idx0 as f32;

        let s0 = self.sample_frame_blep(idx0, warped_phase, phase_inc);
        let s1 = self.sample_frame_blep(idx1, warped_phase, phase_inc);
        if frac < 1e-6 || idx0 == idx1 {
            s0
        } else {
            spectral_crossfade(s0, s1, frac)
        }
    }

    fn sample_frame_blep(&self, frame_idx: usize, phase: f32, phase_inc: f32) -> f32 {
        let frame = self.frame(frame_idx);
        let n = self.frame_size;
        let p = phase.fract();
        let pos = p * n as f32;
        let i0 = pos.floor() as usize % n;
        let i1 = (i0 + 1) % n;
        let f = pos - i0 as f32;
        let mut out = frame[i0] * (1.0 - f) + frame[i1] * f;
        if phase_inc > 0.0 && n > 1 {
            // Match VA saw scaling: discontinuity height ~2 uses half-weighted polyBLEP.
            // Use widened blep_dt so WT seams are not steeper than VA wraps.
            let seam = frame[n - 1] - frame[0];
            let dt = crate::osc::va::blep_dt(phase_inc);
            out -= crate::osc::va::poly_blep(p, dt) * seam * 0.5;
        }
        out
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 + self.frames.len() * 4);
        out.extend_from_slice(REELWT_MAGIC);
        out.extend_from_slice(&REELWT_VERSION.to_le_bytes());
        out.extend_from_slice(&(self.num_frames as u32).to_le_bytes());
        out.extend_from_slice(&(self.frame_size as u32).to_le_bytes());
        for s in &self.frames {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        if data.len() < 16 {
            return Err("truncated .reelwt header".into());
        }
        if &data[0..6] != REELWT_MAGIC {
            return Err("invalid .reelwt magic".into());
        }
        let version = u16::from_le_bytes([data[6], data[7]]);
        if version != REELWT_VERSION {
            return Err(format!("unsupported .reelwt version {version}"));
        }
        let num_frames = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
        let frame_size = u32::from_le_bytes([data[12], data[13], data[14], data[15]]) as usize;
        let expected = 16 + num_frames * frame_size * 4;
        if data.len() != expected {
            return Err(format!("expected {expected} bytes, got {}", data.len()));
        }
        let mut frames = Vec::with_capacity(num_frames * frame_size);
        for chunk in data[16..].chunks_exact(4) {
            frames.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }
        Self::from_flat(num_frames, frame_size, frames)
    }

    pub fn write_file(&self, path: &str) -> Result<(), String> {
        std::fs::write(path, self.to_bytes()).map_err(|e| e.to_string())
    }

    pub fn read_file(path: &str) -> Result<Self, String> {
        let data = std::fs::read(path).map_err(|e| e.to_string())?;
        Self::from_bytes(&data)
    }

    /// Resample a single-cycle waveform into one frame.
    pub fn set_frame_from_cycle(&mut self, frame_idx: usize, cycle: &[f32]) {
        if frame_idx >= self.num_frames || cycle.is_empty() {
            return;
        }
        let frame_size = self.frame_size;
        let out = self.frame_mut(frame_idx);
        let len = cycle.len();
        for i in 0..frame_size {
            let src = (i as f32 / frame_size as f32) * len as f32;
            let i0 = src.floor() as usize % len;
            let i1 = (i0 + 1) % len;
            let f = src - src.floor();
            out[i] = cycle[i0] * (1.0 - f) + cycle[i1] * f;
        }
    }

    /// Paint a pencil segment onto one frame (phase and amplitude in 0..1 view space).
    pub fn apply_pencil_segment(
        &mut self,
        frame_idx: usize,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
    ) {
        if frame_idx >= self.num_frames || self.frame_size == 0 {
            return;
        }
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let steps = ((dx.max(dy) * self.frame_size as f32) as usize).max(1);
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let x = x0 + (x1 - x0) * t;
            let y = y0 + (y1 - y0) * t;
            let idx = sample_index_from_phase(self.frame_size, x);
            let value = sample_from_view_coords(y);
            self.frame_mut(frame_idx)[idx] = value;
        }
    }

    /// Downsample a frame to N evenly-spaced control points (Y values).
    pub fn downsample_frame_control_points(frame: &[f32], n: usize) -> Vec<f32> {
        let n = n.clamp(2, frame.len().max(2));
        (0..n)
            .map(|i| {
                let t = i as f32 / n as f32;
                let idx = (t * frame.len() as f32).floor() as usize;
                frame[idx.min(frame.len() - 1)]
            })
            .collect()
    }

    /// Upsample control points to a full frame via cubic interpolation with wrap.
    pub fn upsample_control_points_to_frame(points: &[f32], out: &mut [f32]) {
        let n = points.len();
        if n < 2 || out.is_empty() {
            return;
        }
        let len = out.len();
        for (i, sample) in out.iter_mut().enumerate() {
            let t = i as f32 / len as f32 * n as f32;
            let idx = t.floor() as usize;
            let frac = t - idx as f32;
            let i0 = idx % n;
            let i1 = (idx + 1) % n;
            let i_m1 = (idx + n - 1) % n;
            let i2 = (idx + 2) % n;
            *sample = cubic_interp(points[i_m1], points[i0], points[i1], points[i2], frac);
        }
    }

    pub fn factory_saw_morph() -> Self {
        let mut bank = Self::new(DEFAULT_NUM_FRAMES, DEFAULT_FRAME_SIZE);
        let frame_size = bank.frame_size;
        for f in 0..bank.num_frames {
            let morph = f as f32 / (bank.num_frames - 1) as f32;
            let frame = bank.frame_mut(f);
            for (i, sample) in frame.iter_mut().enumerate() {
                let p = i as f32 / frame_size as f32;
                let saw = 2.0 * p - 1.0;
                let sine = (p * std::f32::consts::TAU).sin();
                *sample = saw * (1.0 - morph) + sine * morph;
            }
            periodize_frame(bank.frame_mut(f));
        }
        bank
    }

    pub fn factory_square_morph() -> Self {
        let mut bank = Self::new(DEFAULT_NUM_FRAMES, DEFAULT_FRAME_SIZE);
        let frame_size = bank.frame_size;
        for f in 0..bank.num_frames {
            let morph = f as f32 / (bank.num_frames - 1) as f32;
            let frame = bank.frame_mut(f);
            for (i, sample) in frame.iter_mut().enumerate() {
                let p = i as f32 / frame_size as f32;
                let sq = if p < 0.5 { 1.0 } else { -1.0 };
                let tri = 1.0 - 4.0 * (p - 0.5).abs();
                *sample = sq * (1.0 - morph) + tri * morph;
            }
            periodize_frame(bank.frame_mut(f));
        }
        bank
    }

    pub fn factory_sine() -> Self {
        let mut bank = Self::new(DEFAULT_NUM_FRAMES, DEFAULT_FRAME_SIZE);
        let frame_size = bank.frame_size;
        for f in 0..bank.num_frames {
            let frame = bank.frame_mut(f);
            for (i, sample) in frame.iter_mut().enumerate() {
                let p = i as f32 / frame_size as f32;
                *sample = (p * std::f32::consts::TAU).sin();
            }
        }
        bank
    }

    pub fn factory_formant() -> Self {
        let mut bank = Self::new(DEFAULT_NUM_FRAMES, DEFAULT_FRAME_SIZE);
        let frame_size = bank.frame_size;
        for f in 0..bank.num_frames {
            let vowel = f as f32 / (bank.num_frames - 1) as f32;
            let f1 = 300.0 + vowel * 400.0;
            let f2 = 800.0 + vowel * 1200.0;
            let frame = bank.frame_mut(f);
            for (i, sample) in frame.iter_mut().enumerate() {
                let p = i as f32 / frame_size as f32;
                let s1 = (p * f1 * 0.02 * std::f32::consts::TAU).sin();
                let s2 = (p * f2 * 0.015 * std::f32::consts::TAU).sin() * 0.5;
                *sample = (s1 + s2).clamp(-1.0, 1.0);
            }
        }
        bank
    }

    pub fn factory_metallic() -> Self {
        let mut bank = Self::new(DEFAULT_NUM_FRAMES, DEFAULT_FRAME_SIZE);
        let frame_size = bank.frame_size;
        for f in 0..bank.num_frames {
            let det = 1.0 + f as f32 * 0.003;
            let frame = bank.frame_mut(f);
            for (i, sample) in frame.iter_mut().enumerate() {
                let p = i as f32 / frame_size as f32;
                let mut v = 0.0f32;
                for h in 1..=8 {
                    v += (p * det * h as f32 * std::f32::consts::TAU).sin() / h as f32;
                }
                *sample = v.clamp(-1.0, 1.0);
            }
        }
        bank
    }
}

/// Fade the seam so frame[0] ≈ frame[last] (kills raw wrap clicks on factory tables).
fn periodize_frame(frame: &mut [f32]) {
    let n = frame.len();
    if n < 8 {
        return;
    }
    let fade = (n / 32).max(8).min(64);
    let start = frame[0];
    let end = frame[n - 1];
    let delta = start - end;
    for i in 0..fade {
        let w = (i as f32 + 1.0) / (fade as f32 + 1.0);
        frame[n - fade + i] += delta * w;
    }
    // Exact seam close — remaining kink is one sample, BLEP-friendly.
    frame[n - 1] = frame[0];
}

#[allow(dead_code)]
fn linear_crossfade(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

fn cubic_interp(y0: f32, y1: f32, y2: f32, y3: f32, t: f32) -> f32 {
    let a = -0.5 * y0 + 1.5 * y1 - 1.5 * y2 + 0.5 * y3;
    let b = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let c = -0.5 * y0 + 0.5 * y2;
    let d = y1;
    ((a * t + b) * t + c) * t + d
}

/// Spectral-power crossfade: interpolates energy while preserving phase from dominant frame.
/// Reduces beating vs linear amplitude blend when morphing wavetable frames.
fn spectral_crossfade(a: f32, b: f32, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 1e-6 {
        return a;
    }
    if t > 1.0 - 1e-6 {
        return b;
    }
    let power = (1.0 - t) * a * a + t * b * b;
    let sign = if (1.0 - t) * a.abs() >= t * b.abs() {
        a.signum()
    } else {
        b.signum()
    };
    if sign == 0.0 {
        power.sqrt() * if a + b >= 0.0 { 1.0 } else { -1.0 }
    } else {
        sign * power.sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pencil_segment_writes_samples() {
        let mut bank = WavetableBank::factory_sine();
        let before = bank.frame(0)[64];
        bank.apply_pencil_segment(0, 0.03, 0.2, 0.05, 0.8);
        let after = bank.frame(0)[64];
        assert!((after - before).abs() > 0.01);
    }

    #[test]
    fn spectral_differs_from_linear() {
        let a = 0.8_f32;
        let b = -0.6_f32;
        let t = 0.5;
        let lin = linear_crossfade(a, b, t);
        let spec = spectral_crossfade(a, b, t);
        assert!((lin - spec).abs() > 0.01);
    }

    #[test]
    fn morph_continuity() {
        let bank = WavetableBank::factory_saw_morph();
        let mut prev = bank.sample(0.0, 0.25);
        for f in 1..bank.num_frames {
            let pos = f as f32;
            let cur = bank.sample(pos, 0.25);
            assert!(
                (cur - prev).abs() < 0.15,
                "discontinuity at frame {f}: {prev} -> {cur}"
            );
            prev = cur;
        }
    }

    /// Phase wrap on saw_morph must stay within VA-blep ballpark after correction.
    #[test]
    fn phase_wrap_jump_is_bandlimited() {
        let bank = WavetableBank::factory_saw_morph();
        let phase_inc = 440.0 / 44_100.0;
        let pos = 0.0f32;

        let mut phase = 1.0 - 6.0 * phase_inc;
        let mut prev_raw = bank.sample(pos, phase);
        let mut prev = bank.sample_with_inc(pos, phase, phase_inc);
        let mut max_jump = 0.0f32;
        let mut max_raw = 0.0f32;
        for _ in 0..12 {
            phase = (phase + phase_inc).fract();
            let cur_raw = bank.sample(pos, phase);
            let cur = bank.sample_with_inc(pos, phase, phase_inc);
            max_raw = max_raw.max((cur_raw - prev_raw).abs());
            max_jump = max_jump.max((cur - prev).abs());
            prev_raw = cur_raw;
            prev = cur;
        }
        assert!(
            max_jump < 0.85,
            "phase-wrap jump too large (got {max_jump} raw={max_raw})"
        );
        assert!(
            max_jump <= max_raw + 0.02,
            "blep must not worsen wrap: blep={max_jump} raw={max_raw}"
        );
    }

    #[test]
    fn factory_lead_wt_position_wrap_is_bandlimited() {
        let bank = WavetableBank::factory_saw_morph();
        let phase_inc = 440.0 / 44_100.0;
        let pos = 108.0f32;

        let mut phase = 1.0 - 6.0 * phase_inc;
        let mut prev_raw = bank.sample(pos, phase);
        let mut prev = bank.sample_with_inc(pos, phase, phase_inc);
        let mut max_jump = 0.0f32;
        let mut max_raw = 0.0f32;
        for _ in 0..12 {
            phase = (phase + phase_inc).fract();
            let cur_raw = bank.sample(pos, phase);
            let cur = bank.sample_with_inc(pos, phase, phase_inc);
            max_raw = max_raw.max((cur_raw - prev_raw).abs());
            max_jump = max_jump.max((cur - prev).abs());
            prev_raw = cur_raw;
            prev = cur;
        }
        assert!(
            max_jump < 1.05 && max_jump <= max_raw + 0.02,
            "WT pos 108 wrap jump={max_jump} raw={max_raw}"
        );
    }

    #[test]
    fn control_point_roundtrip_sine() {
        let bank = WavetableBank::factory_sine();
        let frame = bank.frame(0);
        let points = WavetableBank::downsample_frame_control_points(frame, 64);
        let mut out = [0.0f32; 2048];
        WavetableBank::upsample_control_points_to_frame(&points, &mut out);
        let mut err = 0.0f32;
        for (a, b) in frame.iter().zip(out.iter()) {
            err += (a - b).abs();
        }
        err /= frame.len() as f32;
        assert!(err < 0.08, "mean err was {err}");
    }

    #[test]
    fn control_point_roundtrip_saw() {
        let mut frame = vec![0.0f32; 2048];
        for (i, s) in frame.iter_mut().enumerate() {
            let p = i as f32 / 2048.0;
            *s = 2.0 * p - 1.0;
        }
        let points = WavetableBank::downsample_frame_control_points(&frame, 128);
        let mut out = [0.0f32; 2048];
        WavetableBank::upsample_control_points_to_frame(&points, &mut out);
        let mut err = 0.0f32;
        for (a, b) in frame.iter().zip(out.iter()) {
            err += (a - b).abs();
        }
        err /= frame.len() as f32;
        assert!(err < 0.15, "mean err was {err}");
    }

    #[test]
    fn roundtrip_bytes() {
        let bank = WavetableBank::factory_sine();
        let bytes = bank.to_bytes();
        let restored = WavetableBank::from_bytes(&bytes).unwrap();
        assert_eq!(restored.num_frames, bank.num_frames);
        assert_eq!(restored.frame_size, bank.frame_size);
        assert!((restored.sample(0.0, 0.5) - bank.sample(0.0, 0.5)).abs() < 1e-5);
    }
}

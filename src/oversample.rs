//! 2× oversampling helpers for alias-sensitive FM + filter paths.

pub const OS_FACTOR: usize = 2;

/// Linear upsample one base-rate sample into `OS_FACTOR` sub-samples.
#[inline]
pub fn upsample_hold(input: f32) -> [f32; OS_FACTOR] {
    [input; OS_FACTOR]
}

/// Box average decimation after processing at 2× rate.
#[inline]
pub fn downsample_avg(samples: [f32; OS_FACTOR]) -> f32 {
    (samples[0] + samples[1]) * 0.5
}

/// Run a closure at 2× rate, returning decimated output.
#[inline]
pub fn process_os<F>(input: f32, mut f: F) -> f32
where
    F: FnMut(f32, usize) -> f32,
{
    let subs = upsample_hold(input);
    let mut acc = [0.0f32; OS_FACTOR];
    for (i, &s) in subs.iter().enumerate() {
        acc[i] = f(s, i);
    }
    downsample_avg(acc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_os() {
        let out = process_os(0.5, |x, _| x);
        assert!((out - 0.5).abs() < 1e-6);
    }
}

//! Embedded Noise2Noise seam restorer (SeamN2N parity with Python `baselines/n2n_seam.py`).
//!
//! Weights: `n2n_seam_weights.bin` exported from `n2n_corrupt_corrupt.pt` (≈53.5k params).
//! Inference resamples arbitrary cycle length → 256 → network → back (training L=256).

use std::sync::OnceLock;

const N2N_L: usize = 256;
const C: usize = 32;
const MC: usize = 8;

/// Flat f32 LE dump in fixed tensor order (see `scripts/export_n2n_weights_bin.py` / export one-liner).
static WEIGHTS_BIN: &[u8] = include_bytes!("n2n_seam_weights.bin");

struct Weights {
    wet: f32,
    enc1_0_w: Vec<f32>, // [32,1,5]
    enc1_0_b: Vec<f32>,
    enc1_2_w: Vec<f32>, // [32,32,5]
    enc1_2_b: Vec<f32>,
    down_w: Vec<f32>, // [64,32,4]
    down_b: Vec<f32>,
    mid0_w: Vec<f32>, // [64,64,3]
    mid0_b: Vec<f32>,
    mid2_w: Vec<f32>,
    mid2_b: Vec<f32>,
    up_w: Vec<f32>, // [64,32,4] weight shape for ConvTranspose1d
    up_b: Vec<f32>,
    dec0_w: Vec<f32>, // [32,64,3]
    dec0_b: Vec<f32>,
    dec2_w: Vec<f32>, // [8,32,3]
    dec2_b: Vec<f32>,
    dec4_w: Vec<f32>, // [1,8,3]
    dec4_b: Vec<f32>,
}

fn load_weights() -> Weights {
    let f = |off: &mut usize, n: usize| -> Vec<f32> {
        let bytes = &WEIGHTS_BIN[*off * 4..(*off + n) * 4];
        *off += n;
        bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    };
    let mut off = 0usize;
    let wet = f(&mut off, 1)[0];
    Weights {
        wet,
        enc1_0_w: f(&mut off, C * 1 * 5),
        enc1_0_b: f(&mut off, C),
        enc1_2_w: f(&mut off, C * C * 5),
        enc1_2_b: f(&mut off, C),
        down_w: f(&mut off, 64 * C * 4),
        down_b: f(&mut off, 64),
        mid0_w: f(&mut off, 64 * 64 * 3),
        mid0_b: f(&mut off, 64),
        mid2_w: f(&mut off, 64 * 64 * 3),
        mid2_b: f(&mut off, 64),
        up_w: f(&mut off, 64 * C * 4),
        up_b: f(&mut off, C),
        dec0_w: f(&mut off, C * 64 * 3),
        dec0_b: f(&mut off, C),
        dec2_w: f(&mut off, MC * C * 3),
        dec2_b: f(&mut off, MC),
        dec4_w: f(&mut off, 1 * MC * 3),
        dec4_b: f(&mut off, 1),
    }
}

fn weights() -> &'static Weights {
    static W: OnceLock<Weights> = OnceLock::new();
    W.get_or_init(load_weights)
}

#[inline]
fn gelu(x: f32) -> f32 {
    // tanh approximation matching common GELU
    let c = (2.0 / std::f32::consts::PI).sqrt();
    0.5 * x * (1.0 + (c * (x + 0.044715 * x * x * x)).tanh())
}

/// Conv1d NCHW single batch: in [C_in, L] → out [C_out, L] with padding.
fn conv1d(
    input: &[f32],
    c_in: usize,
    len: usize,
    weight: &[f32], // [C_out, C_in, K]
    bias: &[f32],
    c_out: usize,
    k: usize,
    stride: usize,
    padding: usize,
) -> (Vec<f32>, usize) {
    let out_len = (len + 2 * padding - k) / stride + 1;
    let mut out = vec![0.0f32; c_out * out_len];
    for oc in 0..c_out {
        for ol in 0..out_len {
            let mut acc = bias[oc];
            let in_start = ol * stride;
            for ic in 0..c_in {
                for kk in 0..k {
                    let ip = in_start + kk;
                    if ip >= padding && ip < padding + len {
                        let ix = ip - padding;
                        let w = weight[oc * c_in * k + ic * k + kk];
                        acc += w * input[ic * len + ix];
                    }
                    // else zero pad
                }
            }
            out[oc * out_len + ol] = acc;
        }
    }
    (out, out_len)
}

/// ConvTranspose1d stride=2, kernel=4, padding=1 (PyTorch default out_padding=0).
fn conv_transpose1d_s2(
    input: &[f32],
    c_in: usize,
    len: usize,
    weight: &[f32], // PyTorch: [C_in, C_out, K]
    bias: &[f32],
    c_out: usize,
    k: usize,
) -> (Vec<f32>, usize) {
    let stride = 2usize;
    let padding = 1usize;
    let out_len = (len - 1) * stride + k - 2 * padding;
    let mut out = vec![0.0f32; c_out * out_len];
    for ic in 0..c_in {
        for il in 0..len {
            let base = il * stride;
            let v = input[ic * len + il];
            for oc in 0..c_out {
                for kk in 0..k {
                    let op = base + kk;
                    if op >= padding && op < padding + out_len {
                        let ox = op - padding;
                        // weight layout [C_in, C_out, K]
                        let w = weight[ic * c_out * k + oc * k + kk];
                        out[oc * out_len + ox] += w * v;
                    }
                }
            }
        }
    }
    for oc in 0..c_out {
        for ol in 0..out_len {
            out[oc * out_len + ol] += bias[oc];
        }
    }
    (out, out_len)
}

fn linear_resample(src: &[f32], dst_len: usize) -> Vec<f32> {
    let n = src.len();
    if n == 0 || dst_len == 0 {
        return vec![0.0; dst_len];
    }
    if n == dst_len {
        return src.to_vec();
    }
    let mut out = vec![0.0f32; dst_len];
    for i in 0..dst_len {
        let t = if dst_len == 1 {
            0.0
        } else {
            i as f32 / (dst_len - 1) as f32
        };
        let x = t * (n - 1) as f32;
        let i0 = x.floor() as usize;
        let i1 = (i0 + 1).min(n - 1);
        let f = x - i0 as f32;
        out[i] = src[i0] * (1.0 - f) + src[i1] * f;
    }
    out
}

fn forward_256(x: &[f32; N2N_L], w: &Weights) -> [f32; N2N_L] {
    // enc1
    let (e0, l0) = conv1d(x, 1, N2N_L, &w.enc1_0_w, &w.enc1_0_b, C, 5, 1, 2);
    let e0: Vec<f32> = e0.into_iter().map(gelu).collect();
    let (e1, l1) = conv1d(&e0, C, l0, &w.enc1_2_w, &w.enc1_2_b, C, 5, 1, 2);
    let e = e1.into_iter().map(gelu).collect::<Vec<_>>();
    debug_assert_eq!(l1, N2N_L);

    let (z0, lz) = conv1d(&e, C, l1, &w.down_w, &w.down_b, 64, 4, 2, 1);
    let (m0, lm) = conv1d(&z0, 64, lz, &w.mid0_w, &w.mid0_b, 64, 3, 1, 1);
    let m0: Vec<f32> = m0.into_iter().map(gelu).collect();
    let (m1, _) = conv1d(&m0, 64, lm, &w.mid2_w, &w.mid2_b, 64, 3, 1, 1);
    let z = m1.into_iter().map(gelu).collect::<Vec<_>>();

    let (u0, lu) = conv_transpose1d_s2(&z, 64, lz, &w.up_w, &w.up_b, C, 4);
    // match encoder length if needed
    let u = if lu != l1 {
        let mut resized = vec![0.0f32; C * l1];
        for c in 0..C {
            let row = linear_resample(&u0[c * lu..(c + 1) * lu], l1);
            resized[c * l1..(c + 1) * l1].copy_from_slice(&row);
        }
        resized
    } else {
        u0
    };

    // concat u, e on channel → [64, L]
    let mut cat = vec![0.0f32; 64 * l1];
    for i in 0..l1 {
        for c in 0..C {
            cat[c * l1 + i] = u[c * l1 + i];
            cat[(C + c) * l1 + i] = e[c * l1 + i];
        }
    }

    let (d0, ld) = conv1d(&cat, 64, l1, &w.dec0_w, &w.dec0_b, C, 3, 1, 1);
    let d0: Vec<f32> = d0.into_iter().map(gelu).collect();
    let (d1, _) = conv1d(&d0, C, ld, &w.dec2_w, &w.dec2_b, MC, 3, 1, 1);
    let d1: Vec<f32> = d1.into_iter().map(gelu).collect();
    let (d2, _) = conv1d(&d1, MC, ld, &w.dec4_w, &w.dec4_b, 1, 3, 1, 1);

    let wet = w.wet.clamp(0.0, 1.0);
    let mut out = [0.0f32; N2N_L];
    for i in 0..N2N_L {
        out[i] = x[i] * (1.0 - wet) + d2[i] * wet;
    }
    out
}

/// In-place Noise2Noise seam restore (wet-blend U-Net lite).
pub fn apply_n2n_seam(frame: &mut [f32]) {
    if frame.len() < 2 {
        return;
    }
    let w = weights();
    let small = linear_resample(frame, N2N_L);
    let mut arr = [0.0f32; N2N_L];
    arr.copy_from_slice(&small);
    let restored = forward_256(&arr, w);
    let back = linear_resample(&restored, frame.len());
    frame.copy_from_slice(&back);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_load_and_forward_finite() {
        let w = weights();
        assert!(w.wet.is_finite());
        assert_eq!(WEIGHTS_BIN.len(), 53506 * 4);
        let mut x = [0.0f32; N2N_L];
        for (i, s) in x.iter_mut().enumerate() {
            *s = (i as f32 * 0.07).sin();
        }
        x[0] = 0.9;
        x[N2N_L - 1] = -0.9;
        let y = forward_256(&x, w);
        assert!(y.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn apply_on_2048_finite() {
        let mut frame = vec![0.0f32; 2048];
        for (i, s) in frame.iter_mut().enumerate() {
            *s = (i as f32 * 0.01).sin();
        }
        frame[0] = 1.0;
        frame[2047] = -1.0;
        apply_n2n_seam(&mut frame);
        assert!(frame.iter().all(|v| v.is_finite()));
    }
}

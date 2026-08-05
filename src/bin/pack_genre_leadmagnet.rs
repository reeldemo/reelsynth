//! Generate 500 original genre-tagged `.reelwt` banks for the ReelSynth lead magnet (MIT).
//!
//! Usage:
//!   cargo run --release --bin reelsynth-pack-genre-leadmagnet -- <output_dir>
//!
//! Banks use 128 frames × 2048 samples (native-compatible, smaller download than full 256).

use reelsynth::wavetable::{WavetableBank, DEFAULT_FRAME_SIZE};
use std::f32::consts::TAU;
use std::path::PathBuf;
use std::process::ExitCode;

const NUM_FRAMES: usize = 128;
const TARGET_COUNT: usize = 500;

fn main() -> ExitCode {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "dist/reelsynth-genre-tables".into()),
    );
    if let Err(e) = std::fs::create_dir_all(&out) {
        eprintln!("create output: {e}");
        return ExitCode::FAILURE;
    }

    let specs = build_specs();
    assert_eq!(
        specs.len(),
        TARGET_COUNT,
        "expected {TARGET_COUNT} specs, got {}",
        specs.len()
    );

    for (i, spec) in specs.iter().enumerate() {
        let dir = out.join(&spec.genre);
        if let Err(e) = std::fs::create_dir_all(&dir) {
            eprintln!("mkdir {}: {e}", dir.display());
            return ExitCode::FAILURE;
        }
        let bank = render_bank(spec);
        let path = dir.join(format!("{:03}_{}.reelwt", i + 1, sanitize(&spec.name)));
        if let Err(e) = bank.write_file(path.to_str().unwrap_or_default()) {
            eprintln!("write {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
        if (i + 1) % 50 == 0 || i + 1 == TARGET_COUNT {
            println!("wrote {} / {TARGET_COUNT}", i + 1);
        }
    }

    let mut by_genre: Vec<(String, usize)> = Vec::new();
    for spec in &specs {
        if let Some((_, n)) = by_genre.iter_mut().find(|(g, _)| g == &spec.genre) {
            *n += 1;
        } else {
            by_genre.push((spec.genre.clone(), 1));
        }
    }
    by_genre.sort_by(|a, b| a.0.cmp(&b.0));

    let mut catalog = String::from("# ReelSynth Genre Tables (500)\n\n");
    catalog.push_str("Original `.reelwt` wavetable banks generated with the ReelSynth engine.\n\n");
    catalog.push_str("**License: MIT** — free for commercial and non-commercial use.\n\n");
    catalog.push_str("## How to load\n\n");
    catalog.push_str("1. Install ReelSynth: https://reeldemo.io/reelsynth/#download\n");
    catalog.push_str("2. **WT → Open .reelwt…** and pick any bank\n");
    catalog.push_str("3. Scrub wavetable position to morph through the 128 frames\n\n");
    catalog.push_str("## Contents\n\n");
    for (genre, n) in &by_genre {
        catalog.push_str(&format!("- **{genre}** — {n} banks\n"));
    }
    catalog.push_str("\n## Format\n\n");
    catalog.push_str(&format!(
        "- Native ReelSynth `.reelwt` (float32)\n- {NUM_FRAMES} frames × {DEFAULT_FRAME_SIZE} samples\n"
    ));
    catalog.push_str("- Not derived from third-party sample packs\n");

    if let Err(e) = std::fs::write(out.join("README.md"), catalog) {
        eprintln!("readme: {e}");
        return ExitCode::FAILURE;
    }
    if let Err(e) = std::fs::write(out.join("LICENSE"), MIT_LICENSE) {
        eprintln!("license: {e}");
        return ExitCode::FAILURE;
    }

    println!("done → {} ({TARGET_COUNT} banks)", out.display());
    ExitCode::SUCCESS
}

const MIT_LICENSE: &str = r#"MIT License

Copyright (c) 2026 Reeldemo

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
"#;

#[derive(Clone)]
struct Spec {
    genre: String,
    name: String,
    kind: Kind,
    seed: u32,
}

#[derive(Clone, Copy)]
enum Kind {
    WarmSaw,
    SoftSquare,
    PulseWidth,
    SyncSweep,
    SubGrowl,
    Reese,
    FmBell,
    FmPluck,
    FormantVowel,
    Metallic,
    Bitcrush,
    Wavefold,
    SuperSaw,
    OrganDrawbar,
    HollowPad,
    Glass,
    NoiseTone,
    OddHarm,
    EvenHarm,
    ChordStack,
    Acid,
    PluckEp,
    Brass,
    Choir,
    Glitch,
}

fn build_specs() -> Vec<Spec> {
    // Quotas sum to 500.
    let genres: &[(&str, usize, &[(Kind, &str)])] = &[
        (
            "Bass",
            55,
            &[
                (Kind::SubGrowl, "Sub Growl"),
                (Kind::Reese, "Reese"),
                (Kind::WarmSaw, "Warm Saw Bass"),
                (Kind::Acid, "Acid Bass"),
                (Kind::FmPluck, "FM Bass Pluck"),
                (Kind::Wavefold, "Fold Bass"),
                (Kind::SoftSquare, "Square Sub"),
                (Kind::SyncSweep, "Sync Bass"),
                (Kind::Bitcrush, "Grit Bass"),
                (Kind::NoiseTone, "Noise Sub"),
            ],
        ),
        (
            "House_Techno",
            50,
            &[
                (Kind::WarmSaw, "Peak Time Saw"),
                (Kind::PulseWidth, "PWM Stab"),
                (Kind::Acid, "303 Morph"),
                (Kind::SuperSaw, "Warehouse Saw"),
                (Kind::SoftSquare, "Minimal Square"),
                (Kind::Bitcrush, "Loft Crush"),
                (Kind::OrganDrawbar, "Organ Stab"),
                (Kind::ChordStack, "Chord Hit"),
                (Kind::SyncSweep, "Rave Sync"),
                (Kind::OddHarm, "Odd Stack"),
            ],
        ),
        (
            "Trap_HipHop",
            45,
            &[
                (Kind::SubGrowl, "808 Body"),
                (Kind::Reese, "Dark Reese"),
                (Kind::PluckEp, "EP Pluck"),
                (Kind::FmBell, "Trap Bell"),
                (Kind::HollowPad, "Smoke Pad"),
                (Kind::Wavefold, "Drill Fold"),
                (Kind::Bitcrush, "Tape Crush"),
                (Kind::Glass, "Sparkle Keys"),
                (Kind::NoiseTone, "Air Hit"),
            ],
        ),
        (
            "DnB_Breaks",
            40,
            &[
                (Kind::Reese, "Neuro Reese"),
                (Kind::SyncSweep, "Amen Sync"),
                (Kind::Glitch, "Glitch Slice"),
                (Kind::Wavefold, "Dist Fold"),
                (Kind::SuperSaw, "Liquid Saw"),
                (Kind::FmPluck, "Neuro Pluck"),
                (Kind::Bitcrush, "Jungle Crush"),
                (Kind::OddHarm, "Razor Stack"),
            ],
        ),
        (
            "Trance_Prog",
            40,
            &[
                (Kind::SuperSaw, "Anthem Saw"),
                (Kind::WarmSaw, "Prog Saw"),
                (Kind::PulseWidth, "Gate PWM"),
                (Kind::ChordStack, "Uplift Chord"),
                (Kind::HollowPad, "Horizon Pad"),
                (Kind::Glass, "Shimmer"),
                (Kind::EvenHarm, "Even Glow"),
                (Kind::FmBell, "Pluck Lead"),
            ],
        ),
        (
            "Ambient_Pad",
            45,
            &[
                (Kind::HollowPad, "Wide Pad"),
                (Kind::Choir, "Air Choir"),
                (Kind::Glass, "Ice Pad"),
                (Kind::FormantVowel, "Breath Formant"),
                (Kind::NoiseTone, "Dust Pad"),
                (Kind::EvenHarm, "Soft Even"),
                (Kind::WarmSaw, "Dawn Saw"),
                (Kind::Metallic, "Bow Metal"),
            ],
        ),
        (
            "Cinematic",
            40,
            &[
                (Kind::Brass, "Epic Brass"),
                (Kind::Choir, "Score Choir"),
                (Kind::Metallic, "Trailer Hit"),
                (Kind::HollowPad, "Tension Pad"),
                (Kind::FmBell, "Ostinato Bell"),
                (Kind::SubGrowl, "Low Drone"),
                (Kind::Glass, "Crystal Rise"),
                (Kind::OddHarm, "Dissonant Stack"),
            ],
        ),
        (
            "RnB_Soul",
            35,
            &[
                (Kind::PluckEp, "Rhodes Pluck"),
                (Kind::WarmSaw, "Velvet Saw"),
                (Kind::FormantVowel, "Ooh Formant"),
                (Kind::Choir, "Soft Stack"),
                (Kind::PulseWidth, "PWM Keys"),
                (Kind::HollowPad, "Silk Pad"),
                (Kind::FmBell, "Chime"),
            ],
        ),
        (
            "Indie_Pop",
            35,
            &[
                (Kind::PluckEp, "Jangle Pluck"),
                (Kind::WarmSaw, "Chorus Saw"),
                (Kind::OrganDrawbar, "Combo Organ"),
                (Kind::Glass, "Glock"),
                (Kind::SoftSquare, "Soft Lead"),
                (Kind::ChordStack, "Hook Chord"),
                (Kind::EvenHarm, "Bright Even"),
            ],
        ),
        (
            "Experimental",
            40,
            &[
                (Kind::Glitch, "Broken Clock"),
                (Kind::Bitcrush, "Aliased"),
                (Kind::Wavefold, "Fold Chaos"),
                (Kind::NoiseTone, "Static Morph"),
                (Kind::Metallic, "Inharmonic"),
                (Kind::SyncSweep, "Hard Sync FX"),
                (Kind::FormantVowel, "Alien Vowel"),
                (Kind::OddHarm, "Prime Stack"),
            ],
        ),
        (
            "Leads",
            35,
            &[
                (Kind::SuperSaw, "Lead Saw"),
                (Kind::SyncSweep, "Sync Lead"),
                (Kind::PulseWidth, "PWM Lead"),
                (Kind::FmPluck, "FM Lead"),
                (Kind::Brass, "Synth Brass"),
                (Kind::Acid, "Screamer"),
                (Kind::WarmSaw, "Classic Lead"),
            ],
        ),
        (
            "Keys_Plucks",
            40,
            &[
                (Kind::PluckEp, "EP Morph"),
                (Kind::FmBell, "Bell Keys"),
                (Kind::Glass, "Celeste"),
                (Kind::OrganDrawbar, "Drawbar"),
                (Kind::ChordStack, "Keys Chord"),
                (Kind::SoftSquare, "Clavi"),
                (Kind::EvenHarm, "Piano-ish"),
                (Kind::HollowPad, "Soft Keys"),
            ],
        ),
    ];

    let mut out = Vec::with_capacity(TARGET_COUNT);
    let mut seed_base = 0xC0FFEE_u32;
    for &(genre, count, kinds) in genres {
        push_genre_seeded(&mut out, genre, count, kinds, &mut seed_base);
    }
    out
}

fn push_genre_seeded(
    out: &mut Vec<Spec>,
    genre: &str,
    count: usize,
    kinds: &[(Kind, &str)],
    seed_base: &mut u32,
) {
    for i in 0..count {
        let (kind, label) = kinds[i % kinds.len()];
        let variant = i / kinds.len() + 1;
        let name = if i < kinds.len() {
            label.to_string()
        } else {
            format!("{label} {variant}")
        };
        *seed_base = seed_base.wrapping_mul(1664525).wrapping_add(1013904223);
        out.push(Spec {
            genre: genre.to_string(),
            name,
            kind,
            seed: *seed_base,
        });
    }
}

fn sanitize(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    s.trim_matches('_').to_string()
}

fn morph_t(frame: usize) -> f32 {
    if NUM_FRAMES <= 1 {
        0.0
    } else {
        frame as f32 / (NUM_FRAMES - 1) as f32
    }
}

fn periodize(frame: &mut [f32]) {
    let n = frame.len();
    if n < 8 {
        return;
    }
    let fade = (n / 64).max(4);
    let mid = 0.5 * (frame[0] + frame[n - 1]);
    for i in 0..fade {
        let t = i as f32 / fade as f32;
        let w = 0.5 - 0.5 * (t * TAU * 0.5).cos();
        frame[i] = frame[i] * (1.0 - w) + mid * w;
        let j = n - 1 - i;
        frame[j] = frame[j] * (1.0 - w) + mid * w;
    }
}

fn hash01(seed: u32, a: u32, b: u32) -> f32 {
    let mut x = seed
        .wrapping_mul(0x9E3779B9)
        .wrapping_add(a.wrapping_mul(0x85EBCA6B))
        .wrapping_add(b.wrapping_mul(0xC2B2AE35));
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846CA68B);
    x ^= x >> 16;
    (x as f32) / (u32::MAX as f32)
}

fn render_bank(spec: &Spec) -> WavetableBank {
    let mut bank = WavetableBank::new(NUM_FRAMES, DEFAULT_FRAME_SIZE);
    let bias = hash01(spec.seed, 1, 2);
    let tint = hash01(spec.seed, 3, 4);
    let drive = 0.85 + hash01(spec.seed, 5, 6) * 0.4;

    for f in 0..NUM_FRAMES {
        let m = (morph_t(f) + tint * 0.15).clamp(0.0, 1.0);
        for i in 0..DEFAULT_FRAME_SIZE {
            let p = i as f32 / DEFAULT_FRAME_SIZE as f32;
            let mut s = sample_kind(spec.kind, m, p, spec.seed, bias);
            s = (s * drive * 1.15).tanh();
            bank.frame_mut(f)[i] = s.clamp(-1.0, 1.0);
        }
        periodize(bank.frame_mut(f));
    }
    bank
}

fn sample_kind(kind: Kind, m: f32, p: f32, seed: u32, bias: f32) -> f32 {
    match kind {
        Kind::WarmSaw => {
            let n = 5 + (m * 12.0) as usize + (bias * 4.0) as usize;
            let mut v = 0.0;
            for h in 1..=n.max(1) {
                v += (p * h as f32 * TAU).sin() / h as f32;
            }
            v * 0.55
        }
        Kind::SoftSquare => {
            let n = 2 + (m * 10.0) as usize;
            let mut v = 0.0;
            for k in 0..n.max(1) {
                let h = 2 * k + 1;
                v += (p * h as f32 * TAU).sin() / h as f32;
            }
            v * 0.7
        }
        Kind::PulseWidth => {
            let pw = (0.08 + m * 0.75 + bias * 0.08).clamp(0.05, 0.95);
            if p < pw {
                1.0
            } else {
                -1.0
            }
        }
        Kind::SyncSweep => {
            let ratio = 1.0 + m * (5.0 + bias * 4.0);
            2.0 * (p * ratio).fract() - 1.0
        }
        Kind::SubGrowl => {
            let fund = (p * TAU).sin();
            let sub = (p * TAU * 0.5).sin() * (0.35 + m * 0.35);
            let grit = (p * TAU * (3.0 + m * 5.0)).sin().tanh() * (0.1 + m * 0.35);
            fund * 0.55 + sub + grit
        }
        Kind::Reese => {
            let det = 0.01 + m * 0.04 + bias * 0.01;
            let a = (p * TAU * (1.0 - det)).sin();
            let b = (p * TAU * (1.0 + det)).sin();
            let c = (p * TAU * 2.0).sin() * (0.2 + m * 0.25);
            (a + b) * 0.4 + c
        }
        Kind::FmBell => {
            let mod_idx = 1.0 + m * 6.0;
            (p * TAU + mod_idx * (p * TAU * (2.0 + bias)).sin()).sin() * (0.7 - m * 0.2)
        }
        Kind::FmPluck => {
            let mod_idx = 0.5 + m * 4.0;
            let ratio = 1.0 + (bias * 3.0).floor();
            (p * TAU + mod_idx * (p * TAU * ratio).sin()).sin() * 0.75
        }
        Kind::FormantVowel => {
            let f1 = 270.0 + m * 450.0 + bias * 40.0;
            let f2 = 800.0 + m * 1300.0;
            (p * f1 * 0.02 * TAU).sin() + (p * f2 * 0.015 * TAU).sin() * 0.45
        }
        Kind::Metallic => {
            let mut v = 0.0;
            let ratios = [1.0, 2.76, 5.15, 8.2, 11.4];
            for (i, r) in ratios.iter().enumerate() {
                v += (p * r * (1.0 + m * 0.08) * TAU).sin() / (i as f32 + 1.0);
            }
            v * 0.4
        }
        Kind::Bitcrush => {
            let saw = 2.0 * p - 1.0;
            let levels = 3.0 + m * 40.0 + bias * 8.0;
            (saw * levels).round() / levels
        }
        Kind::Wavefold => ((p * TAU).sin() * (1.0 + m * 5.0 + bias)).sin(),
        Kind::SuperSaw => {
            let voices = 3 + (m * 4.0) as i32;
            let mut v = 0.0;
            for k in 0..voices {
                let det = (k as f32 - voices as f32 * 0.5) * (0.004 + m * 0.01);
                v += (p * TAU * (1.0 + det)).sin();
            }
            v / voices as f32
        }
        Kind::OrganDrawbar => {
            let draws = [1.0, 0.5 + m * 0.5, 0.3, 0.2 + bias * 0.3, 0.15, m * 0.4];
            let harms = [1, 2, 3, 4, 6, 8];
            let mut v = 0.0;
            for (amp, h) in draws.iter().zip(harms.iter()) {
                v += (p * *h as f32 * TAU).sin() * amp;
            }
            v * 0.35
        }
        Kind::HollowPad => {
            let a = (p * TAU).sin();
            let b = (p * TAU * 2.0).sin() * (0.15 + m * 0.2);
            let c = (p * TAU * 3.0).sin() * 0.08;
            let air = ((p * 30.0 + m * 10.0).sin() * (p * 11.0).sin()) * (0.05 + m * 0.12);
            (a + b + c) * 0.55 + air
        }
        Kind::Glass => {
            let a = (p * TAU * (1.0 + m * 0.5)).sin();
            let b = (p * TAU * (4.1 + bias)).sin() * 0.3;
            let c = (p * TAU * (9.0 + m)).sin() * 0.1;
            (a + b + c) * 0.65
        }
        Kind::NoiseTone => {
            let n = hash01(seed, (p * 2048.0) as u32, (m * 1000.0) as u32) * 2.0 - 1.0;
            (p * TAU).sin() * (1.0 - m * 0.65) + n * (0.12 + m * 0.55)
        }
        Kind::OddHarm => {
            let n = 3 + (m * 14.0) as usize;
            let mut v = 0.0;
            for k in 0..n.max(1) {
                let h = 2 * k + 1;
                v += (p * h as f32 * TAU).sin() / h as f32;
            }
            v * 0.65
        }
        Kind::EvenHarm => {
            let mut v = (p * TAU).sin() * 0.45;
            let n = 2 + (m * 10.0) as usize;
            for k in 1..=n.max(1) {
                let h = 2 * k;
                v += (p * h as f32 * TAU).sin() / h as f32;
            }
            v * 0.55
        }
        Kind::ChordStack => {
            let r = (p * TAU).sin();
            let third = (p * TAU * (5.0 / 4.0 + bias * 0.02)).sin() * (0.35 + m * 0.25);
            let fifth = (p * TAU * (3.0 / 2.0)).sin() * (0.3 + m * 0.2);
            (r + third + fifth) * 0.45
        }
        Kind::Acid => {
            let pw = 0.15 + m * 0.55;
            let sq = if p < pw { 1.0 } else { -1.0 };
            let reso = (p * TAU * (2.0 + m * 8.0)).sin() * (0.15 + m * 0.4);
            sq * 0.55 + reso
        }
        Kind::PluckEp => {
            let mut v = 0.0;
            let n = 6 + (m * 10.0) as usize;
            for h in 1..=n.max(1) {
                let damp = (-0.08 * h as f32 * (0.5 + m)).exp();
                v += (p * h as f32 * TAU).sin() * damp / h as f32;
            }
            v * 0.7
        }
        Kind::Brass => {
            let mut v = 0.0;
            for h in 1..=8 {
                let amp = 1.0 / (h as f32).powf(0.7 + m * 0.4);
                v += (p * h as f32 * TAU).sin() * amp;
            }
            v * 0.4
        }
        Kind::Choir => {
            let f1 = 400.0 + m * 200.0;
            let f2 = 900.0 + m * 700.0;
            let f3 = 2200.0 + bias * 200.0;
            ((p * f1 * 0.015 * TAU).sin()
                + (p * f2 * 0.012 * TAU).sin() * 0.5
                + (p * f3 * 0.008 * TAU).sin() * 0.25)
                * 0.7
        }
        Kind::Glitch => {
            let steps = 6.0 + m * 40.0 + bias * 10.0;
            let q = (p * steps).floor() / steps;
            let tone = 2.0 * q - 1.0;
            let flip = if ((p * (3.0 + m * 8.0)) as u32) % 2 == 0 {
                1.0
            } else {
                -1.0
            };
            tone * flip * (0.7 + m * 0.3)
        }
    }
}

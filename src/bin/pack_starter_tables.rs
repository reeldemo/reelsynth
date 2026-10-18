//! Build the public ReelSynth Starter Tables lead-magnet pack (MIT, original content).
//!
//! Usage:
//!   cargo run --release --bin reelsynth-pack-starter-tables -- <output_dir>
//!
//! Writes categorized `.reelwt` banks + README.md under output_dir.

use reelsynth::wavetable::{WavetableBank, DEFAULT_FRAME_SIZE, DEFAULT_NUM_FRAMES};
use std::f32::consts::TAU;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "dist/reelsynth-starter-tables".into()),
    );
    if let Err(e) = std::fs::create_dir_all(&out) {
        eprintln!("create output: {e}");
        return ExitCode::FAILURE;
    }

    let tables: Vec<(&str, &str, WavetableBank)> = vec![
        ("Basics", "Saw Morph", WavetableBank::factory_saw_morph()),
        ("Basics", "Square Morph", WavetableBank::factory_square_morph()),
        ("Basics", "Sine", WavetableBank::factory_sine()),
        ("Basics", "Triangle Morph", triangle_morph()),
        ("Basics", "Pulse Width Morph", pulse_width_morph()),
        ("Formant", "Formant", WavetableBank::factory_formant()),
        ("Formant", "Vocal Ah", formant_shift(0.0)),
        ("Formant", "Vocal Oh", formant_shift(0.45)),
        ("Formant", "Vocal Ee", formant_shift(0.85)),
        ("Metallic", "Metallic", WavetableBank::factory_metallic()),
        ("Metallic", "Bell Partial", bell_partials()),
        ("Metallic", "Inharmonic Stack", inharmonic_stack()),
        ("Digital", "Bit Crush Morph", bitcrush_morph()),
        ("Digital", "Fold Morph", fold_morph()),
        ("Digital", "Aliased Saw", aliased_saw()),
        ("Analog", "Warm Saw", warm_saw()),
        ("Analog", "Soft Square", soft_square()),
        ("Analog", "Sync Sweep", sync_sweep()),
        ("Spectral", "Odd Harmonics", odd_harmonics()),
        ("Spectral", "Even Harmonics", even_harmonics()),
        ("Spectral", "Noise Seed Morph", noise_seed_morph()),
        ("Motion", "Breath Morph", breath_morph()),
        ("Motion", "Wobble Morph", wobble_morph()),
        ("Motion", "Glass Morph", glass_morph()),
    ];

    let mut count = 0usize;
    for (category, name, bank) in tables {
        let dir = out.join(category);
        if let Err(e) = std::fs::create_dir_all(&dir) {
            eprintln!("mkdir {}: {e}", dir.display());
            return ExitCode::FAILURE;
        }
        let file = format!("{}.reelwt", sanitize(name));
        let path = dir.join(&file);
        if let Err(e) = bank.write_file(path.to_str().unwrap_or_default()) {
            eprintln!("write {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
        println!("{} / {} ({}×{})", category, name, bank.num_frames, bank.frame_size);
        count += 1;
    }

    let readme = format!(
        r#"# ReelSynth Starter Tables

{count} original `.reelwt` wavetable banks for [ReelSynth](https://reeldemo.io/reelsynth/).

## How to load

1. Download and install ReelSynth from https://reeldemo.io/reelsynth/#download
2. Open the standalone app (or Ableton plugin editor)
3. **WT → Open .reelwt…** and pick any file in this pack
4. Or browse categories: Basics, Formant, Metallic, Digital, Analog, Spectral, Motion

## Import your own Serum / Vital / multicycle WAV tables

ReelSynth can import foreign tables without leaving your session:

- **WT → WAV table (multicycle)…** — Serum / Echo Sound Works style concatenated WAVs
- **WT → Vital (.vitaltable)…**
- **WT → WAV folder…** — one single-cycle WAV per frame
- **WT → Serum (.fxp)…** — ReelSynth RSWT subset

## License

MIT — same as ReelSynth. These banks are original Reeldemo content, not third-party sample packs.
Use them in commercial and non-commercial music freely.
"#
    );
    if let Err(e) = std::fs::write(out.join("README.md"), readme) {
        eprintln!("readme: {e}");
        return ExitCode::FAILURE;
    }

    println!("wrote {count} tables → {}", out.display());
    ExitCode::SUCCESS
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

fn morph_t(frame: usize) -> f32 {
    if DEFAULT_NUM_FRAMES <= 1 {
        0.0
    } else {
        frame as f32 / (DEFAULT_NUM_FRAMES - 1) as f32
    }
}

fn periodize(frame: &mut [f32]) {
    let n = frame.len();
    if n < 8 {
        return;
    }
    let fade = (n / 64).max(4);
    let a = frame[0];
    let b = frame[n - 1];
    let mid = 0.5 * (a + b);
    for i in 0..fade {
        let t = i as f32 / fade as f32;
        let w = 0.5 - 0.5 * (t * TAU * 0.5).cos();
        frame[i] = frame[i] * (1.0 - w) + mid * w;
        let j = n - 1 - i;
        frame[j] = frame[j] * (1.0 - w) + mid * w;
    }
}

fn map_bank(mut sample_at: impl FnMut(f32 /*morph*/, f32 /*phase*/) -> f32) -> WavetableBank {
    let mut bank = WavetableBank::new(DEFAULT_NUM_FRAMES, DEFAULT_FRAME_SIZE);
    for f in 0..bank.num_frames {
        let m = morph_t(f);
        for i in 0..bank.frame_size {
            let p = i as f32 / bank.frame_size as f32;
            bank.frame_mut(f)[i] = sample_at(m, p).clamp(-1.0, 1.0);
        }
        periodize(bank.frame_mut(f));
    }
    bank
}

fn triangle_morph() -> WavetableBank {
    map_bank(|m, p| {
        let tri = 1.0 - 4.0 * (p - 0.5).abs();
        let sine = (p * TAU).sin();
        tri * (1.0 - m) + sine * m
    })
}

fn pulse_width_morph() -> WavetableBank {
    map_bank(|m, p| {
        let pw = 0.1 + m * 0.8;
        if p < pw {
            1.0
        } else {
            -1.0
        }
    })
}

fn formant_shift(base: f32) -> WavetableBank {
    map_bank(move |m, p| {
        let vowel = (base + m * 0.35).clamp(0.0, 1.0);
        let f1 = 300.0 + vowel * 500.0;
        let f2 = 700.0 + vowel * 1400.0;
        let s1 = (p * f1 * 0.02 * TAU).sin();
        let s2 = (p * f2 * 0.015 * TAU).sin() * 0.45;
        s1 + s2
    })
}

fn bell_partials() -> WavetableBank {
    map_bank(|m, p| {
        let mut v = 0.0;
        let ratios = [1.0, 2.76, 5.4, 8.9];
        for (i, r) in ratios.iter().enumerate() {
            let amp = 1.0 / (i as f32 + 1.0) * (1.0 - 0.15 * m);
            v += (p * r * (1.0 + m * 0.05) * TAU).sin() * amp;
        }
        v * 0.55
    })
}

fn inharmonic_stack() -> WavetableBank {
    map_bank(|m, p| {
        let mut v = 0.0;
        for h in 1..=12 {
            let stretch = 1.0 + m * 0.08 * h as f32;
            v += (p * h as f32 * stretch * TAU).sin() / (h as f32).sqrt();
        }
        v * 0.35
    })
}

fn bitcrush_morph() -> WavetableBank {
    map_bank(|m, p| {
        let saw = 2.0 * p - 1.0;
        let levels = 4.0 + m * 28.0;
        (saw * levels).round() / levels
    })
}

fn fold_morph() -> WavetableBank {
    map_bank(|m, p| {
        let drive = 1.0 + m * 4.0;
        let x = (p * TAU).sin() * drive;
        x.sin()
    })
}

fn aliased_saw() -> WavetableBank {
    map_bank(|m, p| {
        let steps = 8.0 + m * 48.0;
        let q = (p * steps).floor() / steps;
        2.0 * q - 1.0
    })
}

fn warm_saw() -> WavetableBank {
    map_bank(|m, p| {
        let mut v = 0.0;
        let harmonics = 6 + (m * 10.0) as usize;
        for h in 1..=harmonics {
            v += (p * h as f32 * TAU).sin() / h as f32;
        }
        v * 0.55
    })
}

fn soft_square() -> WavetableBank {
    map_bank(|m, p| {
        let mut v = 0.0;
        let harmonics = 3 + (m * 9.0) as usize;
        for k in 0..harmonics {
            let h = 2 * k + 1;
            v += (p * h as f32 * TAU).sin() / h as f32;
        }
        v * 0.7
    })
}

fn sync_sweep() -> WavetableBank {
    map_bank(|m, p| {
        let ratio = 1.0 + m * 7.0;
        let slave = (p * ratio).fract();
        2.0 * slave - 1.0
    })
}

fn odd_harmonics() -> WavetableBank {
    map_bank(|m, p| {
        let mut v = 0.0;
        let n = 4 + (m * 12.0) as usize;
        for k in 0..n {
            let h = 2 * k + 1;
            v += (p * h as f32 * TAU).sin() / h as f32;
        }
        v * 0.65
    })
}

fn even_harmonics() -> WavetableBank {
    map_bank(|m, p| {
        let fund = (p * TAU).sin() * 0.5;
        let mut v = fund;
        let n = 2 + (m * 10.0) as usize;
        for k in 1..=n {
            let h = 2 * k;
            v += (p * h as f32 * TAU).sin() / h as f32;
        }
        v * 0.6
    })
}

fn noise_seed_morph() -> WavetableBank {
    map_bank(|m, p| {
        // Deterministic "noise" from hashed phase + morph (stable across frames).
        let x = (p * 12345.678 + m * 9876.543).sin() * 43758.5453;
        let n = x.fract() * 2.0 - 1.0;
        let tone = (p * TAU).sin();
        tone * (1.0 - m * 0.7) + n * (0.15 + m * 0.55)
    })
}

fn breath_morph() -> WavetableBank {
    map_bank(|m, p| {
        let air = ((p * 40.0 + m * 8.0).sin() * (p * 17.0).sin()) * (0.2 + m * 0.5);
        let body = (p * TAU).sin() * (1.0 - m * 0.4);
        body + air
    })
}

fn wobble_morph() -> WavetableBank {
    map_bank(|m, p| {
        let am = 1.0 - m * 0.5 * (p * TAU * (2.0 + m * 4.0)).sin().abs();
        (p * TAU).sin() * am
    })
}

fn glass_morph() -> WavetableBank {
    map_bank(|m, p| {
        let a = (p * TAU * (1.0 + m)).sin();
        let b = (p * TAU * (3.2 + m * 2.0)).sin() * 0.35;
        let c = (p * TAU * (7.1 + m)).sin() * 0.12;
        (a + b + c) * 0.7
    })
}

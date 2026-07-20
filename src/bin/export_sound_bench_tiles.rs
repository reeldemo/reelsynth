//! Export ≥20 Rust `sound_bench` engine/ideal tile pairs for the paper matrix.
//!
//! ```bash
//! cargo run -p reelsynth --release --bin export_sound_bench_tiles
//! ```

use reelsynth::sound_bench::{generate_sound, generate_sound_ideal, BenchFamily, BENCH_N};
use serde_json::json;
use std::fs;
use std::path::PathBuf;

fn main() {
    // Two seeds per family (family = seed % 10) → 20 tiles, byte-aligned with Rust generator.
    let seeds: Vec<u64> = BenchFamily::ALL
        .iter()
        .flat_map(|fam| {
            let base = fam.index() as u64;
            [base, base + BenchFamily::ALL.len() as u64]
        })
        .collect();
    assert!(seeds.len() >= 20);

    let mut tiles = Vec::with_capacity(seeds.len());
    for seed in seeds {
        let (fam_e, eng) = generate_sound(seed, BENCH_N);
        let (fam_i, ideal) = generate_sound_ideal(seed, BENCH_N);
        assert_eq!(fam_e, fam_i);
        let wrap_jump = (eng[0] - eng[BENCH_N - 1]).abs();
        tiles.push(json!({
            "seed": seed,
            "family": fam_e.label(),
            "family_index": fam_e.index(),
            "n": BENCH_N,
            "engine": eng,
            "ideal": ideal,
            "wrap_jump_engine": wrap_jump,
        }));
    }

    let out = PathBuf::from("brand/artifacts/sound_bench_tiles_20.json");
    fs::create_dir_all(out.parent().unwrap()).ok();
    let payload = json!({
        "source": "reelsynth::sound_bench::generate_sound / generate_sound_ideal",
        "n": BENCH_N,
        "n_tiles": tiles.len(),
        "note": "Byte-aligned Rust procedural families (not Python make_batch stand-ins).",
        "tiles": tiles,
    });
    fs::write(&out, serde_json::to_string(&payload).expect("serialize")).expect("write");
    eprintln!("wrote {} ({} tiles, N={})", out.display(), payload["n_tiles"], BENCH_N);
}

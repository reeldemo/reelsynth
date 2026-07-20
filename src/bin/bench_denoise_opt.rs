//! Fit DenoiseOpt on the ~100k procedural sound bench and write investigation JSON.
//!
//! ```bash
//! cargo run -p reelsynth --release --bin bench_denoise_opt
//! ```

use reelsynth::denoise_opt::FROZEN_THETA;
use reelsynth::sound_bench::{
    fit_denoise_on_bench, investigate_bench, BENCH_N, BENCH_SIZE,
};

fn main() {
    eprintln!("Fitting DenoiseOpt on {BENCH_SIZE} sounds (N={BENCH_N}, stride=5)…");
    let (theta, fit) = fit_denoise_on_bench(BENCH_SIZE, 5, BENCH_N, 3, 1);
    eprintln!("{}", serde_json::to_string_pretty(&fit).unwrap());

    eprintln!("Investigating full {BENCH_SIZE} bench…");
    let inv = investigate_bench(&theta, BENCH_SIZE, BENCH_N, 512);
    eprintln!("overall: {}", serde_json::to_string_pretty(&inv["overall"]).unwrap());
    eprintln!(
        "per_family: {}",
        serde_json::to_string_pretty(&inv["per_family"]).unwrap()
    );

    let dump = serde_json::json!({
        "fitted_theta": theta.as_slice(),
        "previous_frozen": FROZEN_THETA.as_slice(),
        "fit": fit,
        "overall": inv["overall"].clone(),
        "per_family": inv["per_family"].clone(),
        "delta_quality_vs_note": "compare fitted_theta vs FROZEN_THETA; lock if better",
    });
    std::fs::create_dir_all("brand/artifacts").ok();
    std::fs::write(
        "brand/artifacts/denoise_opt_bench_100k_fit.json",
        serde_json::to_string_pretty(&dump).unwrap(),
    )
    .expect("write fit json");
    eprintln!("wrote brand/artifacts/denoise_opt_bench_100k_fit.json");
    eprintln!("wrote brand/artifacts/denoise_opt_bench_100k.json");
}

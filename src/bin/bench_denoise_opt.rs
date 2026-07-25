//! Fit DenoiseOpt and write investigation / freeze-gate JSON.
//!
//! ```bash
//! # Legacy denoise+shape fit on ~100k bench
//! cargo run -p reelsynth --release --bin bench_denoise_opt
//!
//! # v10.1 R_blend freeze path (seam heal + body identity)
//! cargo run -p reelsynth --release --bin bench_denoise_opt -- --r-blend
//! ```

use reelsynth::denoise_opt::FROZEN_THETA;
use reelsynth::sound_bench::{
    fit_denoise_on_bench, fit_denoise_on_bench_r_blend, investigate_bench, r_blend_freeze_gate,
    BENCH_N, BENCH_SIZE,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let r_blend = args.iter().any(|a| a == "--r-blend" || a == "r_blend")
        || std::env::var("DENOISE_OPT_OBJECTIVE")
            .map(|v| v.eq_ignore_ascii_case("r_blend") || v.eq_ignore_ascii_case("rblend"))
            .unwrap_or(false);

    if r_blend {
        run_r_blend_freeze();
    } else {
        run_legacy_denoise_shape();
    }
}

fn run_legacy_denoise_shape() {
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

fn run_r_blend_freeze() {
    // Fit: stratified subsample of the 100k bench (fast enough for release lock).
    // Holdout: disjoint high seeds so gate is not fit-set contamination.
    let fit_count = std::env::var("DENOISE_OPT_FIT_COUNT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8_000usize);
    let fit_stride = std::env::var("DENOISE_OPT_FIT_STRIDE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8usize);
    let restarts = std::env::var("DENOISE_OPT_RESTARTS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5usize);
    let sweeps = std::env::var("DENOISE_OPT_SWEEPS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2usize);
    let holdout_count = std::env::var("DENOISE_OPT_HOLDOUT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_200usize);
    let holdout_start = 80_000u64;

    eprintln!(
        "v10.1 R_blend fit: count={fit_count} stride={fit_stride} n={BENCH_N} \
         restarts={restarts} sweeps={sweeps}"
    );
    let (theta, fit) =
        fit_denoise_on_bench_r_blend(fit_count, fit_stride, BENCH_N, restarts, sweeps);
    eprintln!("{}", serde_json::to_string_pretty(&fit).unwrap());

    eprintln!("freeze gate holdout_start={holdout_start} count={holdout_count}…");
    let gate = r_blend_freeze_gate(&theta, holdout_start, holdout_count, BENCH_N);
    eprintln!("{}", serde_json::to_string_pretty(&gate).unwrap());

    let dump = serde_json::json!({
        "protocol": "v10.1 R_blend freeze",
        "meta_champ_ref": "brand/artifacts/meta_approach_compare_v10/hybrid_lstm/checkpoint.json",
        "note": "Neural FitCell stays out of engine; this locks best in-engine DenoiseOpt θ under R_blend",
        "fitted_theta": theta.as_slice(),
        "previous_frozen": FROZEN_THETA.as_slice(),
        "fit": fit,
        "gate": gate,
    });
    std::fs::create_dir_all("brand/artifacts").ok();
    let path = "brand/artifacts/denoise_opt_v10_r_blend_freeze.json";
    std::fs::write(path, serde_json::to_string_pretty(&dump).unwrap()).expect("write freeze json");
    eprintln!("wrote {path}");
    if gate["lock_recommended"].as_bool().unwrap_or(false) {
        eprintln!("LOCK RECOMMENDED — update FROZEN_THETA to fitted_theta");
    } else {
        eprintln!("KEEP current FROZEN_THETA — candidate did not clearly beat frozen + DualCosine");
    }
}

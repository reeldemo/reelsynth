//! Meta-learning + hyperparameter / lit-combo / overnight multi-branch search.
//!
//! Primary objective: prolonged residual score ∈ [0,1] (1 = best).
//!
//! ```bash
//! # Lit-combo 500-iter release timing
//! cargo run -p reelsynth --release --bin bench_denoise_meta -- 500
//!
//! # Overnight multi-branch (~273k)
//! set DENOISE_META_MODE=overnight
//! cargo run -p reelsynth --release --bin bench_denoise_meta -- 273000
//!
//! # Short overnight smoke
//! set DENOISE_META_MODE=overnight
//! cargo run -p reelsynth --release --bin bench_denoise_meta -- 100
//! ```

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let env_trials = std::env::var("DENOISE_META_TRIALS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok());
    let n_trials = args
        .get(1)
        .and_then(|s| s.parse::<usize>().ok())
        .or(env_trials)
        .unwrap_or(1500);

    let mode_env = std::env::var("DENOISE_META_MODE")
        .unwrap_or_default()
        .to_lowercase();

    let overnight = mode_env == "overnight"
        || mode_env == "multi"
        || mode_env == "branches"
        || (mode_env.is_empty() && n_trials >= 10_000);

    let lit_combo = !overnight
        && (mode_env == "lit_combo"
            || mode_env == "combo"
            || (mode_env == "auto" && n_trials <= 500)
            || (mode_env.is_empty() && n_trials <= 500));

    let (val_fast, val_final) =     if overnight {
        if n_trials >= 10_000 {
            (32usize, 160usize)
        } else {
            (24usize, 48usize)
        }
    } else if lit_combo {
        if n_trials >= 500 {
            (80usize, 400usize)
        } else {
            (24usize, 48usize)
        }
    } else if n_trials < 1500 {
        (40usize, 80usize)
    } else {
        (400usize, 2000usize)
    };

    let mode_label = if overnight {
        "overnight"
    } else if lit_combo {
        "lit_combo"
    } else {
        "legacy"
    };
    eprintln!(
        "Running {n_trials} DenoiseOpt meta trials mode={mode_label} (residual objective, val_fast={val_fast}, val_final={val_final})…"
    );

    let report = if overnight {
        reelsynth::denoise_meta_overnight::run_overnight_meta_n(n_trials, val_fast, val_final)
    } else if lit_combo {
        reelsynth::denoise_meta::run_lit_combo_meta_n(n_trials, val_fast, val_final)
    } else {
        reelsynth::denoise_meta::run_meta_learning_search_n(n_trials, val_fast, val_final)
    };

    eprintln!(
        "champion: {}",
        serde_json::to_string_pretty(&report["champion"]).unwrap()
    );
    eprintln!(
        "benchmark_matrix_5: {}",
        serde_json::to_string_pretty(&report["benchmark_matrix_5"]).unwrap()
    );
    if let Some(baselines) = report.get("bake_baselines") {
        eprintln!(
            "bake_baselines (first 3): {}",
            serde_json::to_string_pretty(&baselines.as_array().map(|a| &a[..a.len().min(3)]))
                .unwrap_or_else(|_| "{}".into())
        );
    }
    eprintln!(
        "production_frozen residual={:.4} quality={:.4}",
        report["production_frozen"]["residual"]
            .as_f64()
            .unwrap_or(0.0),
        report["production_frozen"]["quality"]
            .as_f64()
            .unwrap_or(0.0)
    );

    let iter_sec = report
        .get("iterations_elapsed_sec")
        .and_then(|v| v.as_f64())
        .or_else(|| report.get("seconds").and_then(|v| v.as_f64()))
        .unwrap_or(0.0);
    let iter_ms = report
        .get("iterations_elapsed_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or((iter_sec * 1000.0) as u64);
    let total_sec = report
        .get("total_elapsed_sec")
        .and_then(|v| v.as_f64())
        .unwrap_or(iter_sec);

    eprintln!(
        "n_trials={} iterations_sec={:.3} total_sec={:.3} artifact={}",
        report["n_trials"],
        iter_sec,
        total_sec,
        report["artifact_path"].as_str().unwrap_or("?")
    );

    if overnight {
        println!("OVERNIGHT_WALL_TIME_SEC={:.6}", iter_sec);
        println!("OVERNIGHT_ITERS={}", n_trials);
        println!("OVERNIGHT_WALL_TIME_MS={}", iter_ms);
    } else if lit_combo && n_trials == 500 {
        println!("500_ITER_WALL_TIME_SEC={:.6}", iter_sec);
        println!("500_ITER_WALL_TIME_MS={}", iter_ms);
    } else if lit_combo {
        println!("{}_ITER_WALL_TIME_SEC={:.6}", n_trials, iter_sec);
    }
}

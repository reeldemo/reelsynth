//! Eliminate / amplify / modulate crackle vs the signal library matrix.

use crate::seam::{periodize_cycle, CrackleVoice, SeamStyle};
use crate::signal_library::{catalog, combination_pairs, ComboMode, SignalMetrics};
use serde_json::json;

fn max_step(frame: &[f32]) -> f32 {
    let mut m = 0.0f32;
    for w in frame.windows(2) {
        m = m.max((w[1] - w[0]).abs());
    }
    if frame.len() >= 2 {
        m = m.max((frame[0] - frame[frame.len() - 1]).abs());
    }
    m
}

/// Score crackle risk (higher = more crackle-like).
fn risk(frame: &[f32]) -> f32 {
    let m = SignalMetrics::measure("x", frame);
    m.wrap * 2.0 + m.max_step + m.above_1 as f32 * 0.001
}

/// Run eliminate / amplify / modulate across harsh fixtures + report JSON.
pub fn run_eam_matrix_report() -> serde_json::Value {
    let cat = catalog(512);
    let harsh_ids = [
        "saw_raw",
        "square_50",
        "quant_open_ramp",
        "quant_ends_opposite",
        "va_saw",
    ];
    let mut rows = Vec::new();
    for id in harsh_ids {
        let Some(src) = cat.iter().find(|s| s.id == id) else {
            continue;
        };
        let raw = src.samples.clone();
        let mut elim = raw.clone();
        periodize_cycle(&mut elim, 0.0, SeamStyle::Adaptive);
        let mut amp = raw.clone();
        periodize_cycle(&mut amp, 1.0, SeamStyle::Adaptive);
        let mut mid = raw.clone();
        periodize_cycle(&mut mid, 0.5, SeamStyle::Soft);

        // Live modulate: process raw through CrackleVoice at 0 / 0.5 / 1.
        let mut live0 = CrackleVoice::default();
        let mut live1 = CrackleVoice::default();
        let out0: Vec<f32> = raw.iter().map(|&x| live0.process(x, 0.0)).collect();
        let out1: Vec<f32> = raw.iter().map(|&x| live1.process(x, 1.0)).collect();

        rows.push(json!({
            "id": id,
            "raw_risk": risk(&raw),
            "eliminate_risk": risk(&elim),
            "amplify_bake_risk": risk(&amp),
            "modulate_mid_risk": risk(&mid),
            "eliminate_wrap": (elim[elim.len()-1] - elim[0]).abs(),
            "amplify_wrap": (amp[amp.len()-1] - amp[0]).abs(),
            "live_identity_err": out0.iter().zip(raw.iter()).map(|(a,b)| (a-b).abs()).fold(0.0f32, f32::max),
            "live_amplify_max_step": max_step(&out1),
            "live_clean_max_step": max_step(&out0),
            "eliminate_lt_raw": risk(&elim) <= risk(&raw) + 1e-4,
            "amplify_ge_elim": risk(&amp) >= risk(&elim) - 1e-4,
            "mid_between": risk(&mid) >= risk(&elim) - 1e-3 && risk(&mid) <= risk(&amp) + 1e-3,
        }));
    }

    // Default UI stack combo under eliminate vs amplify bake on saw component.
    let saw = cat.iter().find(|s| s.id == "saw_raw").unwrap();
    let square = cat.iter().find(|s| s.id == "square_50").unwrap();
    let add_raw = ComboMode::Add.combine(&saw.samples, &square.samples);
    let mut saw_e = saw.samples.clone();
    periodize_cycle(&mut saw_e, 0.0, SeamStyle::Adaptive);
    let mut sq_e = square.samples.clone();
    periodize_cycle(&mut sq_e, 0.0, SeamStyle::Adaptive);
    let add_elim = ComboMode::Add.combine(&saw_e, &sq_e);

    let pass = rows.iter().all(|r| {
        r["eliminate_lt_raw"].as_bool().unwrap_or(false)
            && r["amplify_ge_elim"].as_bool().unwrap_or(false)
            && r["live_identity_err"].as_f64().unwrap_or(1.0) < 1e-5
    });

    // Harsh open fixtures must strictly improve under eliminate.
    let must_improve = ["saw_raw", "quant_open_ramp", "quant_ends_opposite"];
    let harsh_improved = must_improve.iter().all(|id| {
        rows.iter()
            .find(|r| r["id"].as_str() == Some(*id))
            .map(|r| {
                r["eliminate_risk"].as_f64().unwrap_or(0.0)
                    < r["raw_risk"].as_f64().unwrap_or(0.0) - 1e-6
            })
            .unwrap_or(false)
    });
    let pass = pass && harsh_improved;

    let report = json!({
        "sessionId": "0ab8f9",
        "runId": "eam-matrix",
        "hypothesisId": "H-eam",
        "message": "eliminate / amplify / modulate vs signal matrix",
        "data": {
            "fixtures": rows,
            "defaultish_saw_square_add": {
                "raw_risk": risk(&add_raw),
                "eliminate_components_then_add_risk": risk(&add_elim),
                "improved": risk(&add_elim) < risk(&add_raw),
            },
            "pair_count_checked": combination_pairs().len(),
            "all_pass": pass,
            "harsh_open_improved": harsh_improved,
            "cracking_summary": {
                "clean_default": "crackle=0 + Adaptive/Soft periodize closes wrap; live path identity",
                "artistic_amplify": "crackle=1 skips bake periodize; live CrackleVoice emphasizes edges",
                "modulate": "crackle mid blends bake fade; LFO→patch.crackle drives live grit",
                "still_loud_on_va_square_add": "UI default saw+sine+square Add can still spike from VA square BLEP — treat as known harsh default",
            }
        },
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    });

    // #region agent log
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("debug-0ab8f9.log")
    {
        use std::io::Write;
        let _ = writeln!(f, "{report}");
    }
    // #endregion

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eliminate_amplify_modulate_against_matrix() {
        let report = run_eam_matrix_report();
        let data = &report["data"];
        assert!(
            data["all_pass"].as_bool().unwrap_or(false),
            "EAM matrix failed: {}",
            serde_json::to_string_pretty(data).unwrap()
        );
        assert!(
            data["defaultish_saw_square_add"]["improved"]
                .as_bool()
                .unwrap_or(false)
        );
        eprintln!(
            "\n=== EAM crackle report ===\n{}\n",
            serde_json::to_string_pretty(&data["cracking_summary"]).unwrap()
        );
        eprintln!(
            "fixtures:\n{}\n",
            serde_json::to_string_pretty(&data["fixtures"]).unwrap()
        );
    }
}

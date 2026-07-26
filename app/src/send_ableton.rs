//! Send current patch to Ableton User Library inbox (+ optional OSC).

use crate::ableton_osc::{probe_ableton_osc, push_wavetable_params};
use reelsynth::export::{
    default_ableton_inbox_root, write_ableton_send_bundle, ABLETON_MAP_SCHEMA,
};
use reelsynth::{Patch, WavetableBank};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct SendAbletonResult {
    pub inbox_dir: PathBuf,
    #[allow(dead_code)] // reserved for UI / callers
    pub osc_online: bool,
    pub status: String,
}

fn slug(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() {
        "patch".into()
    } else {
        s
    }
}

fn map_params_for_osc(preset: &Patch) -> Vec<(String, f32)> {
    // Mirror export normalization via reading the map file after write, or recompute.
    // Recompute with same rules as export (inline minimal set of alias display names).
    fn norm_cutoff_hz(cutoff: f32) -> f32 {
        let min = 20.0f32;
        let max = 20000.0f32;
        let c = cutoff.clamp(min, max);
        ((c.ln() - min.ln()) / (max.ln() - min.ln())).clamp(0.0, 1.0)
    }
    fn norm_time(seconds: f32, max: f32) -> f32 {
        (seconds / max).clamp(0.0, 1.0)
    }
    let pos = preset
        .oscillators
        .first()
        .map(|o| o.position)
        .unwrap_or(0.0);
    let osc1_pos = if pos <= 1.0 {
        pos.clamp(0.0, 1.0)
    } else {
        (pos / 255.0).clamp(0.0, 1.0)
    };
    vec![
        ("Osc 1 Position".into(), osc1_pos),
        ("Filter Freq".into(), norm_cutoff_hz(preset.filter.cutoff)),
        (
            "Filter Res".into(),
            preset.filter.resonance.clamp(0.0, 1.0),
        ),
        (
            "Amp Attack".into(),
            norm_time(preset.envelope.attack, 5.0),
        ),
        (
            "Amp Release".into(),
            norm_time(preset.envelope.release, 8.0),
        ),
    ]
}

pub fn send_to_ableton(
    preset: &Patch,
    bank: &WavetableBank,
    inbox_override: Option<&Path>,
) -> Result<SendAbletonResult, String> {
    let root = inbox_override
        .map(Path::to_path_buf)
        .unwrap_or_else(default_ableton_inbox_root);
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let bundle = root.join(format!("{}_{}", slug(&preset.name), ts));
    write_ableton_send_bundle(preset, bank, &bundle)?;

    let osc_online = probe_ableton_osc();
    let status = if osc_online {
        match push_wavetable_params(&map_params_for_osc(preset)) {
            Ok(msg) => format!(
                "Sent to {} ({ABLETON_MAP_SCHEMA}). {msg}",
                bundle.display()
            ),
            Err(e) => format!(
                "Sent to {} ({ABLETON_MAP_SCHEMA}). OSC push failed: {e}. Drag table_multicycle.wav onto Wavetable.",
                bundle.display()
            ),
        }
    } else {
        format!(
            "Sent to {} ({ABLETON_MAP_SCHEMA}). AbletonOSC offline — install AbletonOSC and enable the control surface, then drag table_multicycle.wav onto Wavetable. Frames are not auto-loaded.",
            bundle.display()
        )
    };

    // Best-effort open folder
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer")
            .arg(bundle.as_os_str())
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg(bundle.as_os_str())
            .spawn();
    }

    Ok(SendAbletonResult {
        inbox_dir: bundle,
        osc_online,
        status,
    })
}

//! `.reelpreset` → Ableton Wavetable param JSON (v2) + multi-cycle WAV.

use crate::export::wav::write_wav_mono;
use crate::export::{DroppedParam, ExportReport};
use crate::patch::Patch;
use crate::wavetable::WavetableBank;
use serde_json::json;
use std::path::{Path, PathBuf};

pub const ABLETON_MAP_SCHEMA: &str = "reelsynth-ableton-wt-v2";
pub const MULTICYCLE_REL: &str = "synth/ableton/table_multicycle.wav";
pub const MAP_REL: &str = "synth/ableton/wavetable_map.json";
pub const INBOX_ENV: &str = "REELSYNTH_ABLETON_INBOX";

fn norm_cutoff_hz(cutoff: f32) -> f32 {
    let min = 20.0f32;
    let max = 20000.0f32;
    let c = cutoff.clamp(min, max);
    ((c.ln() - min.ln()) / (max.ln() - min.ln())).clamp(0.0, 1.0)
}

fn norm_time(seconds: f32, max: f32) -> f32 {
    (seconds / max).clamp(0.0, 1.0)
}

fn osc_position_norm(preset: &Patch) -> f32 {
    let pos = preset
        .oscillators
        .first()
        .map(|o| o.position)
        .unwrap_or(0.0);
    if pos <= 1.0 {
        pos.clamp(0.0, 1.0)
    } else {
        (pos / 255.0).clamp(0.0, 1.0)
    }
}

fn collect_dropped(preset: &Patch) -> Vec<DroppedParam> {
    let mut dropped = Vec::new();
    if preset.mod_matrix.len() > 4 {
        for (i, _slot) in preset.mod_matrix.iter().enumerate().skip(4) {
            dropped.push(DroppedParam {
                path: format!("mod_matrix[{i}]"),
                reason: "Ableton v2 export provides macro hints only (4 slots)".into(),
            });
        }
    }
    if preset.sub_level > 0.0 {
        dropped.push(DroppedParam {
            path: "sub_level".into(),
            reason: "Wavetable device has no sub osc in v2 map".into(),
        });
    }
    if preset.noise_level > 0.0 {
        dropped.push(DroppedParam {
            path: "noise_level".into(),
            reason: "noise osc not mapped in v2".into(),
        });
    }
    if !preset.effects.is_empty() {
        dropped.push(DroppedParam {
            path: "effects".into(),
            reason: "FX chain not mapped to Ableton Wavetable in v2".into(),
        });
    }
    dropped
}

/// Write Ableton map JSON (v2). When `bank` is provided, `frames` metadata is filled.
pub fn export_ableton_map(preset: &Patch, out_path: &Path) -> ExportReport {
    export_ableton_map_v2(preset, None, out_path)
}

pub fn export_ableton_map_v2(
    preset: &Patch,
    bank: Option<&WavetableBank>,
    out_path: &Path,
) -> ExportReport {
    let dropped = collect_dropped(preset);
    let (frame_count, samples_per_frame) = bank
        .map(|b| (b.num_frames, b.frame_size))
        .unwrap_or((0, 0));

    let doc = json!({
        "schema": ABLETON_MAP_SCHEMA,
        "device": "ableton:wavetable",
        "contract_id": "ableton:wavetable",
        "patch_name": preset.name,
        "parameters": {
            "osc1_pos": osc_position_norm(preset),
            "filter_freq": norm_cutoff_hz(preset.filter.cutoff),
            "filter_res": preset.filter.resonance.clamp(0.0, 1.0),
            "amp_attack": norm_time(preset.envelope.attack, 5.0),
            "amp_release": norm_time(preset.envelope.release, 8.0),
        },
        "live_param_aliases": {
            "osc1_pos": ["Osc 1 Position", "osc1_pos"],
            "filter_freq": ["Filter Freq", "filter_cutoff", "filter_freq"],
            "filter_res": ["Filter Res", "filter_resonance", "filter_res"],
            "amp_attack": ["Amp Attack", "amp_attack"],
            "amp_release": ["Amp Release", "amp_release"],
        },
        "frames": {
            "dir": "synth/wav_frames/",
            "multi_cycle_wav": "synth/ableton/table_multicycle.wav",
            "frame_count": frame_count,
            "samples_per_frame": samples_per_frame,
        },
        "macro_hints": preset.mod_matrix.iter().take(4).enumerate().map(|(i, slot)| {
            json!({
                "macro": i + 1,
                "source": slot.source,
                "target": slot.target,
                "amount": slot.amount,
            })
        }).collect::<Vec<_>>(),
        "notes": "Custom sprite requires one user drag of multi_cycle_wav onto Wavetable; params may be applied via OSC/Extension.",
    });

    if let Some(parent) = out_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return ExportReport::fail("ableton", e.to_string());
        }
    }
    match serde_json::to_string_pretty(&doc) {
        Ok(text) => {
            if let Err(e) = std::fs::write(out_path, text) {
                ExportReport::fail("ableton", e.to_string())
            } else {
                ExportReport::ok("ableton", out_path.display().to_string()).with_dropped(dropped)
            }
        }
        Err(e) => ExportReport::fail("ableton", e.to_string()),
    }
}

/// Concatenate all bank frames into one mono 16-bit PCM WAV @ 44100.
pub fn export_ableton_multicycle_wav(bank: &WavetableBank, out_path: &Path) -> ExportReport {
    let mut samples = Vec::with_capacity(bank.num_frames * bank.frame_size);
    for fi in 0..bank.num_frames {
        samples.extend_from_slice(bank.frame(fi));
    }
    match write_wav_mono(out_path, &samples, 44100) {
        Ok(()) => ExportReport::ok("ableton_multicycle", out_path.display().to_string()),
        Err(e) => ExportReport::fail("ableton_multicycle", e),
    }
}

/// Platform default Ableton User Library ReelSynth inbox root.
pub fn default_ableton_inbox_root() -> PathBuf {
    if let Ok(p) = std::env::var(INBOX_ENV) {
        return PathBuf::from(p);
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(home) = std::env::var("USERPROFILE") {
            return PathBuf::from(home)
                .join("Documents")
                .join("Ableton")
                .join("User Library")
                .join("ReelSynth")
                .join("inbox");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Music")
                .join("Ableton")
                .join("User Library")
                .join("ReelSynth")
                .join("inbox");
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Ableton")
                .join("User Library")
                .join("ReelSynth")
                .join("inbox");
        }
    }
    PathBuf::from("ReelSynth").join("inbox")
}

pub fn ableton_send_readme() -> &'static str {
    "ReelSynth → Ableton (bridge)\n\
     \n\
     1. In Live, create or select the MIDI track with Wavetable (Send may have created one if AbletonOSC is running).\n\
     2. Drag synth/ableton/table_multicycle.wav onto the Wavetable sprite (waveform view).\n\
     3. Params in wavetable_map.json may already be applied via OSC; tweak by ear if needed.\n\
     \n\
     Custom wavetable frames are NOT loaded automatically (Ableton has no API for that).\n\
     For seamless play without drag, install the ReelSynth VST3 plugin when available.\n"
}

/// Write a Send-to-Ableton inbox folder (canonical + map + multicycle + frames + README).
pub fn write_ableton_send_bundle(
    preset: &Patch,
    bank: &WavetableBank,
    bundle_dir: &Path,
) -> Result<(), String> {
    std::fs::create_dir_all(bundle_dir).map_err(|e| e.to_string())?;
    let canonical = bundle_dir.join("canonical");
    std::fs::create_dir_all(&canonical).map_err(|e| e.to_string())?;
    let json = preset.to_json().map_err(|e| e.to_string())?;
    std::fs::write(canonical.join("patch.reelpreset"), json).map_err(|e| e.to_string())?;
    bank.write_file(canonical.join("table.reelwt").to_str().unwrap())
        .map_err(|e| e.to_string())?;

    let map_path = bundle_dir.join(MAP_REL);
    let map_report = export_ableton_map_v2(preset, Some(bank), &map_path);
    if !map_report.success {
        return Err(map_report.errors.join("; "));
    }

    let multi_path = bundle_dir.join(MULTICYCLE_REL);
    let multi_report = export_ableton_multicycle_wav(bank, &multi_path);
    if !multi_report.success {
        return Err(multi_report.errors.join("; "));
    }

    let frames_dir = bundle_dir.join("synth/wav_frames");
    let wav_report = crate::export::wav::export_wav_folder(bank, &frames_dir);
    if !wav_report.success {
        return Err(wav_report.errors.join("; "));
    }

    std::fs::write(bundle_dir.join("README.txt"), ableton_send_readme())
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::ModSlot;

    #[test]
    fn map_v2_schema_and_aliases() {
        let preset = Patch::factory_wt_lead();
        let bank = WavetableBank::factory_sine();
        let dir = std::env::temp_dir().join("reelsynth_ableton_v2");
        let _ = std::fs::create_dir_all(&dir);
        let out = dir.join("map.json");
        let report = export_ableton_map_v2(&preset, Some(&bank), &out);
        assert!(report.success);
        let parsed: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(parsed["schema"], ABLETON_MAP_SCHEMA);
        assert!(parsed["live_param_aliases"]["filter_freq"].is_array());
        assert_eq!(parsed["frames"]["frame_count"], bank.num_frames);
        assert_eq!(parsed["frames"]["samples_per_frame"], bank.frame_size);
    }

    #[test]
    fn multicycle_pcm_length() {
        let bank = WavetableBank::new(4, 8);
        let dir = std::env::temp_dir().join("reelsynth_ableton_multi");
        let _ = std::fs::create_dir_all(&dir);
        let out = dir.join("table_multicycle.wav");
        let report = export_ableton_multicycle_wav(&bank, &out);
        assert!(report.success);
        let bytes = std::fs::read(&out).unwrap();
        assert!(bytes.len() >= 44);
        let pcm_len = bytes.len() - 44;
        assert_eq!(pcm_len, 4 * 8 * 2);
    }

    #[test]
    fn dropped_fx_and_mod() {
        let mut preset = Patch::factory_wt_lead();
        preset.sub_level = 0.5;
        preset.mod_matrix = vec![
            ModSlot {
                source: "lfo1".into(),
                target: "a".into(),
                amount: 0.1,
                enabled: true,
            };
            5
        ];
        let dir = std::env::temp_dir().join("reelsynth_ableton_drop");
        let _ = std::fs::create_dir_all(&dir);
        let out = dir.join("map.json");
        let report = export_ableton_map_v2(&preset, None, &out);
        assert!(report.success);
        assert!(report.dropped.iter().any(|d| d.path == "sub_level"));
        assert!(report.dropped.iter().any(|d| d.path.starts_with("mod_matrix")));
    }
}

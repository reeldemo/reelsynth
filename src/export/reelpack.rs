//! `reelpack/` bundle orchestrator + merged export_report.json.

use crate::export::{
    export_preset, export_wavetable, load_preset, resolve_bank_for_preset, ExportOptions,
    ExportReport, ExportTarget,
};
use crate::patch::Patch;
use crate::wavetable::WavetableBank;
use serde_json::json;
use std::path::Path;

const DEFAULT_TARGETS: &[ExportTarget] = &[
    ExportTarget::Vital,
    ExportTarget::Wav,
    ExportTarget::Serum,
    ExportTarget::Ableton,
    ExportTarget::Sfz,
    ExportTarget::Midi,
    ExportTarget::Audio,
];

pub fn export_reelpack(
    preset_path: &Path,
    out_dir: &Path,
    targets: &[ExportTarget],
    opts: &ExportOptions,
) -> ExportReport {
    let preset = match load_preset(preset_path) {
        Ok(p) => p,
        Err(e) => return ExportReport::fail("reelpack", e),
    };
    let bank = match resolve_bank_for_preset(preset_path, &preset) {
        Ok(b) => b,
        Err(e) => return ExportReport::fail("reelpack", e),
    };
    export_reelpack_with(preset_path, &preset, &bank, out_dir, targets, opts)
}

pub fn export_reelpack_with(
    preset_path: &Path,
    preset: &Patch,
    bank: &WavetableBank,
    out_dir: &Path,
    targets: &[ExportTarget],
    opts: &ExportOptions,
) -> ExportReport {
    let use_targets: Vec<ExportTarget> = if targets.is_empty() {
        DEFAULT_TARGETS.to_vec()
    } else {
        targets.to_vec()
    };

    if let Err(e) = std::fs::create_dir_all(out_dir) {
        return ExportReport::fail("reelpack", e.to_string());
    }

    let canonical = out_dir.join("canonical");
    let synth = out_dir.join("synth");
    let daw = out_dir.join("daw");
    for dir in [&canonical, &synth, &daw] {
        if let Err(e) = std::fs::create_dir_all(dir) {
            return ExportReport::fail("reelpack", e.to_string());
        }
    }

    let mut children = Vec::new();
    let mut merged_dropped = Vec::new();
    let mut warnings = Vec::new();

    // Canonical copies
    let patch_out = canonical.join("patch.reelpreset");
    if let Ok(json) = preset.to_json() {
        let _ = std::fs::write(&patch_out, json);
    }
    let table_out = canonical.join("table.reelwt");
    if let Err(e) = bank.write_file(table_out.to_str().unwrap()) {
        warnings.push(format!("canonical table write: {e}"));
    }

    for target in &use_targets {
        if *target == ExportTarget::Reelpack {
            continue;
        }
        let child = export_target_in_bundle(preset, bank, out_dir, *target, opts);
        merged_dropped.extend(child.dropped.clone());
        warnings.extend(child.warnings.clone());
        children.push(child);
    }

    let manifest = json!({
        "schema": "reelsynth-reelpack-v1",
        "version": 1,
        "preset": preset.name,
        "source_preset": preset_path.display().to_string(),
        "targets": use_targets.iter().map(|t| t.as_str()).collect::<Vec<_>>(),
        "layout": {
            "canonical": "canonical/",
            "synth": "synth/",
            "daw": "daw/",
        },
    });
    let manifest_path = out_dir.join("reelpack.json");
    if let Ok(text) = serde_json::to_string_pretty(&manifest) {
        let _ = std::fs::write(&manifest_path, text);
    }

    let report_path = out_dir.join("export_report.json");
    let mut report = ExportReport {
        version: crate::export::EXPORT_REPORT_VERSION,
        target: "reelpack".into(),
        success: children.iter().any(|c| c.success),
        output_path: out_dir.display().to_string(),
        dropped: merged_dropped,
        warnings,
        errors: children
            .iter()
            .flat_map(|c| c.errors.clone())
            .collect(),
        children,
    };

    // Universal floor: ensure MIDI + audio even if synth exports fail
    let has_midi = report.children.iter().any(|c| c.target == "midi" && c.success);
    let has_audio = report.children.iter().any(|c| c.target == "audio" && c.success);
    if !has_midi {
        let midi_path = daw.join("midi").join("melody.mid");
        let floor = export_preset(
            preset,
            bank,
            ExportTarget::Midi,
            &midi_path,
            opts,
        );
        if floor.success {
            report.warnings.push("universal floor: emitted MIDI".into());
            report.children.push(floor);
        }
    }
    if !has_audio {
        let audio_path = daw.join("audio").join("melody.wav");
        let floor = export_preset(
            preset,
            bank,
            ExportTarget::Audio,
            &audio_path,
            opts,
        );
        if floor.success {
            report.warnings.push("universal floor: emitted audio WAV".into());
            report.children.push(floor);
        }
    }

    report.success = report.children.iter().any(|c| c.success);
    let _ = report.write_json(&report_path);
    report
}

fn export_target_in_bundle(
    preset: &Patch,
    bank: &WavetableBank,
    out_dir: &Path,
    target: ExportTarget,
    opts: &ExportOptions,
) -> ExportReport {
    match target {
        ExportTarget::Vital => {
            let path = out_dir.join("synth/vital/table.vitaltable");
            export_wavetable(bank, target, &path, opts)
        }
        ExportTarget::Wav => {
            let path = out_dir.join("synth/wav_frames");
            export_wavetable(bank, target, &path, opts)
        }
        ExportTarget::Serum => {
            let path = out_dir.join("synth/serum/patch_export.fxp");
            export_preset(preset, bank, target, &path, opts)
        }
        ExportTarget::Ableton => {
            let path = out_dir.join("synth/ableton/wavetable_map.json");
            export_preset(preset, bank, target, &path, opts)
        }
        ExportTarget::Sfz => {
            let path = out_dir.join("daw/sfz/patch.sfz");
            export_preset(preset, bank, target, &path, opts)
        }
        ExportTarget::Midi => {
            let path = out_dir.join("daw/midi/melody.mid");
            export_preset(preset, bank, target, &path, opts)
        }
        ExportTarget::Audio => {
            let path = out_dir.join("daw/audio/melody.wav");
            export_preset(preset, bank, target, &path, opts)
        }
        ExportTarget::Reelpack => ExportReport::fail("reelpack", "nested reelpack not supported"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::ModSlot;
    use std::io::Write;

    #[test]
    fn reelpack_bundle_layout() {
        let dir = std::env::temp_dir().join("reelsynth_reelpack_test");
        let _ = std::fs::remove_dir_all(&dir);
        let preset_dir = dir.join("src");
        std::fs::create_dir_all(&preset_dir).unwrap();
        let bank = WavetableBank::factory_sine();
        let wt_path = preset_dir.join("demo.reelwt");
        bank.write_file(wt_path.to_str().unwrap()).unwrap();
        let preset = Patch {
            name: "demo".into(),
            wavetable_id: Some("demo".into()),
            ..Patch::default_mono()
        };
        let preset_path = preset_dir.join("demo.reelpreset");
        std::fs::File::create(&preset_path)
            .unwrap()
            .write_all(preset.to_json().unwrap().as_bytes())
            .unwrap();

        let out = dir.join("demo.reelpack");
        let report = export_reelpack(
            &preset_path,
            &out,
            DEFAULT_TARGETS,
            &ExportOptions::default(),
        );
        assert!(report.success);
        assert!(out.join("reelpack.json").is_file());
        assert!(out.join("export_report.json").is_file());
        assert!(out.join("canonical/patch.reelpreset").is_file());
        assert!(out.join("canonical/table.reelwt").is_file());
        assert!(out.join("synth/vital/table.vitaltable").is_file());
        assert!(out.join("daw/midi/melody.mid").is_file());
        assert!(out.join("daw/audio/melody.wav").is_file());
    }

    #[test]
    fn export_report_lists_dropped_mod() {
        let dir = std::env::temp_dir().join("reelsynth_reelpack_mod_test");
        let _ = std::fs::remove_dir_all(&dir);
        let preset_dir = dir.join("src");
        std::fs::create_dir_all(&preset_dir).unwrap();
        let bank = WavetableBank::factory_sine();
        let wt_path = preset_dir.join("mod.reelwt");
        bank.write_file(wt_path.to_str().unwrap()).unwrap();
        let mut preset = Patch {
            name: "mod".into(),
            wavetable_id: Some("mod".into()),
            ..Patch::default_mono()
        };
        preset.mod_matrix = (0..16)
            .map(|i| ModSlot {
                source: "lfo1".into(),
                target: format!("osc{}_position", (i % 3) + 1),
                amount: 0.5,
                enabled: true,
            })
            .collect();
        let preset_path = preset_dir.join("mod.reelpreset");
        std::fs::write(&preset_path, preset.to_json().unwrap()).unwrap();
        let out = dir.join("mod.reelpack");
        let report = export_reelpack(
            &preset_path,
            &out,
            &[ExportTarget::Serum],
            &ExportOptions::default(),
        );
        assert!(report.dropped.iter().any(|d| d.path.starts_with("mod_matrix[")));
    }
}

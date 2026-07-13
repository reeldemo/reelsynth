mod ableton;
mod audio;
mod midi;
mod reelpack;
mod serum;
mod sfz;
mod vital;
mod wav;

pub use reelpack::export_reelpack;
pub use vital::export_vital;

use crate::patch::Patch;
use crate::wavetable::WavetableBank;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const EXPORT_REPORT_VERSION: u32 = 1;
pub const SERUM_MOD_SLOTS_V1: usize = 4;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DroppedParam {
    pub path: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportReport {
    pub version: u32,
    pub target: String,
    pub success: bool,
    pub output_path: String,
    pub dropped: Vec<DroppedParam>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ExportReport>,
}

impl ExportReport {
    pub fn ok(target: &str, output_path: impl Into<String>) -> Self {
        Self {
            version: EXPORT_REPORT_VERSION,
            target: target.into(),
            success: true,
            output_path: output_path.into(),
            dropped: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn fail(target: &str, message: impl Into<String>) -> Self {
        Self {
            version: EXPORT_REPORT_VERSION,
            target: target.into(),
            success: false,
            output_path: String::new(),
            dropped: Vec::new(),
            warnings: Vec::new(),
            errors: vec![message.into()],
            children: Vec::new(),
        }
    }

    pub fn with_dropped(mut self, dropped: Vec<DroppedParam>) -> Self {
        self.dropped = dropped;
        self
    }

    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings = warnings;
        self
    }

    pub fn write_json(&self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(path, text).map_err(|e| e.to_string())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExportTarget {
    Vital,
    Wav,
    Serum,
    Ableton,
    Sfz,
    Midi,
    Audio,
    Reelpack,
}

impl ExportTarget {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "vital" => Some(Self::Vital),
            "wav" => Some(Self::Wav),
            "serum" => Some(Self::Serum),
            "ableton" => Some(Self::Ableton),
            "sfz" => Some(Self::Sfz),
            "midi" => Some(Self::Midi),
            "audio" => Some(Self::Audio),
            "reelpack" => Some(Self::Reelpack),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vital => "vital",
            Self::Wav => "wav",
            Self::Serum => "serum",
            Self::Ableton => "ableton",
            Self::Sfz => "sfz",
            Self::Midi => "midi",
            Self::Audio => "audio",
            Self::Reelpack => "reelpack",
        }
    }
}

pub fn parse_targets(raw: &str) -> Vec<ExportTarget> {
    raw.split(',')
        .filter_map(|part| ExportTarget::parse(part))
        .collect()
}

#[derive(Clone, Debug)]
pub struct ExportOptions {
    pub freq: f32,
    pub duration: f32,
    pub midi_note: u8,
    pub sample_rate: u32,
    pub table_name: String,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            freq: 440.0,
            duration: 2.0,
            midi_note: 69,
            sample_rate: 44100,
            table_name: "reelsynth".into(),
        }
    }
}

pub fn export_wavetable(
    bank: &WavetableBank,
    target: ExportTarget,
    out_path: &Path,
    opts: &ExportOptions,
) -> ExportReport {
    match target {
        ExportTarget::Vital => vital::export_vital(bank, out_path, &opts.table_name),
        ExportTarget::Wav => wav::export_wav_folder(bank, out_path),
        ExportTarget::Serum => {
            serum::export_serum_wt(bank, &Patch::default_mono(), out_path, &opts.table_name)
        }
        ExportTarget::Reelpack => ExportReport::fail(
            "reelpack",
            "use export_reelpack() for bundle export (requires .reelpreset)",
        ),
        other => ExportReport::fail(
            other.as_str(),
            format!("target {} requires a .reelpreset patch", other.as_str()),
        ),
    }
}

pub fn export_preset(
    preset: &Patch,
    bank: &WavetableBank,
    target: ExportTarget,
    out_path: &Path,
    opts: &ExportOptions,
) -> ExportReport {
    match target {
        ExportTarget::Vital => vital::export_vital(bank, out_path, &opts.table_name),
        ExportTarget::Wav => wav::export_wav_folder(bank, out_path),
        ExportTarget::Serum => serum::export_serum_wt(bank, preset, out_path, &opts.table_name),
        ExportTarget::Ableton => ableton::export_ableton_map(preset, out_path),
        ExportTarget::Sfz => sfz::export_sfz(preset, bank, out_path, opts),
        ExportTarget::Midi => midi::export_midi(preset, out_path, opts),
        ExportTarget::Audio => audio::export_audio_wav(preset, bank, out_path, opts),
        ExportTarget::Reelpack => ExportReport::fail(
            "reelpack",
            "use export_reelpack() for bundle export",
        ),
    }
}

pub fn resolve_bank_for_preset(preset_path: &Path, preset: &Patch) -> Result<WavetableBank, String> {
    let preset_dir = preset_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| Path::new(".").to_path_buf());

    if let Ok(text) = std::fs::read_to_string(preset_path) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(wt_path) = v.get("wavetable_path").and_then(|p| p.as_str()) {
                let candidate = Path::new(wt_path);
                let resolved = if candidate.is_file() {
                    candidate.to_path_buf()
                } else {
                    preset_dir.join(candidate)
                };
                if resolved.is_file() {
                    return WavetableBank::read_file(resolved.to_str().unwrap());
                }
            }
        }
    }

    if let Some(id) = preset.wavetable_id.as_deref() {
        let sibling = preset_dir.join(format!("{id}.reelwt"));
        if sibling.is_file() {
            return WavetableBank::read_file(sibling.to_str().unwrap());
        }
    }

    Err(format!(
        "could not resolve .reelwt for preset {}",
        preset_path.display()
    ))
}

pub fn load_preset(path: &Path) -> Result<Patch, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    Patch::from_json(&text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::ModSlot;

    #[test]
    fn serum_export_reports_dropped_mod_slots() {
        let mut preset = Patch::default_mono();
        preset.mod_matrix = (0..16)
            .map(|i| ModSlot {
                source: format!("lfo{}", (i % 3) + 1),
                target: format!("osc{}_position", (i % 3) + 1),
                amount: 0.25,
                enabled: true,
            })
            .collect();
        let bank = WavetableBank::factory_sine();
        let dir = std::env::temp_dir().join("reelsynth_export_report_test");
        let _ = std::fs::create_dir_all(&dir);
        let out = dir.join("patch.fxp");
        let report = export_preset(
            &preset,
            &bank,
            ExportTarget::Serum,
            &out,
            &ExportOptions::default(),
        );
        assert!(report.success);
        assert_eq!(report.dropped.len(), 12);
        assert!(report.dropped.iter().all(|d| d.path.starts_with("mod_matrix[")));
    }
}

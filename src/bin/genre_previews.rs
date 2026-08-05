//! Render short morph-sweep WAV previews for each genre folder in the lead-magnet pack.
//!
//! Usage:
//!   cargo run --release --bin reelsynth-genre-previews -- <pack_dir> <output_dir>
//!
//! Expects the pack from `reelsynth-pack-genre-leadmagnet`. Writes 2 × 12 WAV files (~3 s each).

use reelsynth::export::{export_preset, ExportOptions, ExportTarget};
use reelsynth::patch::{Envelope, Filter, Lfo, Patch};
use reelsynth::wavetable::WavetableBank;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Clone, Copy)]
enum Profile {
    Sub,
    Bass,
    Mid,
    Lead,
    Pad,
    Pluck,
    Keys,
    Fx,
}

struct PreviewSpec {
    rel_path: &'static str,
    slug: &'static str,
    label: &'static str,
    profile: Profile,
}

const PREVIEWS: &[PreviewSpec] = &[
    // Bass
    PreviewSpec {
        rel_path: "Bass/001_Sub_Growl.reelwt",
        slug: "sub-growl",
        label: "Sub Growl",
        profile: Profile::Sub,
    },
    PreviewSpec {
        rel_path: "Bass/003_Warm_Saw_Bass.reelwt",
        slug: "warm-saw-bass",
        label: "Warm Saw Bass",
        profile: Profile::Bass,
    },
    // House / Techno
    PreviewSpec {
        rel_path: "House_Techno/056_Peak_Time_Saw.reelwt",
        slug: "peak-saw",
        label: "Peak Time Saw",
        profile: Profile::Mid,
    },
    PreviewSpec {
        rel_path: "House_Techno/058_303_Morph.reelwt",
        slug: "303-morph",
        label: "303 Morph",
        profile: Profile::Mid,
    },
    // Trap / Hip-Hop
    PreviewSpec {
        rel_path: "Trap_HipHop/106_808_Body.reelwt",
        slug: "808-body",
        label: "808 Body",
        profile: Profile::Sub,
    },
    PreviewSpec {
        rel_path: "Trap_HipHop/108_EP_Pluck.reelwt",
        slug: "ep-pluck",
        label: "EP Pluck",
        profile: Profile::Pluck,
    },
    // DnB / Breaks
    PreviewSpec {
        rel_path: "DnB_Breaks/151_Neuro_Reese.reelwt",
        slug: "neuro-reese",
        label: "Neuro Reese",
        profile: Profile::Bass,
    },
    PreviewSpec {
        rel_path: "DnB_Breaks/153_Glitch_Slice.reelwt",
        slug: "glitch-slice",
        label: "Glitch Slice",
        profile: Profile::Fx,
    },
    // Trance / Prog
    PreviewSpec {
        rel_path: "Trance_Prog/191_Anthem_Saw.reelwt",
        slug: "anthem-saw",
        label: "Anthem Saw",
        profile: Profile::Lead,
    },
    PreviewSpec {
        rel_path: "Trance_Prog/193_Gate_PWM.reelwt",
        slug: "gate-pwm",
        label: "Gate PWM",
        profile: Profile::Lead,
    },
    // Ambient / Pad
    PreviewSpec {
        rel_path: "Ambient_Pad/231_Wide_Pad.reelwt",
        slug: "wide-pad",
        label: "Wide Pad",
        profile: Profile::Pad,
    },
    PreviewSpec {
        rel_path: "Ambient_Pad/233_Ice_Pad.reelwt",
        slug: "ice-pad",
        label: "Ice Pad",
        profile: Profile::Pad,
    },
    // Cinematic
    PreviewSpec {
        rel_path: "Cinematic/276_Epic_Brass.reelwt",
        slug: "epic-brass",
        label: "Epic Brass",
        profile: Profile::Lead,
    },
    PreviewSpec {
        rel_path: "Cinematic/278_Trailer_Hit.reelwt",
        slug: "trailer-hit",
        label: "Trailer Hit",
        profile: Profile::Fx,
    },
    // RnB / Soul
    PreviewSpec {
        rel_path: "RnB_Soul/316_Rhodes_Pluck.reelwt",
        slug: "rhodes-pluck",
        label: "Rhodes Pluck",
        profile: Profile::Pluck,
    },
    PreviewSpec {
        rel_path: "RnB_Soul/318_Ooh_Formant.reelwt",
        slug: "ooh-formant",
        label: "Ooh Formant",
        profile: Profile::Keys,
    },
    // Indie / Pop
    PreviewSpec {
        rel_path: "Indie_Pop/351_Jangle_Pluck.reelwt",
        slug: "jangle-pluck",
        label: "Jangle Pluck",
        profile: Profile::Pluck,
    },
    PreviewSpec {
        rel_path: "Indie_Pop/354_Glock.reelwt",
        slug: "glock",
        label: "Glock",
        profile: Profile::Keys,
    },
    // Experimental
    PreviewSpec {
        rel_path: "Experimental/386_Broken_Clock.reelwt",
        slug: "broken-clock",
        label: "Broken Clock",
        profile: Profile::Fx,
    },
    PreviewSpec {
        rel_path: "Experimental/388_Fold_Chaos.reelwt",
        slug: "fold-chaos",
        label: "Fold Chaos",
        profile: Profile::Fx,
    },
    // Leads
    PreviewSpec {
        rel_path: "Leads/426_Lead_Saw.reelwt",
        slug: "lead-saw",
        label: "Lead Saw",
        profile: Profile::Lead,
    },
    PreviewSpec {
        rel_path: "Leads/428_PWM_Lead.reelwt",
        slug: "pwm-lead",
        label: "PWM Lead",
        profile: Profile::Lead,
    },
    // Keys / Plucks
    PreviewSpec {
        rel_path: "Keys_Plucks/461_EP_Morph.reelwt",
        slug: "ep-morph",
        label: "EP Morph",
        profile: Profile::Keys,
    },
    PreviewSpec {
        rel_path: "Keys_Plucks/463_Celeste.reelwt",
        slug: "celeste",
        label: "Celeste",
        profile: Profile::Keys,
    },
];

fn main() -> ExitCode {
    let pack_dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "dist/reelsynth-genre-tables".into()),
    );
    let out_dir = PathBuf::from(
        std::env::args()
            .nth(2)
            .unwrap_or_else(|| "dist/genre-previews".into()),
    );

    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        eprintln!("mkdir {}: {e}", out_dir.display());
        return ExitCode::FAILURE;
    }

    let mut ok = 0usize;
    for spec in PREVIEWS {
        let bank_path = pack_dir.join(spec.rel_path);
        let out_path = out_dir.join(format!("{}.wav", spec.slug));
        match render_preview(&bank_path, &out_path, spec.profile) {
            Ok(()) => {
                ok += 1;
                println!("{} → {}", spec.label, out_path.display());
            }
            Err(e) => eprintln!("{}: {e}", bank_path.display()),
        }
    }

    if ok == 0 {
        eprintln!("no previews rendered — is the pack at {}?", pack_dir.display());
        return ExitCode::FAILURE;
    }

    println!("done: {ok}/{} previews → {}", PREVIEWS.len(), out_dir.display());
    ExitCode::SUCCESS
}

fn render_preview(bank_path: &Path, out_path: &Path, profile: Profile) -> Result<(), String> {
    let bank = WavetableBank::read_file(bank_path.to_str().unwrap_or_default())
        .map_err(|e| format!("read: {e}"))?;
    let (freq, duration) = profile.tuning();
    let patch = profile.patch();
    let opts = ExportOptions {
        freq,
        duration,
        sample_rate: 44_100,
        ..Default::default()
    };
    let report = export_preset(&patch, &bank, ExportTarget::Audio, out_path, &opts);
    if report.success {
        Ok(())
    } else {
        Err(report.errors.join("; "))
    }
}

impl Profile {
    fn tuning(self) -> (f32, f32) {
        match self {
            Profile::Sub => (55.0, 3.0),
            Profile::Bass => (82.0, 3.0),
            Profile::Mid => (165.0, 2.8),
            Profile::Lead => (440.0, 2.8),
            Profile::Pad => (220.0, 4.0),
            Profile::Pluck => (330.0, 2.2),
            Profile::Keys => (392.0, 2.5),
            Profile::Fx => (220.0, 2.5),
        }
    }

    fn patch(self) -> Patch {
        let mut patch = Patch::default_mono();
        patch.filter = Filter {
            cutoff: self.cutoff(),
            resonance: self.resonance(),
            filter_type: "lowpass".into(),
            key_tracking: 0.0,
            drive: 0.0,
        };
        patch.envelope = self.envelope();
        patch.lfo = Lfo {
            rate: 0.12,
            depth: 64.0,
            target: "wt_position".into(),
            shape: "sine".into(),
        };
        patch
    }

    fn cutoff(self) -> f32 {
        match self {
            Profile::Sub | Profile::Bass => 900.0,
            Profile::Mid => 2800.0,
            Profile::Lead => 6000.0,
            Profile::Pad => 3200.0,
            Profile::Pluck | Profile::Keys => 4500.0,
            Profile::Fx => 8000.0,
        }
    }

    fn resonance(self) -> f32 {
        match self {
            Profile::Sub | Profile::Bass => 0.35,
            Profile::Fx => 0.55,
            _ => 0.25,
        }
    }

    fn envelope(self) -> Envelope {
        match self {
            Profile::Sub | Profile::Bass => Envelope {
                attack: 0.02,
                decay: 0.15,
                sustain: 0.85,
                release: 0.6,
            },
            Profile::Pad => Envelope {
                attack: 0.35,
                decay: 0.4,
                sustain: 0.75,
                release: 1.2,
            },
            Profile::Pluck | Profile::Keys => Envelope {
                attack: 0.005,
                decay: 0.35,
                sustain: 0.25,
                release: 0.35,
            },
            Profile::Fx => Envelope {
                attack: 0.01,
                decay: 0.2,
                sustain: 0.5,
                release: 0.4,
            },
            Profile::Mid | Profile::Lead => Envelope {
                attack: 0.01,
                decay: 0.2,
                sustain: 0.7,
                release: 0.5,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_manifest_has_two_per_genre() {
        assert_eq!(PREVIEWS.len(), 24);
        let genres: std::collections::HashSet<&str> = PREVIEWS
            .iter()
            .map(|p| p.rel_path.split('/').next().unwrap())
            .collect();
        assert_eq!(genres.len(), 12);
    }
}

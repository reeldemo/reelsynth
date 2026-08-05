//! Batch-import multicycle WAV tables → `.reelwt` (preserves category folders).
//!
//! Usage:
//!   cargo run --release --bin reelsynth-import-wav-tables -- <input_dir> <output_dir>
//!
//! Example (personal library conversion):
//!   cargo run --release --bin reelsynth-import-wav-tables -- \
//!     "C:/Users/.../Echo Sound Works Core Tables/Echo Sound Works Core Tables" \
//!     "C:/Users/.../ReelSynth/Libraries/ESW-Core-Tables"

use reelsynth::import::import_wav_multicycle;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(input) = args.next() else {
        eprintln!("usage: reelsynth-import-wav-tables <input_dir> <output_dir>");
        return ExitCode::FAILURE;
    };
    let Some(output) = args.next() else {
        eprintln!("usage: reelsynth-import-wav-tables <input_dir> <output_dir>");
        return ExitCode::FAILURE;
    };

    let input = PathBuf::from(input);
    let output = PathBuf::from(output);
    if !input.is_dir() {
        eprintln!("input is not a directory: {}", input.display());
        return ExitCode::FAILURE;
    }
    if let Err(e) = std::fs::create_dir_all(&output) {
        eprintln!("create output: {e}");
        return ExitCode::FAILURE;
    }

    let mut ok = 0usize;
    let mut fail = 0usize;
    let wavs = collect_wavs(&input);
    println!("found {} wav files under {}", wavs.len(), input.display());

    for wav in &wavs {
        let rel = wav.strip_prefix(&input).unwrap_or(wav.as_path());
        let mut out = output.join(rel);
        out.set_extension("reelwt");
        if let Some(parent) = out.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match import_wav_multicycle(wav.to_str().unwrap_or_default()) {
            Ok(bank) => match bank.write_file(out.to_str().unwrap_or_default()) {
                Ok(()) => {
                    ok += 1;
                    println!(
                        "ok  {} → {} ({}×{})",
                        rel.display(),
                        out.display(),
                        bank.num_frames,
                        bank.frame_size
                    );
                }
                Err(e) => {
                    fail += 1;
                    eprintln!("write failed {}: {e}", out.display());
                }
            },
            Err(e) => {
                fail += 1;
                eprintln!("import failed {}: {e}", rel.display());
            }
        }
    }

    println!("done: {ok} ok, {fail} failed");
    if fail > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn collect_wavs(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Skip macOS resource forks
                if path.file_name().and_then(|n| n.to_str()) == Some("__MACOSX") {
                    continue;
                }
                walk(&path, out);
            } else if path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("wav"))
                .unwrap_or(false)
            {
                out.push(path);
            }
        }
    }
    walk(root, &mut out);
    out.sort();
    out
}

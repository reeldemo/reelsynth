mod serum;
mod vital;
mod wav;

pub use serum::import_serum_fxp;
pub use vital::import_vital;
pub use wav::{import_wav_folder, import_wav_multicycle};

use crate::wavetable::WavetableBank;
use std::path::Path;

pub fn import_to_reelwt(source: &str, path: &str, out_path: &str) -> Result<WavetableBank, String> {
    let bank = match source {
        "vital" => import_vital(path)?,
        "wav" => {
            if Path::new(path).is_file() {
                import_wav_multicycle(path)?
            } else {
                import_wav_folder(path)?
            }
        }
        "wav_table" | "multicycle" | "wav_multicycle" => import_wav_multicycle(path)?,
        "serum" => import_serum_fxp(path)?,
        other => return Err(format!("unknown import source: {other}")),
    };
    bank.write_file(out_path)?;
    Ok(bank)
}

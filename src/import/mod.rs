mod vital;
mod wav;
mod serum;

pub use vital::import_vital;
pub use wav::import_wav_folder;
pub use serum::import_serum_fxp;

use crate::wavetable::WavetableBank;

pub fn import_to_reelwt(source: &str, path: &str, out_path: &str) -> Result<WavetableBank, String> {
    let bank = match source {
        "vital" => import_vital(path)?,
        "wav" => import_wav_folder(path)?,
        "serum" => import_serum_fxp(path)?,
        other => return Err(format!("unknown import source: {other}")),
    };
    bank.write_file(out_path)?;
    Ok(bank)
}

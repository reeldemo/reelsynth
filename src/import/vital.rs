//! Vital `.vitaltable` / preset JSON wavetable import.

use crate::wavetable::{WavetableBank, DEFAULT_FRAME_SIZE, DEFAULT_NUM_FRAMES};
use serde_json::Value;

pub fn import_vital(path: &str) -> Result<WavetableBank, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;

    // Vital exports: { "name": "...", "samples": [[frame0...], [frame1...], ...] }
    // or nested under "wavetables"
    let frames_val = if let Some(samples) = v.get("samples") {
        samples.clone()
    } else if let Some(wts) = v.get("wavetables").and_then(|w| w.as_array()).and_then(|a| a.first()) {
        wts.get("samples").cloned().unwrap_or(Value::Null)
    } else if let Some(data) = v.get("data").and_then(|d| d.as_array()) {
        Value::Array(data.clone())
    } else {
        return Err("no wavetable samples found in Vital file".into());
    };

    let frames_arr = frames_val
        .as_array()
        .ok_or("samples must be array of frames")?;

    let num_frames = frames_arr.len().max(1).min(DEFAULT_NUM_FRAMES);
    let mut bank = WavetableBank::new(num_frames, DEFAULT_FRAME_SIZE);

    for (fi, frame) in frames_arr.iter().take(num_frames).enumerate() {
        let samples: Vec<f32> = frame
            .as_array()
            .ok_or("frame must be array")?
            .iter()
            .filter_map(|s| s.as_f64().map(|f| f as f32))
            .collect();
        if !samples.is_empty() {
            bank.set_frame_from_cycle(fi, &samples);
        }
    }
    Ok(bank)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn import_vital_json() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_vital.vitaltable");
        let json = r#"{"name":"test","samples":[[0.0,1.0,0.0,-1.0],[0.5,0.5,-0.5,-0.5]]}"#;
        std::fs::File::create(&path)
            .unwrap()
            .write_all(json.as_bytes())
            .unwrap();
        let bank = import_vital(path.to_str().unwrap()).unwrap();
        assert_eq!(bank.num_frames, 2);
    }
}

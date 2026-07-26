//! Plugin DAW state: `reelsynth-plugin-state-v1` JSON (preset + reelwt bytes).

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use reelsynth::{Patch, WavetableBank};
use serde::{Deserialize, Serialize};

pub const PLUGIN_STATE_SCHEMA: &str = "reelsynth-plugin-state-v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginStateV1 {
    pub schema: String,
    pub preset: Patch,
    pub reelwt_b64: String,
}

impl PluginStateV1 {
    pub fn from_patch_bank(preset: &Patch, bank: &WavetableBank) -> Self {
        Self {
            schema: PLUGIN_STATE_SCHEMA.into(),
            preset: preset.clone(),
            reelwt_b64: B64.encode(bank.to_bytes()),
        }
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| e.to_string())
    }

    pub fn from_json(json: &str) -> Result<(Patch, WavetableBank), String> {
        let st: PluginStateV1 = serde_json::from_str(json).map_err(|e| e.to_string())?;
        st.into_patch_bank()
    }

    pub fn into_patch_bank(self) -> Result<(Patch, WavetableBank), String> {
        if self.schema != PLUGIN_STATE_SCHEMA {
            return Err(format!("unknown plugin state schema {}", self.schema));
        }
        let bytes = B64
            .decode(self.reelwt_b64.as_bytes())
            .map_err(|e| e.to_string())?;
        let bank = WavetableBank::from_bytes(&bytes)?;
        Ok((self.preset, bank))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_state_v1() {
        let preset = Patch::factory_wt_lead();
        let bank = WavetableBank::factory_sine();
        let json = PluginStateV1::from_patch_bank(&preset, &bank)
            .to_json()
            .unwrap();
        let (p2, b2) = PluginStateV1::from_json(&json).unwrap();
        assert_eq!(p2.name, preset.name);
        assert_eq!(b2.num_frames, bank.num_frames);
        assert_eq!(b2.frame_size, bank.frame_size);
    }
}

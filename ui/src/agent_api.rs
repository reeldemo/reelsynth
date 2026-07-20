//! Headless agent API for Design WT / Quant automation (no egui window required).
//!
//! Agents (and the MCP server) drive a [`AgentSession`] to select layers, set Quant /
//! seam modes, promote VA layers, and edit quant slots — then assert on snapshots.

use reelsynth::WavetableBank;
use serde::{Deserialize, Serialize};

use crate::quant_interp::WtQuantInterp;
use crate::state::UiState;
use crate::wt::{
    apply_quant_slot_amplitude, effective_quant_count, frame_index, layer_quant_editable,
    promote_va_layer_for_quant, quant_control_points, set_quant_seam_mode, QuantSeamMode,
    WtEditTool,
};

/// Mutable Design session an agent can drive without opening the GUI.
pub struct AgentSession {
    pub state: UiState,
    pub bank: WavetableBank,
}

impl Default for AgentSession {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentSession {
    pub fn new() -> Self {
        let mut state = UiState::default();
        state.wt_edit_tool = WtEditTool::Select;
        // Sensible Quant default so agent tests exercise knobs immediately.
        if let Some(osc) = state.oscillators.first_mut() {
            if osc.wave_quant == 0 {
                osc.wave_quant = 16;
            }
        }
        let bank = WavetableBank::factory_saw_morph();
        set_quant_seam_mode(state.wt_quant_seam);
        Self { state, bank }
    }

    pub fn snapshot(&self) -> AgentSnapshot {
        let osc = self.state.active_osc();
        let layers: Vec<AgentLayerSnap> = osc
            .wave_layers
            .iter()
            .enumerate()
            .map(|(i, l)| AgentLayerSnap {
                index: i,
                source_type: l.source_type.clone(),
                level: l.level,
                enabled: l.enabled,
                wavetable: l.is_wavetable(),
                residual: l.residual,
                quant_editable: layer_quant_editable(l),
            })
            .collect();
        AgentSnapshot {
            selected_layer: self.state.selected_layer_idx,
            wave_quant: osc.wave_quant,
            seam_mode: self.state.wt_quant_seam.label().to_string(),
            tool: format!("{:?}", self.state.wt_edit_tool),
            layers,
            bank_frames: self.bank.num_frames,
            bank_frame_size: self.bank.frame_size,
        }
    }

    pub fn select_layer(&mut self, index: usize) -> Result<(), String> {
        let n = self.state.active_osc().wave_layers.len();
        if index >= n {
            return Err(format!("layer {index} out of range (0..{n})"));
        }
        self.state.selected_layer_idx = Some(index);
        Ok(())
    }

    pub fn set_wave_quant(&mut self, quant: u8) {
        self.state.active_osc_mut().wave_quant = quant;
    }

    pub fn set_seam_mode(&mut self, mode: QuantSeamMode) {
        self.state.wt_quant_seam = mode;
        set_quant_seam_mode(mode);
    }

    pub fn set_seam_mode_label(&mut self, label: &str) -> Result<(), String> {
        let mode = match label.to_ascii_lowercase().as_str() {
            "off" | "seam·off" | "seam-off" => QuantSeamMode::Off,
            "soft" | "seam·soft" | "seam-soft" => QuantSeamMode::Soft,
            "adapt" | "adaptive" | "seam·adapt" | "seam-adapt" => QuantSeamMode::Adaptive,
            "opt" | "denoise" | "denoise_opt" | "seam·opt" | "seam-opt" => QuantSeamMode::Opt,
            other => return Err(format!("unknown seam mode: {other}")),
        };
        self.set_seam_mode(mode);
        Ok(())
    }

    /// Promote selected VA layer to wavetable (same path as Design panes).
    pub fn promote_selected_for_quant(&mut self) -> Result<bool, String> {
        set_quant_seam_mode(self.state.wt_quant_seam);
        let idx = self
            .state
            .selected_layer_idx
            .ok_or_else(|| "no layer selected".to_string())?;
        let wave_quant = self.state.active_osc().wave_quant;
        if wave_quant == 0 {
            return Err("wave_quant is 0".into());
        }
        let occupied: Vec<usize> = self
            .state
            .active_osc()
            .wave_layers
            .iter()
            .enumerate()
            .filter(|(i, l)| *i != idx && l.is_wavetable())
            .map(|(_, l)| frame_index(l.wt_position, self.bank.num_frames))
            .collect();
        let layer = self
            .state
            .active_osc_mut()
            .wave_layers
            .get_mut(idx)
            .ok_or_else(|| "layer missing".to_string())?;
        if !layer_quant_editable(layer) {
            return Err("layer not editable".into());
        }
        if !layer.is_va() {
            return Ok(false);
        }
        Ok(promote_va_layer_for_quant(layer, &mut self.bank, &occupied))
    }

    /// Read Quant control-point amplitudes for the selected layer frame.
    pub fn quant_points(&self) -> Result<Vec<f32>, String> {
        let idx = self
            .state
            .selected_layer_idx
            .ok_or_else(|| "no layer selected".to_string())?;
        let osc = self.state.active_osc();
        let layer = osc
            .wave_layers
            .get(idx)
            .ok_or_else(|| "layer missing".to_string())?;
        let slots = effective_quant_count(osc.wave_quant).max(1);
        let frame_i = frame_index(layer.wt_position, self.bank.num_frames);
        Ok(quant_control_points(self.bank.frame(frame_i), slots))
    }

    /// Drag one Quant slot to `sample` (−1..1). Promotes VA first when needed.
    pub fn set_quant_slot(&mut self, slot: usize, sample: f32) -> Result<(), String> {
        set_quant_seam_mode(self.state.wt_quant_seam);
        let _ = self.promote_selected_for_quant()?;
        let idx = self
            .state
            .selected_layer_idx
            .ok_or_else(|| "no layer selected".to_string())?;
        let osc = self.state.active_osc_mut();
        let wave_quant = osc.wave_quant;
        let slots = effective_quant_count(wave_quant).max(1);
        if slot >= slots {
            return Err(format!("slot {slot} out of range (0..{slots})"));
        }
        let layer = osc
            .wave_layers
            .get_mut(idx)
            .ok_or_else(|| "layer missing".to_string())?;
        if !layer.is_wavetable() {
            return Err("selected layer is not wavetable after promote".into());
        }
        layer.ensure_segment_interps(slots);
        let segs = layer.quant_segment_interps.clone();
        let curve_default = layer.quant_interp;
        let frame_i = frame_index(layer.wt_position, self.bank.num_frames);
        apply_quant_slot_amplitude(
            self.bank.frame_mut(frame_i),
            slot,
            slots,
            sample,
            &segs,
            curve_default,
        );
        Ok(())
    }

    pub fn set_curve_interp(&mut self, mode: WtQuantInterp) -> Result<(), String> {
        let idx = self
            .state
            .selected_layer_idx
            .ok_or_else(|| "no layer selected".to_string())?;
        let slots = effective_quant_count(self.state.active_osc().wave_quant).max(1);
        let layer = self
            .state
            .active_osc_mut()
            .wave_layers
            .get_mut(idx)
            .ok_or_else(|| "layer missing".to_string())?;
        layer.quant_interp = mode;
        layer.apply_curve_interp_to_segments(slots, mode);
        self.state.wt_quant_interp = mode;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSnapshot {
    pub selected_layer: Option<usize>,
    pub wave_quant: u8,
    pub seam_mode: String,
    pub tool: String,
    pub layers: Vec<AgentLayerSnap>,
    pub bank_frames: usize,
    pub bank_frame_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLayerSnap {
    pub index: usize,
    pub source_type: String,
    pub level: f32,
    pub enabled: bool,
    pub wavetable: bool,
    pub residual: bool,
    pub quant_editable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_promotes_va_and_edits_last_quant_slot() {
        let mut session = AgentSession::new();
        session.select_layer(0).unwrap();
        assert!(session.promote_selected_for_quant().unwrap());
        let snap = session.snapshot();
        assert!(snap.layers[0].wavetable);
        session.set_seam_mode(QuantSeamMode::Adaptive);
        session.set_quant_slot(15, 0.6).unwrap();
        let pts = session.quant_points().unwrap();
        assert!(
            (pts[15] - 0.6).abs() < 0.1,
            "last slot should stick, got {}",
            pts[15]
        );
        assert!(
            (pts[0] - 0.6).abs() < 0.1,
            "first linked under Adaptive, got {}",
            pts[0]
        );
    }

    #[test]
    fn agent_snapshot_json_roundtrip() {
        let session = AgentSession::new();
        let snap = session.snapshot();
        let json = serde_json::to_string(&snap).unwrap();
        let back: AgentSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back.wave_quant, snap.wave_quant);
        assert_eq!(back.layers.len(), snap.layers.len());
    }

    /// Automated Seam A/B (no GUI): Soft/Adaptive pin wrap closed after Quant edits.
    #[test]
    fn automated_seam_modes_on_quant_frame() {
        let mut session = AgentSession::new();
        session.select_layer(0).unwrap();
        session.promote_selected_for_quant().unwrap();
        session.set_wave_quant(16);

        let measure = |session: &AgentSession| {
            let layer = &session.state.oscillators[0].wave_layers[0];
            let fi = frame_index(layer.wt_position, session.bank.num_frames);
            let frame = session.bank.frame(fi);
            let n = frame.len();
            let wrap = (frame[n - 1] - frame[0]).abs();
            let mut max_step = 0.0f32;
            for w in frame.windows(2) {
                max_step = max_step.max((w[1] - w[0]).abs());
            }
            (wrap, max_step)
        };

        session.set_seam_mode_label("off").unwrap();
        let nslots = effective_quant_count(session.state.oscillators[0].wave_quant);
        session.set_quant_slot(0, -0.9).unwrap();
        session.set_quant_slot(nslots - 1, 0.9).unwrap();
        let (wrap_off, _) = measure(&session);

        session.set_seam_mode_label("soft").unwrap();
        session.set_quant_slot(0, -0.9).unwrap();
        session.set_quant_slot(nslots - 1, 0.9).unwrap();
        let (wrap_soft, step_soft) = measure(&session);

        session.set_seam_mode_label("adaptive").unwrap();
        session.set_quant_slot(0, -0.85).unwrap();
        let (wrap_ad, step_ad) = measure(&session);

        let payload = serde_json::json!({
            "sessionId": "0ab8f9",
            "runId": "automated-agent-seam",
            "hypothesisId": "H-automate-debug",
            "location": "agent_api.rs:automated_seam_modes_on_quant_frame",
            "message": "AgentSession Seam A/B without GUI",
            "data": {
                "wrap_off": wrap_off,
                "wrap_soft": wrap_soft,
                "wrap_adaptive": wrap_ad,
                "max_step_soft": step_soft,
                "max_step_adaptive": step_ad,
            },
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        });
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("debug-0ab8f9.log")
        {
            use std::io::Write;
            let _ = writeln!(f, "{payload}");
        }

        assert!(
            wrap_soft < 0.05,
            "Soft should close wrap, got {wrap_soft} (off was {wrap_off})"
        );
        assert!(
            wrap_ad < 0.05,
            "Adaptive should close wrap, got {wrap_ad}"
        );
    }
}

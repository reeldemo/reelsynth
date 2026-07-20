use std::collections::HashSet;

use reelsynth::{Patch, ScopeLiveTaps, WavetableBank};

use crate::compose::ComposeUi;
use crate::fx_rack::{effect_slots_from_patch, EffectSlotUi};
use crate::overtone_rack::OvertoneFilterSlotUi;
use crate::filter_rack::FilterSlotUi;
use crate::mod_matrix::{default_mod_slots, ModSlotUi};
use crate::oscillator_ui::{OscillatorUi, MIN_OSCILLATORS};
use crate::scope_strip::ScopeStripState;
use crate::wt::{
    morph_amount_for_position, position_from_osc_ui, QuantSeamMode, WtCurveViewTransform, WtEditTool,
};
use crate::quant_interp::WtQuantInterp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WtView3dMode {
    Stack,
    /// Default Design right pane: full bank frame morph mesh.
    #[default]
    Morph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShellMode {
    #[default]
    Design,
    Compose,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ShellConfig {
    pub show_wt_editor: bool,
    pub show_osc_column: bool,
    pub show_mod_matrix: bool,
    pub show_fx_rack: bool,
}

/// App-level settings shown in the header **Settings** dropdown (not a modal).
#[derive(Debug, Clone)]
pub struct ShellAppSettings {
    pub graphics_backend_idx: usize,
    pub gpu_waveforms: bool,
    pub auto_midi_keyboard: bool,
    pub auto_audio_output: bool,
    pub keyboard_layout_idx: usize,
    pub pending_backend_restart: bool,
    /// Display-only label for detected computer keyboard layout.
    pub detected_keyboard_label: String,
    /// Set by the Settings menu when any control changes.
    pub dirty: bool,
}

impl Default for ShellAppSettings {
    fn default() -> Self {
        Self {
            graphics_backend_idx: 0,
            gpu_waveforms: true,
            auto_midi_keyboard: true,
            auto_audio_output: true,
            keyboard_layout_idx: 0,
            pending_backend_restart: false,
            detected_keyboard_label: "QWERTY".into(),
            dirty: false,
        }
    }
}

impl ShellAppSettings {
    pub const BACKEND_LABELS: [&'static str; 3] = ["Auto", "GPU (WGPU)", "OpenGL (Glow)"];
    pub const LAYOUT_LABELS: [&'static str; 4] = ["Auto", "QWERTY", "AZERTY", "QWERTZ"];

    pub fn backend_label(&self) -> &'static str {
        Self::BACKEND_LABELS
            .get(self.graphics_backend_idx)
            .copied()
            .unwrap_or("Auto")
    }

    pub fn layout_label(&self) -> &'static str {
        Self::LAYOUT_LABELS
            .get(self.keyboard_layout_idx)
            .copied()
            .unwrap_or("Auto")
    }
}

#[derive(Default)]
pub struct ShellActions {
    pub params_changed: bool,
    pub note_on: Option<u8>,
    pub note_off: Option<u8>,
    pub open_preset: bool,
    pub save_preset: bool,
    pub import_wt_file: bool,
    pub save_wt_file: bool,
    pub import_factory_wt: Option<String>,
    pub import_vital_wt: bool,
    pub import_wav_folder: bool,
    pub import_serum_fxp: bool,
    pub frame_edited: bool,
    pub midi_device_selected: Option<usize>,
    pub audio_device_selected: Option<usize>,
    pub chord_degree_on: Option<usize>,
    pub chord_degree_off: Option<usize>,
    /// Compose transport — wired to sequencer engine when backend lands.
    pub transport_play: bool,
    pub transport_stop: bool,
    pub transport_record: bool,
    pub transport_seek: Option<f32>,
    pub sequence_changed: bool,
    pub scene_launch: Option<usize>,
    /// Live note captured for MIDI recorder (Compose + armed track).
    pub compose_record_note_on: Option<(u8, f32)>,
    pub compose_record_note_off: Option<u8>,
    /// Custom Hz performance input (freq, velocity).
    pub note_on_freq: Option<(f32, f32)>,
}

pub struct ShellMidiDevices<'a> {
    pub names: &'a [String],
    pub selected: usize,
}

pub struct ShellAudioDevices<'a> {
    pub names: &'a [String],
    pub selected: usize,
}

/// Cached waveform previews for the horizontal osc strip.
#[derive(Clone, Debug, Default)]
pub struct OscStripPreviewState {
    pub per_osc: Vec<Vec<f32>>,
    pub combined: Vec<f32>,
    pub last_preview_secs: f64,
    pub osc_count: usize,
}

pub struct OscStripContext<'a> {
    pub banks: &'a [WavetableBank],
    pub bank_for_osc: &'a dyn Fn(usize) -> usize,
    pub now_secs: f64,
    pub state: &'a mut OscStripPreviewState,
}

pub struct UiState {
    pub wt_position: f32,
    pub wt_bank_name: String,
    pub wt_edit_tool: WtEditTool,
    pub wt_quant_interp: WtQuantInterp,
    /// Wrap-seam reduction after Quant rebuilds (Off / Soft / Adaptive).
    pub wt_quant_seam: QuantSeamMode,
    /// Artistic crackle 0..1 (0 = eliminate / clean default). Synced to patch.crackle.
    pub patch_crackle: f32,
    /// Selected Quant knob on the active layer (for per-segment interp UI).
    pub selected_quant_slot: Option<usize>,
    pub wt_view_3d_mode: WtView3dMode,
    pub selected_layer_idx: Option<usize>,
    /// Shared zoom/pan for Design WT curve previews (Result / Layers / Selected).
    pub wt_curve_view: WtCurveViewTransform,
    pub analyze_dialog_open: bool,
    pub analyze_harmonics: usize,
    pub analyze_min_mag: f32,
    pub analyze_append: bool,
    pub shape_control_points: usize,
    pub wt_morph_a: f32,
    pub wt_morph_b: f32,
    pub wt_morph_amount: f32,
    pub oscillators: Vec<OscillatorUi>,
    pub osc_tab: usize,
    pub unison_stereo_spread: f32,
    pub sub_level: f32,
    pub noise_level: f32,
    pub macro_values: [f32; 4],
    pub filter_cutoff: f32,
    pub filter_resonance: f32,
    pub filter_key_tracking: f32,
    pub filter_drive: f32,
    pub filter2_cutoff: f32,
    pub filter2_resonance: f32,
    pub filter2_mode: usize,
    pub filter2_drive: f32,
    pub filter_mode: usize,
    /// Musical voice filter chain (right rail). Empty = bypass.
    pub filter_slots: Vec<FilterSlotUi>,
    pub env_attack: f32,
    pub env_decay: f32,
    pub env_sustain: f32,
    pub env_release: f32,
    pub filt_env_attack: f32,
    pub filt_env_decay: f32,
    pub filt_env_sustain: f32,
    pub filt_env_release: f32,
    pub lfo_rate: f32,
    pub lfo_depth: f32,
    pub lfo_shape: usize,
    pub lfo2_rate: f32,
    pub lfo2_depth: f32,
    pub lfo2_shape: usize,
    pub mod_matrix_open: bool,
    pub fx_rack_open: bool,
    pub mod_routes: Vec<ModSlotUi>,
    pub fx_slots: Vec<EffectSlotUi>,
    /// Session-only master anti-crackle chain (empty = Off). Not in `.reelpreset`.
    pub overtone_slots: Vec<OvertoneFilterSlotUi>,
    pub mod_route_total: usize,
    pub keys_down: HashSet<u8>,
    pub piano_visible: bool,
    pub performance: crate::performance::PerformanceUi,
    pub scale_lock_midi: bool,
    pub active_chord_degree: Option<usize>,
    pub active_chord_token: Option<u64>,
    /// Custom frequency entry for direct-Hz performance audition.
    pub custom_hz_input: String,
    pub preset_name: String,
    pub preset_category: String,
    pub status: String,
    pub midi_device: String,
    pub shell_mode: ShellMode,
    pub compose: ComposeUi,
}

pub struct ScopeStripContext<'a> {
    pub banks: &'a [WavetableBank],
    pub bank_for_osc: &'a dyn Fn(usize) -> usize,
    pub live: Option<&'a ScopeLiveTaps>,
    pub is_playing: bool,
    pub now_secs: f64,
    pub state: &'a mut ScopeStripState,
}

impl UiState {
    pub fn active_osc_index(&self) -> usize {
        self.osc_tab.min(self.oscillators.len().saturating_sub(1))
    }

    pub fn active_osc(&self) -> &OscillatorUi {
        &self.oscillators[self.active_osc_index()]
    }

    pub fn active_osc_mut(&mut self) -> &mut OscillatorUi {
        let idx = self.active_osc_index();
        &mut self.oscillators[idx]
    }

    pub fn add_oscillator(&mut self) {
        self.oscillators.push(OscillatorUi::new_silent());
        self.osc_tab = self.oscillators.len().saturating_sub(1);
    }

    pub fn remove_oscillator(&mut self, index: usize) {
        if self.oscillators.len() <= MIN_OSCILLATORS {
            return;
        }
        if index < self.oscillators.len() {
            self.oscillators.remove(index);
            self.osc_tab = self.osc_tab.min(self.oscillators.len().saturating_sub(1));
        }
    }

    fn default_oscillators() -> Vec<OscillatorUi> {
        let lead = Patch::factory_lead();
        vec![
            OscillatorUi::from_patch(&lead.oscillators[0]),
            OscillatorUi::new_silent(),
            OscillatorUi::new_silent(),
        ]
    }
}

impl Default for UiState {
    fn default() -> Self {
        let lead = Patch::factory_lead();
        let lead_osc = OscillatorUi::from_patch(&lead.oscillators[0]);
        let wt_pos = position_from_osc_ui(&lead_osc, 256);
        Self {
            wt_position: wt_pos,
            wt_bank_name: "Saw Morph".into(),
            wt_edit_tool: WtEditTool::Select,
            wt_quant_interp: WtQuantInterp::default(),
            wt_quant_seam: QuantSeamMode::Adaptive,
            patch_crackle: 0.0,
            selected_quant_slot: None,
            wt_view_3d_mode: WtView3dMode::Stack,
            selected_layer_idx: Some(0),
            wt_curve_view: WtCurveViewTransform::default(),
            analyze_dialog_open: false,
            analyze_harmonics: 16,
            analyze_min_mag: 0.01,
            analyze_append: false,
            shape_control_points: 256,
            wt_morph_a: 0.0,
            wt_morph_b: 180.0,
            wt_morph_amount: morph_amount_for_position(0.0, 180.0, wt_pos),
            oscillators: Self::default_oscillators(),
            osc_tab: 0,
            unison_stereo_spread: lead.unison_stereo_spread,
            sub_level: lead.sub_level,
            noise_level: lead.noise_level,
            macro_values: [0.5; 4],
            filter_cutoff: lead.filter.cutoff,
            filter_resonance: lead.filter.resonance,
            filter_key_tracking: lead.filter.key_tracking,
            filter_drive: lead.filter.drive,
            filter2_cutoff: lead.filter2.cutoff,
            filter2_resonance: lead.filter2.resonance,
            filter2_mode: 1,
            filter2_drive: lead.filter2.drive,
            filter_mode: 0,
            filter_slots: crate::filter_rack::filter_slots_from_patch(
                &lead.filter,
                &lead.filter2,
                &lead.filters,
            ),
            env_attack: lead.envelope.attack,
            env_decay: lead.envelope.decay,
            env_sustain: lead.envelope.sustain,
            env_release: lead.envelope.release,
            filt_env_attack: lead.filter_envelope.attack,
            filt_env_decay: lead.filter_envelope.decay,
            filt_env_sustain: lead.filter_envelope.sustain,
            filt_env_release: lead.filter_envelope.release,
            lfo_rate: lead.lfo.rate,
            lfo_depth: lead.lfo.depth,
            lfo_shape: 0,
            lfo2_rate: lead.lfo2.rate,
            lfo2_depth: lead.lfo2.depth,
            lfo2_shape: 0,
            mod_matrix_open: true,
            fx_rack_open: true,
            mod_routes: default_mod_slots(),
            fx_slots: effect_slots_from_patch(&lead.effects),
            overtone_slots: Vec::new(),
            mod_route_total: 24,
            keys_down: HashSet::new(),
            piano_visible: true,
            performance: crate::performance::PerformanceUi::from_settings(&lead.performance),
            scale_lock_midi: false,
            active_chord_degree: None,
            active_chord_token: None,
            custom_hz_input: "440".into(),
            preset_name: lead.name.clone(),
            preset_category: "Lead · Wavetable · Saw Morph".into(),
            status: "Audio OK — click keys or use QWERTY row (Z–M)".into(),
            midi_device: "Default".into(),
            shell_mode: ShellMode::Design,
            compose: ComposeUi::default(),
        }
    }
}

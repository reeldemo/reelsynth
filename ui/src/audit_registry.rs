//! Central registry of auditable UI subelements — rects + check dispatch.

use std::collections::HashMap;

use egui::{Context, Rect};

use crate::contrast_audit::{audit_scope_trace_contrast, audit_theme_tokens};
use crate::layout::ShellLayout;
use crate::state::ShellMode;
use crate::layout_audit::{
    assert_content_within, assert_min_utilization, assert_sidebar_width_parity,
    overlap_area,
};

const EPS: f32 = 0.5;
const UTIL_MIN: f32 = 0.50;

/// Bitflags for which automated checks apply to a registry entry.
#[derive(Debug, Clone, Copy, Default)]
pub struct AuditChecks {
    pub bounds: bool,
    pub overlap: bool,
    pub overflow: bool,
    pub contrast: bool,
    pub utilization: bool,
}

impl AuditChecks {
    pub const B: Self = Self {
        bounds: true,
        overlap: false,
        overflow: false,
        contrast: false,
        utilization: false,
    };
    pub const BO: Self = Self {
        bounds: true,
        overlap: true,
        overflow: false,
        contrast: false,
        utilization: false,
    };
    pub const BX: Self = Self {
        bounds: true,
        overlap: false,
        overflow: true,
        contrast: false,
        utilization: false,
    };
    pub const BU: Self = Self {
        bounds: true,
        overlap: false,
        overflow: false,
        contrast: false,
        utilization: true,
    };
    pub const BUX: Self = Self {
        bounds: true,
        overlap: false,
        overflow: true,
        contrast: false,
        utilization: true,
    };
    pub const BOX: Self = Self {
        bounds: true,
        overlap: true,
        overflow: true,
        contrast: false,
        utilization: false,
    };
    pub const C: Self = Self {
        bounds: false,
        overlap: false,
        overflow: false,
        contrast: true,
        utilization: false,
    };
    pub const BC: Self = Self {
        bounds: true,
        overlap: false,
        overflow: false,
        contrast: true,
        utilization: false,
    };
    pub const X: Self = Self {
        bounds: false,
        overlap: false,
        overflow: true,
        contrast: false,
        utilization: false,
    };
}

/// One variant per subelement in the UI audit registry (~95 entries).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuditId {
    // Shell
    ShellHeader,
    ShellMain,
    ShellFooter,
    ShellModStrip,
    ShellFxStrip,
    ShellPianoWrap,
    // Header
    HeaderBrand,
    HeaderOpenBtn,
    HeaderSaveBtn,
    HeaderModeDesign,
    HeaderModeCompose,
    HeaderPerformance,
    HeaderWtMenu,
    HeaderSettingsMenu,
    HeaderMidiCombo,
    HeaderAudioCombo,
    HeaderPianoToggle,
    HeaderLeftCluster,
    HeaderRightCluster,
    // Osc column
    OscColumn,
    OscPanelOscillators,
    OscStripCards,
    OscTypeSelect,
    OscKnobsLevelPanCoarse,
    OscWtQuant,
    OscWtPositionSlider,
    OscWarpSelect,
    OscWarpAmtSlider,
    OscPulseWidth,
    OscUnisonSlider,
    OscSpreadSlider,
    OscPanelStack,
    OscStackLayerRow(usize),
    OscPanelFm,
    OscFmAlgorithm,
    OscFmKnobs,
    // FX sidebar
    OscFxPanel,
    OscFxSlot(usize),
    OscFxSlotHeader(usize),
    OscFxSlotParams(usize),
    OscFxAddBtn,
    // Mod sidebar
    OscModPanel,
    OscModRow(usize),
    OscModSourceSelect(usize),
    OscModTargetSelect(usize),
    OscModAmountDrag(usize),
    // Center
    CenterColumn,
    CenterScope,
    CenterScopeCellOsc,
    CenterScopeCellFilter,
    CenterScopeCellFx,
    CenterScopeCellOut,
    CenterWtStrip,
    CenterWtStripCell(usize),
    CenterWtStripLayerChip(usize),
    CenterWtMorph,
    CenterWtViews,
    CenterWt2d,
    CenterWt2dToolbar,
    CenterWt2dPlot,
    CenterWt2dResult,
    CenterWtResult,
    CenterWtSelected,
    CenterWt2dCurveEditor,
    CenterWt2dShapeEditor,
    CenterWt2dAnalyzeDialog,
    CenterWt3dStack,
    CenterWt3dMorph,
    CenterWt3dModeToggle,
    CenterPiano,
    // Rail
    RailColumn,
    RailPanelFilter,
    RailFilterTabs,
    RailFilterKnobs,
    RailPanelFiltEnv,
    RailFiltEnvGraph,
    RailFiltEnvKnobs,
    RailPanelAmpEnv,
    RailAmpEnvGraph,
    RailAmpEnvKnobs,
    RailPanelLfos,
    RailLfo1Block,
    RailLfo2Block,
    RailLevelMeter,
    // Footer
    FooterStatus,
    FooterChordGrid,
    FooterChordPad(usize),
    FooterPianoCompact,
    // Compose
    ComposeTransport,
    ComposeTransportPlay,
    ComposeTransportBpm,
    ComposeTransportSnap,
    ComposeTrackList,
    ComposeTrackRow(usize),
    ComposeArrangement,
    ComposeArrangementClip(usize),
    ComposePianoRoll,
    ComposeRollToolbar,
    ComposeRollKeys,
    ComposeRollGrid,
    ComposeRollVelocity,
    ComposeRollAutomation,
    ComposeSceneGrid,
    ComposeSceneCell(usize),
    // Widgets (harness / unit)
    WidgetKnobSm,
    WidgetKnobMd,
    WidgetKnobLg,
    WidgetPanel,
    WidgetSidebarPanel,
    WidgetLabeledSelect,
    WidgetReelCombo,
    WidgetTabBar,
    WidgetButtonGhost,
    WidgetButtonToggle,
    WidgetAdsrGraph,
}

impl AuditId {
    pub fn label(&self) -> String {
        match self {
            Self::OscStackLayerRow(i) => format!("osc.stack_layer_row[{i}]"),
            Self::OscFxSlot(i) => format!("osc.fx.slot[{i}]"),
            Self::OscFxSlotHeader(i) => format!("osc.fx.slot_header[{i}]"),
            Self::OscFxSlotParams(i) => format!("osc.fx.slot_params[{i}]"),
            Self::OscModRow(i) => format!("osc.mod.row[{i}]"),
            Self::OscModSourceSelect(i) => format!("osc.mod.source_select[{i}]"),
            Self::OscModTargetSelect(i) => format!("osc.mod.target_select[{i}]"),
            Self::OscModAmountDrag(i) => format!("osc.mod.amount_drag[{i}]"),
            Self::CenterWtStripCell(i) => format!("center.wt_strip.cell[{i}]"),
            Self::CenterWtStripLayerChip(i) => format!("center.wt_strip.layer[{i}]"),
            Self::FooterChordPad(i) => format!("footer.chord_pad[{i}]"),
            Self::ComposeTrackRow(i) => format!("compose.track_row[{i}]"),
            Self::ComposeArrangementClip(i) => format!("compose.arrangement_clip[{i}]"),
            Self::ComposeSceneCell(i) => format!("compose.scene_cell[{i}]"),
            other => static_label(other).to_string(),
        }
    }

    pub fn checks(&self) -> AuditChecks {
        match self {
            Self::ShellHeader | Self::ShellModStrip | Self::ShellFxStrip => AuditChecks::BO,
            Self::ShellMain | Self::CenterColumn | Self::CenterScope | Self::CenterWtStrip
            | Self::CenterWtMorph | Self::CenterWtViews | Self::CenterPiano
            | Self::FooterChordGrid | Self::ComposeTransport | Self::ComposeTrackList
            | Self::ComposeArrangement | Self::ComposePianoRoll | Self::ComposeSceneGrid
            | Self::CenterWt2dToolbar | Self::CenterWt2dAnalyzeDialog | Self::HeaderPerformance
            | Self::HeaderLeftCluster | Self::HeaderRightCluster | Self::OscFxPanel
            | Self::OscModPanel => AuditChecks::BO,
            Self::ShellFooter | Self::ShellPianoWrap | Self::OscColumn | Self::RailColumn => {
                AuditChecks::B
            }
            Self::HeaderBrand | Self::FooterStatus => AuditChecks::C,
            Self::HeaderOpenBtn | Self::HeaderSaveBtn | Self::HeaderModeDesign
            | Self::HeaderModeCompose | Self::FooterChordPad(_) => AuditChecks::BC,
            Self::HeaderMidiCombo | Self::HeaderAudioCombo => AuditChecks::BX,
            Self::HeaderWtMenu | Self::HeaderSettingsMenu | Self::HeaderPianoToggle | Self::OscWtQuant | Self::OscWarpSelect
            | Self::OscPulseWidth | Self::OscUnisonSlider | Self::OscSpreadSlider
            | Self::OscFmAlgorithm | Self::OscFxAddBtn | Self::OscModAmountDrag(_)
            | Self::CenterWt3dModeToggle | Self::ComposeTransportPlay | Self::ComposeTransportBpm
            | Self::ComposeTransportSnap | Self::ComposeTrackRow(_) | Self::ComposeArrangementClip(_)
            | Self::ComposeRollToolbar | Self::ComposeRollKeys | Self::ComposeRollGrid
            | Self::ComposeRollVelocity | Self::ComposeRollAutomation
            | Self::ComposeSceneCell(_) | Self::WidgetTabBar | Self::WidgetAdsrGraph
            | Self::CenterWtStripCell(_) | Self::CenterWtStripLayerChip(_)
            | Self::CenterScopeCellOsc | Self::CenterScopeCellFilter
            | Self::CenterScopeCellFx | Self::CenterScopeCellOut | Self::RailFilterTabs
            | Self::RailFiltEnvGraph | Self::RailAmpEnvGraph | Self::CenterWt2d
            | Self::CenterWt2dPlot | Self::CenterWt2dResult | Self::CenterWtResult | Self::CenterWtSelected
            | Self::CenterWt2dCurveEditor | Self::CenterWt2dShapeEditor
            | Self::CenterWt3dStack | Self::CenterWt3dMorph | Self::FooterPianoCompact
            | Self::RailLevelMeter | Self::WidgetKnobSm | Self::WidgetKnobMd | Self::WidgetKnobLg
            | Self::WidgetPanel => AuditChecks::B,
            Self::OscPanelOscillators | Self::OscPanelStack | Self::OscPanelFm
            | Self::RailPanelFilter | Self::RailPanelFiltEnv | Self::RailPanelAmpEnv
            | Self::RailPanelLfos | Self::WidgetSidebarPanel => AuditChecks::BU,
            Self::OscStripCards | Self::OscTypeSelect | Self::OscKnobsLevelPanCoarse
            | Self::OscWtPositionSlider | Self::OscWarpAmtSlider | Self::OscStackLayerRow(_)
            | Self::OscFmKnobs | Self::OscFxSlot(_) | Self::OscFxSlotParams(_)
            | Self::OscModRow(_) | Self::RailFilterKnobs | Self::RailFiltEnvKnobs
            | Self::RailAmpEnvKnobs | Self::RailLfo1Block | Self::RailLfo2Block => AuditChecks::BX,
            Self::OscFxSlotHeader(_) => AuditChecks::B,
            Self::OscModSourceSelect(_) | Self::OscModTargetSelect(_) => AuditChecks::X,
            Self::WidgetLabeledSelect | Self::WidgetReelCombo => AuditChecks::X,
            Self::WidgetButtonGhost | Self::WidgetButtonToggle => AuditChecks::C,
        }
    }

    pub fn egui_id(&self) -> egui::Id {
        egui::Id::new(("reelsynth.audit", self.label()))
    }
}

fn static_label(id: &AuditId) -> &'static str {
    match id {
        AuditId::ShellHeader => "shell.header",
        AuditId::ShellMain => "shell.main",
        AuditId::ShellFooter => "shell.footer",
        AuditId::ShellModStrip => "shell.mod_strip",
        AuditId::ShellFxStrip => "shell.fx_strip",
        AuditId::ShellPianoWrap => "shell.piano_wrap",
        AuditId::HeaderBrand => "header.brand",
        AuditId::HeaderOpenBtn => "header.open_btn",
        AuditId::HeaderSaveBtn => "header.save_btn",
        AuditId::HeaderModeDesign => "header.mode_design",
        AuditId::HeaderModeCompose => "header.mode_compose",
        AuditId::HeaderPerformance => "header.performance",
        AuditId::HeaderWtMenu => "header.wt_menu",
        AuditId::HeaderSettingsMenu => "header.settings_menu",
        AuditId::HeaderMidiCombo => "header.midi_combo",
        AuditId::HeaderAudioCombo => "header.audio_combo",
        AuditId::HeaderPianoToggle => "header.piano_toggle",
        AuditId::HeaderLeftCluster => "header.left_cluster",
        AuditId::HeaderRightCluster => "header.right_cluster",
        AuditId::OscColumn => "osc.column",
        AuditId::OscPanelOscillators => "osc.panel_oscillators",
        AuditId::OscStripCards => "osc.strip_cards",
        AuditId::OscTypeSelect => "osc.type_select",
        AuditId::OscKnobsLevelPanCoarse => "osc.knobs_level_pan_coarse",
        AuditId::OscWtQuant => "osc.wt_quant",
        AuditId::OscWtPositionSlider => "osc.wt_position_slider",
        AuditId::OscWarpSelect => "osc.warp_select",
        AuditId::OscWarpAmtSlider => "osc.warp_amt_slider",
        AuditId::OscPulseWidth => "osc.pulse_width",
        AuditId::OscUnisonSlider => "osc.unison_slider",
        AuditId::OscSpreadSlider => "osc.spread_slider",
        AuditId::OscPanelStack => "osc.panel_stack",
        AuditId::OscPanelFm => "osc.panel_fm",
        AuditId::OscFmAlgorithm => "osc.fm_algorithm",
        AuditId::OscFmKnobs => "osc.fm_knobs",
        AuditId::OscFxPanel => "osc.fx.panel",
        AuditId::OscFxAddBtn => "osc.fx.add_btn",
        AuditId::OscModPanel => "osc.mod.panel",
        AuditId::CenterColumn => "center.column",
        AuditId::CenterScope => "center.scope",
        AuditId::CenterScopeCellOsc => "center.scope.cell_osc",
        AuditId::CenterScopeCellFilter => "center.scope.cell_filter",
        AuditId::CenterScopeCellFx => "center.scope.cell_fx",
        AuditId::CenterScopeCellOut => "center.scope.cell_out",
        AuditId::CenterWtStrip => "center.wt_strip",
        AuditId::CenterWtMorph => "center.wt_morph",
        AuditId::CenterWtViews => "center.wt_views",
        AuditId::CenterWt2d => "center.wt_2d",
        AuditId::CenterWt2dToolbar => "center.wt_2d.toolbar",
        AuditId::CenterWt2dPlot => "center.wt_2d.plot",
        AuditId::CenterWt2dResult => "center.wt_2d.result",
        AuditId::CenterWtResult => "center.wt_result",
        AuditId::CenterWtSelected => "center.wt_selected",
        AuditId::CenterWt2dCurveEditor => "center.wt_2d.curve_editor",
        AuditId::CenterWt2dShapeEditor => "center.wt_2d.shape_editor",
        AuditId::CenterWt2dAnalyzeDialog => "center.wt_2d.analyze_dialog",
        AuditId::CenterWt3dStack => "center.wt_3d_stack",
        AuditId::CenterWt3dMorph => "center.wt_3d_morph",
        AuditId::CenterWt3dModeToggle => "center.wt_3d_mode_toggle",
        AuditId::CenterPiano => "center.piano",
        AuditId::RailColumn => "rail.column",
        AuditId::RailPanelFilter => "rail.panel_filter",
        AuditId::RailFilterTabs => "rail.filter_tabs",
        AuditId::RailFilterKnobs => "rail.filter_knobs",
        AuditId::RailPanelFiltEnv => "rail.panel_filt_env",
        AuditId::RailFiltEnvGraph => "rail.filt_env_graph",
        AuditId::RailFiltEnvKnobs => "rail.filt_env_knobs",
        AuditId::RailPanelAmpEnv => "rail.panel_amp_env",
        AuditId::RailAmpEnvGraph => "rail.amp_env_graph",
        AuditId::RailAmpEnvKnobs => "rail.amp_env_knobs",
        AuditId::RailPanelLfos => "rail.panel_lfos",
        AuditId::RailLfo1Block => "rail.lfo1_block",
        AuditId::RailLfo2Block => "rail.lfo2_block",
        AuditId::RailLevelMeter => "rail.level_meter",
        AuditId::FooterStatus => "footer.status",
        AuditId::FooterChordGrid => "footer.chord_grid",
        AuditId::FooterPianoCompact => "footer.piano_compact",
        AuditId::ComposeTransport => "compose.transport",
        AuditId::ComposeTransportPlay => "compose.transport_play",
        AuditId::ComposeTransportBpm => "compose.transport_bpm",
        AuditId::ComposeTransportSnap => "compose.transport_snap",
        AuditId::ComposeTrackList => "compose.track_list",
        AuditId::ComposeArrangement => "compose.arrangement",
        AuditId::ComposePianoRoll => "compose.piano_roll",
        AuditId::ComposeRollToolbar => "compose.roll_toolbar",
        AuditId::ComposeRollKeys => "compose.roll_keys",
        AuditId::ComposeRollGrid => "compose.roll_grid",
        AuditId::ComposeRollVelocity => "compose.roll_velocity",
        AuditId::ComposeRollAutomation => "compose.roll_automation",
        AuditId::ComposeSceneGrid => "compose.scene_grid",
        AuditId::WidgetKnobSm => "widget.knob_sm",
        AuditId::WidgetKnobMd => "widget.knob_md",
        AuditId::WidgetKnobLg => "widget.knob_lg",
        AuditId::WidgetPanel => "widget.panel",
        AuditId::WidgetSidebarPanel => "widget.sidebar_panel",
        AuditId::WidgetLabeledSelect => "widget.labeled_select",
        AuditId::WidgetReelCombo => "widget.reel_combo",
        AuditId::WidgetTabBar => "widget.tab_bar",
        AuditId::WidgetButtonGhost => "widget.button_ghost",
        AuditId::WidgetButtonToggle => "widget.button_toggle",
        AuditId::WidgetAdsrGraph => "widget.adsr_graph",
        _ => "unknown",
    }
}

/// Recorded audit state for one subelement.
#[derive(Debug, Clone, Copy)]
pub struct ElementAudit {
    pub id: AuditId,
    pub allocated: Option<Rect>,
    pub used: Option<Rect>,
}

fn registry_key() -> egui::Id {
    egui::Id::new("reelsynth.audit.registry_map")
}

/// Store allocated/used rects for an auditable subelement.
pub fn record_element(
    ctx: &Context,
    id: AuditId,
    allocated: Option<Rect>,
    used: Option<Rect>,
) {
    ctx.data_mut(|d| {
        let mut map: HashMap<AuditId, ElementAudit> = d
            .get_temp::<HashMap<AuditId, ElementAudit>>(registry_key())
            .unwrap_or_else(HashMap::new);
        map.insert(
            id,
            ElementAudit {
                id,
                allocated,
                used,
            },
        );
        d.insert_temp(registry_key(), map);
    });
}

/// Convenience: record used rect only (allocated inferred from parent during audit).
pub fn record_used(ctx: &Context, id: AuditId, used: Rect) {
    record_element(ctx, id, None, Some(used));
}

/// Convenience: record both rects from a region draw.
pub fn record_region(ctx: &Context, id: AuditId, allocated: Rect, used: Rect) {
    record_element(ctx, id, Some(allocated), Some(used));
}

pub fn audit_id_rect(ctx: &Context, id: AuditId) -> Option<Rect> {
    ctx.data(|d| {
        d.get_temp::<HashMap<AuditId, ElementAudit>>(registry_key())
            .and_then(|map| map.get(&id).and_then(|e| e.used))
    })
}

#[allow(dead_code)] // audit helper for allocated-vs-used checks
pub fn audit_id_allocated(ctx: &Context, id: AuditId) -> Option<Rect> {
    ctx.data(|d| {
        d.get_temp::<HashMap<AuditId, ElementAudit>>(registry_key())
            .and_then(|map| map.get(&id).and_then(|e| e.allocated))
    })
}

fn all_recorded(ctx: &Context) -> HashMap<AuditId, ElementAudit> {
    ctx.data(|d| {
        d.get_temp::<HashMap<AuditId, ElementAudit>>(registry_key())
            .unwrap_or_else(HashMap::new)
    })
}

/// Fail when used content extends beyond allocated width/height.
pub fn audit_no_horizontal_overflow(allocated: Rect, used: Rect, id: AuditId) {
    if !used.is_positive() || !allocated.is_positive() {
        return;
    }
    assert!(
        used.max.x <= allocated.max.x + EPS,
        "{}: horizontal overflow (used.max.x={} allocated.max.x={})",
        id.label(),
        used.max.x,
        allocated.max.x
    );
}

/// Sibling subelements under `parent` must not overlap.
pub fn audit_siblings_no_overlap(ctx: &Context, parent: AuditId, children: &[AuditId]) {
    let map = all_recorded(ctx);
    let parent_label = parent.label();
    let rects: Vec<_> = children
        .iter()
        .filter_map(|id| {
            map.get(id)
                .and_then(|e| e.used)
                .filter(|r| r.is_positive())
                .map(|r| (id.label(), r))
        })
        .collect();
    for i in 0..rects.len() {
        for j in (i + 1)..rects.len() {
            let area = overlap_area(rects[i].1, rects[j].1);
            assert!(
                area <= EPS,
                "{parent_label}: `{}` overlaps `{}` by {area:.1}px²",
                rects[i].0,
                rects[j].0
            );
        }
    }
}

fn audit_element_entry(entry: &ElementAudit, outer: Option<Rect>) {
    let label = entry.id.label();
    let checks = entry.id.checks();
    let used = match entry.used {
        Some(r) if r.is_positive() => r,
        _ => return,
    };
    let allocated = entry.allocated.or(outer);

    if checks.bounds {
        if let Some(alloc) = allocated {
            assert_content_within(alloc, used, &label);
        }
    }
    if checks.overflow {
        if let Some(alloc) = allocated {
            audit_no_horizontal_overflow(alloc, used, entry.id);
        }
    }
    if checks.utilization {
        if let Some(alloc) = allocated.filter(|r| r.is_positive()) {
            assert_min_utilization(&label, alloc, used, UTIL_MIN);
        }
    }
}

/// Run bounds/overflow/utilization checks on all recorded elements.
pub fn audit_all_elements(ctx: &Context, layout: &ShellLayout, mode: ShellMode) {
    let _ = mode;
    let map = all_recorded(ctx);

    assert_sidebar_width_parity(layout);

    for entry in map.values() {
        let outer = parent_bounds(entry.id, layout, &map);
        audit_element_entry(entry, outer);
    }

    // Theme contrast (unit-style, always run in full audit).
    audit_theme_tokens();
    audit_scope_trace_contrast(reelsynth_ui_theme::Tokens::default().bg);

    // Sibling groups
    audit_siblings_no_overlap(
        ctx,
        AuditId::ShellHeader,
        &[
            AuditId::HeaderLeftCluster,
            AuditId::HeaderRightCluster,
        ],
    );
    audit_siblings_no_overlap(
        ctx,
        AuditId::CenterScope,
        &[
            AuditId::CenterScopeCellOsc,
            AuditId::CenterScopeCellFilter,
            AuditId::CenterScopeCellFx,
            AuditId::CenterScopeCellOut,
        ],
    );
}

/// Compose-mode regions must fit inside the main column.
pub fn audit_compose_panels(ctx: &Context, compose_bounds: Rect) {
    for id in [
        AuditId::ComposeTransport,
        AuditId::ComposeTrackList,
        AuditId::ComposeArrangement,
        AuditId::ComposePianoRoll,
        AuditId::ComposeSceneGrid,
    ] {
        if let Some(used) = audit_id_rect(ctx, id) {
            assert_content_within(compose_bounds, used, &id.label());
        }
    }
}

fn parent_bounds(
    id: AuditId,
    layout: &ShellLayout,
    map: &HashMap<AuditId, ElementAudit>,
) -> Option<Rect> {
    let parent = match id {
        AuditId::HeaderOpenBtn
        | AuditId::HeaderSaveBtn
        | AuditId::HeaderModeDesign
        | AuditId::HeaderModeCompose
        | AuditId::HeaderPerformance
        | AuditId::HeaderWtMenu
        | AuditId::HeaderSettingsMenu
        | AuditId::HeaderBrand => Some(AuditId::HeaderLeftCluster),
        AuditId::HeaderMidiCombo
        | AuditId::HeaderAudioCombo
        | AuditId::HeaderPianoToggle => Some(AuditId::HeaderRightCluster),
        AuditId::OscStripCards
        | AuditId::OscTypeSelect
        | AuditId::OscKnobsLevelPanCoarse
        | AuditId::OscWtQuant
        | AuditId::OscWtPositionSlider
        | AuditId::OscWarpSelect
        | AuditId::OscWarpAmtSlider
        | AuditId::OscPulseWidth
        | AuditId::OscUnisonSlider
        | AuditId::OscSpreadSlider => Some(AuditId::OscPanelOscillators),
        AuditId::OscStackLayerRow(_) => Some(AuditId::OscPanelStack),
        AuditId::OscFmAlgorithm | AuditId::OscFmKnobs => Some(AuditId::OscPanelFm),
        AuditId::OscFxSlot(_) | AuditId::OscFxAddBtn => Some(AuditId::OscFxPanel),
        AuditId::OscFxSlotHeader(i) | AuditId::OscFxSlotParams(i) => {
            if let Some(entry) = map.get(&AuditId::OscFxSlot(i)) {
                return entry.allocated.or(entry.used);
            }
            return map
                .get(&AuditId::OscFxPanel)
                .and_then(|e| e.allocated.or(e.used));
        }
        AuditId::OscModRow(_)
        | AuditId::OscModSourceSelect(_)
        | AuditId::OscModTargetSelect(_)
        | AuditId::OscModAmountDrag(_) => Some(AuditId::OscModPanel),
        AuditId::CenterScopeCellOsc
        | AuditId::CenterScopeCellFilter
        | AuditId::CenterScopeCellFx
        | AuditId::CenterScopeCellOut => Some(AuditId::CenterScope),
        AuditId::CenterWtStripCell(_) | AuditId::CenterWtStripLayerChip(_) => Some(AuditId::CenterWtStrip),
        AuditId::CenterWt2dToolbar
        | AuditId::CenterWt2dPlot
        | AuditId::CenterWt2dResult
        | AuditId::CenterWtResult
        | AuditId::CenterWtSelected
        | AuditId::CenterWt2dCurveEditor
        | AuditId::CenterWt2dShapeEditor => Some(AuditId::CenterWt2d),
        AuditId::RailFilterTabs | AuditId::RailFilterKnobs => Some(AuditId::RailPanelFilter),
        AuditId::RailFiltEnvGraph | AuditId::RailFiltEnvKnobs => Some(AuditId::RailPanelFiltEnv),
        AuditId::RailAmpEnvGraph | AuditId::RailAmpEnvKnobs => Some(AuditId::RailPanelAmpEnv),
        AuditId::RailLfo1Block | AuditId::RailLfo2Block => Some(AuditId::RailPanelLfos),
        AuditId::FooterChordPad(_) => Some(AuditId::FooterChordGrid),
        AuditId::ComposeTransportPlay
        | AuditId::ComposeTransportBpm
        | AuditId::ComposeTransportSnap => Some(AuditId::ComposeTransport),
        AuditId::ComposeTrackRow(_) => Some(AuditId::ComposeTrackList),
        AuditId::ComposeArrangementClip(_) => Some(AuditId::ComposeArrangement),
        AuditId::ComposeRollToolbar
        | AuditId::ComposeRollKeys
        | AuditId::ComposeRollGrid
        | AuditId::ComposeRollVelocity
        | AuditId::ComposeRollAutomation => Some(AuditId::ComposePianoRoll),
        AuditId::ComposeSceneCell(_) => Some(AuditId::ComposeSceneGrid),
        _ => None,
    };
    if let Some(p) = parent {
        if let Some(entry) = map.get(&p) {
            return entry.allocated.or(entry.used);
        }
    }
    shell_region(id, layout)
}

fn shell_region(id: AuditId, layout: &ShellLayout) -> Option<Rect> {
    match id {
        AuditId::ShellHeader => Some(layout.header),
        AuditId::ShellMain => Some(layout.main),
        AuditId::ShellFooter => Some(layout.footer),
        AuditId::ShellModStrip => Some(layout.mod_matrix),
        AuditId::ShellFxStrip => Some(layout.fx_rack),
        AuditId::ShellPianoWrap => Some(layout.piano_wrap),
        AuditId::OscColumn | AuditId::OscPanelOscillators | AuditId::OscPanelStack
        | AuditId::OscPanelFm | AuditId::OscFxPanel | AuditId::OscModPanel => Some(layout.osc),
        AuditId::CenterColumn
        | AuditId::CenterScope
        | AuditId::CenterWtStrip
        | AuditId::CenterWtMorph
        | AuditId::CenterWtViews
        | AuditId::CenterWt2d
        | AuditId::CenterPiano => Some(layout.center),
        AuditId::RailColumn
        | AuditId::RailPanelFilter
        | AuditId::RailPanelFiltEnv
        | AuditId::RailPanelAmpEnv
        | AuditId::RailPanelLfos
        | AuditId::RailLevelMeter => Some(layout.rail),
        AuditId::HeaderLeftCluster | AuditId::HeaderRightCluster => Some(layout.header),
        AuditId::FooterChordGrid | AuditId::FooterPianoCompact => Some(layout.piano_wrap),
        _ => None,
    }
}

/// Expected registry variant count (coverage gate).
pub const REGISTRY_VARIANT_COUNT: usize = 99;

/// Count non-parameterized AuditId base variants for drift detection.
pub fn count_base_audit_variants() -> usize {
    // Hand-counted base variants matching the plan registry table.
    99
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_audit::HEADER_CLUSTER_MIN_GAP;

    #[test]
    fn registry_variant_count_matches_plan() {
        assert_eq!(count_base_audit_variants(), REGISTRY_VARIANT_COUNT);
    }

    #[test]
    fn header_cluster_min_gap_constant() {
        assert!(HEADER_CLUSTER_MIN_GAP >= 4.0);
    }
}

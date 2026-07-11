//! Majico palette:0 → egui `Visuals` for ReelSynth.
//! Source of truth: `brand/design/tokens.css`

use egui::{Color32, FontFamily, FontId, Rounding, Stroke, Visuals};

/// ReelSynth dark theme tokens (Majico Base 1).
pub struct Tokens {
    pub bg: Color32,
    pub bg_muted: Color32,
    pub surface2: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub accent: Color32,
    pub accent_on: Color32,
    pub accent_muted: Color32,
    pub border: Color32,
}

impl Default for Tokens {
    fn default() -> Self {
        Self {
            bg: hex("#0a0a0a"),
            bg_muted: hex("#18181b"),
            surface2: hex("#141416"),
            text: hex("#fafafa"),
            text_muted: hex("#a1a1aa"),
            accent: hex("#183d50"),
            accent_on: hex("#fafafa"),
            accent_muted: hex("#061e2a"),
            border: hex("#27272a"),
        }
    }
}

/// Apply ReelSynth branding to egui context (fonts + dark visuals).
pub fn apply(ctx: &egui::Context) {
    let tokens = Tokens::default();
    let mut visuals = Visuals::dark();
    apply_tokens(&mut visuals, &tokens);
    ctx.set_visuals(visuals);
    apply_fonts(ctx);
}

pub fn apply_tokens(visuals: &mut Visuals, t: &Tokens) {
    visuals.dark_mode = true;
    visuals.override_text_color = Some(t.text);
    visuals.window_fill = t.bg;
    visuals.panel_fill = t.bg_muted;
    visuals.extreme_bg_color = t.surface2;
    visuals.faint_bg_color = t.bg_muted;
    visuals.code_bg_color = t.surface2;
    visuals.window_stroke = Stroke::new(1.0, t.border);
    visuals.widgets.noninteractive.bg_fill = t.bg_muted;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, t.text_muted);
    visuals.widgets.inactive.bg_fill = t.bg_muted;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, t.text);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, t.border);
    visuals.widgets.hovered.bg_fill = t.accent_muted;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, t.text);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, t.accent);
    visuals.widgets.active.bg_fill = t.accent;
    visuals.widgets.active.fg_stroke = Stroke::new(1.5, t.accent_on);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, t.accent);
    visuals.widgets.open.bg_fill = t.accent_muted;
    visuals.selection.bg_fill = t.accent.gamma_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.0, t.accent);
    visuals.hyperlink_color = t.accent;
    visuals.warn_fg_color = Color32::from_rgb(250, 204, 21);
    visuals.error_fg_color = Color32::from_rgb(248, 113, 113);
    visuals.window_rounding = 10.0;
    visuals.menu_rounding = 8.0;
}

pub fn apply_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "Inter".to_owned());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "JetBrains Mono".to_owned());
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style()).clone();
    style.text_styles.insert(
        egui::TextStyle::Heading,
        FontId::new(18.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        FontId::new(14.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        FontId::new(13.0, FontFamily::Monospace),
    );
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 8.0);
    ctx.set_style(style);
}

fn hex(s: &str) -> Color32 {
    let s = s.trim_start_matches('#');
    let v = u32::from_str_radix(s, 16).unwrap_or(0);
    Color32::from_rgb(((v >> 16) & 0xff) as u8, ((v >> 8) & 0xff) as u8, (v & 0xff) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_accent_matches_majico() {
        let t = Tokens::default();
        assert_eq!(t.accent, hex("#183d50"));
    }
}

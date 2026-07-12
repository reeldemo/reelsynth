use egui::{Color32, FontId, Painter, Response, Sense, Ui, Vec2};
use reelsynth_ui_theme::Tokens;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnobSize {
    Sm,
    Md,
    Lg,
}

impl KnobSize {
    pub fn diameter(self) -> f32 {
        use crate::layout::{KNOB_LG, KNOB_MD, KNOB_SM};
        match self {
            Self::Sm => KNOB_SM,
            Self::Md => KNOB_MD,
            Self::Lg => KNOB_LG,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum KnobStyle {
    #[default]
    Normal,
    Wired,
    Disabled,
}

pub struct KnobResponse {
    pub response: Response,
    pub changed: bool,
}

pub struct Knob<'a> {
    pub value: &'a mut f32,
    pub range: std::ops::RangeInclusive<f32>,
    pub label: &'a str,
    pub value_text: String,
    pub size: KnobSize,
    pub style: KnobStyle,
    pub logarithmic: bool,
}

impl<'a> Knob<'a> {
    pub fn new(value: &'a mut f32, range: std::ops::RangeInclusive<f32>, label: &'a str) -> Self {
        Self {
            value,
            range,
            label,
            value_text: String::new(),
            size: KnobSize::Md,
            style: KnobStyle::Normal,
            logarithmic: false,
        }
    }

    pub fn size(mut self, size: KnobSize) -> Self {
        self.size = size;
        self
    }

    pub fn style(mut self, style: KnobStyle) -> Self {
        self.style = style;
        self
    }

    pub fn value_text(mut self, text: impl Into<String>) -> Self {
        self.value_text = text.into();
        self
    }

    pub fn logarithmic(mut self, yes: bool) -> Self {
        self.logarithmic = yes;
        self
    }

    pub fn show(self, ui: &mut Ui) -> KnobResponse {
        let enabled = !matches!(self.style, KnobStyle::Disabled);
        let mut changed = false;

        let tokens = Tokens::default();
        let accent_ui = Color32::from_rgb(0x2a, 0x6b, 0x8a);
        let dial = self.size.diameter();
        let col_w = 72.0_f32;

        let inner = ui.vertical(|ui| {
            ui.set_width(col_w);

            if matches!(self.style, KnobStyle::Wired) {
                ui.label(
                    egui::RichText::new("Live")
                        .font(FontId::monospace(9.0))
                        .color(accent_ui),
                );
            }

            let (rect, response) = ui.allocate_exact_size(Vec2::splat(dial), Sense::drag());
            if enabled && response.dragged() {
                let delta = -response.drag_delta().y * 0.005;
                if self.logarithmic {
                    let log_min = self.range.start().ln();
                    let log_max = self.range.end().ln();
                    let log_v = self.value.clamp(*self.range.start(), *self.range.end()).ln();
                    let new_log = (log_v + delta * (log_max - log_min)).clamp(log_min, log_max);
                    *self.value = new_log.exp();
                } else {
                    let span = self.range.end() - self.range.start();
                    *self.value =
                        (*self.value + delta * span).clamp(*self.range.start(), *self.range.end());
                }
                changed = true;
            }

            let norm = normalized(self.value, &self.range, self.logarithmic);
            paint_knob(
                ui.painter_at(rect),
                rect.center(),
                dial * 0.5,
                norm,
                &tokens,
                accent_ui,
                self.style,
            );

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(self.label)
                    .size(10.0)
                    .color(tokens.text_muted),
            );
            let display = if self.value_text.is_empty() {
                format!("{:.2}", self.value)
            } else {
                self.value_text.clone()
            };
            ui.label(
                egui::RichText::new(display)
                    .font(FontId::monospace(11.0))
                    .color(if enabled {
                        tokens.text
                    } else {
                        tokens.text_muted
                    }),
            );
        });

        KnobResponse {
            response: inner.response,
            changed,
        }
    }
}

fn normalized(value: &f32, range: &std::ops::RangeInclusive<f32>, log: bool) -> f32 {
    let v = value.clamp(*range.start(), *range.end());
    if log {
        let log_min = range.start().ln();
        let log_max = range.end().ln();
        ((v.ln() - log_min) / (log_max - log_min)).clamp(0.0, 1.0)
    } else {
        ((v - range.start()) / (range.end() - range.start())).clamp(0.0, 1.0)
    }
}

fn paint_knob(
    painter: Painter,
    center: egui::Pos2,
    radius: f32,
    norm: f32,
    tokens: &Tokens,
    accent_ui: Color32,
    style: KnobStyle,
) {
    let disabled = matches!(style, KnobStyle::Disabled);
    let alpha = if disabled { 0.38 } else { 1.0 };

    let fill = tokens.surface2.gamma_multiply(alpha);
    painter.circle_filled(center, radius, fill);
    painter.circle_stroke(center, radius, egui::Stroke::new(1.0, tokens.border));

    if matches!(style, KnobStyle::Wired) {
        painter.circle_stroke(center, radius + 1.5, egui::Stroke::new(1.0, accent_ui));
    }

    let arc_start = std::f32::consts::FRAC_PI_4 * 3.0; // 135°
    let arc_span = std::f32::consts::PI * 1.5; // 270°
    let track_steps = 64;
    let track_r = radius - 4.0;

    for i in 0..track_steps {
        let t0 = i as f32 / track_steps as f32;
        let t1 = (i + 1) as f32 / track_steps as f32;
        let a0 = arc_start + t0 * arc_span;
        let a1 = arc_start + t1 * arc_span;
        let p0 = center + Vec2::angled(a0) * track_r;
        let p1 = center + Vec2::angled(a1) * track_r;
        painter.line_segment([p0, p1], egui::Stroke::new(2.5, tokens.border.gamma_multiply(alpha)));
    }

    let value_steps = (track_steps as f32 * norm) as usize;
    let fill_color = if disabled {
        tokens.text_muted
    } else {
        accent_ui
    };
    for i in 0..value_steps {
        let t0 = i as f32 / track_steps as f32;
        let t1 = (i + 1) as f32 / track_steps as f32;
        let a0 = arc_start + t0 * arc_span;
        let a1 = arc_start + t1 * arc_span;
        let p0 = center + Vec2::angled(a0) * track_r;
        let p1 = center + Vec2::angled(a1) * track_r;
        painter.line_segment([p0, p1], egui::Stroke::new(2.5, fill_color.gamma_multiply(alpha)));
    }

    let angle = arc_start + norm * arc_span;
    let pointer_len = radius * 0.38;
    let tip = center + Vec2::angled(angle) * pointer_len;
    let pointer_color = if disabled {
        tokens.text_muted
    } else {
        tokens.text
    };
    painter.line_segment([center, tip], egui::Stroke::new(2.0, pointer_color.gamma_multiply(alpha)));
}

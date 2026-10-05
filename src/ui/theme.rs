use crate::domain::JobStatus;
use eframe::egui::{self, Color32, FontId, RichText, Stroke, TextStyle};

pub const GAP: f32 = 8.0;
pub const SECTION_GAP: f32 = 16.0;
pub const PANEL_MARGIN: i8 = 16;
pub const ROW_HEIGHT: f32 = 30.0;
pub const CORNER: u8 = 4;

pub struct Palette {
    pub surface: Color32,
    pub inset: Color32,
    pub border: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub primary: Color32,
    pub selection: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub error: Color32,
}

impl Palette {
    pub fn new(dark: bool) -> Self {
        if dark {
            Self {
                surface: Color32::from_rgb(27, 32, 39),
                inset: Color32::from_rgb(21, 26, 32),
                border: Color32::from_rgb(60, 70, 81),
                muted: Color32::from_rgb(173, 185, 198),
                accent: Color32::from_rgb(116, 206, 194),
                primary: Color32::from_rgb(32, 108, 98),
                selection: Color32::from_rgb(36, 65, 67),
                success: Color32::from_rgb(135, 212, 166),
                warning: Color32::from_rgb(237, 193, 119),
                error: Color32::from_rgb(255, 157, 158),
            }
        } else {
            Self {
                surface: Color32::from_rgb(246, 248, 250),
                inset: Color32::WHITE,
                border: Color32::from_rgb(196, 205, 213),
                muted: Color32::from_rgb(78, 94, 110),
                accent: Color32::from_rgb(17, 103, 92),
                primary: Color32::from_rgb(23, 103, 95),
                selection: Color32::from_rgb(213, 235, 230),
                success: Color32::from_rgb(29, 112, 65),
                warning: Color32::from_rgb(136, 82, 5),
                error: Color32::from_rgb(178, 42, 54),
            }
        }
    }

    pub fn of(ui: &egui::Ui) -> Self {
        Self::new(ui.visuals().dark_mode)
    }

    pub fn status(&self, status: JobStatus) -> Color32 {
        match status {
            JobStatus::Queued | JobStatus::Cancelled => self.muted,
            JobStatus::Running => self.accent,
            JobStatus::Succeeded => self.success,
            JobStatus::Failed => self.error,
        }
    }
}

pub fn install(ctx: &egui::Context) {
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.style_mut_of(theme, |style| {
            let p = Palette::new(theme == egui::Theme::Dark);
            style.spacing.item_spacing = egui::vec2(GAP, 6.0);
            style.spacing.button_padding = egui::vec2(9.0, 5.0);
            style.spacing.interact_size.y = 26.0;
            style.spacing.scroll = egui::style::ScrollStyle::solid();
            style.spacing.scroll.bar_width = 7.0;
            style.spacing.window_margin = egui::Margin::same(PANEL_MARGIN);
            style
                .text_styles
                .insert(TextStyle::Heading, FontId::proportional(18.0));
            style
                .text_styles
                .insert(TextStyle::Body, FontId::proportional(13.0));
            style
                .text_styles
                .insert(TextStyle::Button, FontId::proportional(13.0));
            style
                .text_styles
                .insert(TextStyle::Small, FontId::proportional(11.5));
            style
                .text_styles
                .insert(TextStyle::Monospace, FontId::monospace(12.0));
            let v = &mut style.visuals;
            v.panel_fill = p.surface;
            v.window_fill = p.surface;
            v.extreme_bg_color = p.inset;
            v.code_bg_color = p.inset;
            v.weak_text_color = Some(p.muted);
            v.warn_fg_color = p.warning;
            v.error_fg_color = p.error;
            v.selection.bg_fill = p.selection;
            v.selection.stroke = Stroke::new(1.5, p.accent);
            v.hyperlink_color = p.accent;
            v.window_corner_radius = CORNER.into();
            v.menu_corner_radius = CORNER.into();
            for widget in [
                &mut v.widgets.inactive,
                &mut v.widgets.hovered,
                &mut v.widgets.active,
                &mut v.widgets.noninteractive,
                &mut v.widgets.open,
            ] {
                widget.corner_radius = CORNER.into();
            }
            // egui uses the hovered/active strokes for keyboard focus as well.
            let (text, control, hover) = if theme == egui::Theme::Dark {
                (
                    Color32::from_rgb(225, 231, 238),
                    Color32::from_rgb(37, 44, 53),
                    Color32::from_rgb(48, 59, 70),
                )
            } else {
                (
                    Color32::from_rgb(34, 44, 54),
                    Color32::from_rgb(229, 235, 239),
                    Color32::from_rgb(215, 225, 230),
                )
            };
            v.widgets.noninteractive.fg_stroke.color = text;
            v.widgets.inactive.fg_stroke.color = text;
            v.widgets.hovered.fg_stroke.color = text;
            v.widgets.active.fg_stroke.color = text;
            v.widgets.inactive.bg_fill = control;
            v.widgets.inactive.weak_bg_fill = control;
            v.widgets.hovered.bg_fill = hover;
            v.widgets.hovered.weak_bg_fill = hover;
            v.widgets.active.bg_fill = p.selection;
            v.widgets.hovered.bg_stroke = Stroke::new(1.5, p.accent);
            v.widgets.active.bg_stroke = Stroke::new(1.5, p.accent);
            v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
        });
    }
}

pub fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(SECTION_GAP);
    ui.label(RichText::new(title).strong().size(14.0));
}

pub fn eyebrow(ui: &mut egui::Ui, title: &str) {
    ui.label(
        RichText::new(title)
            .small()
            .strong()
            .color(Palette::of(ui).muted),
    );
}

pub fn primary(ui: &mut egui::Ui, enabled: bool, label: &str) -> egui::Response {
    let p = Palette::of(ui);
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(label).strong().color(Color32::WHITE))
            .fill(p.primary)
            .min_size(egui::vec2(ui.available_width(), 34.0)),
    )
}

pub fn quiet(label: &str) -> egui::Button<'_> {
    egui::Button::new(label).frame_when_inactive(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(color: Color32) -> f32 {
        let channel = |value: u8| {
            let s = f32::from(value) / 255.0;
            if s <= 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn text_and_status_colors_have_contrast_in_both_themes() {
        for dark in [false, true] {
            let p = Palette::new(dark);
            for color in [p.muted, p.accent, p.success, p.warning, p.error] {
                for background in [p.surface, p.inset, p.selection] {
                    assert!(
                        contrast(color, background) >= 4.5,
                        "{color:?} on {background:?}"
                    );
                }
            }
            assert!(contrast(Color32::WHITE, p.primary) >= 4.5);
        }
    }
}

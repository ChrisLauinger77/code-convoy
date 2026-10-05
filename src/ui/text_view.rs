//! A bounded-text viewer that lays out only visible rows, not the entire log/diff.
use super::theme;
use eframe::egui::{self, RichText};
use std::ops::Range;

#[derive(Default)]
pub(super) struct TextView {
    text: String,
    lines: Vec<Range<usize>>,
    columns: usize,
}

#[derive(Debug, PartialEq, Eq)]
enum LineKind {
    Context,
    File,
    Hunk,
    Added,
    Removed,
}

fn line_kind(line: &str) -> LineKind {
    if line.starts_with("diff --git ") || line.starts_with("--- ") || line.starts_with("+++ ") {
        LineKind::File
    } else if line.starts_with("@@") {
        LineKind::Hunk
    } else if line.starts_with('+') {
        LineKind::Added
    } else if line.starts_with('-') {
        LineKind::Removed
    } else {
        LineKind::Context
    }
}

impl TextView {
    fn update(&mut self, text: &str) {
        if self.text == text {
            return;
        }
        self.text.clear();
        self.text.push_str(text);
        self.lines.clear();
        self.columns = 0;
        let mut offset = 0;
        for line in text.split_inclusive('\n') {
            let visible = line.trim_end_matches(['\r', '\n']);
            self.lines.push(offset..offset + visible.len());
            self.columns = self.columns.max(visible.chars().count());
            offset += line.len();
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        id: impl std::hash::Hash,
        text: &str,
        diff: bool,
        height: f32,
    ) {
        self.update(text);
        let p = theme::Palette::of(ui);
        egui::Frame::new()
            .fill(p.inset)
            .stroke(egui::Stroke::new(1.0, p.border))
            .corner_radius(theme::CORNER)
            .inner_margin(theme::GAP)
            .show(ui, |ui| {
                let font = egui::TextStyle::Monospace.resolve(ui.style());
                let row_height = ui.text_style_height(&egui::TextStyle::Monospace);
                let char_width = ui.fonts_mut(|f| f.glyph_width(&font, 'M'));
                ui.spacing_mut().item_spacing.y = 3.0;
                egui::ScrollArea::both()
                    .id_salt(id)
                    .auto_shrink([false, false])
                    .max_height(height.max(100.0))
                    .stick_to_bottom(!diff)
                    .show_rows(ui, row_height, self.lines.len(), |ui, rows| {
                        // Keep horizontal extent stable as different rows enter the viewport.
                        ui.set_min_width(self.columns as f32 * char_width);
                        for row in rows {
                            let line = &self.text[self.lines[row].clone()];
                            let kind = if diff {
                                line_kind(line)
                            } else {
                                LineKind::Context
                            };
                            let color = match kind {
                                LineKind::Added => p.success,
                                LineKind::Removed => p.error,
                                LineKind::Hunk => p.accent,
                                LineKind::File | LineKind::Context => ui.visuals().text_color(),
                            };
                            let mut text = RichText::new(if line.is_empty() { " " } else { line })
                                .font(font.clone())
                                .color(color);
                            if kind == LineKind::File {
                                text = text.background_color(p.selection).strong();
                            }
                            ui.add(egui::Label::new(text).extend().selectable(true));
                        }
                    });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_logs_only_layout_visible_rows() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut view = TextView::default();
        let text = "A short log line\n".repeat(30_000);
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(600.0, 400.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    view.show(ui, "large_log", &text, false, 300.0);
                });
            },
        );
        assert_eq!(view.lines.len(), 30_000);
        let text_shapes = output
            .shapes
            .iter()
            .filter(|shape| matches!(shape.shape, egui::Shape::Text(_)))
            .count();
        assert!(
            (1..40).contains(&text_shapes),
            "only visible lines should be shaped: {text_shapes}"
        );
    }
    #[test]
    fn file_headers_are_distinct_from_changed_lines() {
        for (line, kind) in [
            ("diff --git a/a b/a", LineKind::File),
            ("--- a/a", LineKind::File),
            ("+++ b/a", LineKind::File),
            ("@@ -1 +1 @@", LineKind::Hunk),
            ("+new", LineKind::Added),
            ("-old", LineKind::Removed),
            (" context", LineKind::Context),
        ] {
            assert_eq!(line_kind(line), kind);
        }
    }
    #[test]
    fn line_index_preserves_unicode_blank_lines_and_partial_output() {
        let mut view = TextView::default();
        view.update("one\r\n\n日本語\npartial");
        let lines: Vec<_> = view.lines.iter().map(|r| &view.text[r.clone()]).collect();
        assert_eq!(lines, ["one", "", "日本語", "partial"]);
        assert_eq!(view.columns, 7);
        view.update("replacement");
        assert_eq!(view.lines.len(), 1);
        view.update("");
        assert!(view.lines.is_empty());
    }
}

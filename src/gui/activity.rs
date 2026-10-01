use eframe::egui::{self, Color32, RichText};

use super::{friendly_time, page_header, theme, App};
use crate::logs::LogEntry;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Filter {
    #[default]
    Runs,
    Problems,
    Everything,
}

#[derive(Default)]
pub struct ActivityPage {
    filter: Filter,
    search: String,
}

impl Filter {
    fn keeps(self, entry: &LogEntry) -> bool {
        match self {
            Filter::Runs => matches!(entry.level.as_str(), "success" | "error" | "warn"),
            Filter::Problems => entry.level == "error" || entry.level == "warn",
            Filter::Everything => true,
        }
    }
}

impl App {
    pub(super) fn activity_page(&mut self, ui: &mut egui::Ui) {
        page_header(ui, "Activity", "Sync history, newest first.");

        ui.horizontal(|ui| {
            let filter = &mut self.activity.filter;
            ui.selectable_value(filter, Filter::Runs, "Runs");
            ui.selectable_value(filter, Filter::Problems, "Problems");
            ui.selectable_value(filter, Filter::Everything, "Everything")
                .on_hover_text("Include raw rclone output kept by older versions");
            ui.add_space(12.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.activity.search)
                    .hint_text("🔍  Filter by folder or message")
                    .desired_width(260.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("⟳ Refresh").clicked() {
                    self.reload_log();
                }
            });
        });
        ui.add_space(8.0);

        let needle = self.activity.search.to_lowercase();
        let filter = self.activity.filter;
        let rows: Vec<&LogEntry> = self
            .log
            .iter()
            .rev()
            .filter(|entry| filter.keeps(entry))
            .filter(|entry| needle.is_empty() || entry.message.to_lowercase().contains(&needle))
            .collect();

        theme::card(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            if rows.is_empty() {
                ui.label(RichText::new("Nothing to show").weak());
                return;
            }
            let row_height = 30.0;
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show_rows(ui, row_height, rows.len(), |ui, range| {
                    for entry in &rows[range] {
                        ui.horizontal(|ui| {
                            ui.set_min_height(row_height - ui.spacing().item_spacing.y);
                            level_badge(ui, &entry.level);
                            let when = entry.time.as_ref().map(friendly_time).unwrap_or_default();
                            ui.allocate_ui_with_layout(
                                egui::vec2(130.0, row_height - 8.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_min_width(130.0);
                                    ui.label(RichText::new(when).weak());
                                },
                            );
                            let text = RichText::new(&entry.message);
                            let text = if entry.level.is_empty() { text.monospace().weak() } else { text };
                            ui.add(egui::Label::new(text).truncate())
                                .on_hover_text(&entry.message);
                        });
                    }
                });
        });
    }
}

fn level_badge(ui: &mut egui::Ui, level: &str) {
    let (text, color) = match level {
        "success" => ("OK", theme::SUCCESS),
        "error" => ("ERROR", theme::DANGER),
        "warn" => ("WARN", theme::WARNING),
        "" => ("rclone", Color32::GRAY),
        other => (other, Color32::GRAY),
    };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(58.0, 20.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 6.0, color.gamma_multiply(0.18));
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::TextStyle::Small.resolve(ui.style()),
        color,
    );
}

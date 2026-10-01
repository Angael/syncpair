use eframe::egui::{self, RichText};

use super::{page_header, theme, App};
use crate::trash::{restore_file, TrashDay};
use crate::utils::{contract_home, human_bytes};

/// Files listed per folder before collapsing the rest into a count.
const FILES_SHOWN: usize = 300;

#[derive(Default)]
pub struct TrashPage {
    pub loading: bool,
    days: Vec<TrashDay>,
    error: Option<String>,
}

impl TrashPage {
    pub fn set(&mut self, result: Result<Vec<TrashDay>, String>) {
        self.loading = false;
        match result {
            Ok(days) => {
                self.days = days;
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }
}

impl App {
    pub(super) fn trash_page(&mut self, ui: &mut egui::Ui) {
        let remote = self.config.as_ref().map(|config| config.remote.clone()).unwrap_or_default();
        page_header(
            ui,
            "Trash",
            &format!(
                "Files replaced or deleted by a sync, kept for two months. Remote copies live in {remote}:trash/."
            ),
        );

        ui.horizontal(|ui| {
            if ui.button("⟳ Refresh").clicked() {
                self.scan_trash();
            }
            if self.trash.loading {
                ui.spinner();
            }
            if let Some(error) = &self.trash.error {
                ui.colored_label(theme::DANGER, format!("⚠ {error}"));
            }
        });
        ui.add_space(8.0);

        if self.trash.days.iter().all(|day| day.folders.is_empty()) && !self.trash.loading {
            theme::card(ui).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("Trash is empty").weak());
            });
            return;
        }

        let mut restore = None;
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            for (day_index, day) in self.trash.days.iter().enumerate() {
                if day.folders.is_empty() {
                    continue;
                }
                let files: usize = day.folders.iter().map(|folder| folder.files.len()).sum();
                let bytes: u64 = day.folders.iter().map(|folder| folder.total_bytes).sum();
                theme::card(ui).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    egui::CollapsingHeader::new(
                        RichText::new(format!("{}   ·   {} · {}", day.date, file_count(files), human_bytes(bytes)))
                            .strong(),
                    )
                    .id_salt(("trash-day", &day.date))
                    .show(ui, |ui| {
                        for (folder_index, folder) in day.folders.iter().enumerate() {
                            let name = folder.local.clone().unwrap_or_else(|| folder.slug.clone());
                            egui::CollapsingHeader::new(format!(
                                "🗀 {name}   ·   {} · {}",
                                file_count(folder.files.len()),
                                human_bytes(folder.total_bytes)
                            ))
                            .id_salt(("trash-folder", &day.date, &folder.slug))
                            .show(ui, |ui| {
                                for (file_index, file) in folder.files.iter().take(FILES_SHOWN).enumerate() {
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::Label::new(file.relative.display().to_string())
                                                .truncate(),
                                        );
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                let button = ui
                                                    .add_enabled(
                                                        folder.local.is_some(),
                                                        egui::Button::new("Restore").small(),
                                                    )
                                                    .on_disabled_hover_text(
                                                        "This folder is no longer in your sync list",
                                                    );
                                                if button.clicked() {
                                                    restore = Some((day_index, folder_index, file_index));
                                                }
                                                ui.label(RichText::new(human_bytes(file.bytes)).small().weak());
                                            },
                                        );
                                    });
                                }
                                let hidden = folder.files.len().saturating_sub(FILES_SHOWN);
                                if hidden > 0 {
                                    ui.label(RichText::new(format!("…and {hidden} more")).weak());
                                }
                            });
                        }
                    });
                });
                ui.add_space(8.0);
            }
        });

        if let Some((day, folder, file)) = restore {
            let folder = &self.trash.days[day].folders[folder];
            let result = folder
                .local
                .as_deref()
                .map(|local| restore_file(&folder.files[file], local));
            match result {
                Some(Ok(path)) => {
                    self.show_toast(format!("Restored {}", contract_home(&path)), false);
                    self.scan_trash();
                }
                Some(Err(error)) => self.show_toast(format!("Restore failed: {error:#}"), true),
                None => {}
            }
        }
    }
}

fn file_count(count: usize) -> String {
    if count == 1 {
        "1 file".into()
    } else {
        format!("{count} files")
    }
}

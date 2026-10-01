use eframe::egui::{self, RichText};
use std::path::Path;

use super::{page_header, theme, App};
use crate::config::{save_config, validate_config, Config, SyncEntry};
use crate::utils::contract_home;

#[derive(Default)]
pub struct FoldersPage {
    original: Option<Config>,
    draft: Option<Config>,
    error: Option<String>,
}

impl FoldersPage {
    pub fn load(&mut self, config: &Config) {
        self.original = Some(config.clone());
        self.draft = Some(config.clone());
        self.error = None;
    }

    fn dirty(&self) -> bool {
        self.original != self.draft
    }

    /// Fills a row's local path from the folder picker, or appends a new row.
    pub fn apply_pick(&mut self, row: Option<usize>, path: &Path) {
        let draft = self.draft.get_or_insert_with(|| Config { remote: String::new(), sync: Vec::new() });
        let local = contract_home(path);
        match row.and_then(|row| draft.sync.get_mut(row)) {
            Some(entry) => entry.local = local,
            None => {
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().trim_start_matches('.').to_string())
                    .unwrap_or_default();
                draft.sync.push(SyncEntry { local, remote: name });
            }
        }
    }
}

impl App {
    pub(super) fn folders_page(&mut self, ui: &mut egui::Ui) {
        page_header(
            ui,
            "Folders",
            "Each local folder is kept in two-way sync with a path on your rclone remote.",
        );

        if self.folders.draft.is_none() {
            // Config failed to parse: start an empty editor the user can save over it.
            self.folders.draft = Some(Config { remote: String::new(), sync: Vec::new() });
        }
        if let Err(error) = &self.config {
            ui.colored_label(theme::DANGER, format!("⚠ {error}"));
            ui.add_space(8.0);
        }

        let running = self.running();
        let bottom_height = 56.0;
        let mut pick_row: Option<Option<usize>> = None;

        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .max_height(ui.available_height() - bottom_height)
            .show(ui, |ui| {
                let Some(draft) = self.folders.draft.as_mut() else { return };

                theme::card(ui).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new("Remote").strong());
                            ui.label(
                                RichText::new("Name of the rclone remote, as listed by `rclone listremotes`")
                                    .small()
                                    .weak(),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut draft.remote)
                                    .hint_text("onedrive")
                                    .desired_width(220.0),
                            );
                        });
                    });
                });
                ui.add_space(12.0);

                theme::card(ui).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let mut remove = None;
                    let mut swap = None;
                    let count = draft.sync.len();

                    let spacing = ui.spacing().item_spacing.x;
                    let prefix = format!("{}:", draft.remote.trim());
                    let prefix_width = ui
                        .painter()
                        .layout_no_wrap(prefix.clone(), egui::TextStyle::Body.resolve(ui.style()), egui::Color32::WHITE)
                        .size()
                        .x;
                    // picker + three row buttons + gaps between the seven widgets
                    let fixed = 36.0 + 3.0 * 30.0 + prefix_width + 7.0 * spacing;
                    let field_width = ((ui.available_width() - fixed) / 2.0).max(120.0);

                    ui.horizontal(|ui| {
                        let header = ui.label(RichText::new("Local folder").small().weak());
                        ui.add_space((field_width + 36.0 - header.rect.width()).max(0.0));
                        ui.label(RichText::new("Remote path").small().weak());
                    });

                    for (row, entry) in draft.sync.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            // Icon buttons are sized by `min_size`, not by the global text padding.
                            ui.spacing_mut().button_padding = egui::vec2(4.0, 4.0);
                            ui.add(
                                egui::TextEdit::singleline(&mut entry.local)
                                    .hint_text("~/folder")
                                    .desired_width(field_width),
                            );
                            if ui
                                .add_sized([36.0, 30.0], egui::Button::new("🗁"))
                                .on_hover_text("Choose folder")
                                .clicked()
                            {
                                pick_row = Some(Some(row));
                            }
                            ui.label(RichText::new(&prefix).weak());
                            ui.add(
                                egui::TextEdit::singleline(&mut entry.remote)
                                    .hint_text("Backups/folder")
                                    .desired_width(field_width),
                            );
                            if ui
                                .add_enabled(row > 0, egui::Button::new("⏶").min_size(egui::vec2(30.0, 30.0)))
                                .on_hover_text("Move up")
                                .clicked()
                            {
                                swap = Some((row, row - 1));
                            }
                            if ui
                                .add_enabled(row + 1 < count, egui::Button::new("⏷").min_size(egui::vec2(30.0, 30.0)))
                                .on_hover_text("Move down")
                                .clicked()
                            {
                                swap = Some((row, row + 1));
                            }
                            if ui
                                .add(
                                    egui::Button::new(RichText::new("✖").color(theme::DANGER))
                                        .min_size(egui::vec2(30.0, 30.0)),
                                )
                                .on_hover_text("Remove from sync (files are not deleted)")
                                .clicked()
                            {
                                remove = Some(row);
                            }
                        });
                    }

                    if let Some((a, b)) = swap {
                        draft.sync.swap(a, b);
                    }
                    if let Some(row) = remove {
                        draft.sync.remove(row);
                    }

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("+  Add folder…").clicked() {
                            pick_row = Some(None);
                        }
                        if ui.button("Add empty row").clicked() {
                            draft.sync.push(SyncEntry { local: String::new(), remote: String::new() });
                        }
                    });
                });

                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!("Saved to {}", self.paths.config_file.display()))
                        .small()
                        .weak(),
                );
            });

        if let Some(row) = pick_row {
            self.pick_folder(row);
        }

        ui.separator();
        ui.horizontal(|ui| {
            let dirty = self.folders.dirty();
            let save = ui
                .add_enabled(dirty && !running, theme::primary_button("Save changes"))
                .on_disabled_hover_text(if running { "Wait for the running sync to finish" } else { "No changes" });
            if save.clicked() {
                self.save_folders();
            }
            if ui.add_enabled(dirty, egui::Button::new("Discard")).clicked() {
                self.reload_config();
            }
            if let Some(error) = &self.folders.error {
                ui.colored_label(theme::DANGER, format!("⚠ {error}"));
            } else if dirty {
                if let Some(Err(error)) = self.folders.draft.as_ref().map(validate_config) {
                    ui.colored_label(theme::WARNING, format!("{error:#}"));
                } else {
                    ui.label(RichText::new("Unsaved changes").weak());
                }
            }
        });
    }

    fn save_folders(&mut self) {
        let Some(draft) = self.folders.draft.clone() else { return };
        match save_config(&self.paths, &draft) {
            Ok(()) => {
                self.reload_config();
                self.show_toast("Folders saved", false);
            }
            Err(error) => self.folders.error = Some(format!("{error:#}")),
        }
    }
}

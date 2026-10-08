use eframe::egui::{self, Color32, RichText};
use std::collections::VecDeque;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

use super::{friendly_time, page_header, status_color, theme, App, Details};
use crate::logs::LogEntry;
use crate::rclone::{ResyncMode, RunMode, RunOutcome, SyncEvent};

/// Lines of rclone output kept for the live output panel.
const OUTPUT_LIMIT: usize = 2_000;
/// Number of past runs shown in the history strip.
const HISTORY_LEN: usize = 30;
/// Width kept free for Details / Sync now / More in the hero card.
const HERO_ACTIONS_WIDTH: f32 = 330.0;

pub enum EntryStatus {
    Pending,
    Running,
    Done,
    Failed(String),
    /// Interrupted by Cancel while rclone was running.
    Stopped,
    /// Never started because the run was cancelled first.
    Skipped,
}

pub struct EntryRun {
    pub index: usize,
    pub status: EntryStatus,
    pub progress: f32,
    pub stats: String,
    /// This folder's share of the run output, for its *Show output* button.
    pub output: VecDeque<String>,
}

pub struct RunState {
    pub mode: RunMode,
    pub dry_run: bool,
    pub entries: Vec<EntryRun>,
    pub output: VecDeque<String>,
    pub cancel: Arc<AtomicBool>,
    pub started: Instant,
    pub outcome: Option<Result<RunOutcome, String>>,
    /// Set when a folder fails so the output panel opens by itself once.
    reveal_output: bool,
}

impl RunState {
    pub fn new(mode: RunMode, dry_run: bool, selected: &[usize], cancel: Arc<AtomicBool>) -> Self {
        Self {
            mode,
            dry_run,
            entries: selected
                .iter()
                .map(|&index| EntryRun {
                    index,
                    status: EntryStatus::Pending,
                    progress: 0.0,
                    stats: String::new(),
                    output: VecDeque::new(),
                })
                .collect(),
            output: VecDeque::new(),
            cancel,
            started: Instant::now(),
            outcome: None,
            reveal_output: false,
        }
    }

    fn entry_mut(&mut self, index: usize) -> Option<&mut EntryRun> {
        self.entries.iter_mut().find(|entry| entry.index == index)
    }

    fn entry(&self, index: usize) -> Option<&EntryRun> {
        self.entries.iter().find(|entry| entry.index == index)
    }

    pub fn apply(&mut self, event: SyncEvent) {
        match event {
            SyncEvent::EntryStarted(index) => {
                if let Some(entry) = self.entry_mut(index) {
                    entry.status = EntryStatus::Running;
                }
            }
            SyncEvent::Output(line) => {
                if let Some(entry) =
                    self.entries.iter_mut().find(|entry| matches!(entry.status, EntryStatus::Running))
                {
                    push_limited(&mut entry.output, line.clone());
                }
                push_limited(&mut self.output, line);
            }
            SyncEvent::Progress(index, fraction, stats) => {
                if let Some(entry) = self.entry_mut(index) {
                    entry.progress = fraction;
                    entry.stats = stats.trim_start_matches("NOTICE:").trim().to_string();
                }
            }
            SyncEvent::EntryFinished(index, result) => {
                if let Some(entry) = self.entry_mut(index) {
                    entry.progress = 1.0;
                    entry.status = match result {
                        Ok(()) => EntryStatus::Done,
                        Err(error) if error == "Cancelled" => EntryStatus::Stopped,
                        Err(error) => EntryStatus::Failed(error),
                    };
                    if matches!(entry.status, EntryStatus::Failed(_)) {
                        self.reveal_output = true;
                    }
                }
            }
        }
    }

    pub fn finish(&mut self, result: Result<RunOutcome, String>) {
        for entry in &mut self.entries {
            if matches!(entry.status, EntryStatus::Pending | EntryStatus::Running) {
                entry.status = EntryStatus::Skipped;
            }
        }
        self.outcome = Some(result);
    }

    fn finished_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| !matches!(entry.status, EntryStatus::Pending | EntryStatus::Running))
            .count()
    }

    fn overall_progress(&self) -> f32 {
        let running: f32 = self
            .entries
            .iter()
            .filter(|entry| matches!(entry.status, EntryStatus::Running))
            .map(|entry| entry.progress)
            .sum();
        (self.finished_count() as f32 + running) / self.entries.len().max(1) as f32
    }

    fn current(&self) -> Option<&EntryRun> {
        self.entries.iter().find(|entry| matches!(entry.status, EntryStatus::Running))
    }
}

impl App {
    pub(super) fn overview(&mut self, ui: &mut egui::Ui) {
        let subtitle = match &self.config {
            Ok(config) => format!("{} folders backed up to {}", config.sync.len(), config.remote),
            Err(_) => String::new(),
        };
        page_header(ui, "Overview", &subtitle);

        if let Err(error) = self.config.clone() {
            theme::card(ui).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.colored_label(theme::DANGER, "⚠ Your configuration could not be loaded");
                ui.label(RichText::new(error).monospace());
                ui.add_space(6.0);
                if ui.button("Open folder editor").clicked() {
                    self.open_page(super::Page::Folders);
                }
            });
            return;
        }

        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            self.hero_card(ui);
            ui.add_space(12.0);
            ui.columns(2, |columns| {
                self.schedule_card(&mut columns[0]);
                self.history_card(&mut columns[1]);
            });
            ui.add_space(12.0);
            self.folders_card(ui);
            if self.run.is_some() {
                ui.add_space(12.0);
                self.output_card(ui);
            }
        });
    }

    fn hero_card(&mut self, ui: &mut egui::Ui) {
        let running = self.running();
        let busy_elsewhere = self.timer.service_running && !running;

        theme::card(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (icon, color, title, detail) = self.hero_status();
                ui.label(RichText::new(icon).size(34.0).color(color));
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    // Leave room for the action buttons on the right so long reasons wrap before them.
                    ui.set_max_width((ui.available_width() - HERO_ACTIONS_WIDTH).max(200.0));
                    ui.label(RichText::new(title).size(18.0).strong());
                    ui.label(RichText::new(detail).weak());
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if running {
                        let cancelling = self
                            .run
                            .as_ref()
                            .is_some_and(|run| run.cancel.load(std::sync::atomic::Ordering::Relaxed));
                        let button = ui.add_enabled(!cancelling, egui::Button::new("Cancel"));
                        if button.clicked() {
                            self.cancel_run();
                        }
                        return;
                    }

                    ui.add_enabled_ui(!busy_elsewhere, |ui| {
                        ui.menu_button("More  ⏷", |ui| {
                            ui.set_min_width(220.0);
                            let mut choice = None;
                            if ui.button("Preview sync (dry run)").clicked() {
                                choice = Some((RunMode::Normal, true));
                            }
                            ui.separator();
                            if ui.button("Resync — newer file wins").clicked() {
                                choice = Some((RunMode::Resync(ResyncMode::Newer), false));
                            }
                            if ui.button("Resync — older file wins").clicked() {
                                choice = Some((RunMode::Resync(ResyncMode::Older), false));
                            }
                            if ui.button("Preview resync (newer wins)").clicked() {
                                choice = Some((RunMode::Resync(ResyncMode::Newer), true));
                            }
                            if ui.button("Preview resync (older wins)").clicked() {
                                choice = Some((RunMode::Resync(ResyncMode::Older), true));
                            }
                            if let Some((mode, dry_run)) = choice {
                                self.request_run(mode, dry_run);
                            }
                        });
                        if ui.add(theme::primary_button("⟳  Sync now")).clicked() {
                            self.request_run(RunMode::Normal, false);
                        }
                    });
                    if !busy_elsewhere {
                        let failure = self.last_run_failures().first().copied();
                        if let Some(failure) = failure.filter(|entry| entry.details.is_some()) {
                            if ui.button("Details").on_hover_text("Show rclone output").clicked() {
                                self.details = Some(Details::from_log(failure));
                            }
                        }
                    }
                });
            });

            if let Some(run) = self.run.as_ref().filter(|_| running) {
                ui.add_space(10.0);
                ui.add(
                    egui::ProgressBar::new(run.overall_progress())
                        .desired_height(8.0)
                        .fill(theme::ACCENT)
                        .animate(true),
                );
                if let Some(current) = run.current().filter(|entry| !entry.stats.is_empty()) {
                    ui.label(RichText::new(&current.stats).small().weak().monospace());
                }
            }
        });
    }

    /// Icon, color, title, detail line for the hero card.
    fn hero_status(&self) -> (&'static str, Color32, String, String) {
        if let Some(run) = self.run.as_ref().filter(|_| self.running()) {
            let label = run.mode.label(run.dry_run);
            let done = run.finished_count();
            let total = run.entries.len();
            let elapsed = run.started.elapsed().as_secs();
            if run.cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return (
                    "⏸",
                    theme::WARNING,
                    "Stopping…".into(),
                    "rclone is finishing the files in flight so nothing is left half-written.".into(),
                );
            }
            return (
                "⟳",
                theme::ACCENT,
                format!("{label} — {} of {total}", (done + 1).min(total)),
                format!("Running for {}m {:02}s", elapsed / 60, elapsed % 60),
            );
        }
        if self.timer.service_running {
            return (
                "⟳",
                theme::ACCENT,
                "Scheduled sync is running".into(),
                "Manual runs are available again once it finishes.".into(),
            );
        }
        match self.log.iter().rev().find(|entry| entry.is_run_result()) {
            Some(entry) => {
                let when = entry.time.as_ref().map(friendly_time).unwrap_or_default();
                if entry.level == "success" {
                    return ("✔", theme::SUCCESS, "Everything is backed up".into(), format!("Last sync {when}"));
                }
                let failures = self.last_run_failures();
                let detail = match failures.first() {
                    Some(first) => {
                        let reason = first.message.split_once(" failed for ").map_or(&*first.message, |(_, rest)| rest);
                        let more = match failures.len() - 1 {
                            0 => String::new(),
                            extra => format!(" (+{extra} more in Activity)"),
                        };
                        format!("{when} · {reason}{more}")
                    }
                    None => format!("{when} · {}", entry.message),
                };
                ("⚠", theme::DANGER, "Last sync had problems".into(), detail)
            }
            None => ("○", theme::WARNING, "Not synced yet".into(), "Run your first sync to get started.".into()),
        }
    }

    /// Per-folder failures logged by the most recent real run, oldest first.
    fn last_run_failures(&self) -> Vec<&LogEntry> {
        let Some(end) = self.log.iter().rposition(LogEntry::is_run_result) else { return Vec::new() };
        let Some((label, _)) = self.log[end].message.split_once(" finished") else { return Vec::new() };
        let prefix = format!("{label} failed for ");
        let start = self.log[..end].iter().rposition(LogEntry::is_run_result).map_or(0, |index| index + 1);
        self.log[start..end]
            .iter()
            .filter(|entry| entry.level == "error" && entry.message.starts_with(&prefix))
            .collect()
    }

    fn schedule_card(&mut self, ui: &mut egui::Ui) {
        theme::card(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(92.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Daily automatic sync").strong());
                    let detail = match (&self.timer.next_run, self.timer.enabled) {
                        (Some(next), true) => format!("Next run {next}"),
                        (None, true) => "Enabled".to_string(),
                        _ => "Off — folders only sync when you press Sync now".to_string(),
                    };
                    ui.label(RichText::new(detail).small().weak());
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.timer_busy {
                        ui.spinner();
                        return;
                    }
                    let mut on = self.timer.enabled;
                    if theme::toggle(ui, &mut on, true).changed() {
                        self.set_timer(on);
                    }
                });
            });
        });
    }

    fn history_card(&mut self, ui: &mut egui::Ui) {
        let runs: Vec<_> = self.log.iter().filter(|entry| entry.is_run_result()).collect();
        let failures = runs.iter().filter(|entry| entry.level != "success").count();

        theme::card(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(92.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Recent runs").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{} total · {failures} with problems", runs.len()))
                            .small()
                            .weak(),
                    );
                });
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let start = runs.len().saturating_sub(HISTORY_LEN);
                for entry in &runs[start..] {
                    let (rect, response) =
                        ui.allocate_exact_size(egui::vec2(10.0, 22.0), egui::Sense::hover());
                    let color = status_color(entry.level == "success");
                    let fill = if response.hovered() { color } else { color.gamma_multiply(0.8) };
                    ui.painter().rect_filled(rect, 3.0, fill);
                    let when = entry.time.as_ref().map(friendly_time).unwrap_or_default();
                    response.on_hover_text(format!("{when}\n{}", entry.message));
                }
                if runs.is_empty() {
                    ui.label(RichText::new("No runs yet").small().weak());
                }
            });
        });
    }

    fn folders_card(&mut self, ui: &mut egui::Ui) {
        let Ok(config) = &self.config else { return };
        let running = self.running();

        theme::card(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new("Folders").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add_enabled(!running, egui::Button::new("Edit").small()).clicked() {
                        self.page = super::Page::Folders;
                    }
                    let all = self.selected.iter().all(|checked| *checked);
                    let label = if all { "Select none" } else { "Select all" };
                    if ui.add_enabled(!running, egui::Button::new(label).small()).clicked() {
                        self.selected.iter_mut().for_each(|checked| *checked = !all);
                    }
                });
            });
            ui.add_space(4.0);

            for (index, entry) in config.sync.iter().enumerate() {
                let state = self.run.as_ref().and_then(|run| run.entry(index));
                ui.separator();
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(!running, |ui| {
                        if let Some(checked) = self.selected.get_mut(index) {
                            ui.checkbox(checked, "");
                        }
                    });
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&entry.local).strong());
                        ui.label(
                            RichText::new(format!("☁ {}", config.remote_target(entry)))
                                .small()
                                .weak(),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        entry_status(ui, state);
                    });
                });
                if let Some(EntryStatus::Failed(error)) = state.map(|state| &state.status) {
                    ui.horizontal(|ui| {
                        if ui.small_button("Show output").clicked() {
                            self.details = state.map(|state| Details {
                                title: format!("{} failed", entry.local),
                                message: error.clone(),
                                output: state.output.iter().cloned().collect(),
                            });
                        }
                        ui.add(egui::Label::new(RichText::new(error).color(theme::DANGER)).wrap());
                    });
                }
            }
        });
    }

    fn output_card(&mut self, ui: &mut egui::Ui) {
        let Some(run) = &mut self.run else { return };
        let reveal = std::mem::take(&mut run.reveal_output);
        theme::card(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::CollapsingHeader::new(RichText::new("rclone output").strong())
                .id_salt("rclone-output")
                .default_open(false)
                .open(reveal.then_some(true))
                .show(ui, |ui| {
                    let row_height = ui.text_style_height(&egui::TextStyle::Monospace);
                    egui::Frame::new()
                        .fill(ui.visuals().extreme_bg_color)
                        .corner_radius(8)
                        .inner_margin(10)
                        .show(ui, |ui| {
                            egui::ScrollArea::both()
                                .max_height(480.0)
                                .auto_shrink([false, true])
                                .stick_to_bottom(true)
                                .show_rows(ui, row_height, run.output.len(), |ui, rows| {
                                    for line in run.output.range(rows) {
                                        ui.label(RichText::new(line).monospace());
                                    }
                                });
                        });
                });
        });
    }

    fn request_run(&mut self, mode: RunMode, dry_run: bool) {
        if dry_run {
            self.start_run(mode, dry_run);
        } else {
            self.confirm = Some((mode, dry_run));
        }
    }

    pub(super) fn confirm_run_modal(&mut self, ctx: &egui::Context) {
        let Some((mode, dry_run)) = self.confirm else { return };
        let count = self.selected.iter().filter(|checked| **checked).count();
        let response = egui::Modal::new(egui::Id::new("confirm-run")).show(ctx, |ui| {
            ui.set_width(400.0);
            ui.heading(mode.label(dry_run));
            ui.add_space(4.0);
            ui.label(mode.warning());
            if let RunMode::Resync(resync) = mode {
                ui.label(
                    RichText::new(format!("When both sides differ, the {} file wins.", resync.as_str()))
                        .weak(),
                );
            }
            ui.add_space(14.0);
            let mut decision = None;
            ui.horizontal(|ui| {
                let label = format!("Sync {count} folder{}", if count == 1 { "" } else { "s" });
                let start = match mode {
                    RunMode::Normal => theme::primary_button(&label),
                    RunMode::Resync(_) => theme::danger_button(&label),
                };
                if ui.add_enabled(count > 0, start).clicked() {
                    decision = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    decision = Some(false);
                }
            });
            decision
        });
        match response.inner {
            Some(true) => {
                self.confirm = None;
                self.start_run(mode, dry_run);
            }
            Some(false) => self.confirm = None,
            None if response.should_close() => self.confirm = None,
            None => {}
        }
    }
}

fn entry_status(ui: &mut egui::Ui, state: Option<&EntryRun>) {
    let Some(state) = state else { return };
    match &state.status {
        EntryStatus::Pending => {
            ui.label(RichText::new("Waiting").small().weak());
        }
        EntryStatus::Running => {
            ui.add(
                egui::ProgressBar::new(state.progress)
                    .desired_width(140.0)
                    .desired_height(6.0)
                    .fill(theme::ACCENT),
            );
            ui.spinner();
        }
        EntryStatus::Done => {
            ui.label(RichText::new("✔ Synced").color(theme::SUCCESS));
        }
        EntryStatus::Failed(_) => {
            ui.label(RichText::new("✖ Failed").color(theme::DANGER));
        }
        EntryStatus::Stopped => {
            ui.label(RichText::new("Stopped").small().color(theme::WARNING));
        }
        EntryStatus::Skipped => {
            ui.label(RichText::new("Skipped").small().weak());
        }
    }
}

fn push_limited(lines: &mut VecDeque<String>, line: String) {
    if lines.len() == OUTPUT_LIMIT {
        lines.pop_front();
    }
    lines.push_back(line);
}

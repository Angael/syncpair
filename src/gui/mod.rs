mod activity;
mod folders;
mod overview;
mod theme;
mod trash_page;

use anyhow::{anyhow, Result};
use chrono::{DateTime, FixedOffset, Local};
use eframe::egui::{self, Color32, Margin, RichText};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::{load_config, Config};
use crate::logs::{read_log, LogEntry};
use crate::paths::AppPaths;
use crate::rclone::{run_sync, RunMode, RunOutcome, RunSpec, SyncEvent, Verbosity};
use crate::timer::{install_timer, remove_timer, timer_state, TimerState};
use crate::trash::{scan_trash, TrashDay};
use crate::utils::notify;

use self::folders::FoldersPage;
use self::overview::RunState;
use self::trash_page::TrashPage;

/// How often systemd timer/service state is re-read while the window is open.
const TIMER_POLL: Duration = Duration::from_secs(15);

/// Window icon for X11; on Wayland the compositor takes it from `syncpair.desktop` via the app id.
const WINDOW_ICON_PNG: &[u8] = include_bytes!("../../packaging/icons/hicolor/256x256/apps/syncpair.png");

pub fn run(paths: AppPaths) -> Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Syncpair")
        .with_app_id("syncpair")
        .with_inner_size([960.0, 680.0])
        .with_min_inner_size([720.0, 480.0]);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(WINDOW_ICON_PNG) {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Syncpair",
        options,
        Box::new(move |cc| {
            theme::install(&cc.egui_ctx);
            Ok(Box::new(App::new(paths, cc.egui_ctx.clone())))
        }),
    )
    .map_err(|error| anyhow!("Failed to start the window: {error}"))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Overview,
    Folders,
    Activity,
    Trash,
}

impl Page {
    const ALL: [Page; 4] = [Page::Overview, Page::Folders, Page::Activity, Page::Trash];

    fn title(self) -> &'static str {
        match self {
            Page::Overview => "Overview",
            Page::Folders => "Folders",
            Page::Activity => "Activity",
            Page::Trash => "Trash",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Page::Overview => "⟳",
            Page::Folders => "🗀",
            Page::Activity => "☰",
            Page::Trash => "🗑",
        }
    }
}

/// Results from background threads, drained once per frame.
enum Msg {
    Sync(SyncEvent),
    SyncDone(Result<RunOutcome, String>),
    Timer(TimerState),
    TimerChanged(Result<(), String>),
    Trash(Result<Vec<TrashDay>, String>),
    PickedFolder { row: Option<usize>, path: Option<PathBuf> },
}

struct Toast {
    text: String,
    error: bool,
    shown_at: Instant,
}

pub struct App {
    paths: AppPaths,
    ctx: egui::Context,
    sender: mpsc::Sender<Msg>,
    receiver: mpsc::Receiver<Msg>,

    page: Page,
    config: Result<Config, String>,
    log: Vec<LogEntry>,
    timer: TimerState,
    timer_polled: Instant,
    timer_busy: bool,

    /// Folders checked for the next manual run, parallel to `config.sync`.
    selected: Vec<bool>,
    run: Option<RunState>,
    confirm: Option<(RunMode, bool)>,
    quit_after_run: bool,
    confirm_quit: bool,

    folders: FoldersPage,
    activity: activity::ActivityPage,
    trash: TrashPage,
    toast: Option<Toast>,
}

impl App {
    fn new(paths: AppPaths, ctx: egui::Context) -> Self {
        let (sender, receiver) = mpsc::channel();
        let mut app = Self {
            paths,
            ctx,
            sender,
            receiver,
            page: Page::Overview,
            config: Err(String::new()),
            log: Vec::new(),
            timer: TimerState::default(),
            timer_polled: Instant::now(),
            timer_busy: false,
            selected: Vec::new(),
            run: None,
            confirm: None,
            quit_after_run: false,
            confirm_quit: false,
            folders: FoldersPage::default(),
            activity: activity::ActivityPage::default(),
            trash: TrashPage::default(),
            toast: None,
        };
        app.reload_config();
        app.reload_log();
        app.poll_timer();
        app
    }

    fn reload_config(&mut self) {
        self.config = load_config(&self.paths).map_err(|error| format!("{error:#}"));
        let count = self.config.as_ref().map(|c| c.sync.len()).unwrap_or(0);
        self.selected = vec![true; count];
        if let Ok(config) = &self.config {
            self.folders.load(config);
        }
    }

    fn reload_log(&mut self) {
        self.log = read_log(&self.paths).unwrap_or_default();
    }

    fn running(&self) -> bool {
        self.run.as_ref().is_some_and(|run| run.outcome.is_none())
    }

    fn spawn<F: FnOnce() -> Msg + Send + 'static>(&self, job: F) {
        let sender = self.sender.clone();
        let ctx = self.ctx.clone();
        thread::spawn(move || {
            let _ = sender.send(job());
            ctx.request_repaint();
        });
    }

    fn poll_timer(&mut self) {
        self.timer_polled = Instant::now();
        self.spawn(|| Msg::Timer(timer_state()));
    }

    fn set_timer(&mut self, enable: bool) {
        self.timer_busy = true;
        let paths = self.paths.clone();
        self.spawn(move || {
            let result = if enable { install_timer(&paths) } else { remove_timer(&paths) };
            Msg::TimerChanged(result.map_err(|error| format!("{error:#}")))
        });
    }

    fn scan_trash(&mut self) {
        self.trash.loading = true;
        let trash_dir = self.paths.trash_dir.clone();
        let config = self.config.as_ref().ok().cloned();
        self.spawn(move || {
            Msg::Trash(scan_trash(&trash_dir, config.as_ref()).map_err(|error| format!("{error:#}")))
        });
    }

    fn pick_folder(&self, row: Option<usize>) {
        self.spawn(move || Msg::PickedFolder {
            row,
            path: rfd::FileDialog::new().set_title("Choose a folder").pick_folder(),
        });
    }

    fn start_run(&mut self, mode: RunMode, dry_run: bool) {
        let Ok(config) = self.config.clone() else { return };
        let selected: Vec<usize> = (0..config.sync.len())
            .filter(|&index| self.selected.get(index).copied().unwrap_or(false))
            .collect();
        if selected.is_empty() {
            self.show_toast("Select at least one folder", true);
            return;
        }

        let cancel = Arc::new(AtomicBool::new(false));
        self.run = Some(RunState::new(mode, dry_run, &selected, cancel.clone()));

        let paths = self.paths.clone();
        let sender = self.sender.clone();
        let ctx = self.ctx.clone();
        thread::spawn(move || {
            let mut forward = |event: SyncEvent| {
                let _ = sender.send(Msg::Sync(event));
                ctx.request_repaint();
            };
            let result = run_sync(
                &paths,
                &config,
                RunSpec { mode, dry_run, verbosity: Verbosity { progress: true, verbose: true } },
                &selected,
                &cancel,
                &mut forward,
            );
            let _ = sender.send(Msg::SyncDone(result.map_err(|error| format!("{error:#}"))));
            ctx.request_repaint();
        });
    }

    fn cancel_run(&self) {
        if let Some(run) = &self.run {
            run.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn show_toast(&mut self, text: impl Into<String>, error: bool) {
        self.toast = Some(Toast { text: text.into(), error, shown_at: Instant::now() });
    }

    fn drain_messages(&mut self) {
        while let Ok(msg) = self.receiver.try_recv() {
            match msg {
                Msg::Sync(event) => {
                    if let Some(run) = &mut self.run {
                        run.apply(event);
                    }
                }
                Msg::SyncDone(result) => self.finish_run(result),
                Msg::Timer(state) => self.timer = state,
                Msg::TimerChanged(result) => {
                    self.timer_busy = false;
                    match result {
                        Ok(()) => self.show_toast("Daily schedule updated", false),
                        Err(error) => self.show_toast(error, true),
                    }
                    self.reload_log();
                    self.poll_timer();
                }
                Msg::Trash(result) => self.trash.set(result),
                Msg::PickedFolder { row, path: Some(path) } => self.folders.apply_pick(row, &path),
                Msg::PickedFolder { path: None, .. } => {}
            }
        }
    }

    fn finish_run(&mut self, result: Result<RunOutcome, String>) {
        let window_focused = self.ctx.input(|input| input.viewport().focused.unwrap_or(true));
        match &result {
            Ok(outcome) => {
                self.show_toast(outcome.message.clone(), !outcome.ok());
                if !window_focused && !outcome.cancelled {
                    let summary = if outcome.ok() { "Task Finished" } else { "Backup Error" };
                    let _ = notify(summary, &outcome.message, !outcome.ok());
                }
            }
            Err(error) => self.show_toast(error.clone(), true),
        }
        if let Some(run) = &mut self.run {
            run.finish(result);
        }
        self.reload_log();
        if self.page == Page::Trash {
            self.scan_trash();
        }
        if self.quit_after_run {
            self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn open_page(&mut self, page: Page) {
        if self.page == page {
            return;
        }
        self.page = page;
        match page {
            Page::Activity => self.reload_log(),
            Page::Trash => self.scan_trash(),
            Page::Overview | Page::Folders => {}
        }
    }

    fn handle_close_request(&mut self, ctx: &egui::Context) {
        if ctx.input(|input| input.viewport().close_requested()) && self.running() {
            // Never exit under a running rclone; `finish_run` closes once it has stopped.
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm_quit = !self.quit_after_run;
        }
    }

    fn confirm_quit_modal(&mut self, ctx: &egui::Context) {
        if !self.confirm_quit {
            return;
        }
        let response = egui::Modal::new(egui::Id::new("quit-modal")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading("Sync in progress");
            ui.add_space(4.0);
            ui.label("Quitting now stops rclone safely and leaves the remaining folders for the next run.");
            ui.add_space(12.0);
            let mut close = false;
            ui.horizontal(|ui| {
                if ui.add(theme::danger_button("Stop and quit")).clicked() {
                    self.quit_after_run = true;
                    self.cancel_run();
                    close = true;
                }
                if ui.button("Keep running").clicked() {
                    close = true;
                }
            });
            close
        });
        if response.inner || response.should_close() {
            self.confirm_quit = false;
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        let frame = egui::Frame::new()
            .fill(ui.visuals().extreme_bg_color)
            .inner_margin(Margin::symmetric(12, 16));
        egui::Panel::left("nav")
            .resizable(false)
            .exact_size(200.0)
            .frame(frame)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("⟲").size(24.0).color(theme::ACCENT));
                    ui.label(RichText::new("Syncpair").size(19.0).strong());
                });
                ui.add_space(18.0);

                for page in Page::ALL {
                    let selected = self.page == page;
                    let text = RichText::new(format!("{}   {}", page.icon(), page.title())).size(15.0);
                    let button = egui::Button::selectable(selected, text)
                        .min_size(egui::vec2(ui.available_width(), 38.0));
                    if ui.add(button).clicked() {
                        self.open_page(page);
                    }
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    if let Ok(config) = &self.config {
                        ui.label(
                            RichText::new(format!("remote: {}", config.remote))
                                .small()
                                .weak(),
                        );
                    }
                    if self.running() {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(RichText::new("Syncing…").small());
                        });
                    }
                });
            });
    }

    fn toast(&mut self, ctx: &egui::Context) {
        let Some(toast) = &self.toast else { return };
        let age = toast.shown_at.elapsed().as_secs_f32();
        let lifetime = if toast.error { 8.0 } else { 4.0 };
        if age > lifetime {
            self.toast = None;
            return;
        }
        let opacity = ((lifetime - age) / 0.3).min(age / 0.15).clamp(0.0, 1.0);
        let color = if toast.error { theme::DANGER } else { theme::SUCCESS };
        egui::Area::new(egui::Id::new("toast"))
            .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -24.0])
            .interactable(false)
            .show(ctx, |ui| {
                ui.set_opacity(opacity);
                theme::card(ui)
                    .stroke(egui::Stroke::new(1.0, color))
                    .shadow(ui.visuals().popup_shadow)
                    .show(ui, |ui| {
                        ui.set_max_width(520.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(if toast.error { "⚠" } else { "✔" }).color(color));
                            ui.label(&toast.text);
                        });
                    });
            });
        ctx.request_repaint_after(Duration::from_millis(33));
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_messages();
        if self.timer_polled.elapsed() >= TIMER_POLL {
            self.poll_timer();
        }
        ctx.request_repaint_after(TIMER_POLL);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_close_request(&ctx);
        self.sidebar(ui);

        let frame = egui::Frame::new()
            .fill(ui.visuals().panel_fill)
            .inner_margin(Margin::symmetric(28, 22));
        egui::CentralPanel::default().frame(frame).show(ui, |ui| match self.page {
            Page::Overview => self.overview(ui),
            Page::Folders => self.folders_page(ui),
            Page::Activity => self.activity_page(ui),
            Page::Trash => self.trash_page(ui),
        });

        self.confirm_run_modal(&ctx);
        self.confirm_quit_modal(&ctx);
        self.toast(&ctx);
    }
}

fn page_header(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.label(RichText::new(title).size(26.0).strong());
    if !subtitle.is_empty() {
        ui.label(RichText::new(subtitle).weak());
    }
    ui.add_space(14.0);
}

/// `Today 08:57`, `Yesterday 23:35`, `Sep 13 08:38`.
fn friendly_time(time: &DateTime<FixedOffset>) -> String {
    let local = time.with_timezone(&Local);
    let today = Local::now().date_naive();
    let date = local.date_naive();
    let clock = local.format("%H:%M");
    if date == today {
        format!("Today {clock}")
    } else if today.pred_opt() == Some(date) {
        format!("Yesterday {clock}")
    } else {
        format!("{} {clock}", local.format("%b %-d"))
    }
}

fn status_color(ok: bool) -> Color32 {
    if ok {
        theme::SUCCESS
    } else {
        theme::DANGER
    }
}

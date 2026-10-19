use super::{App, Work};
use eframe::egui;
impl App {
    pub(super) fn start_app_update_check(&mut self) {
        if self.game_window
            || !self
                .applied_settings
                .as_ref()
                .is_some_and(|s| s.check_app_updates_on_startup)
        {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.startup_app_check = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(crate::app_updates::check().map_err(|e| format!("{e:#}")));
        });
    }
    pub(super) fn poll_app_update_check(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.startup_app_check {
            match rx.try_recv() {
                Ok(result) => {
                    self.startup_app_check = None;
                    match result {
                        Ok(Some(release)) => {
                            self.app_release = Some(release);
                            self.app_update_notice = true;
                            self.app_update_error = None;
                        }
                        Ok(None) => {
                            self.app_update_error = None;
                        }
                        Err(error) => {
                            self.app_update_error =
                                Some(format!("Startup update check unavailable: {error}"))
                        }
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(100))
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.startup_app_check = None;
                }
            }
        }
        if self.app_update_notice && !self.game_window {
            if let Some(release) = self.app_release.clone() {
                egui::Window::new("GamePKG update available")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label(format!("GamePKG {} is available.", release.version));
                        ui.label("Review and install the new release in Settings.");
                        ui.horizontal(|ui| {
                            if ui.button("Review app update").clicked() {
                                self.show_settings = true;
                                self.show_add = false;
                                self.show_support = false;
                                self.app_update_notice = false;
                            }
                            if ui.button("Later").clicked() {
                                self.app_update_notice = false;
                            }
                        });
                    });
            }
        }
    }
    pub(super) fn app_update_controls(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("GamePKG updates");
        ui.checkbox(
            &mut self.settings_draft.check_app_updates_on_startup,
            "Check for app updates when GamePKG opens",
        );
        ui.label("Save settings to apply. Checks run in the background; downloads and installation require your choice.");
        if let Some(error) = &self.app_update_error {
            ui.label(error);
        }
        ui.label(format!("Installed version: {}", env!("CARGO_PKG_VERSION")));
        ui.hyperlink_to(
            "Official releases",
            "https://github.com/centrexity/GamePKG/releases",
        );
        let busy = self.result.is_some();
        if ui
            .add_enabled(!busy, egui::Button::new("Check for GamePKG updates"))
            .clicked()
        {
            self.startup_app_check = None;
            self.app_update_notice = false;
            self.app_update_error = None;
            self.begin_work(|| Ok(Work::AppRelease(crate::app_updates::check()?)));
        }
        if let Some(release) = self.app_release.clone() {
            ui.label(format!(
                "Version {} is available for this computer.",
                release.version
            ));
            ui.label("Choose a folder for the new portable version. Your current copy and game library are kept.");
            if ui
                .add_enabled(
                    !busy,
                    egui::Button::new("Download and install GamePKG update"),
                )
                .clicked()
            {
                self.begin_work(move || {
                    let mut picker = rfd::FileDialog::new()
                        .set_title("Choose where to install the new GamePKG version");
                    if let Some(parent) = crate::app_updates::suggested_parent() {
                        picker = picker.set_directory(parent);
                    }
                    let Some(parent) = picker.pick_folder() else {
                        return Ok(Work::Message("Update installation cancelled.".into()));
                    };
                    Ok(Work::AppInstalled(crate::app_updates::install(
                        &release, &parent,
                    )?))
                });
            }
        }
        if let Some(path) = self.app_installed.clone() {
            ui.label(format!("New version: {}", path.display()));
            ui.label("Update your shortcuts to this copy. The previous version remains available for rollback.");
            // Existing games depend on this host's resources. Let them finish first.
            let running = !self.game_processes.is_empty();
            if running {
                ui.label("Close running games before restarting GamePKG.");
            }
            if ui
                .add_enabled(
                    !busy && !running,
                    egui::Button::new("Restart into updated GamePKG"),
                )
                .clicked()
            {
                let mut command = std::process::Command::new(&path);
                if let Some(root) = path.parent().and_then(|bin| bin.parent()) {
                    command.current_dir(root);
                }
                if let Some(root) = std::env::var_os("GAMEPKG_HOME") {
                    command.env("GAMEPKG_HOME", root);
                }
                match command.spawn() {
                    Ok(_) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                    Err(error) => {
                        self.status = format!(
                        "Unable to start the update: {error}. The current copy is still available."
                    )
                    }
                }
            }
        }
        ui.separator();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_checks_skip_opted_out_and_game_only_windows() {
        for game_window in [false, true] {
            let mut app = App {
                game_window,
                applied_settings: Some(crate::settings::Settings {
                    check_app_updates_on_startup: game_window,
                    ..Default::default()
                }),
                ..Default::default()
            };
            app.start_app_update_check();
            assert!(app.startup_app_check.is_none());
        }
    }
    #[test]
    fn startup_failure_and_current_version_do_not_interrupt_foreground_work() {
        for result in [Ok(None), Err("offline".to_owned())] {
            let failed = result.is_err();
            let (tx, rx) = std::sync::mpsc::channel();
            tx.send(result).unwrap();
            let (_work_tx, work_rx) = std::sync::mpsc::channel();
            let mut app = App {
                startup_app_check: Some(rx),
                result: Some(work_rx),
                status: "Building a game".into(),
                ..Default::default()
            };
            app.poll_app_update_check(&egui::Context::default());
            assert!(app.startup_app_check.is_none());
            assert!(!app.app_update_notice);
            assert!(app.result.is_some());
            assert_eq!(app.status, "Building a game");
            assert_eq!(app.app_update_error.is_some(), failed);
        }
    }
}

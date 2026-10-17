use super::{App, Work};
use eframe::egui;
impl App {
    pub(super) fn app_update_controls(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("GamePKG updates");
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

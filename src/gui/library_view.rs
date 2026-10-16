use super::{theme, App};
use eframe::egui::{self, RichText};

impl App {
    pub(super) fn library_games(&self, ui: &mut egui::Ui) -> Option<String> {
        let mut selected = None;
        let wide = ui.available_width() >= 980.0;
        let entries = self
            .installed
            .iter()
            .map(|m| {
                (
                    m.id.as_str(),
                    m.id.as_str(),
                    m.version.as_str(),
                    if self.built.contains(&m.id) {
                        "Ready"
                    } else {
                        "Not built"
                    },
                    true,
                )
            })
            .chain(
                self.catalog
                    .iter()
                    .filter(|e| !self.installed.iter().any(|m| m.id == e.id))
                    .map(|e| {
                        (
                            e.id.as_str(),
                            e.title.as_str(),
                            e.version.as_deref().unwrap_or("Unknown"),
                            "Not downloaded",
                            false,
                        )
                    }),
            );
        if wide {
            ui.horizontal(|ui| {
                ui.add_space(16.0 + 64.0 + 12.0);
                let name_width = (ui.available_width() - 505.0 - 16.0).max(180.0);
                ui.allocate_ui(egui::vec2(name_width, 24.0), |ui| {
                    ui.set_min_width(name_width);
                    ui.strong("Game");
                });
                for (width, label) in [
                    (70.0, "Version"),
                    (160.0, "Status"),
                    (110.0, "Played"),
                    (90.0, "Storage"),
                ] {
                    ui.allocate_ui(egui::vec2(width, 24.0), |ui| {
                        ui.set_min_width(width);
                        ui.strong(label);
                    });
                }
            });
        }
        for (id, title, version, status, installed) in entries {
            egui::Frame::group(ui.style())
                .fill(ui.visuals().extreme_bg_color)
                .inner_margin(16)
                .corner_radius(12)
                .show(ui, |ui| {
                    ui.set_min_width((ui.available_width() - 2.0).max(0.0));
                    ui.horizontal(|ui| {
                        if let Some(icon) = self.icons.get(id) {
                            if ui
                                .add(
                                    egui::Image::new(icon)
                                        .fit_to_exact_size(egui::vec2(64.0, 64.0))
                                        .sense(egui::Sense::click()),
                                )
                                .clicked()
                            {
                                selected = Some(id.to_owned());
                            }
                        } else if theme::cover(ui, id, 64.0).clicked() {
                            selected = Some(id.to_owned());
                        }
                        let played = self
                            .history
                            .get(id)
                            .map(|r| crate::progress::duration(r.played_ms))
                            .unwrap_or_else(|| "Not played".into());
                        let storage = self
                            .usage
                            .get(id)
                            .map(|u| crate::storage::bytes(u.used_bytes()))
                            .unwrap_or_else(|| "Unknown".into());
                        if wide {
                            let name_width = (ui.available_width() - 505.0).max(180.0);
                            ui.allocate_ui_with_layout(
                                egui::vec2(name_width, 64.0),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    ui.set_min_width(name_width);
                                    ui.set_min_height(64.0);
                                    if ui
                                        .add(
                                            egui::Button::new(RichText::new(title).strong())
                                                .wrap()
                                                .frame(false),
                                        )
                                        .clicked()
                                    {
                                        selected = Some(id.to_owned());
                                    }
                                    if title != id {
                                        ui.small(id);
                                    }
                                },
                            );
                            for (width, text) in [
                                (70.0, version),
                                (160.0, status),
                                (110.0, &played),
                                (90.0, &storage),
                            ] {
                                ui.allocate_ui(egui::vec2(width, 64.0), |ui| {
                                    ui.set_min_width(width);
                                    ui.set_min_height(64.0);
                                    if text == status {
                                        theme::badge(ui, text, status == "Ready");
                                    } else {
                                        ui.label(text);
                                    }
                                });
                            }
                        } else {
                            ui.vertical(|ui| {
                                ui.set_width(ui.available_width());
                                if ui
                                    .add(
                                        egui::Button::new(RichText::new(title).strong())
                                            .wrap()
                                            .frame(false),
                                    )
                                    .clicked()
                                {
                                    selected = Some(id.to_owned());
                                }
                                if title != id {
                                    ui.small(id);
                                }
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(format!("v{version}"));
                                    theme::badge(ui, status, status == "Ready");
                                });
                                if installed {
                                    ui.label(format!("{played}  •  {storage}"));
                                }
                            });
                        }
                    });
                });
            ui.add_space(4.0);
        }
        selected
    }
}

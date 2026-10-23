use super::{theme, App};
use eframe::egui::{self, RichText};

pub(super) struct Pagination {
    page: usize,
    per_page: usize,
    pub(super) scroll_to_top: bool,
}
impl Default for Pagination {
    fn default() -> Self {
        Self {
            page: 0,
            per_page: 20,
            scroll_to_top: false,
        }
    }
}
impl Pagination {
    fn reset(&mut self) {
        self.page = 0;
        self.scroll_to_top = true;
    }
    fn pages(&self, total: usize) -> usize {
        total.div_ceil(self.per_page).max(1)
    }
    fn clamp(&mut self, total: usize) {
        let page = self.page.min(self.pages(total) - 1);
        if page != self.page {
            self.page = page;
            self.scroll_to_top = true;
        }
    }
    fn controls(&mut self, ui: &mut egui::Ui, total: usize, top: bool) {
        if total <= 10 {
            return;
        }
        ui.push_id(
            if top {
                "pagination-top"
            } else {
                "pagination-bottom"
            },
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    if top {
                        let previous_size = self.per_page;
                        egui::ComboBox::from_id_salt("page-size")
                            .selected_text(format!("{} per page", self.per_page))
                            .show_ui(ui, |ui| {
                                for size in [10, 20, 50, 100] {
                                    ui.selectable_value(
                                        &mut self.per_page,
                                        size,
                                        format!("{size} per page"),
                                    );
                                }
                            });
                        if previous_size != self.per_page {
                            self.reset();
                        }
                    }
                    let pages = self.pages(total);
                    if ui
                        .add_enabled(self.page > 0, egui::Button::new("Previous page"))
                        .clicked()
                    {
                        self.page -= 1;
                        self.scroll_to_top = true;
                        ui.ctx().request_repaint();
                    }
                    ui.label(format!("Page {} of {pages}", self.page + 1));
                    if ui
                        .add_enabled(self.page + 1 < pages, egui::Button::new("Next page"))
                        .clicked()
                    {
                        self.page += 1;
                        self.scroll_to_top = true;
                        ui.ctx().request_repaint();
                    }
                    let start = self.page * self.per_page;
                    ui.label(format!(
                        "Showing {}–{} of {total} games",
                        start + 1,
                        (start + self.per_page).min(total)
                    ));
                });
            },
        );
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Filter {
    #[default]
    All,
    Ready,
    NeedsBuild,
    NeedsRom,
    NotDownloaded,
}
impl Filter {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::All => "All games",
            Self::Ready => "Ready to play",
            Self::NeedsBuild => "Needs build",
            Self::NeedsRom => "Needs ROM",
            Self::NotDownloaded => "Not downloaded",
        }
    }
    fn matches(self, installed: bool, built: bool, ready: bool, needs_rom: bool) -> bool {
        match self {
            Self::All => true,
            Self::Ready => ready,
            Self::NeedsBuild => installed && !built,
            Self::NeedsRom => needs_rom,
            Self::NotDownloaded => !installed,
        }
    }
}

impl App {
    fn needs_rom(&self, m: &crate::manifest::Manifest) -> bool {
        m.requires_rom
            && !self
                .rom_paths
                .get(&m.id)
                .is_some_and(|paths| crate::rom_selections::ready(paths, m.required_rom_count()))
    }
    pub(super) fn ready_to_play(&self, m: &crate::manifest::Manifest) -> bool {
        cfg!(target_os = "linux")
            && self.built.contains(&m.id)
            && !self.needs_rom(m)
            && (m.game_data.is_none()
                || (self
                    .data_paths
                    .get(&m.id)
                    .is_some_and(|p| crate::game_data::available_for(m, std::path::Path::new(p)))
                    && self
                        .applied_settings
                        .as_ref()
                        .is_some_and(|s| s.memory_ceiling_mib >= 2048)))
    }
    pub(super) fn library_filters(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Search");
            let search = ui.add(
                egui::TextEdit::singleline(&mut self.library_search)
                    .hint_text("Title, ID, or description")
                    .desired_width((ui.available_width() - 100.0).clamp(40.0, 420.0)),
            );
            if search.changed() {
                self.library_pagination.reset();
            }
            if ui
                .add_enabled(!self.library_search.is_empty(), egui::Button::new("Clear"))
                .clicked()
            {
                self.library_search.clear();
                self.library_pagination.reset();
                search.request_focus();
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Show:");
            for filter in [
                Filter::All,
                Filter::Ready,
                Filter::NeedsBuild,
                Filter::NeedsRom,
                Filter::NotDownloaded,
            ] {
                if ui
                    .selectable_value(&mut self.library_filter, filter, filter.label())
                    .changed()
                {
                    self.library_pagination.reset();
                }
            }
        });
        ui.add_space(8.0);
    }

    pub(super) fn library_games(&mut self, ui: &mut egui::Ui) -> Option<String> {
        let mut selected = None;
        let wide = ui.available_width() >= 980.0;
        let terms: Vec<_> = self
            .library_search
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        let entries: Vec<_> = self
            .installed
            .iter()
            .filter(|m| matches_search(&terms, &m.id, m.display_title(), m.description.as_deref()))
            .filter(|m| {
                self.library_filter.matches(
                    true,
                    self.built.contains(&m.id),
                    self.ready_to_play(m),
                    self.needs_rom(m),
                )
            })
            .map(|m| {
                (
                    m.id.as_str(),
                    m.display_title(),
                    m.version.as_str(),
                    if self.ready_to_play(m) {
                        "Ready"
                    } else if self.needs_rom(m) {
                        "Needs ROM"
                    } else if self.built.contains(&m.id) {
                        "Needs setup"
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
                    .filter(|e| matches_search(&terms, &e.id, &e.title, e.description.as_deref()))
                    .filter(|e| {
                        self.library_filter.matches(
                            false,
                            false,
                            false,
                            e.requires_rom == Some(true),
                        )
                    })
                    .map(|e| {
                        (
                            e.id.as_str(),
                            e.title.as_str(),
                            e.version.as_deref().unwrap_or("Unknown"),
                            "Not downloaded",
                            false,
                        )
                    }),
            )
            .collect();
        let total = entries.len();
        self.library_pagination.clamp(total);
        self.library_pagination.controls(ui, total, true);
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
        for (id, title, version, status, installed) in entries
            .into_iter()
            .skip(self.library_pagination.page * self.library_pagination.per_page)
            .take(self.library_pagination.per_page)
        {
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
        self.library_pagination.controls(ui, total, false);
        if total == 0 && (!self.installed.is_empty() || !self.catalog.is_empty()) {
            ui.label("No games match your search and filters.");
        }
        selected
    }
}

fn matches_search(terms: &[String], id: &str, title: &str, description: Option<&str>) -> bool {
    if terms.is_empty() {
        return true;
    }
    let text = format!("{id}\n{title}\n{}", description.unwrap_or_default()).to_lowercase();
    terms.iter().all(|term| text.contains(term))
}

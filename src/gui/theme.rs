use eframe::egui::{self, Color32, FontFamily, FontId, Stroke, TextStyle};

pub(super) const ACCENT: Color32 = Color32::from_rgb(73, 224, 193);
pub(super) fn install_click_cursor(ctx: &egui::Context) {
    ctx.on_end_pass(
        "gamepkg_click_cursor",
        std::sync::Arc::new(|ctx| {
            // Respect text, resize and drag cursors chosen by individual widgets.
            if ctx.output(|output| output.cursor_icon) != egui::CursorIcon::Default {
                return;
            }
            let hovered = ctx.interaction_snapshot(|interaction| interaction.hovered.clone());
            if hovered.iter().any(|id| {
                ctx.read_response(*id).is_some_and(|response| {
                    response.enabled() && response.hovered() && response.sense.senses_click()
                })
            }) {
                ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        }),
    );
}
pub(super) fn apply(ctx: &egui::Context, dark: bool, scale: u16) {
    let mut style = egui::Style::default();
    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    let (background, surface, raised, text, muted, border) = if dark {
        (
            Color32::from_rgb(10, 19, 32),
            Color32::from_rgb(17, 32, 50),
            Color32::from_rgb(28, 49, 71),
            Color32::from_rgb(241, 247, 255),
            Color32::from_rgb(181, 202, 222),
            Color32::from_rgb(65, 91, 118),
        )
    } else {
        (
            Color32::from_rgb(239, 245, 251),
            Color32::WHITE,
            Color32::from_rgb(221, 234, 246),
            Color32::from_rgb(17, 36, 57),
            Color32::from_rgb(63, 86, 108),
            Color32::from_rgb(150, 174, 197),
        )
    };
    visuals.panel_fill = background;
    visuals.window_fill = surface;
    visuals.extreme_bg_color = surface;
    visuals.faint_bg_color = surface;
    visuals.override_text_color = Some(text);
    visuals.hyperlink_color = if dark {
        ACCENT
    } else {
        Color32::from_rgb(0, 103, 88)
    };
    visuals.selection.bg_fill = if dark {
        Color32::from_rgb(24, 100, 145)
    } else {
        Color32::from_rgb(190, 225, 239)
    };
    visuals.selection.stroke = Stroke::new(1.0_f32, text);
    visuals.widgets.noninteractive.bg_fill = surface;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, muted);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, border);
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.bg_fill = raised;
        widget.weak_bg_fill = raised;
        widget.fg_stroke = Stroke::new(1.0_f32, text);
        widget.bg_stroke = Stroke::new(1.0_f32, border);
        widget.corner_radius = 8.into();
    }
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.5_f32, visuals.hyperlink_color);
    visuals.widgets.active.bg_fill = if dark {
        Color32::from_rgb(24, 100, 145)
    } else {
        Color32::from_rgb(190, 225, 239)
    };
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, text);
    style.visuals = visuals;
    style.wrap_mode = Some(egui::TextWrapMode::Wrap);
    style.spacing.item_spacing = egui::vec2(12.0, 12.0);
    style.spacing.button_padding = egui::vec2(14.0, 9.0);
    style.spacing.interact_size = egui::vec2(40.0, 38.0);
    style.spacing.window_margin = egui::Margin::same(20);
    for (kind, size) in [
        (TextStyle::Heading, 28.0),
        (TextStyle::Body, 17.0),
        (TextStyle::Button, 17.0),
        (TextStyle::Small, 14.0),
        (TextStyle::Monospace, 15.0),
    ] {
        style.text_styles.insert(
            kind.clone(),
            FontId::new(
                size,
                if kind == TextStyle::Monospace {
                    FontFamily::Monospace
                } else {
                    FontFamily::Proportional
                },
            ),
        );
    }
    ctx.set_style(style);
    ctx.set_zoom_factor(f32::from(scale) / 100.0);
}

pub(super) fn primary(label: &str) -> egui::Button<'_> {
    egui::Button::new(
        egui::RichText::new(label)
            .strong()
            .color(Color32::from_rgb(5, 35, 35)),
    )
    .fill(ACCENT)
    .min_size(egui::vec2(110.0, 44.0))
}

pub(super) fn badge(ui: &mut egui::Ui, text: &str, ready: bool) {
    let dark = ui.visuals().dark_mode;
    let color = if ready {
        if dark {
            ACCENT
        } else {
            Color32::from_rgb(0, 100, 79)
        }
    } else if dark {
        Color32::from_rgb(255, 210, 134)
    } else {
        Color32::from_rgb(125, 74, 0)
    };
    ui.label(egui::RichText::new(text).strong().color(color));
}

pub(super) fn cover(ui: &mut egui::Ui, id: &str, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    let palette = [
        ACCENT,
        Color32::from_rgb(143, 184, 255),
        Color32::from_rgb(255, 201, 122),
        Color32::from_rgb(210, 170, 255),
    ];
    let index = id
        .bytes()
        .fold(0usize, |sum, b| sum.wrapping_add(b as usize))
        % palette.len();
    let accent = palette[index];
    let painter = ui.painter();
    painter.rect_filled(rect, 12, Color32::from_rgb(27, 49, 72));
    let body = egui::Rect::from_center_size(rect.center(), egui::vec2(size * 0.70, size * 0.40));
    painter.rect_filled(body, 8, accent);
    let dark = Color32::from_rgb(15, 34, 53);
    let left = body.center() - egui::vec2(size * 0.18, 0.0);
    painter.line_segment(
        [
            left - egui::vec2(size * 0.07, 0.0),
            left + egui::vec2(size * 0.07, 0.0),
        ],
        Stroke::new(3.0_f32, dark),
    );
    painter.line_segment(
        [
            left - egui::vec2(0.0, size * 0.07),
            left + egui::vec2(0.0, size * 0.07),
        ],
        Stroke::new(3.0_f32, dark),
    );
    for offset in [
        egui::vec2(size * 0.14, size * 0.04),
        egui::vec2(size * 0.23, -size * 0.04),
    ] {
        painter.circle_filled(body.center() + offset, size * 0.035, dark);
    }
    if response.hovered() {
        painter.rect_stroke(
            rect,
            12,
            Stroke::new(2.0_f32, accent),
            egui::StrokeKind::Inside,
        );
    }
    response
}

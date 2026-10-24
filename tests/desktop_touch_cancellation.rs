#![cfg(feature = "gui")]
use eframe::egui::{self, Event, PointerButton, Pos2};

fn press(button: PointerButton, down: bool) -> Event {
    Event::PointerButton {
        pos: Pos2::new(20.0, 20.0),
        button,
        pressed: down,
        modifiers: Default::default(),
    }
}
fn frame(ctx: &egui::Context, time: f64, events: Vec<Event>) -> (bool, bool, bool, bool) {
    let mut observed = (false, false, false, false);
    let _ = ctx.run(
        egui::RawInput {
            time: Some(time),
            focused: true,
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(400.0, 300.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            observed = ctx.input(|i| {
                (
                    i.pointer.primary_down(),
                    i.pointer.button_released(PointerButton::Primary),
                    i.pointer.button_clicked(PointerButton::Primary),
                    i.pointer.button_down(PointerButton::Secondary),
                )
            });
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    assert!(
                        !ui.add_sized([100.0, 60.0], egui::Button::new("Touch action"))
                            .clicked(),
                        "Cancellation clicked a widget"
                    );
                });
        },
    );
    observed
}
#[test]
fn cancelled_touch_releases_without_click_in_same_or_later_frame() {
    for same_frame in [false, true] {
        let ctx = egui::Context::default();
        frame(&ctx, -0.01, vec![]);
        let mut events = vec![
            Event::PointerMoved(Pos2::new(20.0, 20.0)),
            press(PointerButton::Primary, true),
        ];
        if !same_frame {
            assert!(frame(&ctx, 0.0, events).0);
            events = vec![];
        }
        events.extend([
            Event::PointerButtonCancelled {
                button: PointerButton::Primary,
            },
            Event::PointerGone,
        ]);
        assert_eq!(frame(&ctx, 0.01, events), (false, true, false, false));
        assert_eq!(frame(&ctx, 0.02, vec![]), (false, false, false, false));
        assert_eq!(
            frame(
                &ctx,
                0.03,
                vec![Event::PointerButtonCancelled {
                    button: PointerButton::Primary
                }]
            ),
            (false, false, false, false)
        );
        let mut click = false;
        let mut widget_click = false;
        let _ = ctx.run(
            egui::RawInput {
                time: Some(0.04),
                events: vec![
                    press(PointerButton::Primary, true),
                    press(PointerButton::Primary, false),
                ],
                focused: true,
                ..Default::default()
            },
            |ctx| {
                click = ctx.input(|i| i.pointer.button_clicked(PointerButton::Primary));
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| {
                        widget_click = ui
                            .add_sized([100.0, 60.0], egui::Button::new("Touch action"))
                            .clicked();
                    });
            },
        );
        assert!(click, "A fresh tap failed after cancellation");
        assert!(widget_click, "A fresh widget tap failed after cancellation");
    }
}
#[test]
fn pointer_disappearance_alone_is_not_a_touch_cancellation() {
    let ctx = egui::Context::default();
    assert!(frame(&ctx, 0.0, vec![press(PointerButton::Primary, true)]).0);
    assert!(
        frame(&ctx, 0.01, vec![Event::PointerGone]).0,
        "The original red case must retain its pressed button"
    );
    assert_eq!(
        frame(
            &ctx,
            0.02,
            vec![Event::PointerButtonCancelled {
                button: PointerButton::Primary
            }]
        ),
        (false, true, false, false)
    );
}
#[test]
fn primary_touch_cancel_preserves_other_held_mouse_buttons() {
    let ctx = egui::Context::default();
    frame(
        &ctx,
        0.0,
        vec![
            press(PointerButton::Secondary, true),
            press(PointerButton::Primary, true),
        ],
    );
    assert_eq!(
        frame(
            &ctx,
            0.01,
            vec![Event::PointerButtonCancelled {
                button: PointerButton::Primary
            }]
        ),
        (false, true, false, true)
    );
}

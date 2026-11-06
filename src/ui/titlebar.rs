use super::{
    ACCENT, BG, LINE, MUTED, PANEL,
    window_platform::{self, Control},
};
use gpui::{
    AnyElement, App, Context, PathBuilder, Render, SharedString, Window, canvas, div, point,
    prelude::*, px, rgb,
};
use photo_frame::tr;

pub(super) fn render(window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    if window_platform::titlebar_height(window) == px(0.) {
        return None;
    }
    let controls = window.window_controls();
    let drag_area = window_platform::bind_drag_area(
        div()
            .id("window-drag-area")
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .items_center()
            .px_4()
            .text_size(px(12.))
            .text_color(rgb(ACCENT))
            .overflow_hidden()
            .child(tr!("app.name")),
        window,
        cx,
    );
    Some(
        div()
            .id("window-titlebar")
            .relative()
            .flex()
            .h(window_platform::titlebar_height(window))
            .flex_shrink_0()
            .bg(rgb(PANEL))
            // Paint the separator behind the buttons so hover fills their entire height.
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .h(px(1.))
                    .bg(rgb(LINE)),
            )
            .child(drag_area)
            .when(controls.minimize, |bar| {
                bar.child(button(Control::Minimize, window))
            })
            .when(controls.maximize, |bar| {
                bar.child(button(Control::Maximize, window))
            })
            .child(button(Control::Close, window))
            .into_any_element(),
    )
}

fn button(control: Control, window: &Window) -> impl IntoElement {
    let maximized = window.is_maximized();
    let (id, label) = match control {
        Control::Minimize => ("window-minimize", tr!("window.minimize")),
        Control::Maximize if maximized => ("window-maximize", tr!("window.restore")),
        Control::Maximize => ("window-maximize", tr!("window.maximize")),
        Control::Close => ("window-close", tr!("window.close")),
    };
    div()
        .id(id)
        .relative()
        .w(px(46.))
        .h_full()
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .hover(move |style| {
            style.bg(rgb(match control {
                Control::Close => 0xb83a3a,
                _ => 0x34483b,
            }))
        })
        .map(|button| window_platform::bind_button(button, control, window))
        .tooltip(move |_, cx| cx.new(|_| ControlTooltip(label.clone().into())).into())
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    // Draw controls as vectors so they do not depend on system icon fonts.
                    let lines: &[&[(f32, f32)]] = match control {
                        Control::Minimize => &[&[(1., 9.), (11., 9.)]],
                        Control::Maximize if maximized => &[
                            &[(4., 3.), (4., 1.), (11., 1.), (11., 8.), (9., 8.)],
                            &[(1., 4.), (8., 4.), (8., 11.), (1., 11.), (1., 4.)],
                        ],
                        Control::Maximize => {
                            &[&[(1., 1.), (11., 1.), (11., 11.), (1., 11.), (1., 1.)]]
                        }
                        Control::Close => &[&[(1., 1.), (11., 11.)], &[(11., 1.), (1., 11.)]],
                    };
                    let mut path = PathBuilder::stroke(px(1.));
                    for line in lines {
                        for (index, &(x, y)) in line.iter().enumerate() {
                            let point = bounds.origin + point(px(x), px(y));
                            if index == 0 {
                                path.move_to(point);
                            } else {
                                path.line_to(point);
                            }
                        }
                    }
                    if let Ok(path) = path.build() {
                        window.paint_path(path, rgb(0xe7ede5));
                    }
                },
            )
            .size(px(12.)),
        )
}

struct ControlTooltip(SharedString);
impl Render for ControlTooltip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgb(BG))
            .border_1()
            .border_color(rgb(LINE))
            .text_size(px(12.))
            .text_color(rgb(MUTED))
            .child(self.0.clone())
    }
}

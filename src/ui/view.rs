use crate::ui::components::{ACCENT, BG, LINE, MUTED, PANEL, col, label, row};
use crate::ui::window;
use crate::ui::{Command, Studio};
use gpui::{prelude::*, *};
use photo_frame::tr;

impl Render for Studio {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let titlebar = window::render_titlebar(window, cx);
        let resize_handles = window::resize_handles(window);
        let filmstrip = self.filmstrip(window, cx);
        let status = match &self.preference_error {
            Some(error) => format!("{} · {error}", self.status),
            None => self.status.clone(),
        };
        let empty = self.photos.is_empty();
        let preview_panel = self.preview_panel(window, cx);
        col()
            .id("studio")
            .relative()
            .size_full()
            .bg(rgb(BG))
            .text_color(rgb(0xe7ede5))
            .text_sm()
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if let Some(image) = this.preview.display.image()
                    && this.preview.viewport.move_pointer(
                        event.position,
                        event.dragging(),
                        image.size(0),
                    )
                {
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.preview.viewport.release();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.preview.viewport.release();
                }),
            )
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                this.add(paths.paths().to_vec(), cx)
            }))
            .when_some(titlebar, |view, titlebar| view.child(titlebar))
            .child(
                row()
                    .items_start()
                    .flex_1()
                    .min_h_0()
                    .child(
                        col()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .child(preview_panel)
                            .child(
                                col()
                                    .flex_shrink_0()
                                    .bg(rgb(PANEL))
                                    .border_t_1()
                                    .border_color(rgb(LINE))
                                    .px_4()
                                    .py_3()
                                    .gap_2()
                                    .child(
                                        row()
                                            .flex_shrink_0()
                                            .justify_between()
                                            .child(label(&tr!(
                                                "photos.count",
                                                count = self.photos.len()
                                            )))
                                            .when(!empty, |d| {
                                                d.child(
                                                    row()
                                                        .gap_2()
                                                        .child(self.button(
                                                            "remove",
                                                            tr!("photos.remove"),
                                                            false,
                                                            Command::Remove,
                                                            cx,
                                                        ))
                                                        .child(self.button(
                                                            "clear",
                                                            tr!("photos.clear"),
                                                            false,
                                                            Command::Clear,
                                                            cx,
                                                        )),
                                                )
                                            }),
                                    )
                                    .child(filmstrip),
                            ),
                    )
                    .child(
                        col()
                            .w(px(360.))
                            .h_full()
                            .flex_shrink_0()
                            .bg(rgb(PANEL))
                            .border_l_1()
                            .border_color(rgb(LINE))
                            .child(
                                col()
                                    .flex_shrink_0()
                                    .px_4()
                                    .py_3()
                                    .gap_2()
                                    .border_b_1()
                                    .border_color(rgb(LINE))
                                    .child(
                                        row()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .text_base()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .child(tr!("studio.title")),
                                            )
                                            .child(
                                                div()
                                                    .px_2()
                                                    .py_1()
                                                    .rounded_full()
                                                    .bg(rgb(0x2a3b2f))
                                                    .text_size(px(10.))
                                                    .text_color(rgb(ACCENT))
                                                    .child(tr!("studio.live")),
                                            ),
                                    )
                                    .child(
                                        row()
                                            .justify_between()
                                            .child(label(tr!("studio.subtitle")))
                                            .child(
                                                row()
                                                    .gap_3()
                                                    .child(
                                                        div()
                                                            .id("reset")
                                                            .text_size(px(11.))
                                                            .text_color(rgb(MUTED))
                                                            .cursor_pointer()
                                                            .hover(|s| s.text_color(rgb(0xffffff)))
                                                            .on_click(cx.listener(
                                                                |this, _, _, cx| {
                                                                    this.command(Command::Reset, cx)
                                                                },
                                                            ))
                                                            .child(tr!("common.reset")),
                                                    )
                                                    .child(
                                                        div()
                                                            .id("open-settings")
                                                            .text_size(px(11.))
                                                            .text_color(rgb(ACCENT))
                                                            .cursor_pointer()
                                                            .hover(|s| s.text_color(rgb(0xffffff)))
                                                            .on_click(cx.listener(
                                                                |this, _, window, cx| {
                                                                    this.open_settings(window, cx)
                                                                },
                                                            ))
                                                            .child(tr!("settings.title")),
                                                    ),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .id("settings")
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_y_scroll()
                                    .p_3()
                                    .child(self.layout_panel(cx)),
                            )
                            .child(
                                div()
                                    .id("export-area")
                                    .flex_shrink_0()
                                    .p_4()
                                    .border_t_1()
                                    .border_color(rgb(LINE))
                                    .bg(rgb(0x16231b))
                                    .child(self.export_panel(cx)),
                            ),
                    ),
            )
            .child(
                div()
                    .h(px(28.))
                    .flex_shrink_0()
                    .px_4()
                    .flex()
                    .items_center()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .overflow_hidden()
                    .child(status),
            )
            .when_some(self.export_dialog.clone(), |d, dialog| d.child(dialog))
            .when_some(self.settings_dialog.clone(), |d, dialog| d.child(dialog))
            .children(resize_handles)
    }
}

use super::*;

impl Render for Studio {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let filmstrip = self.filmstrip(window, cx);
        let now = std::time::Instant::now();
        self.preview.advance(now);
        for image in self.preview.take_retired() {
            let _ = window.drop_image(image);
        }
        if self.preview.is_animating() {
            window.request_animation_frame();
        }
        let status = match &self.preference_error {
            Some(error) => format!("{} · {error}", self.status),
            None => self.status.clone(),
        };
        let empty = self.photos.is_empty();
        let previous_target = self.preview_target;
        let preview_owner = cx.entity().downgrade();
        let preview_observer = canvas(
            move |bounds, window, _| {
                let target = preview_pixels(
                    f32::from(bounds.size.width),
                    f32::from(bounds.size.height),
                    window.scale_factor(),
                );
                if target.0 > 0 && target.1 > 0 && target != previous_target {
                    window.on_next_frame(move |_, cx| {
                        let _ = preview_owner.update(cx, |this, cx| {
                            if this.preview_target != target {
                                this.preview_target = target;
                                this.refresh(PreviewTrigger::Resize, cx);
                            }
                        });
                    });
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0();
        let title = self
            .photos
            .get(self.selected)
            .map(|p| {
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            })
            .unwrap_or_default();
        let preview = if let Some(image) = self.preview.image().cloned() {
            let opacity = self.preview.opacity(now);
            div()
                .relative()
                .size_full()
                .min_w_0()
                .min_h_0()
                .overflow_hidden()
                .when_some(self.preview.previous().cloned(), |layer, previous| {
                    layer.child(preview_layer(previous, 1. - opacity))
                })
                .child(preview_layer(image, opacity))
                .into_any_element()
        } else {
            col()
                .items_center()
                .gap_4()
                .child(
                    div()
                        .text_size(px(42.))
                        .text_color(rgb(ACCENT))
                        .child(if empty { "+" } else { "…" }),
                )
                .child(div().text_xl().child(if empty {
                    tr!("preview.import")
                } else if self.rendering {
                    tr!("preview.loading")
                } else {
                    tr!("preview.failed")
                }))
                .child(label(if empty {
                    tr!("preview.drop_hint")
                } else {
                    tr!("preview.switch_hint")
                }))
                .when(empty, |d| d.child(label("JPEG / PNG / WebP / TIFF / BMP")))
                .into_any_element()
        };
        col()
            .id("studio")
            .relative()
            .size_full()
            .bg(rgb(BG))
            .text_color(rgb(0xe7ede5))
            .text_sm()
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                this.add(paths.paths().to_vec(), cx)
            }))
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
                            .child(
                                col()
                                    .flex_1()
                                    .min_h_0()
                                    .p_5()
                                    .gap_3()
                                    .child(
                                        row()
                                            .h(px(22.))
                                            .justify_between()
                                            .child(div().overflow_hidden().child(title))
                                            .child(label(if self.rendering {
                                                tr!("preview.updating")
                                            } else if empty {
                                                String::new()
                                            } else {
                                                tr!("preview.title")
                                            })),
                                    )
                                    .child(
                                        div()
                                            .id("preview-canvas")
                                            .relative()
                                            .flex_1()
                                            .min_h_0()
                                            .w_full()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .rounded_lg()
                                            .overflow_hidden()
                                            .bg(rgb(0x0b1410))
                                            .border_1()
                                            .border_color(rgb(LINE))
                                            .when(empty, |d| {
                                                d.cursor_pointer().hover(|s| {
                                                    s.bg(rgb(0x17291e)).border_color(rgb(ACCENT))
                                                })
                                            })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if this.photos.is_empty() {
                                                    this.command(Command::Import, cx);
                                                }
                                            }))
                                            .child(
                                                div()
                                                    .absolute()
                                                    .inset_0()
                                                    .p_4()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        div()
                                                            .relative()
                                                            .size_full()
                                                            .min_w_0()
                                                            .min_h_0()
                                                            .overflow_hidden()
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .child(preview_observer)
                                                            .child(preview),
                                                    ),
                                            ),
                                    ),
                            )
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
    }
}

fn preview_layer(image: Arc<RenderImage>, opacity: f32) -> Div {
    let image_size = image.size(0);
    let image = canvas(
        move |bounds, _, _| preview_image_bounds(bounds, image_size),
        move |bounds, fitted, window, _| {
            if let Some(fitted) = fitted {
                window.with_content_mask(Some(ContentMask { bounds }), |window| {
                    let _ = window.paint_image(fitted, Corners::default(), image, 0, false);
                });
            }
        },
    )
    .size_full();
    // A canvas has no intrinsic photo size/aspect ratio to expand the layout during a fade.
    div()
        .absolute()
        .inset_0()
        .size_full()
        .min_w_0()
        .min_h_0()
        .overflow_hidden()
        .opacity(opacity)
        .child(image)
}

fn preview_image_bounds(
    viewport: Bounds<Pixels>,
    image_size: Size<DevicePixels>,
) -> Option<Bounds<Pixels>> {
    if viewport.size.width <= px(0.) || viewport.size.height <= px(0.) {
        return None;
    }
    Some(ObjectFit::Contain.get_bounds(viewport, image_size))
}

// A multiple of the decoded-photo cache's 512 px buckets keeps requests within this cap.
const PREVIEW_MAX_EDGE: u32 = 1536;

fn preview_pixels(width: f32, height: f32, scale: f32) -> (u32, u32) {
    let width = (f64::from(width) * f64::from(scale)).ceil().max(0.);
    let height = (f64::from(height) * f64::from(scale)).ceil().max(0.);
    let limit = f64::from(PREVIEW_MAX_EDGE);
    let fit = (limit / width.max(height)).min(1.);
    (
        (width * fit).ceil().min(limit) as u32,
        (height * fit).ceil().min(limit) as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::{preview_image_bounds, preview_pixels};
    use gpui::{Bounds, point, px, size};

    #[test]
    fn both_fade_layers_fit_the_viewport_for_landscape_portrait_and_extreme_ratios() {
        for (width, height) in [(800., 500.), (400., 700.), (500., 500.)] {
            let viewport = Bounds::new(point(px(20.), px(40.)), size(px(width), px(height)));
            for (w, h) in [
                (1536u32, 864u32),
                (864, 1536),
                (1536, 1536),
                (8000, 1),
                (1, 8000),
            ] {
                let fitted = preview_image_bounds(viewport, size(w.into(), h.into())).unwrap();
                let epsilon = px(0.001);
                assert!(fitted.left() >= viewport.left() - epsilon);
                assert!(fitted.top() >= viewport.top() - epsilon);
                assert!(fitted.right() <= viewport.right() + epsilon);
                assert!(fitted.bottom() <= viewport.bottom() + epsilon);
                assert!((fitted.center().x - viewport.center().x).abs() < epsilon);
                assert!((fitted.center().y - viewport.center().y).abs() < epsilon);
            }
        }
        let viewport = Bounds::new(point(px(20.), px(40.)), size(px(800.), px(500.)));
        let landscape =
            preview_image_bounds(viewport, size(1536u32.into(), 864u32.into())).unwrap();
        let epsilon = px(0.001);
        assert!((landscape.left() - px(20.)).abs() < epsilon);
        assert!((landscape.top() - px(65.)).abs() < epsilon);
        assert!((landscape.size.width - px(800.)).abs() < epsilon);
        assert!((landscape.size.height - px(450.)).abs() < epsilon);
        let portrait = preview_image_bounds(viewport, size(864u32.into(), 1536u32.into())).unwrap();
        assert!((portrait.size.width - px(281.25)).abs() < epsilon);
        assert!((portrait.size.height - px(500.)).abs() < epsilon);
        assert!(
            preview_image_bounds(
                Bounds::new(point(px(0.), px(0.)), size(px(0.), px(500.))),
                size(100u32.into(), 100u32.into())
            )
            .is_none()
        );
    }

    #[test]
    fn preview_tracks_display_pixels_with_a_proportional_resolution_cap() {
        assert_eq!(preview_pixels(960., 540., 1.), (960, 540));
        assert_eq!(preview_pixels(960., 540., 2.), (1536, 864));
        assert_eq!(preview_pixels(1600., 900., 1.5), (1536, 864));
        assert_eq!(preview_pixels(900., 1600., 1.5), (864, 1536));
        assert_eq!(preview_pixels(768., 768., 2.), (1536, 1536));
        assert_eq!(preview_pixels(333., 777., 1.5), (500, 1166));
        assert_eq!(preview_pixels(0., 0., 2.), (0, 0));
    }
}

use super::*;
use std::ops::Range;

const ITEM_WIDTH: f32 = 126.;
const ITEM_STRIDE: f32 = ITEM_WIDTH + 12.;
const OVERSCAN: usize = 2;

struct FilmstripWindow {
    visible: Range<usize>,
    rendered: Range<usize>,
    offset: f32,
}

impl FilmstripWindow {
    /// Slots include the trailing import button. Clamp offsets after removals/resizes.
    fn new(slots: usize, offset: f32, width: f32) -> Self {
        let width = width.max(0.);
        let offset = offset
            .max(0.)
            .min((slots as f32 * ITEM_STRIDE - width).max(0.));
        let start = ((offset / ITEM_STRIDE).floor() as usize).min(slots);
        let end = (((offset + width) / ITEM_STRIDE).ceil() as usize).min(slots);
        Self {
            visible: start..end,
            rendered: start.saturating_sub(OVERSCAN)..end.saturating_add(OVERSCAN).min(slots),
            offset,
        }
    }

    fn wanted(&self, photos: &[PathBuf]) -> Vec<PathBuf> {
        // Visible photos first, then the immediate neighbors (never the import slot).
        self.visible
            .clone()
            .chain((self.rendered.start..self.visible.start).rev())
            .chain(self.visible.end..self.rendered.end)
            .filter_map(|index| photos.get(index).cloned())
            .collect()
    }
}

impl Studio {
    pub(super) fn filmstrip(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // Removing CPU buffers alone does not free GPUI's uploaded image tiles.
        for image in self.thumbnails.take_retired() {
            let _ = window.drop_image(image);
        }
        let slots = self.photos.len() + 1;
        let offset = self.filmstrip_scroll.offset();
        let viewport = FilmstripWindow::new(slots, -f32::from(offset.x), self.filmstrip_width);
        if f32::from(offset.x) != -viewport.offset {
            self.filmstrip_scroll
                .set_offset(point(px(-viewport.offset), px(0.)));
        }
        self.thumbnails.set_wanted(viewport.wanted(&self.photos));
        self.load_thumbnails(cx);
        let previous_width = self.filmstrip_width;
        let owner = cx.entity().downgrade();
        let observer = canvas(
            move |bounds, window, _| {
                let width = f32::from(bounds.size.width);
                if width != previous_width {
                    window.on_next_frame(move |_, cx| {
                        let _ = owner.update(cx, |this, cx| {
                            if this.filmstrip_width != width {
                                this.filmstrip_width = width;
                                cx.notify();
                            }
                        });
                    });
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0();

        let leading = viewport.rendered.start as f32 * ITEM_STRIDE;
        let trailing = (slots - viewport.rendered.end) as f32 * ITEM_STRIDE;
        let items: Vec<_> = viewport
            .rendered
            .map(|index| {
                let card = if let Some(path) = self.photos.get(index) {
                    col()
                        .id(("photo", index))
                        .w(px(ITEM_WIDTH))
                        .h(px(88.))
                        .p_1()
                        .gap_1()
                        .rounded_md()
                        .cursor_pointer()
                        .border_2()
                        .border_color(rgb(if self.selected == index { ACCENT } else { LINE }))
                        .bg(rgb(if self.selected == index {
                            0x304333
                        } else {
                            0x202e26
                        }))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.command(Command::Select(index), cx)
                        }))
                        .child(
                            div()
                                .h(px(56.))
                                .w_full()
                                .overflow_hidden()
                                .flex()
                                .justify_center()
                                .items_center()
                                .child(if let Some(image) = self.thumbnails.image(path) {
                                    img(image.clone())
                                        .w_full()
                                        .h_full()
                                        .object_fit(ObjectFit::Contain)
                                        .into_any_element()
                                } else {
                                    label(format!("{:02}", index + 1)).into_any_element()
                                }),
                        )
                        .child(
                            div().text_xs().overflow_hidden().child(
                                path.file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .into_owned(),
                            ),
                        )
                        .into_any_element()
                } else {
                    col()
                        .id("filmstrip-import")
                        .w(px(ITEM_WIDTH))
                        .h(px(88.))
                        .items_center()
                        .justify_center()
                        .gap_1()
                        .border_1()
                        .border_color(rgb(LINE))
                        .rounded_md()
                        .cursor_pointer()
                        .hover(|s| s.border_color(rgb(ACCENT)).bg(rgb(0x304333)))
                        .on_click(cx.listener(|this, _, _, cx| this.command(Command::Import, cx)))
                        .child(div().text_size(px(26.)).text_color(rgb(ACCENT)).child("+"))
                        .child(label(tr!("photos.add")))
                        .into_any_element()
                };
                div()
                    .w(px(ITEM_STRIDE))
                    .h(px(88.))
                    .flex_shrink_0()
                    .child(card)
            })
            .collect();

        div()
            .id("filmstrip-wheel")
            .relative()
            .h(px(96.))
            .flex_shrink_0()
            .min_w_0()
            .child(observer)
            .child(
                row()
                    .id("filmstrip")
                    .h_full()
                    .w_full()
                    .py_1()
                    .overflow_x_scroll()
                    .track_scroll(&self.filmstrip_scroll)
                    .child(
                        row()
                            .w(px(slots as f32 * ITEM_STRIDE))
                            .flex_shrink_0()
                            .child(div().w(px(leading)).flex_shrink_0())
                            .children(items)
                            .child(div().w(px(trailing)).flex_shrink_0()),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{FilmstripWindow, ITEM_STRIDE};
    use std::path::PathBuf;

    fn paths(count: usize) -> Vec<PathBuf> {
        (0..count)
            .map(|index| PathBuf::from(format!("photo-{index}.png")))
            .collect()
    }

    #[test]
    fn large_filmstrips_render_only_the_viewport_and_two_neighbors_on_each_side() {
        let photos = paths(100_000);
        let view = FilmstripWindow::new(photos.len() + 1, 50_000. * ITEM_STRIDE, 3. * ITEM_STRIDE);
        assert_eq!(view.visible, 50_000..50_003);
        assert_eq!(view.rendered, 49_998..50_005);
        assert_eq!(
            view.wanted(&photos),
            [50_000, 50_001, 50_002, 49_999, 49_998, 50_003, 50_004]
                .map(|index| photos[index].clone())
        );
    }

    #[test]
    fn filmstrip_clamps_scroll_after_removing_photos_and_expanding_the_window() {
        let photos = paths(5);
        let end = FilmstripWindow::new(6, 100_000., ITEM_STRIDE * 2.);
        assert_eq!(end.offset, ITEM_STRIDE * 4.);
        assert_eq!(end.visible, 4..6);
        assert_eq!(
            end.wanted(&photos),
            [4, 3, 2].map(|index| photos[index].clone())
        );
        let wide = FilmstripWindow::new(6, end.offset, 10. * ITEM_STRIDE);
        assert_eq!(wide.offset, 0.);
        assert_eq!(wide.rendered, 0..6);
        let empty = FilmstripWindow::new(1, end.offset, 600.);
        assert_eq!(empty.rendered, 0..1);
        assert!(empty.wanted(&[]).is_empty());
        let partial = FilmstripWindow::new(20, ITEM_STRIDE * 4.5, ITEM_STRIDE);
        assert_eq!(partial.visible, 4..6);
    }
}

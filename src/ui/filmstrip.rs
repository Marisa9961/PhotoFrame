use super::thumbnails::{FilmstripWindow, ITEM_STRIDE, ITEM_WIDTH};
use super::*;

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

    fn load_thumbnails(&mut self, cx: &mut Context<Self>) {
        if self.thumbnail_task.is_some() || !self.thumbnails.has_pending() {
            return;
        }
        self.thumbnail_task = Some(cx.spawn(async move |this, cx| {
            loop {
                let Ok(job) = this.update(cx, |this, _| {
                    let job = this.thumbnails.next_job();
                    if job.is_none() {
                        this.thumbnail_task = None;
                    }
                    job
                }) else {
                    return;
                };
                let Some(job) = job else {
                    return;
                };
                let source = job.clone();
                let result = cx
                    .background_executor()
                    .spawn(async move { super::thumbnails::decode(&source).ok().flatten() })
                    .await;
                if this
                    .update(cx, |this, cx| {
                        this.thumbnails.finish(job, result);
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
            }
        }));
    }
}

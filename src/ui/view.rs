use super::*;

impl Render for Studio {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                                this.refresh(cx);
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
        let preview = if let Some(image) = self.preview.clone() {
            img(image)
                .size_full()
                .object_fit(ObjectFit::Contain)
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
        col().id("studio").relative().size_full().bg(rgb(BG)).text_color(rgb(0xe7ede5)).font_family("Microsoft YaHei").text_sm()
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx|this.add(paths.paths().to_vec(),cx)))
            .child(row().items_start().flex_1().min_h_0()
                .child(col().flex_1().min_w_0().h_full()
                    .child(col().flex_1().min_h_0().p_5().gap_3()
                        .child(row().h(px(22.)).justify_between()
                            .child(div().overflow_hidden().child(title))
                            .child(label(if self.rendering {tr!("preview.updating")} else if empty {String::new()} else {tr!("preview.title")})))
                        .child(div().id("preview-canvas").relative().flex_1().min_h_0().w_full().flex().items_center().justify_center()
                            .rounded_lg().bg(rgb(0x0b1410)).border_1().border_color(rgb(LINE))
                            .when(empty, |d|d.cursor_pointer().hover(|s|s.bg(rgb(0x17291e)).border_color(rgb(ACCENT))))
                            .on_click(cx.listener(|this,_,_,cx|{if this.photos.is_empty() {this.command(Command::Import,cx);}}))
                            .child(div().absolute().inset_0().p_4().flex().items_center().justify_center()
                                .child(div().relative().size_full().flex().items_center().justify_center().child(preview_observer).child(preview)))))
                    .child(col().flex_shrink_0().bg(rgb(PANEL)).border_t_1().border_color(rgb(LINE)).px_4().py_3().gap_2()
                        .child(row().flex_shrink_0().justify_between()
                            .child(label(&tr!("photos.count", count = self.photos.len())))
                            .when(!empty, |d|d.child(row().gap_2()
                                .child(self.button("remove",tr!("photos.remove"),false,Command::Remove,cx))
                                .child(self.button("clear",tr!("photos.clear"),false,Command::Clear,cx)))))
                        .child(div().id("filmstrip-wheel").h(px(96.)).flex_shrink_0().min_w_0()
                            .child(row().id("filmstrip").h_full().w_full().py_1().gap_3().overflow_x_scroll().track_scroll(&self.filmstrip_scroll)
                                .children(self.photos.iter().enumerate().map(|(i,p)| {
                                    col().id(("photo",i)).w(px(126.)).h(px(88.)).flex_shrink_0().p_1().gap_1().rounded_md().cursor_pointer()
                                        .border_2().border_color(rgb(if self.selected==i {ACCENT} else {LINE}))
                                        .bg(rgb(if self.selected==i {0x304333} else {0x202e26}))
                                        .on_click(cx.listener(move |this,_,_,cx|this.command(Command::Select(i),cx)))
                                        .child(div().h(px(56.)).w_full().overflow_hidden().flex().justify_center().items_center()
                                            .child(if let Some(image)=self.thumbnails.get(p) {
                                                img(image.clone()).w_full().h_full().object_fit(ObjectFit::Contain).into_any_element()
                                            } else {label(format!("{:02}",i+1)).into_any_element()}))
                                        .child(div().text_xs().overflow_hidden().child(p.file_name().unwrap_or_default().to_string_lossy().into_owned()))
                                }))
                                .child(col().id("filmstrip-import").w(px(126.)).h(px(88.)).flex_shrink_0().items_center().justify_center().gap_1()
                                    .border_1().border_color(rgb(LINE)).rounded_md().cursor_pointer().hover(|s|s.border_color(rgb(ACCENT)).bg(rgb(0x304333)))
                                    .on_click(cx.listener(|this,_,_,cx|this.command(Command::Import,cx)))
                                    .child(div().text_size(px(26.)).text_color(rgb(ACCENT)).child("+"))
                                    .child(label(tr!("photos.add"))))))))
                .child(col().w(px(360.)).h_full().flex_shrink_0().bg(rgb(PANEL)).border_l_1().border_color(rgb(LINE))
                    .child(col().flex_shrink_0().px_4().py_3().gap_2().border_b_1().border_color(rgb(LINE))
                        .child(row().justify_between().child(div().text_base().font_weight(FontWeight::SEMIBOLD).child(tr!("studio.title")))
                            .child(div().px_2().py_1().rounded_full().bg(rgb(0x2a3b2f)).text_size(px(10.)).text_color(rgb(ACCENT)).child(tr!("studio.live"))))
                        .child(row().justify_between().child(label(tr!("studio.subtitle")))
                            .child(row().gap_3()
                                .child(div().id("reset").text_size(px(11.)).text_color(rgb(MUTED)).cursor_pointer()
                                    .hover(|s|s.text_color(rgb(0xffffff))).on_click(cx.listener(|this,_,_,cx|this.command(Command::Reset,cx))).child(tr!("common.reset")))
                                .child(div().id("open-settings").text_size(px(11.)).text_color(rgb(ACCENT)).cursor_pointer()
                                    .hover(|s|s.text_color(rgb(0xffffff))).on_click(cx.listener(|this,_,window,cx|this.open_settings(window,cx))).child(tr!("settings.title"))))))
                    .child(div().id("settings").flex_1().min_h_0().overflow_y_scroll().p_3().child(self.layout_panel(cx)))
                    .child(div().id("export-area").flex_shrink_0().p_4().border_t_1().border_color(rgb(LINE)).bg(rgb(0x16231b)).child(self.export_panel(cx)))))
            .child(div().h(px(28.)).flex_shrink_0().px_4().flex().items_center().border_t_1().border_color(rgb(LINE)).text_xs().text_color(rgb(MUTED)).overflow_hidden().child(status))
            .when_some(self.export_dialog.clone(), |d, dialog| d.child(dialog))
            .when_some(self.settings_dialog.clone(), |d, dialog| d.child(dialog))
    }
}

fn preview_pixels(width: f32, height: f32, scale: f32) -> (u32, u32) {
    (
        (width * scale).ceil().max(0.) as u32,
        (height * scale).ceil().max(0.) as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::preview_pixels;

    #[test]
    fn preview_tracks_window_size_and_monitor_scale() {
        assert_eq!(preview_pixels(960., 540., 1.), (960, 540));
        assert_eq!(preview_pixels(960., 540., 2.), (1920, 1080));
        assert_eq!(preview_pixels(1600., 900., 1.5), (2400, 1350));
        assert_eq!(preview_pixels(0., 0., 2.), (0, 0));
    }
}

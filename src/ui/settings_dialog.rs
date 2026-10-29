use super::*;

pub(super) struct SettingsDialog {
    parent: WeakEntity<Studio>,
    language: Language,
    pub(super) focus_handle: FocusHandle,
    previous_focus: Option<FocusHandle>,
}

impl SettingsDialog {
    pub(super) fn new(
        parent: WeakEntity<Studio>,
        language: Language,
        previous_focus: Option<FocusHandle>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            parent,
            language,
            previous_focus,
            focus_handle: cx.focus_handle(),
        }
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self.parent.update(cx, |parent, cx| {
            parent.settings_dialog = None;
            cx.notify();
        });
        if let Some(focus) = &self.previous_focus {
            window.focus(focus);
        } else {
            window.blur();
        }
    }

    fn change_language(&mut self, language: Language, window: &mut Window, cx: &mut Context<Self>) {
        if language == self.language && language != Language::System {
            return;
        }
        self.language = language;
        language.activate();
        let _ = self.parent.update(cx, |parent, cx| {
            parent.settings.language = language;
            parent.status = tr!("status.ready");
            for input in [
                &parent.title_input,
                &parent.subtitle_input,
                &parent.color_input,
                &parent.template_name,
            ] {
                input.update(cx, |_, cx| cx.notify());
            }
            let _ = parent.flush_preferences();
            cx.notify();
        });
        window.set_window_title(&tr!("app.name"));
        cx.notify();
    }
}

impl Render for SettingsDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().id("settings-overlay").absolute().inset_0().occlude().bg(rgba(0x030a07cc))
            .flex().items_center().justify_center().p_5().track_focus(&self.focus_handle)
            .on_mouse_down(MouseButton::Left, cx.listener(|this,_,window,cx| {
                window.focus(&this.focus_handle); cx.stop_propagation();
            }))
            .on_scroll_wheel(|_,_,cx| cx.stop_propagation())
            .on_drop(|_: &ExternalPaths,_,cx| cx.stop_propagation())
            .on_key_down(cx.listener(|this,event: &KeyDownEvent,window,cx| {
                if event.keystroke.key == "escape" { this.close(window,cx); }
                cx.stop_propagation();
            }))
            .child(col().w(px(560.)).max_w_full().max_h_full().min_h_0()
                .rounded_xl().overflow_hidden().shadow_2xl().border_1().border_color(rgb(0x49614c))
                .bg(rgb(PANEL)).text_color(rgb(0xe7ede5)).text_sm().font_family("Microsoft YaHei")
                .child(row().px_5().py_4().justify_between().flex_shrink_0().border_b_1().border_color(rgb(LINE))
                    .child(section(tr!("settings.title")))
                    .child(div().id("close-settings").size(px(30.)).rounded_md().cursor_pointer()
                        .flex().items_center().justify_center().text_color(rgb(MUTED)).hover(|s|s.bg(rgb(0x304333)))
                        .on_click(cx.listener(|this,_,window,cx|this.close(window,cx))).child("×")))
                .child(div().id("settings-content").min_h_0().overflow_y_scroll().p_5()
                    .child(col().gap_4()
                        .child(section(tr!("settings.language")))
                        .child(row().gap_2().children([
                            (Language::System, tr!("settings.system_language")),
                            (Language::English, "English".to_owned()),
                            (Language::SimplifiedChinese, "简体中文".to_owned()),
                        ].into_iter().enumerate().map(|(i,(language,name))| {
                            let active = self.language == language;
                            div().id(("language",i)).flex_1().px_3().py_2().rounded_md().text_center()
                                .bg(rgb(if active {0x354c37} else {0x25352d}))
                                .border_1().border_color(rgb(if active {ACCENT} else {LINE}))
                                .cursor_pointer().hover(|s|s.border_color(rgb(ACCENT)))
                                .on_click(cx.listener(move |this,_,window,cx|this.change_language(language,window,cx)))
                                .child(name)
                        })))
                        .child(label(tr!("settings.language_hint")))
                        .child(div().h(px(1.)).bg(rgb(LINE)))
                        .child(section(tr!("settings.about")))
                        .child(col().gap_1()
                            .child(div().text_size(px(24.)).font_weight(FontWeight::SEMIBOLD).child(tr!("app.name")))
                            .child(label(tr!("settings.version", version = env!("CARGO_PKG_VERSION")))))
                        .child(label(tr!("settings.description")))
                        .child(row().justify_between().child(label(tr!("settings.author"))).child("Marisa9961"))
                        .child(row().justify_between().child(label(tr!("settings.license"))).child("MIT"))
                        .child(label(tr!("settings.third_party")))
                        .child(label("This software is based in part on the work of the Independent JPEG Group."))))
                .child(row().px_5().py_4().justify_between().flex_shrink_0().border_t_1().border_color(rgb(LINE))
                    .child(label("© 2026 Marisa9961"))
                    .child(div().id("dismiss-settings").px_5().py_2().rounded_md().bg(rgb(ACCENT)).text_color(rgb(0x192a1c)).cursor_pointer()
                        .hover(|s|s.bg(rgb(0xd7e5cd))).on_click(cx.listener(|this,_,window,cx|this.close(window,cx))).child(tr!("common.close")))))
    }
}

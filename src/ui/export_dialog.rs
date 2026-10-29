use super::*;

pub(super) struct ExportDialog {
    parent: WeakEntity<Studio>,
    settings: Settings,
    count: usize,
    all: bool,
    error: String,
    pub(super) focus_handle: FocusHandle,
    previous_focus: Option<FocusHandle>,
}

#[derive(Clone)]
enum Action {
    Format(Format),
    Scope(bool),
    Edge(u32),
    Number(&'static str, i32),
    Directory,
    Save,
    Start,
    Close,
}

impl ExportDialog {
    pub(super) fn new(
        parent: WeakEntity<Studio>,
        settings: Settings,
        count: usize,
        previous_focus: Option<FocusHandle>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            parent,
            settings,
            count,
            all: count > 1,
            error: String::new(),
            focus_handle: cx.focus_handle(),
            previous_focus,
        }
    }
    fn action(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            Action::Format(format) => self.settings.format = format,
            Action::Scope(all) => self.all = all,
            Action::Edge(edge) => self.settings.max_edge = edge,
            Action::Number(key, delta) => match key {
                "quality" => {
                    self.settings.quality =
                        (i32::from(self.settings.quality) + delta).clamp(1, 100) as u8
                }
                _ => {
                    self.settings.max_edge =
                        (self.settings.max_edge as i32 + delta).clamp(0, 16000) as u32
                }
            },
            Action::Directory => {
                let response = cx.prompt_for_paths(PathPromptOptions {
                    files: false,
                    directories: true,
                    multiple: false,
                    prompt: Some(tr!("export.choose_directory").into()),
                });
                cx.spawn(async move |this, cx| match response.await {
                    Ok(Ok(Some(paths))) => {
                        let _ = this.update(cx, |this, cx| {
                            if let Some(path) = paths.into_iter().next() {
                                this.settings.output = Some(path);
                            }
                            this.error.clear();
                            cx.notify();
                        });
                    }
                    Ok(Err(e)) => {
                        let _ = this.update(cx, |this, cx| {
                            this.error = tr!("errors.directory", error = e);
                            cx.notify();
                        });
                    }
                    _ => {}
                })
                .detach();
            }
            Action::Save | Action::Start => {
                let start = matches!(action, Action::Start);
                if start && (self.count == 0 || self.settings.output.is_none()) {
                    return;
                }
                self.settings.normalize();
                let draft = self.settings.clone();
                let all = self.all;
                let result = self.parent.update(cx, |parent, cx| {
                    // Apply output fields while preserving the frame and typography settings.
                    parent.settings.format = draft.format;
                    parent.settings.quality = draft.quality;
                    parent.settings.max_edge = draft.max_edge;
                    parent.settings.output = draft.output;
                    if start {
                        parent.export(all, cx);
                    } else {
                        parent.status = tr!("export.applied");
                        cx.notify();
                    }
                });
                if result.is_ok() {
                    self.close(window, cx);
                } else {
                    self.error = tr!("errors.main_closed");
                }
            }
            Action::Close => self.close(window, cx),
        }
        cx.notify();
    }
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self.parent.update(cx, |parent, cx| {
            parent.export_dialog = None;
            cx.notify();
        });
        if let Some(focus) = &self.previous_focus {
            window.focus(focus);
        } else {
            window.blur();
        }
    }
    fn button(
        &self,
        id: impl Into<ElementId>,
        text: impl Into<SharedString>,
        active: bool,
        enabled: bool,
        action: Action,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(id)
            .flex_1()
            .min_w_0()
            .h(px(36.))
            .px_3()
            .rounded_md()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(12.))
            .bg(rgb(if !enabled {
                0x28372d
            } else if active {
                ACCENT
            } else {
                0x2a3b2f
            }))
            .text_color(rgb(if !enabled {
                0x738578
            } else if active {
                0x192a1c
            } else {
                0xe7ede5
            }))
            .when(enabled, |d| {
                d.cursor_pointer()
                    .hover(|s| s.bg(rgb(if active { 0xd7e5cd } else { 0x3e5541 })))
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                if enabled {
                    this.action(action.clone(), window, cx);
                }
            }))
            .child(text.into())
    }
    fn number(
        &self,
        key: &'static str,
        name: impl AsRef<str>,
        value: u32,
        step: i32,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        row()
            .justify_between()
            .gap_3()
            .child(name.as_ref().to_owned())
            .child(
                row()
                    .w(px(160.))
                    .gap_1()
                    .child(self.button(
                        (key, 0usize),
                        "−",
                        false,
                        true,
                        Action::Number(key, -step),
                        cx,
                    ))
                    .child(div().w(px(62.)).text_center().text_size(px(12.)).child(
                        if key == "edge" && value == 0 {
                            tr!("export.original_size")
                        } else {
                            value.to_string()
                        },
                    ))
                    .child(self.button(
                        (key, 1usize),
                        "+",
                        false,
                        true,
                        Action::Number(key, step),
                        cx,
                    )),
            )
    }
}

impl Render for ExportDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ready = self.count > 0 && self.settings.output.is_some();
        let total = if self.all {
            self.count
        } else {
            self.count.min(1)
        };
        let format = match self.settings.format {
            Format::Jpeg => "JPEG",
            Format::Png => "PNG",
            Format::Webp => "WebP",
        };
        div().id("export-modal-overlay").absolute().inset_0().occlude()
            .bg(rgba(0x030a07cc)).flex().items_center().justify_center().p_5()
            .track_focus(&self.focus_handle)
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                window.focus(&this.focus_handle); cx.stop_propagation();
            }))
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .on_drop(|_: &ExternalPaths, _, cx| cx.stop_propagation())
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" { this.close(window, cx); }
                cx.stop_propagation();
            }))
            .child(col().id("export-modal").w(px(760.)).max_w_full().h_full().max_h(px(660.))
                .min_h_0().rounded_xl().overflow_hidden().border_1().border_color(rgb(0x49614c))
                .shadow_2xl().bg(rgb(PANEL)).text_color(rgb(0xe7ede5)).font_family("Microsoft YaHei").text_sm()
                .child(row().px_5().py_4().gap_3().flex_shrink_0().border_b_1().border_color(rgb(LINE))
                    .child(div().size(px(42.)).rounded_lg().bg(rgb(0x324a35)).text_color(rgb(ACCENT))
                        .flex().items_center().justify_center().text_size(px(23.)).child("↗"))
                    .child(col().flex_1().gap_1()
                        .child(div().text_size(px(21.)).font_weight(FontWeight::SEMIBOLD).child(tr!("export.title")))
                        .child(label(tr!("export.hint"))))
                    .child(div().id("close-export-modal").size(px(32.)).rounded_md().text_color(rgb(MUTED))
                        .flex().items_center().justify_center().cursor_pointer().hover(|s|s.bg(rgb(0x304333)).text_color(rgb(0xffffff)))
                        .on_click(cx.listener(|this,_,window,cx| { this.close(window,cx); cx.stop_propagation(); }))
                        .child("×")))
                .child(div().id("export-settings-scroll").flex_1().min_h_0().overflow_y_scroll().p_5().bg(rgb(0x142018))
                    .child(col().gap_4()
                        .child(row().items_start().gap_4()
                            .child(col().flex_1().min_w_0().gap_4()
                                .child(panel_card().child(panel_heading("01", tr!("export.scope"), tr!("export.scope_hint")))
                                    .child(row().gap_2()
                                        .child(self.button("scope-current", tr!("export.current"), !self.all, true, Action::Scope(false), cx))
                                        .child(self.button("scope-all", tr!("export.all", count = self.count),self.all,true,Action::Scope(true),cx)))
                                    .child(label(tr!("export.concurrency_hint"))))
                                .child(panel_card().child(panel_heading("02", tr!("export.format"), tr!("export.format_hint")))
                                    .child(row().gap_2().children([
                                        (Format::Jpeg,"JPEG",tr!("export.jpeg_hint")), (Format::Png,"PNG",tr!("export.png_hint")), (Format::Webp,"WebP",tr!("export.webp_hint")),
                                    ].into_iter().enumerate().map(|(i,(value,name,hint))| {
                                        let active = self.settings.format == value;
                                        col().id(("export-format",i)).flex_1().min_w_0().h(px(64.)).rounded_lg().gap_1()
                                            .items_center().justify_center().border_1()
                                            .border_color(rgb(if active { ACCENT } else { LINE }))
                                            .bg(rgb(if active { 0x354c37 } else { 0x152219 }))
                                            .cursor_pointer().hover(|s|s.bg(rgb(0x304333)))
                                            .on_click(cx.listener(move |this,_,window,cx| this.action(Action::Format(value),window,cx)))
                                            .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD)
                                                .text_color(rgb(if active { ACCENT } else { 0xe7ede5 })).child(name))
                                            .child(div().text_size(px(10.)).text_color(rgb(MUTED)).child(hint))
                                    })))
                                    .when(self.settings.format == Format::Jpeg, |d| d
                                        .child(self.number("quality",tr!("export.quality"),u32::from(self.settings.quality),5,cx))
                                        .child(label(tr!("export.quality_hint"))))
                                    .when(self.settings.format != Format::Jpeg, |d|d.child(label(tr!("export.lossless_hint"))))))
                            .child(col().flex_1().min_w_0().gap_4()
                                .child(panel_card().child(panel_heading("03", tr!("export.size"), tr!("export.size_hint")))
                                    .child(row().gap_1()
                                        .child(self.button("edge-original",tr!("export.original"),self.settings.max_edge==0,true,Action::Edge(0),cx))
                                        .child(self.button("edge-2000","2000px",self.settings.max_edge==2000,true,Action::Edge(2000),cx))
                                        .child(self.button("edge-4000","4000px",self.settings.max_edge==4000,true,Action::Edge(4000),cx)))
                                    .child(self.number("edge",tr!("export.custom_edge"),self.settings.max_edge,1000,cx))
                                    .child(label(tr!("export.resize_hint"))))
                                .child(panel_card().child(panel_heading("04", tr!("export.location"), tr!("export.location_hint")))
                                    .child(div().min_h(px(46.)).p_3().rounded_md().bg(rgb(0x111d15)).text_size(px(11.)).text_color(rgb(MUTED))
                                        .child(self.settings.output.as_ref().map(|p|p.display().to_string()).unwrap_or_else(||tr!("export.no_directory"))))
                                    .child(row().child(self.button("choose-directory",if self.settings.output.is_some(){tr!("export.change_directory")}else{tr!("export.select_directory")},false,true,Action::Directory,cx))))))
                        .when(!self.error.is_empty(),|d|d.child(div().p_3().rounded_md().bg(rgb(0x392a25)).text_xs().text_color(rgb(0xf5b3a3)).child(self.error.clone())))))
                .child(col().px_5().py_4().gap_3().flex_shrink_0().border_t_1().border_color(rgb(LINE))
                    .child(row().justify_between().gap_3()
                        .child(div().text_size(px(12.)).font_weight(FontWeight::MEDIUM).child(tr!("export.summary", count = total, format = format)))
                        .child(div().text_size(px(10.)).text_color(rgb(MUTED)).child(tr!("export.keep_originals"))))
                    .child(row().gap_3().items_center()
                        .child(col().flex_1().min_w_0().child(label(if self.count==0 {tr!("export.import_first")}
                            else if self.settings.output.is_none(){tr!("export.choose_first")}else{tr!("export.ready")})))
                        .child(row().w(px(320.)).gap_2()
                            .child(self.button("cancel",tr!("common.cancel"),false,true,Action::Close,cx))
                            .child(self.button("save",tr!("export.apply"),false,true,Action::Save,cx))
                            .child(self.button("start",tr!("export.start"),true,ready,Action::Start,cx))))))
    }
}

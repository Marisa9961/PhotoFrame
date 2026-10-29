use super::*;

impl Studio {
    pub(super) fn button(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        active: bool,
        command: Command,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(id)
            .px_3()
            .py_2()
            .rounded_md()
            .text_sm()
            .cursor_pointer()
            .bg(rgb(if active { ACCENT } else { 0x25352d }))
            .text_color(rgb(if active { 0x1a2a20 } else { 0xdfe7df }))
            .hover(|s| s.bg(rgb(if active { 0xd7e5cd } else { 0x34483b })))
            .on_click(cx.listener(move |this, _, _, cx| this.command(command.clone(), cx)))
            .child(label.into())
    }
    pub(super) fn number(
        &self,
        key: &'static str,
        name: impl AsRef<str>,
        value: u32,
        step: i32,
        unit: &str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        row()
            .h(px(34.))
            .justify_between()
            .gap_2()
            .child(div().text_sm().child(name.as_ref().to_owned()))
            .child(
                row()
                    .h(px(30.))
                    .rounded_md()
                    .bg(rgb(0x111c16))
                    .border_1()
                    .border_color(rgb(LINE))
                    .child(self.step_button(key, "−", -step, cx))
                    .child(
                        div()
                            .w(px(62.))
                            .text_center()
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(format!("{value}{unit}")),
                    )
                    .child(self.step_button(key, "+", step, cx)),
            )
    }
    pub(super) fn step_button(
        &self,
        key: &'static str,
        text: &str,
        delta: i32,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .id((key, if delta < 0 { 0usize } else { 1usize }))
            .w(px(28.))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .text_color(rgb(MUTED))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(0x304333)).text_color(rgb(0xffffff)))
            .on_click(
                cx.listener(move |this, _, _, cx| this.command(Command::Number(key, delta), cx)),
            )
            .child(text.to_owned())
    }
    pub(super) fn toggle(
        &self,
        key: &'static str,
        name: impl AsRef<str>,
        on: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        row()
            .id(key)
            .h(px(34.))
            .justify_between()
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| this.command(Command::Toggle(key), cx)))
            .child(name.as_ref().to_owned())
            .child(
                row()
                    .gap_2()
                    .child(label(if on {
                        tr!("common.on")
                    } else {
                        tr!("common.off")
                    }))
                    .child(
                        div()
                            .w(px(32.))
                            .h(px(18.))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .bg(rgb(if on { ACCENT } else { 0x3b4941 }))
                            .child(
                                div()
                                    .ml(px(if on { 16. } else { 2. }))
                                    .size(px(14.))
                                    .rounded_full()
                                    .bg(rgb(if on { 0x203324 } else { 0xa0afa4 })),
                            ),
                    ),
            )
    }
    pub(super) fn style_card(
        &self,
        index: usize,
        name: impl AsRef<str>,
        description: impl AsRef<str>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let description = description.as_ref().to_owned();
        let active = StylePreset::ALL.get(index).copied() == self.settings.selected_preset;
        let modified = active && self.settings.preset_modified();
        let confirming = modified && self.pending_preset_restore == self.settings.selected_preset;
        let surround = match index {
            1 => 0x121a17,
            2 => 0x4b6550,
            _ => 0xe5e2d8,
        };
        col()
            .id(("style-card", index))
            .flex_1()
            .min_w_0()
            .p_2()
            .gap_1()
            .rounded_lg()
            .border_1()
            .border_color(rgb(if active { ACCENT } else { LINE }))
            .bg(rgb(if active { 0x2a3b2f } else { 0x1b2921 }))
            .cursor_pointer()
            .hover(|s| s.border_color(rgb(0x91aa8a)).bg(rgb(0x2a3b2f)))
            .on_click(cx.listener(move |this, _, _, cx| this.command(Command::Preset(index), cx)))
            .child(
                div()
                    .h(px(32.))
                    .w_full()
                    .rounded_md()
                    .bg(rgb(surround))
                    .flex()
                    .justify_center()
                    .items_center()
                    .child(
                        col()
                            .w(px(58.))
                            .h(px(26.))
                            .gap(px(3.))
                            .child(
                                div()
                                    .w_full()
                                    .h(px(if index == 3 { 17. } else { 21. }))
                                    .rounded(px(if index == 2 { 3. } else { 0. }))
                                    .bg(rgb(0x819782))
                                    .flex()
                                    .items_end()
                                    .justify_end()
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .w(px(37.))
                                            .h(px(18.))
                                            .rounded_tl(px(18.))
                                            .bg(rgb(0x385444)),
                                    ),
                            )
                            .child(div().w(px(23.)).h(px(2.)).mx_auto().rounded_full().bg(rgb(
                                if index == 1 || index == 2 {
                                    0xf0f2e8
                                } else {
                                    0x7a8076
                                },
                            ))),
                    ),
            )
            .child(
                row()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(name.as_ref().to_owned()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(if active { ACCENT } else { LINE }))
                            .child(if active { "●" } else { "○" }),
                    ),
            )
            .child(
                row()
                    .min_h(px(22.))
                    .gap_1()
                    .justify_between()
                    .when(!confirming, |row| {
                        row.child(
                            div()
                                .min_w_0()
                                .text_size(px(10.))
                                .text_color(rgb(MUTED))
                                .child(
                                    if modified {
                                        tr!("presets.modified")
                                    } else {
                                        description
                                    }
                                    .to_owned(),
                                ),
                        )
                    })
                    .when(modified && !confirming, |row| {
                        row.child(self.style_restore_button(
                            ("restore-style", index),
                            tr!("presets.restore"),
                            Command::RequestPresetRestore(index),
                            cx,
                        ))
                    })
                    .when(confirming, |row| {
                        row.child(self.style_restore_button(
                            ("confirm-restore-style", index),
                            tr!("presets.confirm_restore"),
                            Command::ConfirmPresetRestore(index),
                            cx,
                        ))
                        .child(self.style_restore_button(
                            ("cancel-restore-style", index),
                            tr!("common.cancel"),
                            Command::CancelPresetRestore,
                            cx,
                        ))
                    }),
            )
    }

    fn style_restore_button(
        &self,
        id: (&'static str, usize),
        label: String,
        command: Command,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(id)
            .flex_shrink_0()
            .px_1()
            .py(px(2.))
            .rounded_md()
            .border_1()
            .border_color(rgb(LINE))
            .bg(rgb(PANEL))
            .text_color(rgb(ACCENT))
            .text_size(px(10.))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(0x34483b)).border_color(rgb(ACCENT)))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.command(command.clone(), cx);
            }))
            .child(label)
    }

    pub(super) fn layout_panel(&self, cx: &Context<Self>) -> AnyElement {
        col()
            .gap_4()
            .child(
                panel_card()
                    .child(self.fold_header(
                        "styles",
                        tr!("presets.title"),
                        if self.settings.selected_preset.is_none() {
                            tr!("presets.custom_hint")
                        } else {
                            tr!("presets.hint")
                        },
                        cx,
                    ))
                    .when(self.styles_open, |d| {
                        d.child(
                            row()
                                .gap_2()
                                .child(self.style_card(
                                    2,
                                    tr!("presets.atmosphere"),
                                    tr!("presets.atmosphere_hint"),
                                    cx,
                                ))
                                .child(self.style_card(
                                    0,
                                    tr!("presets.light"),
                                    tr!("presets.light_hint"),
                                    cx,
                                )),
                        )
                        .child(
                            row()
                                .gap_2()
                                .child(self.style_card(
                                    1,
                                    tr!("presets.dark"),
                                    tr!("presets.dark_hint"),
                                    cx,
                                ))
                                .child(self.style_card(
                                    3,
                                    tr!("presets.polaroid"),
                                    tr!("presets.polaroid_hint"),
                                    cx,
                                )),
                        )
                    }),
            )
            .child(
                panel_card()
                    .child(self.fold_header(
                        "composition",
                        tr!("layout.title"),
                        tr!("layout.hint"),
                        cx,
                    ))
                    .when(self.composition_open, |d| {
                        d.child(
                            row().gap_1().children(
                                [
                                    (Ratio::Auto, tr!("layout.auto"), 17., 12.),
                                    (Ratio::Square, "1:1".to_owned(), 13., 13.),
                                    (Ratio::Landscape, "3:2".to_owned(), 18., 12.),
                                    (Ratio::Portrait, "2:3".to_owned(), 10., 15.),
                                    (Ratio::Wide, "16:9".to_owned(), 20., 11.),
                                ]
                                .into_iter()
                                .enumerate()
                                .map(|(i, (ratio, name, w, h))| {
                                    let active = self.settings.ratio == ratio;
                                    col()
                                        .id(("ratio", i))
                                        .flex_1()
                                        .min_w_0()
                                        .h(px(55.))
                                        .gap_2()
                                        .items_center()
                                        .justify_center()
                                        .rounded_md()
                                        .cursor_pointer()
                                        .bg(rgb(if active { 0x344936 } else { 0x152219 }))
                                        .border_1()
                                        .border_color(rgb(if active { 0x92ae84 } else { LINE }))
                                        .text_color(rgb(if active { ACCENT } else { MUTED }))
                                        .hover(|s| s.bg(rgb(0x304333)))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.command(Command::Ratio(ratio), cx)
                                        }))
                                        .child(
                                            div()
                                                .w(px(w))
                                                .h(px(h))
                                                .rounded(px(2.))
                                                .border_1()
                                                .border_color(rgb(if active {
                                                    ACCENT
                                                } else {
                                                    MUTED
                                                })),
                                        )
                                        .child(div().text_size(px(10.)).child(name))
                                }),
                            ),
                        )
                        .child(div().h(px(1.)).bg(rgb(LINE)))
                        .child(
                            col()
                                .gap_1()
                                .child(
                                    self.number(
                                        "horizontal_padding",
                                        tr!("layout.horizontal_padding"),
                                        self.settings
                                            .horizontal_padding
                                            .unwrap_or(self.settings.padding),
                                        1,
                                        "%",
                                        cx,
                                    ),
                                )
                                .child(self.number(
                                    "padding",
                                    tr!("layout.padding"),
                                    self.settings.padding,
                                    1,
                                    "%",
                                    cx,
                                ))
                                .child(self.number(
                                    "footer",
                                    tr!("layout.footer"),
                                    self.settings.footer,
                                    1,
                                    "%",
                                    cx,
                                )),
                        )
                        .child(label(tr!("layout.padding_hint")))
                    }),
            )
            .child(
                panel_card()
                    .child(self.fold_header(
                        "layout",
                        tr!("background.title"),
                        tr!("background.hint"),
                        cx,
                    ))
                    .when(self.layout_open, |d| {
                        d.child(
                            row()
                                .gap_2()
                                .child(self.choice(
                                    "solid",
                                    tr!("background.solid"),
                                    self.settings.background == Background::Solid,
                                    Command::Background(Background::Solid),
                                    cx,
                                ))
                                .child(self.choice(
                                    "blur",
                                    tr!("background.blur"),
                                    self.settings.background == Background::Blur,
                                    Command::Background(Background::Blur),
                                    cx,
                                )),
                        )
                        .when(self.settings.background == Background::Solid, |d| {
                            d.child(
                                row().gap_2().children(
                                    [
                                        [250, 249, 246],
                                        [231, 225, 210],
                                        [192, 207, 186],
                                        [25, 27, 29],
                                    ]
                                    .into_iter()
                                    .enumerate()
                                    .map(|(i, color)| {
                                        div()
                                            .id(("swatch", i))
                                            .size(px(26.))
                                            .rounded_full()
                                            .border_2()
                                            .border_color(rgb(if self.settings.color == color {
                                                ACCENT
                                            } else {
                                                LINE
                                            }))
                                            .bg(rgb((u32::from(color[0]) << 16)
                                                | (u32::from(color[1]) << 8)
                                                | u32::from(color[2])))
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.command(Command::Color(color), cx)
                                            }))
                                    }),
                                ),
                            )
                            .child(self.color_input.clone())
                        })
                        .when(self.settings.background == Background::Blur, |d| {
                            d.child(self.number(
                                "blur",
                                tr!("background.amount"),
                                self.settings.blur,
                                5,
                                "",
                                cx,
                            ))
                            .child(self.number(
                                "background_dim",
                                tr!("background.dim"),
                                self.settings.background_dim,
                                5,
                                "%",
                                cx,
                            ))
                        })
                        .child(self.number(
                            "radius",
                            tr!("background.radius"),
                            self.settings.radius,
                            1,
                            "%",
                            cx,
                        ))
                        .child(self.toggle(
                            "shadow",
                            tr!("background.shadow"),
                            self.settings.shadow,
                            cx,
                        ))
                    }),
            )
            .child(self.typography_panel(cx))
            .children(self.failures.iter().map(|e| {
                div()
                    .p_3()
                    .rounded_md()
                    .bg(rgb(0x392a25))
                    .text_xs()
                    .text_color(rgb(0xf5b3a3))
                    .child(e.clone())
            }))
            .into_any_element()
    }
    pub(super) fn fold_header(
        &self,
        key: &'static str,
        title: impl AsRef<str>,
        subtitle: impl AsRef<str>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let (number, open) = match key {
            "styles" => ("01", self.styles_open),
            "composition" => ("02", self.composition_open),
            "layout" => ("03", self.layout_open),
            _ => ("04", self.text_open),
        };
        row()
            .id(key)
            .justify_between()
            .gap_2()
            .rounded_md()
            .p_1()
            .cursor_pointer()
            .hover(|s| s.bg(rgb(0x26392c)))
            .on_click(cx.listener(move |this, _, _, cx| this.command(Command::Expand(key), cx)))
            .child(panel_heading(number, title, subtitle))
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(if open { ACCENT } else { MUTED }))
                    .child(if open {
                        tr!("common.collapse")
                    } else {
                        tr!("common.expand")
                    }),
            )
    }
    pub(super) fn choice(
        &self,
        id: &'static str,
        name: impl AsRef<str>,
        active: bool,
        command: Command,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(id)
            .flex_1()
            .min_w_0()
            .h(px(32.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .border_1()
            .border_color(rgb(if active { 0x92ae84 } else { LINE }))
            .bg(rgb(if active { 0x344936 } else { 0x152219 }))
            .text_size(px(12.))
            .text_color(rgb(if active { ACCENT } else { MUTED }))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(0x304333)))
            .on_click(cx.listener(move |this, _, _, cx| this.command(command.clone(), cx)))
            .child(name.as_ref().to_owned())
    }
    pub(super) fn typography_panel(&self, cx: &Context<Self>) -> impl IntoElement {
        panel_card()
            .child(self.fold_header("text", tr!("text.title"), tr!("text.hint"), cx))
            .when(self.text_open, |d| {
                d.child(
                    row()
                        .gap_1()
                        .child(
                            self.choice(
                                "font-sans",
                                tr!("text.sans"),
                                self.settings.font.is_none()
                                    && self
                                        .settings
                                        .font_family
                                        .as_deref()
                                        .is_none_or(|v| v == "sans-serif"),
                                Command::Font("sans-serif"),
                                cx,
                            ),
                        )
                        .child(self.choice(
                            "font-serif",
                            tr!("text.serif"),
                            self.settings.font.is_none()
                                && self.settings.font_family.as_deref() == Some("serif"),
                            Command::Font("serif"),
                            cx,
                        ))
                        .child(self.choice(
                            "font-mono",
                            tr!("text.monospace"),
                            self.settings.font.is_none()
                                && self.settings.font_family.as_deref() == Some("monospace"),
                            Command::Font("monospace"),
                            cx,
                        )),
                )
                .child(
                    self.button(
                        "custom-font",
                        self.settings
                            .font
                            .as_ref()
                            .map(|p| {
                                tr!(
                                    "text.font_file",
                                    name = p.file_name().unwrap_or_default().to_string_lossy()
                                )
                            })
                            .unwrap_or_else(|| tr!("text.import_font")),
                        self.settings.font.is_some(),
                        Command::Choose("font"),
                        cx,
                    ),
                )
                .child(self.number(
                    "font_size",
                    tr!("text.font_size"),
                    self.settings.font_size,
                    1,
                    "",
                    cx,
                ))
                .child(self.number(
                    "line_gap",
                    tr!("text.line_gap"),
                    self.settings.line_gap,
                    1,
                    "",
                    cx,
                ))
                .child(self.toggle("font_bold", tr!("text.bold"), self.settings.font_bold, cx))
                .child(self.toggle(
                    "font_italic",
                    tr!("text.italic"),
                    self.settings.font_italic,
                    cx,
                ))
                .child(
                    row()
                        .gap_1()
                        .child(self.choice(
                            "align-left",
                            tr!("text.left"),
                            self.settings.align == 0,
                            Command::Align(0),
                            cx,
                        ))
                        .child(self.choice(
                            "align-center",
                            tr!("text.center"),
                            self.settings.align == 1,
                            Command::Align(1),
                            cx,
                        ))
                        .child(self.choice(
                            "align-right",
                            tr!("text.right"),
                            self.settings.align == 2,
                            Command::Align(2),
                            cx,
                        )),
                )
                .child(label(tr!("text.title_label")))
                .child(self.title_input.clone())
                .child(label(tr!("text.subtitle_label")))
                .child(self.subtitle_input.clone())
                .child(label(tr!("text.variables")))
                .child(self.button(
                    "caption-default",
                    tr!("text.default_template"),
                    false,
                    Command::CaptionDefault,
                    cx,
                ))
                .child(div().h(px(1.)).bg(rgb(LINE)))
                .child(section(tr!("text.templates")))
                .child(self.template_name.clone())
                .child(self.button(
                    "save-template",
                    tr!("text.save_template"),
                    true,
                    Command::TemplateSave,
                    cx,
                ))
                .child(label(tr!("text.template_hint")))
                .children(self.settings.caption_templates.iter().enumerate().map(
                    |(index, template)| {
                        row()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .child(self.button(
                                        ("template", index),
                                        template.name.clone(),
                                        false,
                                        Command::TemplateApply(index),
                                        cx,
                                    )),
                            )
                            .child(self.button(
                                ("delete-template", index),
                                tr!("common.delete"),
                                false,
                                Command::TemplateDelete(index),
                                cx,
                            ))
                    },
                ))
            })
    }
    pub(super) fn export_panel(&self, cx: &Context<Self>) -> AnyElement {
        col()
            .gap_2()
            .when(!self.exporting, |d| {
                d.child(
                    div()
                        .id("open-export")
                        .h(px(46.))
                        .w_full()
                        .rounded_lg()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(rgb(ACCENT))
                        .text_color(rgb(0x192a1c))
                        .font_weight(FontWeight::SEMIBOLD)
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(0xd7e5cd)))
                        .on_click(cx.listener(|this, _, window, cx| this.open_export(window, cx)))
                        .child(tr!("export.open")),
                )
            })
            .when(self.exporting, |d| {
                d.child(
                    row()
                        .justify_between()
                        .child(section(tr!("export.running")))
                        .child(label(format!("{} / {}", self.progress.0, self.progress.1))),
                )
                .child(
                    div().h(px(3.)).w_full().rounded_full().bg(rgb(LINE)).child(
                        div()
                            .h_full()
                            .w(relative(
                                self.progress.0 as f32 / self.progress.1.max(1) as f32,
                            ))
                            .rounded_full()
                            .bg(rgb(ACCENT)),
                    ),
                )
                .child(self.button(
                    "cancel",
                    tr!("export.stop"),
                    false,
                    Command::Cancel,
                    cx,
                ))
            })
            .into_any_element()
    }
}

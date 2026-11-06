use crate::ui::components::{ACCENT, LINE, MUTED, col, label, panel_card, row};
use crate::ui::{Command, PanelSection, Studio, ToggleField};
use gpui::{prelude::*, *};
use photo_frame::settings::{Background, Ratio, StyleNumber};
use photo_frame::tr;
impl Studio {
    pub(in crate::ui) fn layout_panel(&self, cx: &Context<Self>) -> AnyElement {
        col()
            .gap_4()
            .child(
                panel_card()
                    .child(self.fold_header(
                        PanelSection::Styles,
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
                        PanelSection::Composition,
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
                                    let active = self.settings.style.ratio == ratio;
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
                                        StyleNumber::HorizontalPadding,
                                        tr!("layout.horizontal_padding"),
                                        self.settings
                                            .style
                                            .horizontal_padding
                                            .unwrap_or(self.settings.style.padding),
                                        1,
                                        "%",
                                        cx,
                                    ),
                                )
                                .child(self.number(
                                    StyleNumber::Padding,
                                    tr!("layout.padding"),
                                    self.settings.style.padding,
                                    1,
                                    "%",
                                    cx,
                                ))
                                .child(self.number(
                                    StyleNumber::Footer,
                                    tr!("layout.footer"),
                                    self.settings.style.footer,
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
                        PanelSection::Layout,
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
                                    self.settings.style.background == Background::Solid,
                                    Command::Background(Background::Solid),
                                    cx,
                                ))
                                .child(self.choice(
                                    "blur",
                                    tr!("background.blur"),
                                    self.settings.style.background == Background::Blur,
                                    Command::Background(Background::Blur),
                                    cx,
                                )),
                        )
                        .when(self.settings.style.background == Background::Solid, |d| {
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
                                            .border_color(rgb(
                                                if self.settings.style.color == color {
                                                    ACCENT
                                                } else {
                                                    LINE
                                                },
                                            ))
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
                        .when(self.settings.style.background == Background::Blur, |d| {
                            d.child(self.number(
                                StyleNumber::Blur,
                                tr!("background.amount"),
                                self.settings.style.blur,
                                5,
                                "",
                                cx,
                            ))
                            .child(self.number(
                                StyleNumber::BackgroundDim,
                                tr!("background.dim"),
                                self.settings.style.background_dim,
                                5,
                                "%",
                                cx,
                            ))
                        })
                        .child(self.number(
                            StyleNumber::Radius,
                            tr!("background.radius"),
                            self.settings.style.radius,
                            1,
                            "%",
                            cx,
                        ))
                        .child(self.toggle(
                            ToggleField::Shadow,
                            tr!("background.shadow"),
                            self.settings.style.shadow,
                            cx,
                        ))
                    }),
            )
            .child(self.typography_panel(cx))
            .children(self.exports.failures().iter().map(|e| {
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
}

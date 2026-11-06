use crate::ui::components::{ACCENT, LINE, MUTED, PANEL, col, row};
use crate::ui::{Command, Studio};
use gpui::{prelude::*, *};
use photo_frame::settings::StylePreset;
use photo_frame::tr;
impl Studio {
    pub(in crate::ui) fn style_card(
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
}

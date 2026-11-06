use crate::ui::components::{ACCENT, LINE, MUTED, label, panel_heading, row};
use crate::ui::{Command, PanelSection, Studio, ToggleField};
use gpui::{prelude::*, *};
use photo_frame::settings::StyleNumber;
use photo_frame::tr;
impl Studio {
    pub(in crate::ui) fn button(
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
    pub(in crate::ui) fn number(
        &self,
        key: StyleNumber,
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
    pub(in crate::ui) fn step_button(
        &self,
        key: StyleNumber,
        text: &str,
        delta: i32,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .id((key.id(), if delta < 0 { 0usize } else { 1usize }))
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
    pub(in crate::ui) fn toggle(
        &self,
        key: ToggleField,
        name: impl AsRef<str>,
        on: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        row()
            .id(key.id())
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
    pub(in crate::ui) fn fold_header(
        &self,
        key: PanelSection,
        title: impl AsRef<str>,
        subtitle: impl AsRef<str>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let (number, open) = match key {
            PanelSection::Styles => ("01", self.styles_open),
            PanelSection::Composition => ("02", self.composition_open),
            PanelSection::Layout => ("03", self.layout_open),
            PanelSection::Text => ("04", self.text_open),
        };
        row()
            .id(key.id())
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
    pub(in crate::ui) fn choice(
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
}

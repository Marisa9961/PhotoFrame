use crate::ui::components::{LINE, label, panel_card, row, section};
use crate::ui::{Command, FileChoice, PanelSection, Studio, ToggleField};
use gpui::{prelude::*, *};
use photo_frame::settings::StyleNumber;
use photo_frame::tr;
impl Studio {
    pub(in crate::ui) fn typography_panel(&self, cx: &Context<Self>) -> impl IntoElement {
        panel_card()
            .child(self.fold_header(PanelSection::Text, tr!("text.title"), tr!("text.hint"), cx))
            .when(self.text_open, |d| {
                d.child(
                    row()
                        .gap_1()
                        .child(
                            self.choice(
                                "font-sans",
                                tr!("text.sans"),
                                self.settings.style.font.is_none()
                                    && self
                                        .settings
                                        .style
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
                            self.settings.style.font.is_none()
                                && self.settings.style.font_family.as_deref() == Some("serif"),
                            Command::Font("serif"),
                            cx,
                        ))
                        .child(self.choice(
                            "font-mono",
                            tr!("text.monospace"),
                            self.settings.style.font.is_none()
                                && self.settings.style.font_family.as_deref() == Some("monospace"),
                            Command::Font("monospace"),
                            cx,
                        )),
                )
                .child(
                    self.button(
                        "custom-font",
                        self.settings
                            .style
                            .font
                            .as_ref()
                            .map(|p| {
                                tr!(
                                    "text.font_file",
                                    name = p.file_name().unwrap_or_default().to_string_lossy()
                                )
                            })
                            .unwrap_or_else(|| tr!("text.import_font")),
                        self.settings.style.font.is_some(),
                        Command::Choose(FileChoice::Font),
                        cx,
                    ),
                )
                .child(self.number(
                    StyleNumber::FontSize,
                    tr!("text.font_size"),
                    self.settings.style.font_size,
                    1,
                    "",
                    cx,
                ))
                .child(self.number(
                    StyleNumber::LineGap,
                    tr!("text.line_gap"),
                    self.settings.style.line_gap,
                    1,
                    "",
                    cx,
                ))
                .child(self.toggle(
                    ToggleField::FontBold,
                    tr!("text.bold"),
                    self.settings.style.font_bold,
                    cx,
                ))
                .child(self.toggle(
                    ToggleField::FontItalic,
                    tr!("text.italic"),
                    self.settings.style.font_italic,
                    cx,
                ))
                .child(
                    row()
                        .gap_1()
                        .child(self.choice(
                            "align-left",
                            tr!("text.left"),
                            self.settings.style.align == 0,
                            Command::Align(0),
                            cx,
                        ))
                        .child(self.choice(
                            "align-center",
                            tr!("text.center"),
                            self.settings.style.align == 1,
                            Command::Align(1),
                            cx,
                        ))
                        .child(self.choice(
                            "align-right",
                            tr!("text.right"),
                            self.settings.style.align == 2,
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
}

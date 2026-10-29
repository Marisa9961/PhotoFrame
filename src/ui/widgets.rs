use super::*;

pub(super) fn col() -> Div {
    div().flex().flex_col()
}
pub(super) fn row() -> Div {
    div().flex().flex_row().items_center()
}
pub(super) fn section(text: impl AsRef<str>) -> Div {
    div()
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(0xdce2d6))
        .child(text.as_ref().to_owned())
}
pub(super) fn panel_card() -> Div {
    col()
        .p_3()
        .gap_3()
        .rounded_lg()
        .border_1()
        .border_color(rgb(LINE))
        .bg(rgb(0x1c2a22))
}
pub(super) fn panel_heading(
    number: &str,
    title: impl AsRef<str>,
    subtitle: impl AsRef<str>,
) -> Div {
    row()
        .gap_2()
        .child(
            div()
                .text_size(px(10.))
                .text_color(rgb(0x8da185))
                .child(number.to_owned()),
        )
        .child(
            col().gap_1().child(section(title)).child(
                div()
                    .text_size(px(10.))
                    .text_color(rgb(MUTED))
                    .child(subtitle.as_ref().to_owned()),
            ),
        )
}
pub(super) fn label(text: impl AsRef<str>) -> Div {
    div()
        .text_xs()
        .text_color(rgb(MUTED))
        .child(text.as_ref().to_owned())
}
pub(super) fn hex(color: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2])
}
pub(super) fn parse_color(s: &str) -> Option<[u8; 3]> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 || !s.is_ascii() {
        return None;
    }
    Some([
        u8::from_str_radix(&s[0..2], 16).ok()?,
        u8::from_str_radix(&s[2..4], 16).ok()?,
        u8::from_str_radix(&s[4..6], 16).ok()?,
    ])
}

mod controller;
mod export_dialog;
mod export_session;
mod filmstrip;
mod input;
mod panels;
mod preview;
mod preview_coordinator;
mod preview_display;
mod settings_dialog;
mod thumbnails;
mod view;
mod widgets;

use export_dialog::ExportDialog;
use gpui::{prelude::*, *};
pub(crate) use input::init;
use input::{Changed, TextInput};
use photo_frame::settings::{
    Background, ExportNumber, Format, Ratio, Settings, StyleNumber, StylePreset,
};
use photo_frame::{i18n::Language, tr};
use preview::PreviewTrigger;
use settings_dialog::SettingsDialog;
use std::{path::PathBuf, sync::Arc};
use widgets::*;

const BG: u32 = 0x101a17;
const PANEL: u32 = 0x19241f;
const LINE: u32 = 0x2d3d34;
const MUTED: u32 = 0x98aaa0;
const ACCENT: u32 = 0xc4d7b5;

pub struct Studio {
    settings: Settings,
    persisted_settings: Settings,
    preference_error: Option<String>,
    photos: Arc<Vec<PathBuf>>,
    selected: usize,
    preview: preview_coordinator::PreviewCoordinator,
    exports: export_session::ExportSession,
    thumbnails: thumbnails::Thumbnails,
    thumbnail_task: Option<Task<()>>,
    filmstrip_scroll: ScrollHandle,
    filmstrip_width: f32,
    styles_open: bool,
    composition_open: bool,
    layout_open: bool,
    text_open: bool,
    export_dialog: Option<Entity<ExportDialog>>,
    settings_dialog: Option<Entity<SettingsDialog>>,
    pending_preset_restore: Option<StylePreset>,
    status: String,
    color_input: Entity<TextInput>,
    title_input: Entity<TextInput>,
    subtitle_input: Entity<TextInput>,
    template_name: Entity<TextInput>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone)]
enum Command {
    Import,
    Clear,
    Remove,
    Select(usize),
    Expand(PanelSection),
    Preset(usize),
    RequestPresetRestore(usize),
    ConfirmPresetRestore(usize),
    CancelPresetRestore,
    Ratio(Ratio),
    Background(Background),
    Color([u8; 3]),
    Toggle(ToggleField),
    Number(StyleNumber, i32),
    Font(&'static str),
    Align(u32),
    CaptionDefault,
    TemplateSave,
    TemplateApply(usize),
    TemplateDelete(usize),
    Choose(FileChoice),
    Reset,
    Cancel,
}

#[derive(Clone, Copy)]
enum PanelSection {
    Styles,
    Composition,
    Layout,
    Text,
}
impl PanelSection {
    fn id(self) -> &'static str {
        match self {
            Self::Styles => "styles",
            Self::Composition => "composition",
            Self::Layout => "layout",
            Self::Text => "text",
        }
    }
}

#[derive(Clone, Copy)]
enum ToggleField {
    Shadow,
    FontBold,
    FontItalic,
}
impl ToggleField {
    fn id(self) -> &'static str {
        match self {
            Self::Shadow => "shadow",
            Self::FontBold => "font_bold",
            Self::FontItalic => "font_italic",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FileChoice {
    Photos,
    Font,
    Output,
}

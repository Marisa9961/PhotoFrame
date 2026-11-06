mod components;
mod controller;
mod export;
mod gallery;
mod preview;
mod settings;
mod view;
mod window;

pub(crate) use components::init;
pub(crate) use window::window_options;

use components::TextInput;
use export::{ExportDialog, ExportSession};
use gallery::Thumbnails;
use gpui::{Entity, ScrollHandle, Subscription, Task};
use photo_frame::settings::{Background, Ratio, Settings, StyleNumber, StylePreset};
use preview::PreviewCoordinator;
use settings::SettingsDialog;
use std::{path::PathBuf, sync::Arc};

pub struct Studio {
    settings: Settings,
    persisted_settings: Settings,
    preference_error: Option<String>,
    photos: Arc<Vec<PathBuf>>,
    selected: usize,
    preview: PreviewCoordinator,
    exports: ExportSession,
    thumbnails: Thumbnails,
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

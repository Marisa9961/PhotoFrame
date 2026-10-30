mod controller;
mod export_dialog;
mod filmstrip;
mod input;
mod panels;
mod preview;
mod preview_display;
mod settings_dialog;
mod thumbnails;
mod view;
mod widgets;

use export_dialog::ExportDialog;
use gpui::{prelude::*, *};
pub(crate) use input::init;
use input::{Changed, TextInput};
use photo_frame::{i18n::Language, tr};
use photo_frame::{
    render::{Engine, PreviewCache},
    settings::{Background, Format, Ratio, Settings, StylePreset},
};
use preview::{PreviewRequest, PreviewTrigger};
use settings_dialog::SettingsDialog;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
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
    photos: Vec<PathBuf>,
    selected: usize,
    preview: preview_display::PreviewDisplay,
    preview_size: (u32, u32),
    preview_target: (u32, u32),
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
    pending_export: Option<bool>,
    pending_preset_restore: Option<StylePreset>,
    status: String,
    rendering: bool,
    exporting: bool,
    progress: (usize, usize),
    failures: Vec<String>,
    preview_state: Arc<Mutex<Option<PreviewState>>>,
    generation: Arc<AtomicU64>,
    preview_task: Option<Task<()>>,
    cancel: Arc<AtomicBool>,
    color_input: Entity<TextInput>,
    title_input: Entity<TextInput>,
    subtitle_input: Entity<TextInput>,
    template_name: Entity<TextInput>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Default)]
struct PreviewState {
    engine: Engine,
    photos: PreviewCache,
}
#[derive(Clone)]
enum Command {
    Import,
    Clear,
    Remove,
    Select(usize),
    Expand(&'static str),
    Preset(usize),
    RequestPresetRestore(usize),
    ConfirmPresetRestore(usize),
    CancelPresetRestore,
    Ratio(Ratio),
    Background(Background),
    Color([u8; 3]),
    Toggle(&'static str),
    Number(&'static str, i32),
    Font(&'static str),
    Align(u32),
    CaptionDefault,
    TemplateSave,
    TemplateApply(usize),
    TemplateDelete(usize),
    Choose(&'static str),
    Reset,
    Cancel,
}

use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaptionTemplate {
    pub name: String,
    pub title: String,
    pub subtitle: String,
    pub family: Option<String>,
    pub font: Option<PathBuf>,
    pub size: u32,
    pub gap: u32,
    pub bold: bool,
    pub italic: bool,
    pub align: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Background {
    #[default]
    Solid,
    Blur,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Ratio {
    #[default]
    Auto,
    Square,
    Landscape,
    Portrait,
    Wide,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Format {
    #[default]
    Jpeg,
    Png,
    Webp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum StylePreset {
    Light,
    Dark,
    Atmosphere,
    Polaroid,
}

impl StylePreset {
    pub const ALL: [Self; 4] = [Self::Light, Self::Dark, Self::Atmosphere, Self::Polaroid];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleNumber {
    Padding,
    HorizontalPadding,
    Footer,
    Radius,
    Blur,
    BackgroundDim,
    FontSize,
    LineGap,
    Align,
}

impl StyleNumber {
    pub fn id(self) -> &'static str {
        match self {
            Self::Padding => "padding",
            Self::HorizontalPadding => "horizontal_padding",
            Self::Footer => "footer",
            Self::Radius => "radius",
            Self::Blur => "blur",
            Self::BackgroundDim => "background_dim",
            Self::FontSize => "font_size",
            Self::LineGap => "line_gap",
            Self::Align => "align",
        }
    }

    pub fn clamp(self, value: u32) -> u32 {
        let (min, max) = match self {
            Self::Padding | Self::HorizontalPadding => (0, 30),
            Self::Footer | Self::LineGap => (0, 40),
            Self::Radius => (0, 20),
            Self::Blur => (0, 100),
            Self::BackgroundDim => (0, 90),
            Self::FontSize => (8, 80),
            Self::Align => (0, 2),
        };
        value.clamp(min, max)
    }

    pub fn adjust(self, style: &mut StyleSettings, delta: i32) {
        let value = match self {
            Self::Padding => {
                style.horizontal_padding.get_or_insert(style.padding);
                &mut style.padding
            }
            Self::HorizontalPadding => style.horizontal_padding.get_or_insert(style.padding),
            Self::Footer => &mut style.footer,
            Self::Radius => &mut style.radius,
            Self::Blur => &mut style.blur,
            Self::BackgroundDim => &mut style.background_dim,
            Self::FontSize => &mut style.font_size,
            Self::LineGap => &mut style.line_gap,
            Self::Align => &mut style.align,
        };
        *value = self.clamp(value.saturating_add_signed(delta));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportNumber {
    Quality,
    MaxEdge,
}

impl ExportNumber {
    pub fn id(self) -> &'static str {
        match self {
            Self::Quality => "quality",
            Self::MaxEdge => "edge",
        }
    }

    pub fn clamp(self, value: u32) -> u32 {
        match self {
            Self::Quality => value.clamp(1, 100),
            Self::MaxEdge => value.min(16000),
        }
    }

    pub fn adjust(self, value: u32, delta: i32) -> u32 {
        self.clamp(value.saturating_add_signed(delta))
    }
}

/// Parameters consumed by rendering and batch workers, without preference history.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderSettings {
    pub style: StyleSettings,
    pub format: Format,
    pub quality: u8,
    pub max_edge: u32,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Settings::default().render_settings()
    }
}

impl RenderSettings {
    pub fn normalize(&mut self) {
        self.style.normalize();
        self.quality = ExportNumber::Quality.clamp(u32::from(self.quality)) as u8;
        self.max_edge = ExportNumber::MaxEdge.clamp(self.max_edge);
    }
}

/// Visual state of one preset; export preferences and saved templates are independent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleSettings {
    pub background: Background,
    pub ratio: Ratio,
    pub color: [u8; 3],
    pub text_color: [u8; 3],
    pub padding: u32,
    pub horizontal_padding: Option<u32>,
    pub footer: u32,
    pub radius: u32,
    pub blur: u32,
    pub background_dim: u32,
    pub shadow: bool,
    pub font_size: u32,
    pub line_gap: u32,
    pub align: u32,
    pub uppercase: bool,
    pub title: String,
    pub subtitle: String,
    pub custom: BTreeMap<String, String>,
    pub force_custom: bool,
    pub font: Option<PathBuf>,
    pub font_family: Option<String>,
    pub font_bold: bool,
    pub font_italic: bool,
    pub logo: Option<PathBuf>,
}

impl Default for StyleSettings {
    fn default() -> Self {
        Self {
            background: Background::Blur,
            ratio: Ratio::Auto,
            color: [250, 249, 246],
            text_color: [244, 244, 240],
            padding: 4,
            horizontal_padding: Some(9),
            footer: 5,
            radius: 2,
            blur: 70,
            background_dim: 38,
            shadow: true,
            font_size: 24,
            line_gap: 0,
            align: 1,
            uppercase: false,
            title: "{camera}".into(),
            subtitle: "{focal} {aperture} {shutter} ISO{iso}".into(),
            custom: BTreeMap::new(),
            force_custom: false,
            font: None,
            font_family: None,
            font_bold: true,
            font_italic: true,
            logo: None,
        }
    }
}

impl StyleSettings {
    fn preset(preset: StylePreset) -> Self {
        let mut settings = Self::default();
        if preset != StylePreset::Atmosphere {
            settings.background = Background::Solid;
            settings.color = if preset == StylePreset::Dark {
                [25, 27, 29]
            } else {
                [250, 249, 246]
            };
            settings.text_color = if preset == StylePreset::Dark {
                [244, 244, 240]
            } else {
                [45, 45, 43]
            };
            settings.padding = if preset == StylePreset::Polaroid {
                6
            } else {
                3
            };
            settings.horizontal_padding = None;
            settings.footer = if preset == StylePreset::Polaroid {
                12
            } else {
                6
            };
            settings.radius = 0;
            settings.background_dim = 0;
            settings.shadow = false;
            settings.font_size = 20;
            settings.line_gap = 6;
            settings.font_bold = false;
            settings.font_italic = false;
        }
        settings
    }

    fn normalize(&mut self) {
        self.padding = StyleNumber::Padding.clamp(self.padding);
        self.horizontal_padding = self
            .horizontal_padding
            .map(|v| StyleNumber::HorizontalPadding.clamp(v));
        self.footer = StyleNumber::Footer.clamp(self.footer);
        self.radius = StyleNumber::Radius.clamp(self.radius);
        self.blur = StyleNumber::Blur.clamp(self.blur);
        self.background_dim = StyleNumber::BackgroundDim.clamp(self.background_dim);
        self.font_size = StyleNumber::FontSize.clamp(self.font_size);
        self.line_gap = StyleNumber::LineGap.clamp(self.line_gap);
        self.align = StyleNumber::Align.clamp(self.align);
    }
}

// Active styles historically defaulted these two missing fields independently of
// the current preset. Inactive snapshots retain StyleSettings's own defaults.
fn deserialize_active_style<'de, D>(deserializer: D) -> Result<StyleSettings, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    let legacy_margins = value.get("horizontal_padding").is_none();
    let legacy_dim = value.get("background_dim").is_none();
    let mut style: StyleSettings =
        serde_json::from_value(value).map_err(serde::de::Error::custom)?;
    if legacy_margins {
        style.horizontal_padding = None;
    }
    if legacy_dim {
        style.background_dim = 0;
    }
    Ok(style)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    #[serde(flatten, deserialize_with = "deserialize_active_style")]
    pub style: StyleSettings,
    pub language: crate::i18n::Language,
    pub selected_preset: Option<StylePreset>,
    /// Only inactive presets are stored here; style is the active edit.
    pub preset_edits: BTreeMap<StylePreset, StyleSettings>,
    pub caption_templates: Vec<CaptionTemplate>,
    pub format: Format,
    pub quality: u8,
    pub max_edge: u32,
    pub output: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            style: StyleSettings::default(),
            language: crate::i18n::Language::default(),
            selected_preset: Some(StylePreset::Atmosphere),
            preset_edits: BTreeMap::new(),
            caption_templates: vec![],
            format: Format::Jpeg,
            quality: 85,
            max_edge: 0,
            output: None,
        }
    }
}
impl Settings {
    pub fn render_settings(&self) -> RenderSettings {
        RenderSettings {
            style: self.style.clone(),
            format: self.format,
            quality: self.quality,
            max_edge: self.max_edge,
        }
    }
    fn remember_current_style(&mut self) {
        if let Some(preset) = self.selected_preset {
            if self.matches_preset(preset) {
                self.preset_edits.remove(&preset);
            } else {
                self.preset_edits.insert(preset, self.style.clone());
            }
        }
    }

    /// Select a preset's last edit, leaving the active style unchanged on re-selection.
    pub fn switch_preset(&mut self, preset: StylePreset) {
        if self.selected_preset == Some(preset) {
            return;
        }
        self.remember_current_style();
        let style = self
            .preset_edits
            .remove(&preset)
            .unwrap_or_else(|| StyleSettings::preset(preset));
        self.style = style;
        self.selected_preset = Some(preset);
    }

    /// Explicitly discard only this preset's edits after the UI confirms the action.
    pub fn restore_preset(&mut self, preset: StylePreset) {
        self.switch_preset(preset);
        self.preset_edits.remove(&preset);
        self.style = StyleSettings::preset(preset);
    }

    fn matches_preset(&self, preset: StylePreset) -> bool {
        self.style == StyleSettings::preset(preset)
    }

    pub fn preset_modified(&self) -> bool {
        self.selected_preset
            .is_some_and(|preset| !self.matches_preset(preset))
    }

    pub fn caption_template(&self, name: String) -> CaptionTemplate {
        CaptionTemplate {
            name,
            title: self.style.title.clone(),
            subtitle: self.style.subtitle.clone(),
            family: self.style.font_family.clone(),
            font: self.style.font.clone(),
            size: self.style.font_size,
            gap: self.style.line_gap,
            bold: self.style.font_bold,
            italic: self.style.font_italic,
            align: self.style.align,
        }
    }
    pub fn apply_caption_template(&mut self, template: &CaptionTemplate) {
        self.style.title = template.title.clone();
        self.style.subtitle = template.subtitle.clone();
        self.style.font_family = template.family.clone();
        self.style.font = template.font.clone();
        self.style.font_size = template.size;
        self.style.line_gap = template.gap;
        self.style.font_bold = template.bold;
        self.style.font_italic = template.italic;
        self.style.align = template.align;
        self.normalize();
    }
    pub fn normalize(&mut self) {
        self.style.normalize();
        self.quality = ExportNumber::Quality.clamp(u32::from(self.quality)) as u8;
        self.max_edge = ExportNumber::MaxEdge.clamp(self.max_edge);
    }
    pub fn config_path() -> PathBuf {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
            .unwrap_or_else(|| PathBuf::from(".local"))
            .join("PhotoFrame/settings.json")
    }
    pub fn load() -> anyhow::Result<Self> {
        Self::load_from(&Self::config_path())
    }
    pub fn load_from(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let json: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
        let missing_preset = json.get("selected_preset").is_none();
        // The removed JPEG option identified older settings before schema versioning.
        let legacy = json.get("settings_version").is_none() && json.get("compact_jpeg").is_none();
        let legacy_font = json.get("font_bold").is_none();
        let mut value: Self = serde_json::from_value(json)?;
        if legacy {
            if value.style.footer == 14 {
                value.style.footer = 6;
            }
            if value.style.padding == 5 {
                value.style.padding = 3;
            }
            if value.style.font_size == 24 {
                value.style.font_size = 20;
            }
            if value.style.line_gap == 12 {
                value.style.line_gap = 6;
            }
            if value.style.background == Background::Solid
                && value.style.color == [250, 249, 246]
                && value.style.text_color == [45, 45, 43]
                && value.style.radius == 0
                && !value.style.shadow
            {
                value.style.background = Background::Blur;
                value.style.text_color = [244, 244, 240];
                value.style.radius = 2;
                value.style.shadow = true;
            }
        }
        if legacy_font {
            value.style.font_bold = value.style.background == Background::Blur;
            value.style.font_italic = value.style.background == Background::Blur;
        }
        value.normalize();
        if missing_preset {
            // Recognize only an exact match; never guess a preset from a few colors.
            value.selected_preset = StylePreset::ALL
                .into_iter()
                .find(|preset| value.matches_preset(*preset));
        }
        // Keep active fields authoritative, including when upgrading an older config.
        if let Some(preset) = value.selected_preset {
            value.preset_edits.remove(&preset);
        }
        value.preset_edits.retain(|preset, style| {
            style.normalize();
            *style != StyleSettings::preset(*preset)
        });
        Ok(value)
    }
    pub fn save(&self) -> anyhow::Result<()> {
        self.save_to(&Self::config_path())
    }
    /// Replace the configuration only after a complete write in the same directory.
    pub fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        let mut json = serde_json::to_value(self)?;
        json["settings_version"] = 2.into();
        let bytes = serde_json::to_vec_pretty(&json)?;
        let directory = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(directory)?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let temporary = directory.join(format!(".settings-{}-{stamp}.tmp", std::process::id()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let result = (|| -> anyhow::Result<()> {
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            std::fs::rename(&temporary, path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result
    }
    pub fn extension(&self) -> &'static str {
        self.format.extension()
    }
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Format::Jpeg => "jpg",
            Format::Png => "png",
            Format::Webp => "webp",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_edits_detach_legacy_margins_and_saturate_without_overflow() {
        let mut style = StyleSettings {
            padding: 3,
            horizontal_padding: None,
            ..StyleSettings::default()
        };
        StyleNumber::Padding.adjust(&mut style, i32::MAX);
        assert_eq!((style.padding, style.horizontal_padding), (30, Some(3)));
        StyleNumber::HorizontalPadding.adjust(&mut style, i32::MIN);
        assert_eq!((style.padding, style.horizontal_padding), (30, Some(0)));
        StyleNumber::FontSize.adjust(&mut style, i32::MIN);
        assert_eq!(style.font_size, 8);
        assert_eq!(ExportNumber::Quality.adjust(1, i32::MIN), 1);
        assert_eq!(ExportNumber::Quality.adjust(100, i32::MAX), 100);
        assert_eq!(ExportNumber::MaxEdge.adjust(0, i32::MIN), 0);
        assert_eq!(ExportNumber::MaxEdge.adjust(u32::MAX, i32::MAX), 16000);
    }

    #[test]
    fn presets_replace_previous_visual_edits_and_preserve_user_preferences() {
        for preset in StylePreset::ALL {
            let mut edited = Settings {
                style: StyleSettings {
                    ratio: Ratio::Square,
                    padding: 18,
                    horizontal_padding: Some(21),
                    footer: 35,
                    background: Background::Solid,
                    color: [1, 2, 3],
                    text_color: [4, 5, 6],
                    radius: 15,
                    blur: 20,
                    background_dim: 70,
                    shadow: false,
                    font_size: 45,
                    line_gap: 20,
                    align: 2,
                    uppercase: true,
                    title: "旧标题".into(),
                    subtitle: "旧副标题".into(),
                    custom: BTreeMap::from([("camera".into(), "自定义相机".into())]),
                    force_custom: true,
                    font: Some(PathBuf::from("old.ttf")),
                    font_family: Some("旧字体".into()),
                    font_bold: false,
                    font_italic: false,
                    logo: Some(PathBuf::from("old-logo.png")),
                },
                format: Format::Webp,
                quality: 97,
                max_edge: 3000,
                output: Some(PathBuf::from("exports")),
                ..Settings::default()
            };
            edited
                .caption_templates
                .push(edited.caption_template("保留的模板".into()));
            let preferences = edited.clone();
            edited.restore_preset(preset);
            let mut clean = Settings::default();
            clean.restore_preset(preset);
            assert_eq!(edited.style, clean.style);
            assert_eq!(edited.selected_preset, Some(preset));
            assert!(!edited.preset_modified());
            assert_eq!(edited.style.ratio, Ratio::Auto);
            assert_eq!(edited.style.title, "{camera}");
            assert!(edited.style.font.is_none() && edited.style.logo.is_none());
            assert!(edited.style.custom.is_empty() && !edited.style.force_custom);
            assert_eq!(edited.caption_templates, preferences.caption_templates);
            assert_eq!(
                (edited.format, edited.quality, edited.max_edge),
                (Format::Webp, 97, 3000)
            );
            assert_eq!(edited.output, preferences.output);
        }
    }

    #[test]
    fn preset_modification_tracks_visual_edits_and_ignores_global_preferences() {
        let mut settings = Settings::default();
        settings.style.padding += 1;
        assert!(settings.preset_modified());
        settings.style.padding -= 1;
        assert!(!settings.preset_modified());
        settings.quality = 99;
        settings.format = Format::Png;
        settings
            .caption_templates
            .push(settings.caption_template("模板".into()));
        assert!(!settings.preset_modified());
    }

    #[test]
    fn caption_templates_round_trip_and_preserve_frame_and_export_settings() {
        let source = Settings {
            style: StyleSettings {
                title: "旅行 · {camera}".into(),
                subtitle: "{date} / {focal}".into(),
                font_family: Some("serif".into()),
                font: Some(PathBuf::from("my-font.ttf")),
                font_size: 30,
                line_gap: 8,
                font_bold: false,
                font_italic: true,
                align: 0,
                ..Default::default()
            },
            ..Settings::default()
        };
        let mut saved = source.clone();
        saved
            .caption_templates
            .push(source.caption_template("旅行模板".into()));
        let decoded: Settings =
            serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        let mut target = Settings {
            style: StyleSettings {
                padding: 12,
                ..Default::default()
            },
            quality: 92,
            ..Settings::default()
        };
        target.apply_caption_template(&decoded.caption_templates[0]);
        assert_eq!(target.style.title, source.style.title);
        assert_eq!(target.style.subtitle, source.style.subtitle);
        assert_eq!(target.style.font, source.style.font);
        assert_eq!(target.style.font_family, source.style.font_family);
        assert_eq!(
            (
                target.style.font_size,
                target.style.line_gap,
                target.style.align
            ),
            (30, 8, 0)
        );
        assert!(!target.style.font_bold);
        assert!(target.style.font_italic);
        assert_eq!((target.style.padding, target.quality), (12, 92));
    }
}

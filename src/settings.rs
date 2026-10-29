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

/// Visual state of one preset; export preferences and saved templates are independent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleSettings {
    background: Background,
    ratio: Ratio,
    color: [u8; 3],
    text_color: [u8; 3],
    padding: u32,
    horizontal_padding: Option<u32>,
    footer: u32,
    radius: u32,
    blur: u32,
    background_dim: u32,
    shadow: bool,
    font_size: u32,
    line_gap: u32,
    align: u32,
    uppercase: bool,
    title: String,
    subtitle: String,
    custom: BTreeMap<String, String>,
    force_custom: bool,
    font: Option<PathBuf>,
    font_family: Option<String>,
    font_bold: bool,
    font_italic: bool,
    logo: Option<PathBuf>,
}

impl Default for StyleSettings {
    fn default() -> Self {
        Self::capture(&Settings::default())
    }
}

impl StyleSettings {
    fn capture(settings: &Settings) -> Self {
        Self {
            background: settings.background,
            ratio: settings.ratio,
            color: settings.color,
            text_color: settings.text_color,
            padding: settings.padding,
            horizontal_padding: settings.horizontal_padding,
            footer: settings.footer,
            radius: settings.radius,
            blur: settings.blur,
            background_dim: settings.background_dim,
            shadow: settings.shadow,
            font_size: settings.font_size,
            line_gap: settings.line_gap,
            align: settings.align,
            uppercase: settings.uppercase,
            title: settings.title.clone(),
            subtitle: settings.subtitle.clone(),
            custom: settings.custom.clone(),
            force_custom: settings.force_custom,
            font: settings.font.clone(),
            font_family: settings.font_family.clone(),
            font_bold: settings.font_bold,
            font_italic: settings.font_italic,
            logo: settings.logo.clone(),
        }
    }

    fn apply_to(&self, settings: &mut Settings) {
        settings.background = self.background;
        settings.ratio = self.ratio;
        settings.color = self.color;
        settings.text_color = self.text_color;
        settings.padding = self.padding;
        settings.horizontal_padding = self.horizontal_padding;
        settings.footer = self.footer;
        settings.radius = self.radius;
        settings.blur = self.blur;
        settings.background_dim = self.background_dim;
        settings.shadow = self.shadow;
        settings.font_size = self.font_size;
        settings.line_gap = self.line_gap;
        settings.align = self.align;
        settings.uppercase = self.uppercase;
        settings.title.clone_from(&self.title);
        settings.subtitle.clone_from(&self.subtitle);
        settings.custom.clone_from(&self.custom);
        settings.force_custom = self.force_custom;
        settings.font.clone_from(&self.font);
        settings.font_family.clone_from(&self.font_family);
        settings.font_bold = self.font_bold;
        settings.font_italic = self.font_italic;
        settings.logo.clone_from(&self.logo);
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub language: crate::i18n::Language,
    pub selected_preset: Option<StylePreset>,
    /// Only inactive presets are stored here; the fields below are the active edit.
    pub preset_edits: BTreeMap<StylePreset, StyleSettings>,
    pub background: Background,
    pub ratio: Ratio,
    pub color: [u8; 3],
    pub text_color: [u8; 3],
    pub padding: u32,
    /// None keeps the uniform margins of previously saved configurations.
    #[serde(default)]
    pub horizontal_padding: Option<u32>,
    pub footer: u32,
    pub radius: u32,
    pub blur: u32,
    #[serde(default)]
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
    pub caption_templates: Vec<CaptionTemplate>,
    pub logo: Option<PathBuf>,
    pub format: Format,
    pub quality: u8,
    pub max_edge: u32,
    pub output: Option<PathBuf>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            language: crate::i18n::Language::default(),
            selected_preset: Some(StylePreset::Atmosphere),
            preset_edits: BTreeMap::new(),
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
            caption_templates: vec![],
            logo: None,
            format: Format::Jpeg,
            quality: 85,
            max_edge: 0,
            output: None,
        }
    }
}
impl Settings {
    fn preset_settings(preset: StylePreset) -> Self {
        let mut settings = Self {
            selected_preset: Some(preset),
            ..Self::default()
        };
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

    fn remember_current_style(&mut self) {
        if let Some(preset) = self.selected_preset {
            if self.matches_preset(preset) {
                self.preset_edits.remove(&preset);
            } else {
                self.preset_edits
                    .insert(preset, StyleSettings::capture(self));
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
            .unwrap_or_else(|| StyleSettings::capture(&Self::preset_settings(preset)));
        style.apply_to(self);
        self.selected_preset = Some(preset);
    }

    /// Explicitly discard only this preset's edits after the UI confirms the action.
    pub fn restore_preset(&mut self, preset: StylePreset) {
        self.switch_preset(preset);
        self.preset_edits.remove(&preset);
        StyleSettings::capture(&Self::preset_settings(preset)).apply_to(self);
    }

    fn matches_preset(&self, preset: StylePreset) -> bool {
        StyleSettings::capture(self) == StyleSettings::capture(&Self::preset_settings(preset))
    }

    pub fn preset_modified(&self) -> bool {
        self.selected_preset
            .is_some_and(|preset| !self.matches_preset(preset))
    }

    pub fn caption_template(&self, name: String) -> CaptionTemplate {
        CaptionTemplate {
            name,
            title: self.title.clone(),
            subtitle: self.subtitle.clone(),
            family: self.font_family.clone(),
            font: self.font.clone(),
            size: self.font_size,
            gap: self.line_gap,
            bold: self.font_bold,
            italic: self.font_italic,
            align: self.align,
        }
    }
    pub fn apply_caption_template(&mut self, template: &CaptionTemplate) {
        self.title = template.title.clone();
        self.subtitle = template.subtitle.clone();
        self.font_family = template.family.clone();
        self.font = template.font.clone();
        self.font_size = template.size;
        self.line_gap = template.gap;
        self.font_bold = template.bold;
        self.font_italic = template.italic;
        self.align = template.align;
        self.normalize();
    }
    pub fn normalize(&mut self) {
        self.padding = self.padding.min(30);
        self.horizontal_padding = self.horizontal_padding.map(|v| v.min(30));
        self.footer = self.footer.min(40);
        self.radius = self.radius.min(20);
        self.blur = self.blur.min(100);
        self.background_dim = self.background_dim.min(90);
        self.font_size = self.font_size.clamp(8, 80);
        self.line_gap = self.line_gap.min(40);
        self.align = self.align.min(2);
        self.quality = self.quality.clamp(1, 100);
        self.max_edge = self.max_edge.min(16000);
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
            if value.footer == 14 {
                value.footer = 6;
            }
            if value.padding == 5 {
                value.padding = 3;
            }
            if value.font_size == 24 {
                value.font_size = 20;
            }
            if value.line_gap == 12 {
                value.line_gap = 6;
            }
            if value.background == Background::Solid
                && value.color == [250, 249, 246]
                && value.text_color == [45, 45, 43]
                && value.radius == 0
                && !value.shadow
            {
                value.background = Background::Blur;
                value.text_color = [244, 244, 240];
                value.radius = 2;
                value.shadow = true;
            }
        }
        if legacy_font {
            value.font_bold = value.background == Background::Blur;
            value.font_italic = value.background == Background::Blur;
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
            let mut settings = Self::preset_settings(*preset);
            style.apply_to(&mut settings);
            settings.normalize();
            *style = StyleSettings::capture(&settings);
            !settings.matches_preset(*preset)
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
        match self.format {
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
    fn presets_replace_previous_visual_edits_and_preserve_user_preferences() {
        for preset in StylePreset::ALL {
            let mut edited = Settings {
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
            assert_eq!(
                StyleSettings::capture(&edited),
                StyleSettings::capture(&clean)
            );
            assert_eq!(edited.selected_preset, Some(preset));
            assert!(!edited.preset_modified());
            assert_eq!(edited.ratio, Ratio::Auto);
            assert_eq!(edited.title, "{camera}");
            assert!(edited.font.is_none() && edited.logo.is_none());
            assert!(edited.custom.is_empty() && !edited.force_custom);
            assert_eq!(edited.caption_templates, preferences.caption_templates);
            assert_eq!(
                (edited.format, edited.quality, edited.max_edge),
                (Format::Webp, 97, 3000)
            );
            assert_eq!(edited.output, preferences.output);
        }
    }

    #[test]
    fn preset_identity_survives_edits_and_explicit_restore_clears_them() {
        for preset in StylePreset::ALL {
            let mut settings = Settings::default();
            settings.restore_preset(preset);
            let original = settings.clone();
            settings.title = "临时文字".into();
            settings.font_italic = !settings.font_italic;
            settings.ratio = Ratio::Portrait;
            assert_eq!(settings.selected_preset, Some(preset));
            assert!(settings.preset_modified());
            settings.restore_preset(preset);
            assert_eq!(settings, original);
            assert!(!settings.preset_modified());
        }
        let mut settings = Settings::default();
        settings.padding += 1;
        assert!(settings.preset_modified());
        settings.padding -= 1;
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
            title: "旅行 · {camera}".into(),
            subtitle: "{date} / {focal}".into(),
            font_family: Some("serif".into()),
            font: Some(PathBuf::from("my-font.ttf")),
            font_size: 30,
            line_gap: 8,
            font_bold: false,
            font_italic: true,
            align: 0,
            ..Settings::default()
        };
        let mut saved = source.clone();
        saved
            .caption_templates
            .push(source.caption_template("旅行模板".into()));
        let decoded: Settings =
            serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        let mut target = Settings {
            padding: 12,
            quality: 92,
            ..Settings::default()
        };
        target.apply_caption_template(&decoded.caption_templates[0]);
        assert_eq!(target.title, source.title);
        assert_eq!(target.subtitle, source.subtitle);
        assert_eq!(target.font, source.font);
        assert_eq!(target.font_family, source.font_family);
        assert_eq!(
            (target.font_size, target.line_gap, target.align),
            (30, 8, 0)
        );
        assert!(!target.font_bold);
        assert!(target.font_italic);
        assert_eq!((target.padding, target.quality), (12, 92));
    }
}

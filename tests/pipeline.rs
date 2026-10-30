use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use photo_frame::{
    render::{self, Engine, Photo},
    settings::{Background, Format, Ratio, Settings, StylePreset},
};
use std::{collections::BTreeMap, path::PathBuf};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/test-output")
            .join(format!(
                "{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn photo() -> Photo {
    Photo {
        image: DynamicImage::ImageRgba8(RgbaImage::from_pixel(160, 100, Rgba([180, 60, 30, 255]))),
        fields: BTreeMap::from([("model".into(), "相机测试".into())]),
    }
}

#[test]
fn cached_preview_redraws_changed_logos_and_recovers_after_missing_logo() {
    let directory = Scratch::new();
    let logo = directory.0.join("logo.png");
    let photo = std::sync::Arc::new(photo());
    let mut engine = Engine::default();
    let mut reference = Engine::default();
    let mut settings = Settings {
        footer: 30,
        title: "Caption".into(),
        subtitle: String::new(),
        ..Settings::default()
    };
    let without_logo = engine
        .compose_cached_preview(&photo, &settings, (300, 300))
        .unwrap();
    settings.logo = Some(logo.clone());
    assert!(
        engine
            .compose_cached_preview(&photo, &settings, (300, 300))
            .is_err()
    );
    let mut previous = without_logo.clone();
    for color in [[255, 0, 0, 255], [0, 255, 0, 128]] {
        RgbaImage::from_pixel(20, 20, Rgba(color))
            .save(&logo)
            .unwrap();
        let actual = engine
            .compose_cached_preview(&photo, &settings, (300, 300))
            .unwrap();
        assert_eq!(
            actual,
            reference
                .compose_preview(&photo, &settings, (300, 300))
                .unwrap()
        );
        assert_ne!(actual, previous);
        previous = actual;
    }
    settings.logo = None;
    assert_eq!(
        engine
            .compose_cached_preview(&photo, &settings, (300, 300))
            .unwrap(),
        without_logo
    );
}

#[test]
fn language_is_an_independent_preference_and_older_configs_follow_system() {
    use photo_frame::i18n::Language;
    let dir = Scratch::new();
    let path = dir.0.join("settings.json");
    let mut settings = Settings::default();
    assert_eq!(settings.language, Language::System);
    settings.language = Language::SimplifiedChinese;
    assert!(!settings.preset_modified());
    for preset in StylePreset::ALL {
        settings.switch_preset(preset);
        assert_eq!(settings.language, Language::SimplifiedChinese);
        assert!(!settings.preset_modified());
        settings.title = "User caption · {camera}".into();
        settings.restore_preset(preset);
        assert_eq!(settings.language, Language::SimplifiedChinese);
    }
    settings.title = "用户文字 · User caption".into();
    settings.save_to(&path).unwrap();
    assert_eq!(Settings::load_from(&path).unwrap(), settings);
    let mut json = serde_json::to_value(&settings).unwrap();
    json["settings_version"] = 2.into();
    json.as_object_mut().unwrap().remove("language");
    std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    let migrated = Settings::load_from(&path).unwrap();
    assert_eq!(migrated.language, Language::System);
    assert_eq!(migrated.title, settings.title);
    assert_eq!(migrated.selected_preset, settings.selected_preset);
    migrated.save_to(&path).unwrap();
    assert_eq!(
        Settings::load_from(&path).unwrap().language,
        Language::System
    );
    for language in [Language::English, Language::SimplifiedChinese] {
        settings.language = language;
        settings.save_to(&path).unwrap();
        assert_eq!(Settings::load_from(&path).unwrap().language, language);
    }
}

#[test]
fn each_preset_keeps_its_edits_across_switches_and_restarts() {
    let dir = Scratch::new();
    let path = dir.0.join("settings.json");
    let mut settings = Settings::default();
    for (i, preset) in StylePreset::ALL.into_iter().enumerate() {
        settings.switch_preset(preset);
        assert!(!settings.preset_modified());
        settings.title = format!("预设 {i}");
        settings.padding = 10 + i as u32;
        settings.ratio = Ratio::Portrait;
        settings.font = Some(PathBuf::from(format!("font-{i}.ttf")));
        settings.logo = Some(PathBuf::from(format!("logo-{i}.png")));
        settings.custom.insert("camera".into(), format!("相机 {i}"));
        settings.force_custom = true;
        let before = settings.clone();
        settings.switch_preset(preset);
        assert_eq!(settings, before, "Re-selecting must not reset edits");
    }
    settings.save_to(&path).unwrap();
    let mut restored = Settings::load_from(&path).unwrap();
    assert_eq!(restored, settings);
    assert_eq!(restored.selected_preset, Some(StylePreset::Polaroid));
    for (i, preset) in StylePreset::ALL.into_iter().enumerate() {
        restored.switch_preset(preset);
        assert_eq!(restored.title, format!("预设 {i}"));
        assert_eq!(restored.padding, 10 + i as u32);
        assert_eq!(restored.ratio, Ratio::Portrait);
        assert_eq!(restored.font, Some(PathBuf::from(format!("font-{i}.ttf"))));
        assert_eq!(restored.logo, Some(PathBuf::from(format!("logo-{i}.png"))));
        assert_eq!(restored.custom["camera"], format!("相机 {i}"));
        assert!(restored.force_custom && restored.preset_modified());
        assert!(!restored.preset_edits.contains_key(&preset));
    }
}

#[test]
fn restoring_one_preset_preserves_other_presets_and_global_preferences() {
    let dir = Scratch::new();
    let path = dir.0.join("settings.json");
    let mut settings = Settings {
        language: photo_frame::i18n::Language::SimplifiedChinese,
        output: Some(dir.0.join("exports")),
        format: Format::Webp,
        quality: 96,
        max_edge: 4000,
        ..Settings::default()
    };
    settings
        .caption_templates
        .push(settings.caption_template("保留模板".into()));
    let templates = settings.caption_templates.clone();
    for (i, preset) in StylePreset::ALL.into_iter().enumerate() {
        settings.switch_preset(preset);
        settings.title = format!("修改 {i}");
    }
    settings.switch_preset(StylePreset::Dark);
    // Restore an edited preset after a restart, while preserving the other histories.
    settings.save_to(&path).unwrap();
    let loaded = Settings::load_from(&path).unwrap();
    assert_eq!(loaded, settings);
    assert!(loaded.preset_modified());
    let mut settings = loaded;
    let other_edits = settings.preset_edits.clone();
    settings.restore_preset(StylePreset::Dark);
    assert_eq!(settings.preset_edits, other_edits);
    assert_eq!(settings.title, "{camera}");
    assert!(!settings.preset_modified());
    settings.save_to(&path).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(json["preset_edits"].as_object().unwrap().len(), 3);
    for style in json["preset_edits"].as_object().unwrap().values() {
        for key in [
            "quality",
            "output",
            "format",
            "max_edge",
            "caption_templates",
            "preset_edits",
        ] {
            assert!(
                style.get(key).is_none(),
                "Global preferences must not be saved per preset"
            );
        }
    }
    let mut restored = Settings::load_from(&path).unwrap();
    for (i, preset) in StylePreset::ALL.into_iter().enumerate() {
        restored.switch_preset(preset);
        if preset == StylePreset::Dark {
            assert_eq!(restored.title, "{camera}");
            assert!(!restored.preset_modified());
        } else {
            assert_eq!(restored.title, format!("修改 {i}"));
        }
        assert_eq!(
            (restored.format, restored.quality, restored.max_edge),
            (Format::Webp, 96, 4000)
        );
        assert_eq!(restored.output, settings.output);
        assert_eq!(
            restored.language,
            photo_frame::i18n::Language::SimplifiedChinese
        );
        assert_eq!(restored.caption_templates, templates);
    }
}

#[test]
fn version_one_configuration_keeps_active_edits_when_migrated_to_preset_history() {
    let dir = Scratch::new();
    let path = dir.0.join("settings.json");
    let mut settings = Settings::default();
    settings.restore_preset(StylePreset::Dark);
    settings.title = "旧版本的修改".into();
    settings.font_italic = true;
    let mut json = serde_json::to_value(&settings).unwrap();
    json["settings_version"] = 1.into();
    json.as_object_mut().unwrap().remove("preset_edits");
    std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    let mut migrated = Settings::load_from(&path).unwrap();
    assert_eq!(migrated, settings);
    migrated.switch_preset(StylePreset::Atmosphere);
    assert_eq!(migrated.title, "{camera}");
    migrated.switch_preset(StylePreset::Dark);
    assert_eq!(migrated.title, "旧版本的修改");
    assert!(migrated.font_italic);
    migrated.save_to(&path).unwrap();
    assert_eq!(Settings::load_from(&path).unwrap(), migrated);
}

#[test]
fn stored_preset_edits_are_normalized_without_overwriting_active_fields() {
    let dir = Scratch::new();
    let path = dir.0.join("settings.json");
    let mut settings = Settings::default();
    settings.switch_preset(StylePreset::Dark);
    settings.padding = 9;
    settings.switch_preset(StylePreset::Atmosphere);
    settings.title = "当前编辑".into();
    let mut json = serde_json::to_value(&settings).unwrap();
    json["settings_version"] = 2.into();
    json["preset_edits"]["Dark"]["padding"] = 900.into();
    json["preset_edits"]["Atmosphere"] = json["preset_edits"]["Dark"].clone();
    std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    let mut restored = Settings::load_from(&path).unwrap();
    assert_eq!(restored.title, "当前编辑");
    assert!(!restored.preset_edits.contains_key(&StylePreset::Atmosphere));
    restored.switch_preset(StylePreset::Dark);
    assert_eq!(restored.padding, 30);
    restored.switch_preset(StylePreset::Atmosphere);
    assert_eq!(restored.title, "当前编辑");
}

#[test]
fn old_settings_only_select_a_preset_when_the_whole_style_matches() {
    let dir = Scratch::new();
    let path = dir.0.join("settings.json");
    for preset in StylePreset::ALL {
        let mut settings = Settings::default();
        settings.restore_preset(preset);
        settings.save_to(&path).unwrap();
        let mut json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        json.as_object_mut().unwrap().remove("selected_preset");
        std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        let exact = Settings::load_from(&path).unwrap();
        assert_eq!(exact.selected_preset, Some(preset));
        json["title"] = "旧的自定义文字".into();
        std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        let custom = Settings::load_from(&path).unwrap();
        assert!(custom.selected_preset.is_none());
        assert_eq!(custom.title, "旧的自定义文字");
        assert_eq!(custom.color, settings.color);
    }
}

#[test]
fn actual_exports_round_trip_and_never_overwrite() {
    let dir = Scratch::new();
    let source = dir.0.join("照片.png");
    photo().image.save(&source).unwrap();
    let original = std::fs::read(&source).unwrap();
    let mut engine = Engine::default();
    for format in [Format::Jpeg, Format::Png, Format::Webp] {
        let s = Settings {
            format,
            max_edge: 200,
            ..Settings::default()
        };
        let first = render::export(&mut engine, &source, &dir.0, &s).unwrap();
        let before = std::fs::read(&first).unwrap();
        let second = render::export(&mut engine, &source, &dir.0, &s).unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read(&first).unwrap(), before);
        let decoded = image::open(second).unwrap();
        assert!(decoded.width().max(decoded.height()) <= 200);
        assert!(decoded.width() > 160);
    }
    assert_eq!(std::fs::read(source).unwrap(), original);
}

#[test]
fn memory_estimate_accounts_for_high_precision_inputs() {
    let directory = Scratch::new();
    let ordinary = directory.0.join("ordinary.png");
    let high_precision = directory.0.join("high-precision.png");
    RgbaImage::from_pixel(64, 48, Rgba([10, 20, 30, 255]))
        .save(&ordinary)
        .unwrap();
    image::ImageBuffer::from_pixel(64, 48, Rgba([1000u16, 2000, 3000, u16::MAX]))
        .save(&high_precision)
        .unwrap();
    let settings = Settings::default();
    assert!(
        photo_frame::batch::estimate_peak_bytes(&high_precision, &settings).unwrap()
            > photo_frame::batch::estimate_peak_bytes(&ordinary, &settings).unwrap()
    );
    let preview = render::load_preview(&high_precision, 32).unwrap();
    assert_eq!(preview.image.color(), image::ColorType::Rgba8);
    assert_eq!(preview.image.dimensions(), (32, 24));
    assert_eq!(
        render::load(&high_precision).unwrap().image.color(),
        image::ColorType::Rgba16
    );
}

#[test]
fn compositions_cover_backgrounds_ratios_and_small_images() {
    let mut engine = Engine::default();
    for ratio in [
        Ratio::Auto,
        Ratio::Square,
        Ratio::Landscape,
        Ratio::Portrait,
        Ratio::Wide,
    ] {
        for background in [Background::Solid, Background::Blur] {
            let s = Settings {
                ratio,
                background,
                max_edge: 117,
                radius: 10,
                shadow: true,
                ..Settings::default()
            };
            let image = engine.compose(&photo(), &s, false).unwrap();
            assert!(image.width().max(image.height()) <= 117);
            assert_eq!(
                image.get_pixel(image.width() / 2, image.height() / 3).0[3],
                255
            );
            if ratio == Ratio::Square {
                assert_eq!(image.width(), image.height());
            }
        }
    }
    let tiny = Photo {
        image: DynamicImage::new_rgb8(1, 1),
        fields: BTreeMap::new(),
    };
    assert!(engine.compose(&tiny, &Settings::default(), false).is_ok());
}

#[test]
fn no_footer_and_missing_font_are_handled() {
    let mut engine = Engine::default();
    let s = Settings {
        padding: 0,
        horizontal_padding: None,
        footer: 0,
        radius: 0,
        shadow: false,
        ..Settings::default()
    };
    let out = engine.compose(&photo(), &s, false).unwrap();
    assert_eq!(out, photo().image.to_rgba8());
    let s = Settings {
        font: Some(PathBuf::from("missing-test-font.ttf")),
        ..s
    };
    assert!(engine.compose(&photo(), &s, false).is_err());
}

#[test]
fn atmosphere_dims_only_backdrop_and_matches_reference_proportions() {
    let mut engine = Engine::default();
    let settings = Settings {
        shadow: false,
        ..Settings::default()
    };
    let photo = Photo {
        image: DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            1200,
            800,
            Rgba([180, 100, 60, 255]),
        )),
        fields: BTreeMap::new(),
    };
    let layout = render::layout(1200, 800, &settings);
    assert_eq!(
        (layout.width, layout.height, layout.x, layout.y),
        (1416, 956, 108, 48)
    );
    let image = engine.compose(&photo, &settings, false).unwrap();
    assert_eq!(image.get_pixel(0, 0).0, [111, 62, 37, 255]);
    assert_eq!(
        image.get_pixel(layout.x + 600, layout.y + 400).0,
        [180, 100, 60, 255]
    );
    assert_eq!(image.get_pixel(layout.x, layout.y).0, [111, 62, 37, 255]);
}

#[test]
fn previous_settings_keep_uniform_margins_and_undimmed_background() {
    let settings: Settings = serde_json::from_str(r#"{"padding":3,"blur":30}"#).unwrap();
    assert_eq!(settings.horizontal_padding, None);
    assert_eq!(settings.background_dim, 0);
    let layout = render::layout(1200, 800, &settings);
    assert_eq!((layout.x, layout.y), (36, 36));
}

#[test]
fn preferences_replace_and_reload_from_a_single_settings_file() {
    let directory = Scratch::new();
    let path = directory.0.join("settings.json");
    std::fs::write(&path, br#"{"quality":95}"#).unwrap();
    assert_eq!(Settings::load_from(&path).unwrap().quality, 95);
    let mut settings = Settings {
        padding: 8,
        font_family: Some("serif".into()),
        title: "旅途 · {camera}".into(),
        format: Format::Webp,
        max_edge: 2000,
        output: Some(directory.0.join("exports")),
        ..Settings::default()
    };
    settings
        .caption_templates
        .push(settings.caption_template("旅行".into()));
    settings.save_to(&path).unwrap();
    assert_eq!(Settings::load_from(&path).unwrap(), settings);
    settings.padding = 5;
    settings.footer = 14;
    settings.font_size = 24;
    settings.line_gap = 12;
    settings.title = "下一段旅程".into();
    settings.format = Format::Jpeg;
    settings.quality = 95;
    settings.save_to(&path).unwrap();
    assert_eq!(Settings::load_from(&path).unwrap(), settings);
    let mut saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert!(saved.get("compact_jpeg").is_none());
    assert_eq!(saved["settings_version"], 2);
    // The obsolete option in existing preferences is ignored, without migrating modern styles.
    saved.as_object_mut().unwrap().remove("settings_version");
    saved["compact_jpeg"] = true.into();
    std::fs::write(&path, serde_json::to_vec(&saved).unwrap()).unwrap();
    assert_eq!(Settings::load_from(&path).unwrap(), settings);
    // Replacements leave exactly the settings file, with no session file or temporary debris.
    let files = std::fs::read_dir(&directory.0)
        .unwrap()
        .map(|v| v.unwrap().file_name())
        .collect::<Vec<_>>();
    assert_eq!(files, vec![std::ffi::OsString::from("settings.json")]);
}

#[test]
fn failed_settings_replacement_preserves_target_and_removes_temporary_file() {
    let directory = Scratch::new();
    let blocked = directory.0.join("settings.json");
    std::fs::create_dir(&blocked).unwrap();
    let sentinel = blocked.join("existing-data");
    std::fs::write(&sentinel, b"keep").unwrap();
    assert!(Settings::default().save_to(&blocked).is_err());
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"keep");
    assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn caption_font_choices_change_rendered_text_without_changing_composition() {
    let mut engine = Engine::default();
    let settings = Settings {
        footer: 20,
        font_size: 48,
        title: "Camera 123".into(),
        subtitle: String::new(),
        ..Settings::default()
    };
    let original = engine.compose(&photo(), &settings, false).unwrap();
    let changed = engine
        .compose(
            &photo(),
            &Settings {
                font_family: Some("monospace".into()),
                font_bold: false,
                font_italic: false,
                ..settings
            },
            false,
        )
        .unwrap();
    assert_eq!(original.dimensions(), changed.dimensions());
    assert_ne!(original, changed);
}

#[test]
fn jpeg_export_preserves_requested_quality_and_size_above_the_former_budget() {
    let directory = Scratch::new();
    let source = directory.0.join("pattern.png");
    // PNG compresses repeating pixels well; JPEG needs much more space at quality 95.
    let pixels = image::RgbImage::from_fn(800, 600, |x, y| {
        image::Rgb([
            (x % 2 * 255) as u8,
            (y % 2 * 255) as u8,
            ((x + y) % 2 * 255) as u8,
        ])
    });
    pixels.save(&source).unwrap();
    let expected = render::encode_jpeg(pixels, 95).unwrap();
    assert!(expected.len() > 256 * 1024);
    let settings = Settings {
        background: Background::Solid,
        padding: 0,
        horizontal_padding: Some(0),
        footer: 0,
        radius: 0,
        shadow: false,
        quality: 95,
        ..Settings::default()
    };
    let report =
        render::export_profiled(&mut Engine::default(), &source, &directory.0, &settings).unwrap();
    let actual = std::fs::read(&report.path).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(image::image_dimensions(report.path).unwrap(), (800, 600));
    assert_eq!(report.timings.jpeg_encodes, 1);
}

#[test]
fn exif_orientation_rotates_decoded_photo() {
    let dir = Scratch::new();
    let path = dir.0.join("rotated.jpg");
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
        .encode_image(&photo().image.to_rgb8())
        .unwrap();
    let exif = b"Exif\0\0II*\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
    let mut file = vec![0xff, 0xd8, 0xff, 0xe1];
    file.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
    file.extend_from_slice(exif);
    file.extend_from_slice(&jpeg[2..]);
    std::fs::write(&path, file).unwrap();
    let loaded = render::load(&path).unwrap();
    assert_eq!((loaded.image.width(), loaded.image.height()), (100, 160));
    let preview = render::load_preview(&path, 80).unwrap();
    assert_eq!(preview.image.dimensions(), (50, 80));
    assert_eq!(preview.fields, loaded.fields);
}

#[test]
fn preview_cache_reuses_pixels_upgrades_resolution_and_invalidates_changed_files() {
    use std::sync::Arc;
    let dir = Scratch::new();
    let path = dir.0.join("preview.png");
    RgbaImage::from_pixel(1200, 800, Rgba([20, 40, 60, 100]))
        .save(&path)
        .unwrap();
    let mut cache = render::PreviewCache::default();
    let small = cache.load(&path, 300).unwrap();
    assert_eq!(small.image.width(), 512);
    assert_eq!(small.image.to_rgba8().get_pixel(200, 100)[3], 100);
    assert_eq!(small.fields["width"], "1200");
    assert_eq!(small.fields["height"], "800");
    let same = cache.load(&path, 400).unwrap();
    assert!(Arc::ptr_eq(&small, &same));
    let large = cache.load(&path, 800).unwrap();
    assert_eq!(large.image.width(), 1024);
    assert!(!Arc::ptr_eq(&small, &large));
    assert!(Arc::ptr_eq(&large, &cache.load(&path, 300).unwrap()));
    assert_eq!(render::load(&path).unwrap().image.width(), 1200);

    // Changing only the modification timestamp must invalidate the cached pixels.
    let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1000))
        .unwrap();
    drop(file);
    let refreshed = cache.load(&path, 800).unwrap();
    assert!(!Arc::ptr_eq(&large, &refreshed));
    RgbaImage::from_pixel(600, 400, Rgba([200, 40, 60, 255]))
        .save(&path)
        .unwrap();
    let changed = cache.load(&path, 800).unwrap();
    assert_eq!(changed.image.width(), 600);
    assert_eq!(changed.image.to_rgba8().get_pixel(0, 0)[0], 200);
    assert!(Arc::ptr_eq(&changed, &cache.load(&path, 2400).unwrap()));
    assert!(cache.load(&path, 0).is_err());
    assert!(render::load_preview(&path, 0).is_err());
}

#[test]
fn preview_cache_evicts_by_pixel_budget_and_releases_removed_photos() {
    use std::sync::Arc;
    let dir = Scratch::new();
    let paths = ["a.png", "b.png", "c.png"].map(|name| dir.0.join(name));
    for path in &paths {
        image::RgbImage::from_pixel(600, 300, image::Rgb([20, 40, 60]))
            .save(path)
            .unwrap();
    }
    let bytes = 512 * 256 * 3;
    let mut cache = render::PreviewCache::new(bytes * 2);
    let a = Arc::downgrade(&cache.load(&paths[0], 300).unwrap());
    let b = Arc::downgrade(&cache.load(&paths[1], 300).unwrap());
    assert!(Arc::ptr_eq(
        &a.upgrade().unwrap(),
        &cache.load(&paths[0], 300).unwrap()
    ));
    let c = Arc::downgrade(&cache.load(&paths[2], 300).unwrap());
    assert!(b.upgrade().is_none());
    assert!(a.upgrade().is_some());
    cache.retain(&paths[2..]);
    assert!(a.upgrade().is_none());
    cache.retain(&[]);
    assert!(c.upgrade().is_none());

    let mut tiny = render::PreviewCache::new(bytes - 1);
    let oversized = Arc::downgrade(&tiny.load(&paths[0], 300).unwrap());
    assert!(oversized.upgrade().is_none());
}

#[test]
fn jpeg_preview_reduces_pixels_and_preserves_original_metadata_and_export_size() {
    let dir = Scratch::new();
    let source = dir.0.join("large.jpg");
    image::RgbImage::from_pixel(1600, 1200, image::Rgb([180, 60, 30]))
        .save(&source)
        .unwrap();
    let preview = render::load_preview(&source, 320).unwrap();
    assert_eq!(preview.image.dimensions(), (320, 240));
    let full = render::load(&source).unwrap();
    assert_eq!(full.image.dimensions(), (1600, 1200));
    assert_eq!(preview.fields, full.fields);
    assert!(preview.image.as_bytes().len() < full.image.as_bytes().len() / 20);
    assert_eq!(
        render::load_preview(&source, 3000)
            .unwrap()
            .image
            .dimensions(),
        (1600, 1200)
    );
    let settings = Settings {
        padding: 0,
        horizontal_padding: Some(0),
        footer: 0,
        shadow: false,
        format: Format::Png,
        ..Settings::default()
    };
    // The UI's 1536 px request must not round up to a larger decoded-cache bucket.
    let cached = render::PreviewCache::default().load(&source, 1536).unwrap();
    assert_eq!(cached.image.dimensions(), (1536, 1152));
    assert_eq!(cached.fields, full.fields);
    let mut engine = Engine::default();
    let composed = engine
        .compose_cached_preview(&cached, &settings, (1536, 864))
        .unwrap();
    assert!(composed.width() <= 1536 && composed.height() <= 864);
    let output = render::export(&mut engine, &source, &dir.0, &settings).unwrap();
    assert_eq!(image::image_dimensions(output).unwrap(), (1600, 1200));
}

#[test]
fn row_copy_preserves_opaque_and_transparent_photo_pixels() {
    let mut engine = Engine::default();
    for alpha in [0, 128, 255] {
        for radius in [0, 10, 20] {
            let settings = Settings {
                background: Background::Solid,
                color: [50, 80, 120],
                padding: 4,
                horizontal_padding: Some(9),
                footer: 0,
                radius,
                shadow: false,
                ..Settings::default()
            };
            let photo = Photo {
                image: DynamicImage::ImageRgba8(RgbaImage::from_fn(80, 60, |x, y| {
                    Rgba([x as u8, y as u8, 200, alpha])
                })),
                fields: BTreeMap::new(),
            };
            let actual = engine.compose(&photo, &settings, false).unwrap();
            let layout = render::layout(80, 60, &settings);
            let mut expected =
                RgbaImage::from_pixel(layout.width, layout.height, Rgba([50, 80, 120, 255]));
            let foreground = photo.image.to_rgba8();
            let r = (60. * radius as f32 / 100.).round() as i64;
            for y in 0..60i64 {
                for x in 0..80i64 {
                    let dx = (r - x).max(x - (79 - r)).max(0);
                    let dy = (r - y).max(y - (59 - r)).max(0);
                    if dx * dx + dy * dy <= r * r {
                        use image::Pixel;
                        expected
                            .get_pixel_mut(layout.x + x as u32, layout.y + y as u32)
                            .blend(foreground.get_pixel(x as u32, y as u32));
                    }
                }
            }
            assert_eq!(actual, expected, "alpha={alpha}, radius={radius}");
        }
    }
}

#[test]
fn batch_bounds_parallelism_preserves_files_and_continues_after_errors() {
    use photo_frame::batch::{self, BatchEvent, BatchOptions};
    use std::sync::{Arc, atomic::AtomicBool};
    let directory = Scratch::new();
    let source = directory.0.join("source.png");
    photo().image.save(&source).unwrap();
    let original = std::fs::read(&source).unwrap();
    let mut paths = vec![source.clone(); 6];
    paths.insert(1, directory.0.join("missing.jpg"));
    let options = BatchOptions {
        workers: 2,
        memory_budget: 512 * 1024 * 1024,
    };
    let receiver = batch::start(
        paths,
        directory.0.clone(),
        Settings::default(),
        Arc::new(AtomicBool::new(false)),
        options,
    )
    .unwrap();
    let mut active = std::collections::BTreeMap::new();
    let (mut completed, mut failed) = (0, 0);
    let mut files = std::collections::BTreeSet::new();
    while let Ok(event) = receiver.recv_timeout(std::time::Duration::from_secs(30)) {
        match event {
            BatchEvent::Configured { .. } => {}
            BatchEvent::Started {
                index,
                estimated_bytes,
            } => {
                assert!(active.insert(index, estimated_bytes).is_none());
                assert!(active.len() <= 2);
                assert!(active.len() == 1 || active.values().sum::<u64>() <= options.memory_budget);
            }
            BatchEvent::Finished { index, result, .. } => {
                active.remove(&index);
                completed += 1;
                match result {
                    Ok(report) => {
                        assert!(report.timings.jpeg_encodes > 0);
                        assert!(files.insert(report.path));
                    }
                    Err(_) => failed += 1,
                }
            }
        }
    }
    assert!(active.is_empty());
    assert_eq!((completed, failed, files.len()), (7, 1, 6));
    assert_eq!(std::fs::read(source).unwrap(), original);
}

#[test]
fn batch_cancellation_finishes_active_image_and_skips_waiting_jobs() {
    use photo_frame::batch::{self, BatchEvent, BatchOptions};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let directory = Scratch::new();
    let source = directory.0.join("source.png");
    photo().image.save(&source).unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    // Each image exceeds this deliberately tiny budget: only one may be admitted at once.
    let receiver = batch::start(
        vec![source; 10],
        directory.0.join("exports"),
        Settings::default(),
        cancel.clone(),
        BatchOptions {
            workers: 2,
            memory_budget: 1,
        },
    )
    .unwrap();
    let first = loop {
        let event = receiver
            .recv_timeout(std::time::Duration::from_secs(30))
            .unwrap();
        if !matches!(event, BatchEvent::Configured { .. }) {
            break event;
        }
    };
    assert!(matches!(first, BatchEvent::Started { .. }));
    cancel.store(true, Ordering::Relaxed);
    let mut finished = 0;
    while let Ok(event) = receiver.recv_timeout(std::time::Duration::from_secs(30)) {
        match event {
            BatchEvent::Configured { .. } => {}
            BatchEvent::Started { .. } => panic!("A waiting job started after cancellation"),
            BatchEvent::Finished { result, .. } => {
                assert!(result.is_ok());
                finished += 1;
            }
        }
    }
    assert_eq!(finished, 1);
    assert_eq!(
        std::fs::read_dir(directory.0.join("exports"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn batch_runs_oversized_jobs_alone_without_stalling_the_queue() {
    use photo_frame::batch::{self, BatchEvent, BatchOptions};
    use std::sync::{Arc, atomic::AtomicBool};
    let directory = Scratch::new();
    let source = directory.0.join("source.png");
    photo().image.save(&source).unwrap();
    let receiver = batch::start(
        vec![source; 3],
        directory.0.join("exports"),
        Settings::default(),
        Arc::new(AtomicBool::new(false)),
        BatchOptions {
            workers: 2,
            memory_budget: 1,
        },
    )
    .unwrap();
    let (mut active, mut completed) = (0, 0);
    while let Ok(event) = receiver.recv_timeout(std::time::Duration::from_secs(30)) {
        match event {
            BatchEvent::Configured { .. } => {}
            BatchEvent::Started { .. } => {
                active += 1;
                assert_eq!(active, 1);
            }
            BatchEvent::Finished { result, .. } => {
                assert!(result.is_ok());
                active -= 1;
                completed += 1;
            }
        }
    }
    assert_eq!((active, completed), (0, 3));
}

#[test]
fn automatic_batch_can_admit_more_than_two_images_when_cpu_and_memory_allow() {
    use photo_frame::batch::{self, BatchEvent, BatchOptions};
    use std::sync::{Arc, atomic::AtomicBool};
    let directory = Scratch::new();
    let source = directory.0.join("automatic.png");
    photo()
        .image
        .resize_exact(640, 400, image::imageops::FilterType::Nearest)
        .save(&source)
        .unwrap();
    let receiver = batch::start(
        vec![source; 8],
        directory.0.join("exports"),
        Settings::default(),
        Arc::new(AtomicBool::new(false)),
        BatchOptions {
            workers: 0,
            memory_budget: 1024 * 1024 * 1024,
        },
    )
    .unwrap();
    let (mut configured, mut peak, mut completed) = (0, 0, 0);
    let mut active = std::collections::BTreeMap::new();
    while let Ok(event) = receiver.recv_timeout(std::time::Duration::from_secs(30)) {
        match event {
            BatchEvent::Configured { workers, .. } => configured = workers,
            BatchEvent::Started {
                index,
                estimated_bytes,
            } => {
                active.insert(index, estimated_bytes);
                peak = peak.max(active.len());
                assert!(active.len() <= configured);
                assert!(active.values().sum::<u64>() <= 1024 * 1024 * 1024);
            }
            BatchEvent::Finished { index, result, .. } => {
                active.remove(&index);
                assert!(result.is_ok());
                completed += 1;
            }
        }
    }
    assert_eq!(completed, 8);
    assert!(active.is_empty());
    if configured > 2 {
        assert!(peak > 2, "Automatic mode remained capped at two images");
    }
}

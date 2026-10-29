use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "system")]
    System,
    #[serde(rename = "en")]
    English,
    #[serde(rename = "zh-CN")]
    SimplifiedChinese,
}

impl Language {
    pub fn locale(self) -> &'static str {
        match self {
            Self::System => system_language().locale(),
            Self::English => "en",
            Self::SimplifiedChinese => "zh-CN",
        }
    }

    pub fn activate(self) {
        rust_i18n::set_locale(self.locale());
    }
}

#[cfg(windows)]
fn language_from_windows_id(id: u16) -> Language {
    // Use the available Simplified Chinese catalog for Chinese UI variants.
    if id & 0x03ff == 0x04 {
        Language::SimplifiedChinese
    } else {
        Language::English
    }
}

#[cfg(windows)]
fn system_language() -> Language {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    // SAFETY: this Win32 query takes no pointers and has no preconditions.
    language_from_windows_id(unsafe { GetUserDefaultUILanguage() })
}

#[cfg(not(windows))]
fn system_language() -> Language {
    for name in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(locale) = std::env::var(name)
            && !locale.is_empty()
        {
            return if locale
                .split(['-', '_', '.', '@'])
                .next()
                .is_some_and(|name| name.eq_ignore_ascii_case("zh"))
            {
                Language::SimplifiedChinese
            } else {
                Language::English
            };
        }
    }
    Language::English
}

pub fn text(key: &str) -> String {
    rust_i18n::t!(key).into_owned()
}

pub fn interpolate(key: &str, parameters: &[(&str, &dyn std::fmt::Display)]) -> String {
    let (names, values): (Vec<_>, Vec<_>) = parameters
        .iter()
        .map(|(name, value)| (*name, value.to_string()))
        .unzip();
    rust_i18n::replace_patterns(&text(key), &names, &values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn catalog(source: &str) -> serde_json::Map<String, serde_json::Value> {
        serde_json::from_str::<serde_json::Value>(source)
            .unwrap()
            .as_object()
            .unwrap()
            .clone()
    }

    fn placeholders(text: &str) -> BTreeSet<&str> {
        text.split("%{")
            .skip(1)
            .map(|part| part.split('}').next().unwrap())
            .collect()
    }

    #[test]
    fn catalogs_have_matching_keys_and_interpolation_parameters() {
        let en = catalog(include_str!("../locales/en.json"));
        let zh = catalog(include_str!("../locales/zh-CN.json"));
        assert_eq!(en.keys().collect::<Vec<_>>(), zh.keys().collect::<Vec<_>>());
        for (key, value) in &en {
            if key == "_version" {
                continue;
            }
            let english = value.as_str().unwrap();
            let chinese = zh[key].as_str().unwrap();
            assert!(
                !english.trim().is_empty() && !chinese.trim().is_empty(),
                "{key}"
            );
            assert_eq!(placeholders(english), placeholders(chinese), "{key}");
            assert_eq!(crate::_rust_i18n_translate("en", key), english, "{key}");
            assert_eq!(crate::_rust_i18n_translate("zh-CN", key), chinese, "{key}");
        }
    }

    #[test]
    fn every_static_translation_call_has_both_translations() {
        let en = catalog(include_str!("../locales/en.json"));
        let zh = catalog(include_str!("../locales/zh-CN.json"));
        for source in [
            include_str!("main.rs"),
            include_str!("ui/controller.rs"),
            include_str!("ui/view.rs"),
            include_str!("ui/panels.rs"),
            include_str!("ui/export_dialog.rs"),
            include_str!("ui/settings_dialog.rs"),
            include_str!("ui/input.rs"),
            include_str!("render/mod.rs"),
            include_str!("render/photo.rs"),
            include_str!("render/preview.rs"),
            include_str!("render/jpeg.rs"),
            include_str!("render/export.rs"),
            include_str!("batch/mod.rs"),
            include_str!("batch/memory.rs"),
        ] {
            for call in source.split("tr!(").skip(1) {
                let call = call.trim_start();
                if let Some(call) = call.strip_prefix('"') {
                    let key = call.split('"').next().unwrap();
                    assert!(
                        en.contains_key(key) && zh.contains_key(key),
                        "Missing translation: {key}"
                    );
                }
            }
        }
    }

    #[test]
    fn unsupported_locale_falls_back_to_english() {
        assert_eq!(
            crate::_rust_i18n_translate("fr", "settings.title"),
            "Settings"
        );
        assert!(crate::_rust_i18n_try_translate("en", "unknown.key").is_none());
        assert_eq!(Language::default(), Language::System);
    }

    #[cfg(windows)]
    #[test]
    fn system_language_maps_chinese_variants_and_falls_back_to_english() {
        for id in [0x0004, 0x0804, 0x1004, 0x0404, 0x0c04, 0x1404, 0x7804] {
            assert_eq!(language_from_windows_id(id), Language::SimplifiedChinese);
        }
        for id in [0, 0x0409, 0x0809, 0x040c, 0x0411, 0x1400] {
            assert_eq!(language_from_windows_id(id), Language::English);
        }
        assert_eq!(Language::System.locale(), system_language().locale());
        assert_eq!(
            serde_json::to_string(&Language::System).unwrap(),
            "\"system\""
        );
        assert_eq!(
            serde_json::from_str::<Language>("\"system\"").unwrap(),
            Language::System
        );
    }

    #[test]
    fn runtime_language_switch_updates_owned_text_and_interpolation() {
        struct RestoreLocale(String);
        impl Drop for RestoreLocale {
            fn drop(&mut self) {
                rust_i18n::set_locale(&self.0);
            }
        }
        let _restore = RestoreLocale(rust_i18n::locale().to_string());
        Language::English.activate();
        assert_eq!(crate::tr!("settings.title"), "Settings");
        assert_eq!(
            crate::tr!("export.summary", count = 3, format = "JPEG"),
            "Photos: 3 · JPEG"
        );
        Language::SimplifiedChinese.activate();
        assert_eq!(crate::tr!("settings.title"), "设置");
        assert_eq!(
            crate::tr!("export.summary", count = 3, format = "JPEG"),
            "3 张照片 · JPEG"
        );
        assert!(crate::tr!("text.variables").contains("{camera}"));
    }
}

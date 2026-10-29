pub mod batch;
pub mod i18n;
pub mod metadata;
pub mod render;
pub mod settings;

rust_i18n::i18n!("locales", fallback = "en");

/// Owned translations work with both GPUI elements and background error messages.
#[macro_export]
macro_rules! tr {
    ($key:expr) => { $crate::i18n::text($key) };
    ($key:expr, $($name:ident = $value:expr),+ $(,)?) => {{
        $crate::i18n::interpolate($key, &[$((stringify!($name), &($value) as &dyn std::fmt::Display)),+])
    }};
}

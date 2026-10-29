use crate::settings::Settings;
use std::collections::BTreeMap;

/// Read bounded TIFF IFDs from a decoder's EXIF block. Unsupported fields are ignored.
pub fn parse(bytes: &[u8]) -> BTreeMap<String, String> {
    let bytes = bytes.strip_prefix(b"Exif\0\0").unwrap_or(bytes);
    let mut result = BTreeMap::new();
    if bytes.len() < 8 {
        return result;
    }
    let little = match &bytes[..2] {
        b"II" => true,
        b"MM" => false,
        _ => return result,
    };
    let reader = Tiff { bytes, little };
    if reader.u16(2) != Some(42) {
        return result;
    }
    if let Some(offset) = reader.u32(4) {
        reader.ifd(offset as usize, 0, &mut result);
    }
    result
}
struct Tiff<'a> {
    bytes: &'a [u8],
    little: bool,
}
impl Tiff<'_> {
    fn u16(&self, n: usize) -> Option<u16> {
        let a = self.bytes.get(n..n.checked_add(2)?)?.try_into().ok()?;
        Some(if self.little {
            u16::from_le_bytes(a)
        } else {
            u16::from_be_bytes(a)
        })
    }
    fn u32(&self, n: usize) -> Option<u32> {
        let a = self.bytes.get(n..n.checked_add(4)?)?.try_into().ok()?;
        Some(if self.little {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        })
    }
    fn ifd(&self, offset: usize, depth: u8, out: &mut BTreeMap<String, String>) {
        if depth > 2 {
            return;
        }
        let Some(count) = self.u16(offset) else {
            return;
        };
        for i in 0..usize::from(count).min(1024) {
            let Some(at) = offset.checked_add(2 + i * 12) else {
                break;
            };
            let (Some(tag), Some(kind), Some(count), Some(value)) = (
                self.u16(at),
                self.u16(at + 2),
                self.u32(at + 4),
                self.u32(at + 8),
            ) else {
                break;
            };
            if tag == 0x8769 {
                self.ifd(value as usize, depth + 1, out);
                continue;
            }
            let unit = match kind {
                1 | 2 | 7 => 1,
                3 => 2,
                4 | 9 => 4,
                5 | 10 => 8,
                _ => continue,
            };
            let Some(len) = (count as usize).checked_mul(unit) else {
                continue;
            };
            let pos = if len <= 4 { at + 8 } else { value as usize };
            let Some(end) = pos.checked_add(len) else {
                continue;
            };
            let Some(data) = self.bytes.get(pos..end) else {
                continue;
            };
            let key = match tag {
                0x010f => "make",
                0x0110 => "model",
                0xa434 => "lens",
                0x9003 => "date",
                0x829a => "shutter",
                0x829d => "aperture",
                0x8827 => "iso",
                0x920a => "focal",
                0xa405 => "focal35",
                0x013b => "artist",
                0x8298 => "copyright",
                _ => continue,
            };
            let text = if kind == 2 {
                String::from_utf8_lossy(data)
                    .trim_matches('\0')
                    .trim()
                    .to_owned()
            } else if kind == 5 && count > 0 {
                let (Some(n), Some(d)) = (self.u32(pos), self.u32(pos + 4)) else {
                    continue;
                };
                if d == 0 {
                    continue;
                }
                let v = n as f64 / d as f64;
                match key {
                    "shutter" if v > 0.0 && v < 1.0 => format!("1/{:.0}s", 1.0 / v),
                    "shutter" => format!("{v:.1}s"),
                    "aperture" => format!("f/{v:.1}"),
                    "focal" => format!("{v:.0}mm"),
                    _ => format!("{v:.1}"),
                }
            } else if kind == 3 && count > 0 {
                self.u16(pos).unwrap_or(0).to_string()
            } else if kind == 4 && count > 0 {
                value.to_string()
            } else {
                continue;
            };
            if !text.is_empty() {
                out.insert(key.to_owned(), text);
            }
        }
    }
}

pub fn expand(template: &str, metadata: &BTreeMap<String, String>, settings: &Settings) -> String {
    let mut fields = metadata.clone();
    for (key, value) in &settings.custom {
        if settings.force_custom || fields.get(key).is_none_or(String::is_empty) {
            fields.insert(key.clone(), value.clone());
        }
    }
    // EXIF's legal manufacturer name is not a useful camera caption.
    if let Some(make) = fields.get_mut("make")
        && ["NIKON CORPORATION", "NIKON COPERATION", "NIKON"]
            .iter()
            .any(|v| make.trim().eq_ignore_ascii_case(v))
    {
        *make = "Nikon".into();
    }
    let make = fields.get("make").cloned().unwrap_or_default();
    let model = fields.get("model").cloned().unwrap_or_default();
    let short_model = model
        .get(..make.len())
        .filter(|prefix| !make.is_empty() && prefix.eq_ignore_ascii_case(&make))
        .and_then(|_| model.get(make.len()..))
        .filter(|rest| rest.starts_with(char::is_whitespace))
        .map(str::trim)
        .unwrap_or(&model);
    let camera = match (make.is_empty(), short_model.is_empty()) {
        (false, false) => format!("{make} {short_model}"),
        (false, true) => make.clone(),
        (true, false) => model.clone(),
        _ => "—".into(),
    };
    // Preserve explicitly provided custom camera values.
    fields.entry("camera".into()).or_insert(camera);
    if template.contains("{make}") && template.contains("{model}") {
        fields.insert("model".into(), short_model.to_owned());
    }
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('}') else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let key = &rest[start + 1..start + end];
        out.push_str(fields.get(key).map(String::as_str).unwrap_or("—"));
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    if settings.uppercase {
        out.to_uppercase()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_exif_is_safe() {
        for len in 0..64 {
            assert!(parse(&vec![255; len]).is_empty());
        }
        assert!(parse(b"II*\0\xff\xff\xff\xff").is_empty());
    }
    #[test]
    fn endian_ascii() {
        let data = b"II*\0\x08\0\0\0\x01\0\x0f\x01\x02\0\x06\0\0\0\x1a\0\0\0\0\0\0\0Canon\0";
        assert_eq!(parse(data).get("make").unwrap(), "Canon");
        let big = b"MM\0*\0\0\0\x08\0\x01\x01\x0f\0\x02\0\0\0\x06\0\0\0\x1a\0\0\0\0Canon\0";
        assert_eq!(parse(big).get("make").unwrap(), "Canon");
    }
    #[test]
    fn custom_fallback_and_override() {
        let fields = BTreeMap::from([("model".into(), "Camera".into())]);
        let mut s = Settings::default();
        s.custom.insert("model".into(), "My camera".into());
        assert_eq!(expand("{model} {unknown}", &fields, &s), "Camera —");
        s.force_custom = true;
        assert_eq!(expand("{model}", &fields, &s), "My camera");
        assert_eq!(expand("中文 {unfinished", &fields, &s), "中文 {unfinished");
    }
    #[test]
    fn nikon_captions_remove_corporation_and_duplicate_brand() {
        let settings = Settings::default();
        for make in ["NIKON CORPORATION", "nikon coperation", "NIKON", "Nikon"] {
            let fields = BTreeMap::from([
                ("make".into(), make.into()),
                ("model".into(), "NIKON D7200".into()),
            ]);
            assert_eq!(expand("{camera}", &fields, &settings), "Nikon D7200");
            assert_eq!(expand("{make} {model}", &fields, &settings), "Nikon D7200");
            assert_eq!(expand("{model}", &fields, &settings), "NIKON D7200");
        }
        let fields = BTreeMap::from([("model".into(), "NIKON D7200".into())]);
        assert_eq!(expand("{camera}", &fields, &settings), "NIKON D7200");
        assert_eq!(expand("{camera}", &BTreeMap::new(), &settings), "—");
    }
}

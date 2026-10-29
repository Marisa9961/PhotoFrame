# Third-party notices

PhotoFrame's original code is Copyright (c) 2026 Marisa9961 and licensed under the MIT License in `LICENSE`. Third-party code retains its own licenses and copyright notices as described below.

`src/ui/input.rs` is adapted from `examples/input.rs` in GPUI 0.2.2, published by Zed Industries in the [zed-industries/zed](https://github.com/zed-industries/zed) repository. GPUI is licensed under Apache License 2.0. A copy is included in `licenses/GPUI-APACHE-2.0.txt`.

Modifications include a reusable entity constructor, change events, Windows and macOS editing shortcuts, dark styling, focus handling, and corrected IME selection offsets.

Other Rust dependency licenses remain with their respective authors and are described in each crate's package metadata. `Cargo.lock` records the dependency versions.

Interface localization uses `rust-i18n` 4.2.4 (MIT), resolved through Cargo. Translation catalogs are embedded at compile time. Full license texts remain in the project and dependency packages; the settings window shows brief credits only.

JPEG encoding and reduced-resolution preview decoding use the official `libjpeg-turbo-rs` 0.8.0 Rust crate (MIT OR Apache-2.0), resolved through Cargo's registry and cache. Its original license texts are included in `licenses/JPEG-MIT.txt` and `licenses/JPEG-APACHE-2.0.txt`. Cargo.lock records the package version, registry source, and checksum. No C libjpeg-turbo library is linked. Full-resolution JPEG input and preview fallback use `image` / `zune-jpeg` under their respective licenses.

This software is based in part on the work of the Independent JPEG Group.

The yiyin project is referenced for feature inspiration only. Its GPL-3.0 code and assets are not incorporated into this implementation.

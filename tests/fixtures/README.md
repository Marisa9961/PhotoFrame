# JPEG migration references

These three small JPEGs were generated from procedural gradients by the old
TurboJPEG backend (`turbojpeg` 1.5.1, C libjpeg-turbo 3.1.0), not by the new Rust
encoder. All are 257×129:

- `baseline-444.jpg`: RGB, baseline, 4:4:4, quality 85.
- `progressive-420.jpg`: RGB, progressive, 4:2:0, quality 90.
- `cmyk.jpg`: CMYK, baseline, quality 90, Adobe marker.

The procedural input uses RGB values `(x % 256, (y * 2) % 256,
((x + y) / 2) % 256)`; the CMYK input adds a fourth channel fixed at 220.
Regeneration requires the old TurboJPEG dependency. These fixed reference images
are original PhotoFrame test material under the project's MIT license.

# Changelog

All notable changes to arthash are listed here. The wire format
(`docs/SPEC.md`) is **independently versioned** — SDK API revisions don't
imply byte-format changes unless explicitly noted.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased] — 0.2.0

Major API-ergonomics revision. **Wire format unchanged** — hashes produced by
0.1 still decode under 0.2, and vice versa. See [`docs/MIGRATION.md`](./docs/MIGRATION.md)
for the upgrade walkthrough.

### Breaking changes

#### Rust (`arthash`)

- `Codec` is now a `pub enum` (tagged union per shape mode) instead of a
  struct with twelve fields. Construct via factory methods —
  `Codec::dct()` / `Codec::triangle(n)` / `Codec::circle(n)` /
  `Codec::square(n)` / `Codec::rect(n)` / `Codec::rotated_rect(n)` /
  `Codec::pixel(n)`. Each variant carries only fields that apply.
- `decode()` returns a named `DecodeOutput { width, height, rgba }` instead
  of a `(u32, u32, Vec<u8>)` tuple.
- `ShapeType` and the field-level struct `CodecConfig` are now
  `#[doc(hidden)]` — internal SPEC plumbing. Test/FFI code that needs
  per-field control should construct a `CodecConfig` and wrap with
  `Codec::Raw(...)`.
- `arthash::shape` submodules are now `pub(crate)` (except `options`,
  `pixel`, `raster`, `svg` which contain public types).

#### Python (`arthash`)

- `decode()` always returns `(w, h, ndarray(h, w, 4) uint8 RGBA)` regardless
  of codec mode. Previously branched: DCT returned raw bytes, shape modes
  returned a `(h, w, 3)` RGB ndarray. Downstream code no longer needs to
  special-case codec mode.
- `decode()` `aa` parameter now actually works (was silently ignored). Pass
  `aa=2` for 4× supersampling on shape modes. Default remains `aa=1`.
- `encode()` `target_size` parameter is no longer silently overridden for
  shape codecs. Pass `target_size=None` (default) to get the codec-natural
  thumbnail (`100` for DCT, `48` for shape modes).

#### TypeScript (`arthash`)

- `encode` / `decode` / `toSvg` are now async (wasm auto-loads on first
  call). `await init()` is no longer required. Use the `encodeSync` /
  `decodeSync` / `toSvgSync` variants for tight loops after explicit `init()`.
- Codec is now a discriminated union built via the `codec` factory
  namespace — `codec.triangle({ n: 64 })`, `codec.preset(Preset.DetailTriangle)`,
  etc. The previous "spread `CodecOptions` into `EncodeOptions`/`DecodeOptions`"
  pattern is gone.

### Added

#### Cross-language

- **Named presets.** `Preset::TinyDct`, `PlaceholderTriangle`,
  `PlaceholderCircle`, `PlaceholderPixel`, `MediumTriangle`, `MediumCircle`,
  `MediumPixel`, `DetailTriangle`, `DetailCircle`, `DetailPixel`. Wired into
  all three SDKs + CLI. The preset matrix is symmetric across triangle /
  circle / pixel at three byte budgets.
- **`is_byte_compatible_with(other)`.** Verifies two codecs would decode each
  other's hashes byte-for-byte. Looser than `==` — ignores construction
  style (factory vs raw). Available in Rust + Python.
- **`bytes_total()`.** Predicts hash length without running the encoder.
  Newly added to Rust + TS (Python already had it).

#### Rust

- `Codec`, `Palette`, `ColorMode`, `Preset` derive `PartialEq` (and `Eq` /
  `Hash` where field types allow).
- `Codec::Raw(CodecConfig)` escape hatch for conformance tests / FFI bindings
  that need full SPEC field control.
- `Preset::all()`, `Preset::name()`, `Preset::from_name(&str)` for
  iteration / serialization.
- `arthash::encode_image(path, codec, opts)` convenience under the
  `image-io` feature — handles file loading + thumbnail resize.

#### Python

- Top-level preset shortcuts: `from arthash import detail_triangle, …`
  spares the `Codec.preset(Preset.X)` ceremony for common slots.
- `Codec.to_dict()` / `Codec.from_dict()` — JSON-safe serialization, useful
  for storing codec metadata alongside hashes.
- Factory classmethods on `Codec`: `Codec.dct()`, `Codec.triangle(n=…)`, etc.
  The previous dataclass constructor still works as a low-level alternative.
- `Codec.with_palette(p)` / `Codec.with_color_bits(bits)` builders return
  modified copies (codec stays frozen).

#### TypeScript

- `encodeRgba()` — was missing from TS, now matches Rust/Python.
- `encodeImage(source, codec)` — browser convenience: accepts URL string,
  `Blob`, `HTMLImageElement`, or `ImageBitmap`. Loads, resizes to the codec's
  thumbnail target, encodes in one call.
- `palette.fromHex(["#000000", ...])` and `palette.fromRgb([[r,g,b], ...])`
  factories — no more manual `Uint8Array` plumbing.
- `codec.bytesTotal(c)` / `codec.isPaletteMode(c)` / `codec.withPalette(c, p)`
  helpers.
- `codec.raw(spec)` low-level escape hatch matching Rust's `Codec::Raw`.
- `search` options exposed on `encode()` (hill-climb tuning) — previously
  only available in Rust + Python.

### Removed / Fixed

- **(Python)** `decode(..., aa=True)` no longer silently ignored.
- **(Python)** `encode(..., target_size=N)` no longer silently overridden
  for shape codecs.
- **(TS)** Forgetting `await init()` is no longer a footgun — auto-init on
  first async call.
- **(TS)** `DecodeResult` is now a plain object (`free()` no longer the
  caller's concern).

### Documentation

- New `docs/MIGRATION.md` with 0.1 → 0.2 code examples for every SDK.
- All three SDK READMEs updated to lead with factory methods + named presets.

---

## 0.1.2 — 2026-05-15

Last release of the field-struct `Codec` API. See git log for details.

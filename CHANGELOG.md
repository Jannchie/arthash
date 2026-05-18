## [Unreleased] — 0.3.0

Preset naming overhaul. **Wire format unchanged** — hashes produced by 0.2 still
decode under 0.3 codecs. See [`docs/MIGRATION.md`](./docs/MIGRATION.md#02--03)
for the full rename table.

### Added

- **New size-axis preset naming.** `Preset` size buckets renamed to
  `small_*` / `medium_*` / `large_*` (n=12 / 24 / 64; pixel small=16).
  `tiny_dct` renamed to plain `dct`. `medium_*` names unchanged.
- **Rect and Square presets.** Six new presets — `{small,medium,large}_rect`
  and `{small,medium,large}_square` — wired across Rust / TS / Python / CLI /
  playground. Rect and square shape factories were already available at the
  `Codec::rect(n)` / `Codec::square(n)` layer; this surfaces them as named
  recipes.
- **Playground rows.** The compare view now shows all 16 presets in one row,
  grouped by size tier (DCT, then small / medium / large × triangle, circle,
  pixel, rect, square).

### Deprecated

The pre-0.3 preset names are kept as aliases that produce byte-identical
codecs to their replacements. They will be removed in 1.0.

- Rust: `Preset::TinyDct` / `PlaceholderTriangle` / `PlaceholderCircle` /
  `PlaceholderPixel` / `DetailTriangle` / `DetailCircle` / `DetailPixel`
  carry `#[deprecated(since = "0.3.0")]`.
- TypeScript: same keys on the `Preset` object carry `@deprecated` JSDoc.
- Python: `Preset.TINY_DCT` etc. still parse; top-level shortcuts
  (`tiny_dct()` / `placeholder_*()` / `detail_*()`) emit `DeprecationWarning`.
- CLI: `--preset tiny-dct` / `placeholder-*` / `detail-*` still parse and
  appear in `--help` as `Deprecated alias for ...`.

---

## v0.2.0

[v0.1.2...v0.2.0](https://github.com/Jannchie/arthash/compare/v0.1.2...v0.2.0)

### :rocket: Breaking Changes

- **docs**: add changelog and migration documentation - By [Jianqi Pan](mailto:jannchie@gmail.com) in [1143d99](https://github.com/Jannchie/arthash/commit/1143d99)

### :sparkles: Features

- **animation**: add animation view and search controls - By [Jianqi Pan](mailto:jannchie@gmail.com) in [616cdc7](https://github.com/Jannchie/arthash/commit/616cdc7)
- **docs**: initialize documentation and add user guide - By [Jianqi Pan](mailto:jannchie@gmail.com) in [94bc8da](https://github.com/Jannchie/arthash/commit/94bc8da)
- **docs**: add comprehensive documentation and benchmarks - By [Jianqi Pan](mailto:jannchie@gmail.com) in [fc2300a](https://github.com/Jannchie/arthash/commit/fc2300a)
- **logo**: add logo generation script and svg file - By [Jianqi Pan](mailto:jannchie@gmail.com) in [61add74](https://github.com/Jannchie/arthash/commit/61add74)
- **ui**: add medium circle and pixel presets - By [Jianqi Pan](mailto:jannchie@gmail.com) in [9fa070b](https://github.com/Jannchie/arthash/commit/9fa070b)

### :adhesive_bandage: Fixes

- **ci**: bump all workflows to Node 22 - By [Jianqi Pan](mailto:jannchie@gmail.com) and [Claude Opus 4.7 (1M context)](mailto:noreply@anthropic.com) in [0780e72](https://github.com/Jannchie/arthash/commit/0780e72)
- **ci**: pin pnpm to 11.1.2 for npm OIDC Trusted Publishing - By [Jianqi Pan](mailto:jannchie@gmail.com) and [Claude Opus 4.7 (1M context)](mailto:noreply@anthropic.com) in [d53761e](https://github.com/Jannchie/arthash/commit/d53761e)
- **ci**: upgrade pnpm to 10 for npm OIDC Trusted Publishing - By [Jianqi Pan](mailto:jannchie@gmail.com) and [Claude Opus 4.7 (1M context)](mailto:noreply@anthropic.com) in [73df796](https://github.com/Jannchie/arthash/commit/73df796)

### :memo: Documentation

- **readme**: update footprint information for wasm - By [Jianqi Pan](mailto:jannchie@gmail.com) in [16d3517](https://github.com/Jannchie/arthash/commit/16d3517)
- **readme**: update readme with project details and benchmarks - By [Jianqi Pan](mailto:jannchie@gmail.com) in [2045704](https://github.com/Jannchie/arthash/commit/2045704)

### :wrench: Chores

- **build**: update build scripts and ignore patterns - By [Jianqi Pan](mailto:jannchie@gmail.com) in [dd2b7f6](https://github.com/Jannchie/arthash/commit/dd2b7f6)
- **deps**: update dependencies and workspace configuration - By [Jianqi Pan](mailto:jannchie@gmail.com) in [117e955](https://github.com/Jannchie/arthash/commit/117e955)
- **deps**: allow esbuild postinstall script - By [Jianqi Pan](mailto:jannchie@gmail.com) and [Claude Opus 4.7 (1M context)](mailto:noreply@anthropic.com) in [0b435c8](https://github.com/Jannchie/arthash/commit/0b435c8)
- **docs**: update readme with new icon and layout - By [Jianqi Pan](mailto:jannchie@gmail.com) in [59da81c](https://github.com/Jannchie/arthash/commit/59da81c)

## v0.1.2

[v0.1.1...v0.1.2](https://github.com/Jannchie/arthash/compare/v0.1.1...v0.1.2)

### :adhesive_bandage: Fixes

- **ci**: work around pnpm 9 --filter pack/publish bug - By [Jianqi Pan](mailto:jannchie@gmail.com) and [Claude Opus 4.7 (1M context)](mailto:noreply@anthropic.com) in [e74fbe3](https://github.com/Jannchie/arthash/commit/e74fbe3)

## v0.1.1

[py-v0.1.0...v0.1.1](https://github.com/Jannchie/arthash/compare/py-v0.1.0...v0.1.1)

### :sparkles: Features

- **docs**: update README and SPEC for new shape modes - By [Jianqi Pan](mailto:jannchie@gmail.com) in [619c87d](https://github.com/Jannchie/arthash/commit/619c87d)
- **release**: add bump-version and check-versions scripts - By [Jianqi Pan](mailto:jannchie@gmail.com) and [Claude Opus 4.7 (1M context)](mailto:noreply@anthropic.com) in [e354818](https://github.com/Jannchie/arthash/commit/e354818)
- **workflow**: add ci workflows for npm and crates publishing - By [Jianqi Pan](mailto:jannchie@gmail.com) in [ecc917c](https://github.com/Jannchie/arthash/commit/ecc917c)

### :lipstick: Styles

- **styles**: remove transition from encoding progress fill - By [Jianqi Pan](mailto:jannchie@gmail.com) in [796639f](https://github.com/Jannchie/arthash/commit/796639f)

### :wrench: Chores

- **ci**: update base url in workflow && remove sqip-bench package lock - By [Jianqi Pan](mailto:jannchie@gmail.com) in [3b2dcab](https://github.com/Jannchie/arthash/commit/3b2dcab)
- **ci**: update node and pnpm actions - By [Jianqi Pan](mailto:jannchie@gmail.com) in [083822f](https://github.com/Jannchie/arthash/commit/083822f)

## py-v0.1.0

[75421de685c7d026ca80f91dcf1feddc6322d9e7...py-v0.1.0](https://github.com/Jannchie/arthash/compare/75421de685c7d026ca80f91dcf1feddc6322d9e7...py-v0.1.0)

### :sparkles: Features

- **bench**: add hill-climb benchmarking and performance optimizations - By [Jianqi Pan](mailto:jannchie@gmail.com) in [39a3dac](https://github.com/Jannchie/arthash/commit/39a3dac)
- **canvas**: add blurring support for canvas element - By [Jianqi Pan](mailto:jannchie@gmail.com) in [ed32176](https://github.com/Jannchie/arthash/commit/ed32176)
- **gallery**: add encoding progress UI for image gallery - By [Jianqi Pan](mailto:jannchie@gmail.com) in [00f84c7](https://github.com/Jannchie/arthash/commit/00f84c7)
- **optimization**: add residual-driven init and drift-free Gaussian step - By [Jianqi Pan](mailto:jannchie@gmail.com) in [1d3203b](https://github.com/Jannchie/arthash/commit/1d3203b)
- **shapes**: add square and rotated rectangle support - By [Jianqi Pan](mailto:jannchie@gmail.com) in [a035086](https://github.com/Jannchie/arthash/commit/a035086)

### :adhesive_bandage: Fixes

- **py**: sync DEFAULT_SEARCH n_random with Rust default, regenerate test vectors - By [Jianqi Pan](mailto:jannchie@gmail.com) in [9fe1969](https://github.com/Jannchie/arthash/commit/9fe1969)

### :memo: Documentation

- update readme with project highlights and benchmarks - By [Jianqi Pan](mailto:jannchie@gmail.com) in [461b41d](https://github.com/Jannchie/arthash/commit/461b41d)

### :construction_worker: CI

- **github-actions**: add github actions for playground deployment - By [Jianqi Pan](mailto:jannchie@gmail.com) in [eb8c15c](https://github.com/Jannchie/arthash/commit/eb8c15c)

### :wrench: Chores

- **ci**: update workflow for arthash package - By [Jianqi Pan](mailto:jannchie@gmail.com) in [ee4dfca](https://github.com/Jannchie/arthash/commit/ee4dfca)
- **deps**: update lock files - By [Jianqi Pan](mailto:jannchie@gmail.com) in [be900c6](https://github.com/Jannchie/arthash/commit/be900c6)
- **modules**: rename @arthash/ts to arthash across the project - By [Jianqi Pan](mailto:jannchie@gmail.com) in [a6e1e1d](https://github.com/Jannchie/arthash/commit/a6e1e1d)

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

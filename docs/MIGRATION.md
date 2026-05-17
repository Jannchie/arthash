# Migration: arthash 0.1 → 0.2

The 0.2 release reshapes the public API around ease of use. The wire format
(`docs/SPEC.md`) is **unchanged** — hashes produced by 0.1 still decode under
0.2 codecs, byte for byte. Only the SDK surfaces changed.

If you only used `Codec::default()` / `Codec()` with no field overrides, no
changes are required.

---

## Rust

### `Codec` is now a tagged enum

`Codec` was a struct with twelve fields, many of which only applied to certain
shapes. It is now an enum, with one variant per mode.

```rust
// 0.1
let codec = Codec {
    shape: ShapeType::Triangle,
    n_shapes: 64,
    ..Codec::default()
};

// 0.2
let codec = Codec::triangle(64);
```

Factories: `Codec::dct()`, `Codec::circle(n)`, `Codec::triangle(n)`,
`Codec::square(n)`, `Codec::rect(n)`, `Codec::rotated_rect(n)`,
`Codec::pixel(n)`. Each variant carries only the fields that actually apply
to its mode.

Builders for the remaining knobs:

```rust
codec.with_color(ColorMode::Rgb888)         // 24-bit color
codec.with_palette(Palette::from_rgb(&...)) // palette mode
codec.with_theta_bits(7)                    // RotatedRect only
codec.with_grid_aspect(1.5)                 // Pixel only
```

### `decode` returns a named struct

```rust
// 0.1
let (w, h, rgba) = decode(&hash, &codec, opts);

// 0.2
let out = decode(&hash, &codec, opts);
// out.width / out.height / out.rgba
```

### `ShapeType` is hidden

`ShapeType` is now `#[doc(hidden)]` and used only inside the bit-layout
plumbing. Test/FFI code that needs SPEC-level field control should construct
a `CodecConfig` and wrap with `Codec::Raw(...)`. Normal users should never
need either.

### Codec equality + presets

```rust
assert_eq!(Codec::triangle(64), Codec::triangle(64));        // PartialEq
let placeholder = Preset::DetailTriangle.codec();             // named recipes
for p in Preset::all() { ... }                                // iterate
```

`Codec::is_byte_compatible_with(other)` is a looser check that ignores
construction style (e.g. `Codec::triangle(64)` matches the equivalent
`Codec::Raw(...)`).

### Convenience image loading (feature `image-io`)

```rust
let hash = arthash::encode_image("photo.jpg", &codec, EncodeOptions::default())?;
```

Replaces the manual "load + resize-to-thumbnail + `encode_rgb`" three-liner.

---

## Python

### `decode` always returns RGBA ndarray

The old API branched return types by codec:

```python
# 0.1
w, h, rgba_bytes = decode(h, dct_codec)        # raw bytes
w, h, rgb_array  = decode(h, shape_codec)      # (h, w, 3) ndarray, no alpha
```

That meant downstream code had to special-case codec mode. 0.2 always returns
a `(h, w, 4)` uint8 RGBA ndarray:

```python
# 0.2
w, h, rgba = decode(h, any_codec)
assert rgba.shape == (h, w, 4)
```

Alpha is `255` for every codec mode except DCT-with-alpha.

### `aa` parameter removed (was non-functional)

The old `decode(..., aa=True)` was advertised but never reached the Rust core.
It now actually does anti-aliased supersampling:

```python
decode(h, codec, base_size=512, aa=2)   # 4 samples per output pixel
```

(default `aa=1` matches the historical actual behavior.)

### `target_size` is no longer silently ignored for shape codecs

The old API silently snapped `target_size` to the shape thumbnail (48) for
shape codecs, regardless of what you passed. 0.2 honors the argument — pass
`target_size=None` (the default) to get the codec-natural thumbnail
(`100` for DCT, `48` for shape modes).

### Factory methods + top-level shortcuts

```python
# Factory methods (recommended)
Codec.dct()
Codec.triangle(n=64)
Codec.triangle(n=64, palette=PICO8)
Codec.rotated_rect(n=12, theta_bits=7)

# Or use the top-level shortcuts for the most common slots
from arthash import detail_triangle, placeholder_circle
codec = detail_triangle()

# Or named presets
Codec.preset(Preset.DETAIL_TRIANGLE)
```

The old `Codec(shape=ShapeType.X, n_shapes=N, ...)` form still works (it's
the same dataclass) but the factories self-document what fields apply to
which mode.

### Serialization helpers

`Codec.to_dict()` / `Codec.from_dict()` are JSON-safe — useful when storing
codec metadata alongside hashes (e.g. as a `codec` column in a DB).

```python
codec_meta = json.dumps(codec.to_dict())
# … later, in another process …
codec = Codec.from_dict(json.loads(codec_meta))
```

### `is_byte_compatible_with`

`codec.is_byte_compatible_with(other)` — true when two codecs would decode
each other's hashes byte-for-byte. Useful for sanity-checking that encode/
decode sides hold equivalent codecs.

---

## TypeScript

### `await init()` is no longer required

```ts
// 0.1
await init();
const hash = encode(rgb, w, h, opts);

// 0.2 — wasm auto-loads on first call
const hash = await encode(rgb, w, h, codec);
```

If you need synchronous variants (e.g. inside a tight render loop), call
`await init()` once and use `encodeSync` / `decodeSync` / `toSvgSync`.

### Codec is a discriminated union

```ts
// 0.1 — codec fields scattered into EncodeOptions / DecodeOptions
const opts = { shape: Shape.TRIANGLE, nShapes: 64 };
const hash = encode(rgb, w, h, opts);
const { rgba } = decode(hash, opts);  // had to repeat the same fields

// 0.2 — single Codec value passed to both sides
import { codec } from "arthash";
const c = codec.triangle({ n: 64 });
const hash = await encode(rgb, w, h, c);
const { rgba } = await decode(hash, c);
```

Factories: `codec.dct()`, `codec.circle({ n })`, `codec.triangle({ n })`,
`codec.square({ n })`, `codec.rect({ n })`, `codec.rotatedRect({ n })`,
`codec.pixel({ n })`. Use `codec.preset(Preset.X)` for named recipes,
`codec.raw(spec)` for fully-explicit SPEC field control.

### New APIs

```ts
// Palette construction
palette.fromHex(["#000000", "#ffffff", /* …14 more for K=16 */])
palette.fromRgb([[0,0,0], [255,255,255], /* … */])

// Helpers
codec.bytesTotal(c)      // hash byte length without encoding
codec.isPaletteMode(c)
codec.withPalette(c, p)  // swap to palette indexing

// Browser convenience
const hash = await encodeImage(imageUrlOrBlob, c)
```

### `encodeRgba` is exposed

Previously, only `encode` (RGB) was wired into TS. 0.2 surfaces `encodeRgba`
matching the Rust/Python SDKs — shape codecs composite alpha over white before
encoding.

### Search options exposed

`encode(rgb, w, h, codec, { seed: 0, search: { nRandom: 100, ... } })` — the
hill-climb tunables were already on the Rust side but missing in TS. Now
matches PyO3 binding.

---

## CLI

The `arthash` CLI flags didn't change; only internal types did. Existing
shell scripts continue to work.

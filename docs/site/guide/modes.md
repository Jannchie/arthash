# Modes & Codecs

arthash has seven codec modes. Each picks a different trade-off between byte
budget, encode cost, and visual style.

## Mode summary

| Mode           | Look                               | n=12 bytes | n=64 bytes | SVG out |
| -------------- | ---------------------------------- | ---------: | ---------: | :-----: |
| `DCT`          | blurry thumbnail (thumbhash-style) |      17–24 |          — |    ✗    |
| `PIXEL`        | palette pixel mosaic               |         25 |        129 |    ✗    |
| `CIRCLE`       | overlapping circles                |         53 |        267 |    ✓    |
| `SQUARE`       | axis-aligned squares               |         53 |        267 |    ✓    |
| `RECT`         | axis-aligned rectangles            |         59 |        299 |    ✓    |
| `ROTATED_RECT` | rotated rectangles                 |         66 |        339 |    ✓    |
| `TRIANGLE`     | triangle mosaic                    |         77 |        395 |    ✓    |

`toSvg` works for `circle` / `triangle` / `square` / `rect` / `rotrect`. `DCT`
and `PIXEL` have no natural SVG primitive form and throw.

## Named presets

The fastest way to pick a codec — these are the playground defaults, ordered by
byte budget.

| Preset           | Mode     | n   | Approx. bytes |
| ---------------- | -------- | --- | ------------: |
| `Dct`            | DCT      | —   |          ~21  |
| `SmallCircle`    | CIRCLE   | 12  |            53 |
| `SmallSquare`    | SQUARE   | 12  |            53 |
| `SmallRect`      | RECT     | 12  |            59 |
| `SmallTriangle`  | TRIANGLE | 12  |            77 |
| `SmallPixel`     | PIXEL    | 16  |            33 |
| `MediumCircle`   | CIRCLE   | 24  |           102 |
| `MediumSquare`   | SQUARE   | 24  |           102 |
| `MediumRect`     | RECT     | 24  |           114 |
| `MediumTriangle` | TRIANGLE | 24  |           150 |
| `MediumPixel`    | PIXEL    | 24  |            49 |
| `LargeCircle`    | CIRCLE   | 64  |           267 |
| `LargeSquare`    | SQUARE   | 64  |           267 |
| `LargeRect`      | RECT     | 64  |           299 |
| `LargeTriangle`  | TRIANGLE | 64  |           395 |
| `LargePixel`     | PIXEL    | 64  |           129 |

Pre-0.3 names (`TinyDct` / `Placeholder*` / `Detail*`) are kept as deprecated
aliases — see [`docs/MIGRATION.md`](https://github.com/Jannchie/arthash/blob/main/docs/MIGRATION.md#02--03).

```ts
codec.preset(Preset.MediumTriangle)
```

```python
Codec.preset(Preset.MEDIUM_TRIANGLE)
```

```rust
Preset::MediumTriangle.codec()
```

## Factory builders

When you want explicit control over `n` and colour mode:

::: code-group

```ts [TypeScript]
codec.dct()
codec.circle({ n: 12 })
codec.triangle({ n: 64 })
codec.square({ n: 12 })
codec.rect({ n: 12 })
codec.rotatedRect({ n: 12, thetaBits: 5 })
codec.pixel({ n: 16 })

// Wrap any shape codec in a palette:
codec.withPalette(codec.triangle({ n: 24 }), { bytes: paletteBytes })

// Low-level escape hatch for conformance tests:
codec.raw({ shape: "triangle", nShapes: 12, alphaBits: 2, colorBits: 24 })
```

```python [Python]
Codec.dct()
Codec.circle(n=12)
Codec.triangle(n=64)
Codec.square(n=12)
Codec.rect(n=12)
Codec.rotated_rect(n=12, theta_bits=5)
Codec.pixel(n=16)

# Palette mode
Codec.triangle(n=24, palette=PICO8)
```

```rust [Rust]
Codec::dct();
Codec::circle(12);
Codec::triangle(64);
Codec::square(12);
Codec::rect(12);
Codec::rotated_rect(12);
Codec::pixel(16);
```

:::

## Choosing a byte budget

| Need                                | Recommendation              |
| ----------------------------------- | --------------------------- |
| Inline placeholder in CSS / HTML    | DCT (~21 B) or PIXEL n=16   |
| Email-friendly LQIP                 | TRIANGLE n=12 (77 B)        |
| Above-the-fold hero placeholder     | TRIANGLE n=24 (~150 B)      |
| Detail-faithful gallery thumbnails  | TRIANGLE n=64 (~400 B)      |

Lower `n` to shrink the hash; raise it to recover detail. The relationship is
roughly linear in `n`.

## Colour modes

By default shape modes store colour as **RGB-565** (16 bit per shape). Two
alternatives are available:

| Mode      | Bits / shape colour | When to use                                                                |
| --------- | ------------------- | -------------------------------------------------------------------------- |
| `rgb565`  | 16 bit              | Default — strong perceptual quality, half the size of RGB-888              |
| `rgb888`  | 24 bit              | High-fidelity reproduction when bytes are not the constraint               |
| `palette` | `log₂(K)` bit       | K=16 → 4 bit per colour; gives a consistent brand / retro look             |

Switching to a palette is one call:

```ts
import { codec, palette } from "arthash";

const brand = palette.fromHex(["#0a0a0a", "#ffffff", "#0284c7", "#22c55e", ...]); // K must be power of 2
const c = codec.withPalette(codec.triangle({ n: 24 }), brand);
```

```python
from arthash.palettes import PICO8
codec = Codec.triangle(n=24, palette=PICO8)
```

See [Palettes](./palettes) for built-in palettes and design tips.

## Bytes-total formula

The hash length is fully determined by the codec — no header, no length prefix.

```text
bytes = ceil((header_bits + n_shapes × per_shape_bits) / 8)
```

| Shape          | `per_shape_bits` (with colour `C` and alpha `A`) |
| -------------- | ------------------------------------------------ |
| `CIRCLE`       | `cx + cy + r + C + A`                            |
| `SQUARE`       | `cx + cy + r + C + A`                            |
| `RECT`         | `cx + cy + 2·r + C + A`                          |
| `ROTATED_RECT` | `cx + cy + 2·r + θ + C + A`                      |
| `TRIANGLE`     | `3·(cx + cy) + C + A`                            |
| `PIXEL`        | `C` (geometry derived from aspect, not stored)   |

Defaults: `cx = cy = 5`, `r = 4`, `α = 3`, `θ = 5`, `C = 16` (or `log₂(K)` for
palette). The exact byte layout is pinned in
[`docs/SPEC.md`](https://github.com/Jannchie/arthash/blob/main/docs/SPEC.md).

To get the answer at runtime:

```ts
codec.bytesTotal(c)             // TS
```

```python
codec.bytes_total()             # Python
```

```rust
codec.bytes_total()             // Rust
```

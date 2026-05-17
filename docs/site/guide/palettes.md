# Palettes

Wrapping a shape codec in an external palette shrinks the per-shape colour
field from 16 / 24 bit to `log₂(K)` bit (K=16 → 4 bit), and stamps the entire
output with a consistent visual style — brand colours, retro game palettes,
Morandi tones, whatever you like.

## Why use a palette

| Without palette                  | With palette (K = 16)             |
| -------------------------------- | --------------------------------- |
| 16 bit per shape colour          | 4 bit per shape colour            |
| hash ~150 B at n=24              | hash ~114 B at n=24 (saves ~36 B) |
| Free-form RGB-565                | Visual cohesion across all images |
| Looks "computer generated"       | Looks like an intentional style   |

The encoder quantises each shape's colour to the nearest palette entry in Oklab
space, so the visual mapping is perceptually uniform rather than RGB-distance
naive.

## Constructing a palette

A palette is just a flat row-major `Uint8Array` of `K × 3` sRGB bytes where
`K ∈ {2, 4, 8, 16, 32, 64, 128, 256, 512, 1024}` (any power of two from 2 to
1024).

::: code-group

```ts [TypeScript]
import { palette, codec } from "arthash";

// From hex strings — easiest
const brand = palette.fromHex([
  "#0a0a0a", "#ffffff", "#0284c7", "#22c55e",
  "#f59e0b", "#ef4444", "#a855f7", "#14b8a6",
  "#1e293b", "#f8fafc", "#0369a1", "#16a34a",
  "#d97706", "#dc2626", "#9333ea", "#0d9488",
]);                                                  // K = 16

// From [r, g, b] triplets
const grayscale = palette.fromRgb([
  [0, 0, 0], [64, 64, 64], [128, 128, 128], [255, 255, 255],
]);                                                  // K = 4

// Wrap any shape codec
const c = codec.withPalette(codec.triangle({ n: 24 }), brand);
```

```python [Python]
from arthash import Codec
from arthash.palettes import PICO8, GAMEBOY, MORANDI

codec = Codec.triangle(n=24, palette=PICO8)

# Or build your own
from arthash import Palette
brand = Palette.from_hex([
    "#0a0a0a", "#ffffff", "#0284c7", "#22c55e",
    "#f59e0b", "#ef4444", "#a855f7", "#14b8a6",
    "#1e293b", "#f8fafc", "#0369a1", "#16a34a",
    "#d97706", "#dc2626", "#9333ea", "#0d9488",
])
codec = Codec.triangle(n=24, palette=brand)
```

:::

::: warning K must be a power of two
The bit-packing math assumes `K ∈ {2, 4, 8, …, 1024}`. Passing any other size
throws at codec construction time, not at encode time.
:::

## Built-in palettes

The TypeScript and Python SDKs ship a small set of well-known palettes:

| Name       | K  | Style                              |
| ---------- | -- | ---------------------------------- |
| `PICO8`    | 16 | PICO-8 console palette             |
| `GAMEBOY`  | 4  | Original Game Boy DMG green        |
| `NES`      | 64 | NES Famicom palette                |
| `MORANDI`  | 16 | Muted Morandi-style neutral tones  |
| `MONO`     | 4  | Black / dark grey / light grey / white |

::: code-group

```ts [TypeScript]
import { palettes, codec } from "arthash";
const c = codec.withPalette(codec.triangle({ n: 24 }), palettes.PICO8);
```

```python [Python]
from arthash.palettes import PICO8
codec = Codec.triangle(n=24, palette=PICO8)
```

:::

## Design tips

- **K = 16 is the sweet spot.** 4 bits per shape, big enough to cover a
  meaningful style range, small enough to stay distinctive.
- **Include extremes.** A good palette has at least one near-black, one
  near-white, and reasonably saturated mid-tones. Without them the encoder has
  no high-contrast option for sharp boundaries.
- **Cluster perceptually, not in RGB.** Picking 16 evenly-spaced RGB values
  looks washed out. Tools like [Coolors](https://coolors.co/) or
  [Lospec](https://lospec.com/palette-list/) curate palettes that work.
- **The same palette must be present at decode.** It's part of the `Codec`
  contract and **is not stored inside the hash byte stream**. Share it as a
  constant in both encoder and decoder. If the palette varies per image and
  must travel with the hash, design your own outer frame (e.g. prepend the
  palette bytes) — the SDK does not ship a built-in packaging format for this.

## Mixing palettes across modes

Palette colour applies to every shape mode (`CIRCLE`, `SQUARE`, `RECT`,
`ROTATED_RECT`, `TRIANGLE`) and to `PIXEL`. `DCT` ignores the palette — it
quantises in Oklab DCT-coefficient space and has no per-pixel colour to remap.

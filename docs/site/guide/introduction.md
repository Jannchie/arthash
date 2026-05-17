# Introduction

**arthash** is a placeholder-image hash family: a tiny byte string (17 B to 400 B)
that decodes back into a recognisable preview of the original image. You use it
the same way you would use [blurhash](https://blurha.sh/),
[thumbhash](https://evanw.github.io/thumbhash/), or
[sqip](https://github.com/axe312ger/sqip) — store the hash next to the image
URL, render the preview instantly while the real image streams in.

The core is written in Rust. Python and TypeScript share that same Rust code
via a PyO3 wheel and a wasm-bindgen package respectively. A hash produced by
any binding decodes on any other.

## What it replaces

| If you use            | Switch to arthash's        | Main wins                                                                |
| --------------------- | -------------------------- | ------------------------------------------------------------------------ |
| blurhash / thumbhash  | `DCT` mode                 | same byte budget, +0.4 dB PSNR; JS encode 1.9× / decode 1.4×             |
| sqip (primitive only) | `TRIANGLE` / `CIRCLE` mode | 1/9 – 1/16 the size; 50–67× faster encode; runs natively in browser wasm |

Shape / `PIXEL` modes can also take an external palette, dropping per-shape colour
to 4 bit and giving the output a natural visual style (brand palette, retro,
Morandi, ...).

## When to pick which mode

| Goal                                  | Pick                                                            |
| ------------------------------------- | --------------------------------------------------------------- |
| Smallest possible bytes, blurry look  | `DCT` (≤ 24 B)                                                  |
| Sharp shapes, SVG preview             | `TRIANGLE` / `CIRCLE` / `RECT` (50 – 400 B depending on `n`)    |
| Retro / brand-consistent placeholders | any shape mode + palette (drops colour to 4 bit)                |
| Pixel-art mosaic look                 | `PIXEL` (25 – 130 B)                                            |

The playground default — `TRIANGLE n=64 / baseSize 512 / RGB-565` — is a
reasonable starting point. Lower `n` if you need smaller bytes; switch to
`DCT` if you want the absolute minimum.

## Relation to thumbhash and sqip

**thumbhash** (Evan Wallace, 2023) is the successor to blurhash — same idea
(DCT-encoded blurry thumbnail) with a tighter bit layout, ~24 bytes per image,
pure JS. arthash's `DCT` mode targets the same niche.

**sqip** (Tobias Baldauf, 2017) is a Node plugin framework. Its most-used
plugin (`sqip-plugin-primitive`) shells out to the Go
[`primitive`](https://github.com/fogleman/primitive) binary to hill-climb N
geometric shapes onto the image and writes an SVG. Typical use is build-time
generation, inlined into HTML as an LQIP. arthash's shape modes target the
primitive part of sqip — but they run natively in browser wasm, so you can
also encode at request time.

### Feature comparison

| Feature                           |     arthash     |        thumbhash         |          sqip           |
| --------------------------------- | :-------------: | :----------------------: | :---------------------: |
| DCT blurry thumbnail (17–24 B)    |        ✅        |            ✅             |            ❌            |
| Geometric SVG primitives          |   ✅ 5 shapes    |            ❌             |   ✅ multiple plugins    |
| Pixel mosaic                      |        ✅        |            ❌             |            ❌            |
| External palette (colour → 4 bit) |        ✅        |            ❌             |            ❌            |
| Potrace-style SVG tracing         |        ❌        |            ❌             | ✅ `sqip-plugin-potrace` |
| WebP output                       |        ❌        |            ❌             |   ✅ via some plugins    |
| Decode to arbitrary output size   |        ✅        |     ⚠️ default ~32 px     |     ✅ (SVG, vector)     |
| Web / browser wasm                |        ✅        |        ✅ pure JS         | ❌ (needs Go subprocess) |
| Python binding                    |  ✅ PyO3 wheel   | ⚠️ pure-Python 80× slower |            ❌            |
| Rust crate                        |        ✅        |            ✅             |            ❌            |
| Deployment                        | request / build |     request / build      |     build-time only     |

arthash **does not** cover sqip's Potrace tracing mode (bitmap contour → SVG
path), nor does it produce WebP / data-URI output. If you need those, sqip is
still the better fit.

## How arthash saves bits

Every bit goes to image data; no header overhead. The savings stack in four
layers:

1. **No header — two-sided consensus codec.** The byte stream is not
   self-describing: no magic number, no mode tag, no bit-widths. Mode, shape
   count, quantization bit widths, palette are all in the `Codec`, configured
   on both encode and decode.
2. **Bit packing, last byte zero-padded.** LSB-first bit packing. Hash length
   is fully determined by the codec.
3. **DCT mode — frequency-domain + perceptual-space squeeze.** Oklab quant,
   `AB_SCALE = 5`, signed-power compander, triangular high-frequency mask,
   per-channel adaptive AC scale.
4. **Shape / PIXEL — frugal geometry and colour.** Log-scale radius
   quantisation, optional RGB-565 or palette-indexed colour, discrete alpha
   levels, π-symmetric theta with half-step bias.

The byte format is pinned in [`docs/SPEC.md`](https://github.com/Jannchie/arthash/blob/main/docs/SPEC.md).

## Next steps

- [**Installation**](./installation) — get the SDK for your language.
- [**Basic Usage**](./basic-usage) — encode, decode, render SVG.
- [**Modes & Codecs**](./modes) — choose the right mode for your byte budget.

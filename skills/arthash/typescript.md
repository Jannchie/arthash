---
name: arthash-typescript
description: arthash TypeScript / JavaScript SDK — wasm-bindgen wrapper. Async by default, sync variants after `await init()`. Runs in browsers and in Node ≥ 22.
---

# arthash — TypeScript / JavaScript SDK

`npm i arthash` (also works with pnpm / yarn / bun). The package is a thin TypeScript wrapper around a wasm core built from `arthash-rs`.

## Runtime requirements

- **Browsers**: any evergreen browser with WebAssembly + ES2022 (Chrome ≥ 110, Safari ≥ 16, Firefox ≥ 110).
- **Node**: **Node ≥ 22**. Node 20 went out of LTS support in 2026 and is not covered by CI; Node 22+ has the `globalThis.crypto`, `fetch`, and stable WebAssembly streaming APIs the SDK relies on.
- **Bundlers**: works out of the box with Vite, webpack 5, Rspack, esbuild. The wasm is loaded with `WebAssembly.instantiateStreaming` against a relative URL — make sure your bundler emits the `.wasm` asset alongside the JS.

## Recommended starting point

Start with the default rectangle codec — small bytes, SVG output, no palette setup:

```ts
import { encodeImage, decode, toSvg, codec } from "arthash";

const c = codec.rect({ n: 24 });                  // ~119 B, recognisable rectangle mosaic
const hash = await encodeImage(fileOrUrl, c);     // accepts URL / Blob / File / HTMLImageElement
container.innerHTML = await toSvg(hash, c, { baseSize: 512, blur: 4 });
```

`codec.rect({ n: 24 })` is a good default for most placeholder slots: bytes stay under 120, SVG output inlines cleanly into SSR HTML, and you don't need a palette to look presentable. Scale `n` up (48 / 64) for hero images, down (12) when you want to match a thumbhash-like 60-byte budget.

## End-to-end — encode, store, decode

```ts
import { encode, encodeImage, decode, toSvg, codec, init } from "arthash";

// Optional: preload the wasm so the first call is sync-fast.
// await init();

const c = codec.rect({ n: 24 });

// Encode from a user-uploaded File / a fetched Blob / an <img> URL:
const hashBytes: Uint8Array = await encodeImage(fileOrUrl, c);

// Persist as base64 in a regular text column:
const stored = btoa(String.fromCharCode(...hashBytes));

// Later: decode the placeholder to a raster preview at any size.
const restored = Uint8Array.from(atob(stored), ch => ch.charCodeAt(0));
const { w, h, rgba } = await decode(restored, c, { baseSize: 512 });
// rgba is Uint8ClampedArray (length 4*w*h) — paint to canvas / put on a 2D context.

// Or render the same codec as an inline SVG (shape modes only):
const svgString = await toSvg(restored, c, { baseSize: 512, blur: 4 });
container.innerHTML = svgString;
```

Low-level encode if you already have raw RGB bytes at the encoder's expected thumbnail size:

```ts
const hash = await encode(rgbBytes, width, height, c);  // expects width/height ≤ 48 for shape modes
```

## Codec factories

```ts
codec.rect({ n: 24 })            // axis-aligned rectangles — recommended default
codec.square({ n: 24 })          // squares (rect with side = side)
codec.rotatedRect({ n: 24 })     // rotated rectangles
codec.triangle({ n: 24 })        // triangle mosaic
codec.circle({ n: 24 })          // overlapping circles
codec.pixel({ n: 16 })           // palette pixel mosaic
codec.dct()                      // blurry frequency-domain placeholder — see below
codec.preset(Preset.MediumRect)  // (or any other named preset)
```

DCT mode is intentionally not the default — reach for it only when you specifically want a **blurry, blurhash/thumbhash-style** look at the smallest possible byte budget. It cannot output SVG.

## Palettes

```ts
import { codec, palettes } from "arthash";

const c = codec.rect({ n: 24 });
const pc = codec.withPalette(c, { bytes: palettes.PICO8 });   // 4-bit colour per rect
const hash = await encodeImage(file, pc);
```

Custom palettes: pass a flat byte array of `K` RGB triplets where `K ∈ {2, 4, 8, 16, 32}`. The decoder MUST receive the same palette bytes — bundle them alongside the hash or use one of the named palettes (`PICO8`, `NES`, `GAMEBOY`, `MORANDI`, …) which are stable across versions.

## Sync vs async

All the top-level entry points are async because the wasm module loads lazily on first call:

```ts
const hash = await encode(rgbBytes, w, h, c);
```

For hot paths, preload once and use the `*Sync` variants:

```ts
import { init, encodeSync, decodeSync, toSvgSync } from "arthash";
await init();                                  // wait once at startup
const hash = encodeSync(rgbBytes, w, h, c);    // no per-call await overhead
```

Calling `*Sync` before `await init()` resolves throws.

## Bundle footprint

- wasm core: ~67 KB brotli / ~93 KB gzip on first load, HTTP-cached afterwards.
- SDK on top of the wasm: tree-shakes to ~6 KB if you only import `decode` (+ `init` for preload). Drop `encode` / `toSvg` / `encodeImage` from your imports to shave the encoder out of the JS surface — the wasm itself is monolithic though, see the next note.
- The wasm bundle is **not** split per mode: it ships every codec and the hill-climb encoder regardless of which you import. A decode-only wasm build would shave ~15–20 KB brotli; open an issue at github.com/Jannchie/arthash if you need that for a JS-bundle-sensitive app.

## Server-side usage in Node

`encodeImage` in Node falls back to `sharp` for resize. If you only call `encode` (with pre-thumbnailed RGB bytes) or `decode`, you don't need any native dependencies.

```ts
import sharp from "sharp";
import { encode, codec } from "arthash";

const c = codec.rect({ n: 24 });
const { data, info } = await sharp("photo.jpg")
  .resize({ fit: "inside", width: 48, height: 48 })
  .raw()
  .toBuffer({ resolveWithObject: true });
const hash = await encode(new Uint8Array(data.buffer), info.width, info.height, c);
```

## Common TS-side pitfalls

- **Encoding a 4K image with `encode` instead of `encodeImage`.** `encode` does not resize; you'll pay orders of magnitude more time for the same output. Either resize first or use `encodeImage` (which resizes via OffscreenCanvas / sharp).
- **Decoding with a different codec than was used to encode.** The bytes are not self-describing — you must pair `(hash, codec)`. Store the codec discriminator alongside the hash if you support more than one.
- **Calling `toSvg` on a `DCT` or `PIXEL` hash.** Throws — both modes are inherently raster.
- **Forgetting `await init()` before `*Sync` calls.** They throw if the wasm isn't loaded yet.
- **Targeting Node ≤ 20.** Unsupported — upgrade to Node ≥ 22.

---
layout: home

hero:
  name: arthash
  text: A compact placeholder-image hash
  tagline: 17 B to 400 B per image — enough to render a recognisable preview while the real image loads. Rust core, Python and TypeScript bindings, same byte format on all sides.
  actions:
    - theme: brand
      text: Get Started
      link: /guide/introduction
    - theme: alt
      text: View on GitHub
      link: https://github.com/Jannchie/arthash

features:
  - icon: 🔤
    title: Tiny hashes, sharp previews
    details: 17 B DCT placeholder beats thumbhash by +0.4 dB PSNR. Shape modes deliver SVG previews at 1/9 – 1/16 the size of sqip output.
  - icon: ⚡
    title: Fast on every runtime
    details: JS encode 1.9× / decode 1.4× vs thumbhash-js. Native Rust + PyO3 are 5.9× / 4.7× faster than thumbhash-go on display-sized decode.
  - icon: 🎨
    title: 7 codec modes
    details: DCT, PIXEL, CIRCLE, SQUARE, RECT, ROTATED_RECT, TRIANGLE. Optional external palettes shrink colour to 4 bit and stamp a visual style.
  - icon: 📦
    title: One shared spec
    details: Hashes produced by the Rust crate, the PyO3 wheel, or the wasm-bindgen package decode bit-for-bit identically on every binding.
  - icon: 🌐
    title: Browser ready
    details: ~67 KB brotli of wasm on first load (HTTP-cached afterwards), ~6 KB of SDK in your bundle. Encode at request time, no Node subprocess.
  - icon: 🔧
    title: Headerless byte stream
    details: No magic number, no mode tag. Every bit goes to image data — the Codec is the shared consensus between encode and decode.
---

## Quick taste

```ts
import { encode, decode, toSvg, codec, Preset } from "arthash";

const c = codec.preset(Preset.DetailTriangle);   // triangle, n=64
const hash = await encode(rgbBytes, width, height, c);
//   → Uint8Array(395)  — your full image as a 395-byte blob

const svg = await toSvg(hash, c, { baseSize: 512, blur: 8 });
//   → '<svg xmlns="..." viewBox="...">...</svg>'  — drop in as LQIP
```

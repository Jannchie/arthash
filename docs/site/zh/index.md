---
layout: home

hero:
  name: arthash
  text: 紧凑的占位图哈希
  tagline: 17 B 到 400 B 描述一张图，足以渲染出可辨识的占位图，等真图加载完再替换。核心用 Rust 写，Python / TypeScript 共用同一份代码，跨绑定字节互通。
  actions:
    - theme: brand
      text: 开始使用
      link: /zh/guide/introduction
    - theme: alt
      text: GitHub
      link: https://github.com/Jannchie/arthash

features:
  - icon: 🔤
    title: 字节小，画面清晰
    details: 17 B 的 DCT 占位图比 thumbhash 高 0.4 dB PSNR；形状模式输出的 SVG 比 sqip 小 1/9 – 1/16。
  - icon: ⚡
    title: 全平台高性能
    details: JS 端 encode 1.9× / decode 1.4× 于 thumbhash-js；原生 Rust + PyO3 在显示尺寸 decode 上比 thumbhash-go 快 5.9× / 4.7×。
  - icon: 🎨
    title: 7 种 codec 模式
    details: DCT、PIXEL、CIRCLE、SQUARE、RECT、ROTATED_RECT、TRIANGLE；外部调色板可把颜色压到 4 bit，并赋予一致的视觉风格。
  - icon: 📦
    title: 一套字节规范
    details: Rust crate、PyO3 wheel、wasm-bindgen 包产出的 hash 字节级互通，任何一端编码都能在其他端解码。
  - icon: 🌐
    title: 浏览器就绪
    details: wasm 核心首次加载 ~67 KB brotli（HTTP 缓存后免下载），SDK 进 bundle ~6 KB；浏览器内即可请求期实时编码。
  - icon: 🔧
    title: 无 header 字节流
    details: 没有 magic number，没有模式标签。每一个 bit 都是图像信息——Codec 是 encode 与 decode 之间的两端共识。
---

## 30 秒体验

```ts
import { encode, decode, toSvg, codec, Preset } from "arthash";

const c = codec.preset(Preset.LargeTriangle);    // triangle, n=64
const hash = await encode(rgbBytes, width, height, c);
//   → Uint8Array(395)  — 整张图被压成 395 字节

const svg = await toSvg(hash, c, { baseSize: 512, blur: 8 });
//   → '<svg xmlns="..." viewBox="...">...</svg>'  — 直接当 LQIP 内联
```

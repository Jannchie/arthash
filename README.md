# arthash

**用 17 B 到 400 B 描述一张图，足以渲染出一张可辨识的占位图，等真图加载完再替换掉。**

核心用 Rust 写，**Python 和 TypeScript 是同一份 Rust 代码的薄绑定**（分别走 PyO3 wheel 和 wasm-bindgen）。任何一端编出来的 hash，任何一端都能解。

> **DCT 模式是 blurhash / thumbhash 的优化平替：相同甚至更少的字节数，PSNR 持平或更高（17 B 时 +0.4 dB），同时支持解码到任意显示尺寸**——直接迁移过来就行，无需调整前端的尺寸预算。

## 为什么选 arthash

- **DCT：blurhash / thumbhash 的直接替代** —— 同样 17 字节，arthash DCT 比 thumbhash 高 0.4 dB PSNR，比 blurhash（~30 B）字节更少；并且解码支持任意输出尺寸，省掉前端 CSS 上采样。
- **TRIANGLE / SQIP 风格 SVG 模式** —— n=64 约 395 字节即可逼近 sqip 12-原语 SVG（~1 KB）的视觉效果，体积只有 sqip 的 **2/5**。
- **更快（浏览器）** —— wasm 直接跑，不需要 Node 原生模块。**vs thumbhash JS**：解码默认只输出 ~32 px 缩略图、靠 CSS 上采样；arthash 直接解码到显示尺寸，原生侧同尺寸对比快 6×（见下方表）。**vs sqip**：sqip 每次调用都要 spawn Go primitive 二进制，**根本不能在浏览器里跑**。
- **形态多** —— 一套 Codec API 切七种模式：DCT 模糊缩略图、CIRCLE / TRIANGLE / SQUARE / RECT / ROTATED_RECT 五种 SVG 形状马赛克、PIXEL 复古调色板。
- **跨语言一致** —— 字节格式定在 [`docs/SPEC.md`](./docs/SPEC.md)，跨实现编出来的 hash 互相能解。

| 模式           | 视觉效果                             | n=12 字节 | n=64 字节 |
| -------------- | ------------------------------------ | --------: | --------: |
| `DCT`          | **blurhash / thumbhash 优化平替**    |     17–24 |         — |
| `CIRCLE`       | SQIP 风格叠加圆，输出 SVG            |        53 |       268 |
| `TRIANGLE`     | Primitive 风格三角形马赛克，输出 SVG |        77 |   **395** |
| `SQUARE`       | 轴对齐方块 (cx, cy, side)，输出 SVG  |        53 |       268 |
| `RECT`         | 轴对齐矩形 (cx, cy, w, h)，输出 SVG  |        59 |       299 |
| `ROTATED_RECT` | 旋转矩形 (+theta)，输出 SVG          |        66 |       339 |
| `PIXEL`        | 调色板像素马赛克                     |        25 |       129 |

> playground 默认 **TRIANGLE n=64 / baseSize 512 / RGB-565**，这是质量优先、占用约 400 字节的预设；想再省可以切 DCT (≤24 B) 或上调色板把颜色压到 4 bit。

## 怎么省 bit 的

arthash 把每一个 bit 都花在了图像信息上。具体做法分四层：

**1. 不要 header —— 两端共识的 Codec。**
hash 字节流本身不自描述，不带 magic number、不带模式标签、不带 bit width。模式、形状数、量化位宽、调色板都由 **Codec** 同时配给 encode 和 decode。换走"自描述"，换来"每一个 bit 都是图像信息"。

**2. 按位打包，最后一个字节零填充。**
LSB-first bit packing，hash 长度由 codec 完全决定（`ceil((header_bits + n_shapes × per_shape_bits) / 8)`），最多浪费 7 个填充 bit。

**3. DCT 模式 —— 频域 + 感知空间双重压榨。**
- **Oklab 而非 sRGB** —— 在感知均匀色空间里做量化，每 bit 的视觉差异更稳定。
- **AB_SCALE = 5** —— 把 a/b 通道的动态范围撑到和 L 通道一致，让共享的 4-bit AC 量化器一个码位都不浪费。
- **Compander（带符号幂函数）** —— L 用 `^0.6`、a/b 用 `^0.5`、α 用 `^0.6`，让信号在量化前的分布更接近均匀，4-bit nibble 用得更充分。
- **三角形掩码** —— 只存 `cx · ny < nx · (ny − cy)` 的左上角系数，对高频系数自然丢弃。
- **DC 6 bit、AC 4 bit、AC scale 4–5 bit** —— 每个通道独立选 scale，相当于给每张图自适应一份量化表。
- **`(lx, ly)` 由 aspect 推导** —— luma 网格的形状从 aspect_code 算出来，不进 hash。

**4. Shape / PIXEL 模式 —— 几何与颜色的精打细算。**
- **Log-scale 半径量化** —— 半径在 `[min(w,h)/24, max(w,h)]` 区间按 log 分桶，4 bit 就够覆盖人眼能区分的所有尺度。
- **RGB-565 vs RGB-888 可选** —— 默认 16 bit，每个形状省 8 bit；24 bit 留给高保真场景。
- **调色板模式** —— 颜色字段从 16/24 bit 压成 `log₂(K)` bit；K=16 时颜色只占 4 bit。
- **离散 alpha 等级** —— 3 bit 索引 8 档 alpha，默认 `linspace(0.20, 0.90, 8)`；过低的 alpha 给出的视觉差异不值这个码位，剪掉。
- **`theta ∈ [0, π)` 半步偏移** —— 旋转矩形 π-对称，theta 只占半圈；解码时 `+0.5` bias 落在桶中心，把量化误差减半。
- **8-bit aspect code 覆盖 1/8–8 的宽高比** —— log 空间均匀分布的 255 级，code 255 保留。
- **PIXEL 网格形状由 aspect 推导** —— `grid_w × grid_h = n_shapes`，网格形状从 aspect 求得，不进 hash。

字节格式定在 [`docs/SPEC.md`](./docs/SPEC.md)，越改越细一点都不会破坏向后兼容。

## Benchmarks

数据来源 [`docs/benchmarks/CROSS_IMPL.md`](./docs/benchmarks/CROSS_IMPL.md)，同一台机器、100×100 输入。

### 浏览器 / Node 场景

| 实现                                   |           encode (100×100) |          decode |                         字节 | 备注                                                     |
| -------------------------------------- | -------------------------: | --------------: | ---------------------------: | -------------------------------------------------------- |
| **arthash · ts (wasm)**                |             同 Rust 量级\* |  同 Rust 量级\* | 17–24 (DCT) / 53–395 (shape) | wasm-bindgen，浏览器直跑                                 |
| thumbhash · JS (npm `thumbhash@0.1.1`) |                     540 µs | 165 µs @ ~32 px |                           24 | 解码默认输出 ~32 px 缩略图，靠 CSS 拉伸                  |
| sqip · Node (`sqip@0.3` + `primitive`) | ≥ 100 ms (spawn Go 二进制) |               — |                    ~1 kB SVG | **不能在浏览器运行**，每次调用 spawn Go primitive 子进程 |

> \* arthash-ts 共用 Rust 核心；wasm 端没有独立 benchmark，但走的是同一份 SGEMM 解码路径。基线参考下方 Rust 数据，wasm 通常在原生 1.2–1.8× 之间。

### 原生（Rust / Python / Go）

**编码** (image → bytes, median)

| 实现                                     |   时间 | 字节 |
| ---------------------------------------- | -----: | ---: |
| thumbhash · Rust (`evanw/thumbhash`)     | 308 µs |   24 |
| thumbhash · Go (`go.n16f.net/thumbhash`) | 415 µs |   24 |
| **arthash · Rust**                       | 796 µs |   24 |
| **arthash · Python (PyO3)**              | 620 µs |   24 |
| thumbhash · Python (PyPI)                |  25 ms |   24 |

**解码到 256 长边** (bytes → RGBA)

| 实现                 |    时间 |
| -------------------- | ------: |
| **arthash · Rust**   | 2.06 ms |
| **arthash · Python** | 2.60 ms |
| thumbhash · Go @ 256 | 12.2 ms |

> thumbhash 的 Rust crate 在它自己的 **~32 px 默认输出尺寸** 下比 arthash 解码快；一旦要求显示尺寸缓冲（占位图实际场景），arthash 反超 **6×**。

### 画质对比 (256 长边, PSNR)

| 输出                      |    字节 |    PSNR |
| ------------------------- | ------: | ------: |
| sqip · 12 原语 SVG        | ~1100 B | 24.4 dB |
| **arthash · DCT**         |    17 B | 23.3 dB |
| thumbhash                 |    17 B | 22.9 dB |
| **arthash · TRIANGLE 12** |    77 B | 21.4 dB |
| **arthash · CIRCLE 12**   |    53 B | 20.7 dB |

## 快速上手

### Rust

```rust
use arthash::{Codec, encode_rgb, decode, EncodeOptions, DecodeOptions};
let codec = Codec::default();
let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
let (out_w, out_h, rgba) = decode(&hash, &codec, DecodeOptions {
    base_size: 256, ..Default::default()
});
```

### Python（PyO3 wheel）

```python
from arthash import Codec, ShapeType, encode, decode, to_svg

# DCT —— thumbhash 风格模糊占位图
hash_bytes = encode("photo.jpg")
w, h, rgba = decode(hash_bytes, base_size=256)

# Shape 模式 → SVG
codec = Codec(shape=ShapeType.TRIANGLE, n_shapes=64)
hash_bytes = encode("photo.jpg", codec, seed=0)
svg = to_svg(hash_bytes, codec, base_size=512, blur=8.0)
```

### TypeScript（wasm-bindgen，浏览器 / Node）

```ts
import { init, encode, decode, toSvg, Shape } from "arthash";

await init();  // ~70 KB gzip，可重复调用

const opts = { shape: Shape.TRIANGLE, nShapes: 64 };
const hash = encode(rgbBytes, width, height, opts);
const { w, h, rgba } = decode(hash, { ...opts, baseSize: 512 });
const svg = toSvg(hash, { ...opts, baseSize: 512, blur: 8 });
```

`Shape` 包含 `DCT` / `CIRCLE` / `TRIANGLE` / `SQUARE` / `RECT` / `ROTATED_RECT` / `PIXEL`，详见 [`packages/arthash-ts/README.md`](./packages/arthash-ts/README.md)。

## 仓库结构

```
packages/
├── arthash-rs/          Rust SDK（标准实现）
├── arthash-py/          Python SDK —— PyO3 binding
├── arthash-ts/          TypeScript SDK —— wasm-bindgen binding
└── arthash-playground/  Vue playground（TRIANGLE n=64 默认）

docs/
├── SPEC.md             字节格式权威定义
└── benchmarks/         RESULTS.md, CROSS_IMPL.md, NDJSON
```

## License

MIT.

<p align="center">
  <img src="./docs/site/public/icon.png" width="128" height="128" alt="arthash">
</p>

<h1 align="center">arthash</h1>

<p align="center">
  <a href="https://codetime.dev"><img src="https://shields.jannchie.com/endpoint?style=social&color=0284c7&url=https%3A%2F%2Fcodetime.dev%2Fv3%2Fusers%2Fshield%3Fuid%3D2%26tag%3Darthash" alt="CodeTime Badge"></a>
</p>

> [English](./README.md) · **中文**

用 17 B 到 400 B 描述一张图，足以渲染出一张可辨识的占位图，等真图加载完再替换。

核心用 Rust 写，Python 和 TypeScript 共用同一份 Rust 代码（PyO3 wheel / wasm-bindgen），任何一端编出来的 hash 都能互通。

## 它能替代什么

| 你现在用               | 换成 arthash 的            | 主要收益                                                 |
| ---------------------- | -------------------------- | -------------------------------------------------------- |
| blurhash / thumbhash   | `DCT` 模式                 | 同字节数 PSNR 高 0.4 dB；JS 端 encode 1.9× / decode 1.4× |
| sqip（primitive 部分） | `TRIANGLE` / `CIRCLE` 模式 | 体积 1/9 – 1/16；编码 50–67× 快；能在浏览器原生 wasm 跑  |

shape / PIXEL 模式还可以接收外部调色板，把颜色字段压成 4 bit；同时画面自然带上调色板的视觉风格（品牌色、复古、莫兰迪等）。

## 快速上手

### Rust

```rust
use arthash::{Codec, Preset, encode_rgb, decode, EncodeOptions, DecodeOptions};

// 命名预设（推荐）
let codec = Preset::DetailTriangle.codec();          // triangle, n=64
let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
let out = decode(&hash, &codec, DecodeOptions::default());
// out.width / out.height / out.rgba

// 或者用工厂方法
let codec = Codec::triangle(64);
// Codec::dct(), Codec::circle(n), Codec::square(n), Codec::rect(n),
// Codec::rotated_rect(n), Codec::pixel(n)
```

### Python（PyO3 wheel）

```python
from arthash import Codec, Preset, encode, decode, to_svg

# DCT —— thumbhash 风格模糊占位图
hash_bytes = encode("photo.jpg")
w, h, rgba = decode(hash_bytes, base_size=256)   # rgba shape (h, w, 4)

# 命名预设
codec = Codec.preset(Preset.DETAIL_TRIANGLE)
hash_bytes = encode("photo.jpg", codec)
svg = to_svg(hash_bytes, codec, base_size=512, blur=8.0)

# 工厂方法 + 调色板
from arthash.palettes import PICO8
codec = Codec.triangle(n=24, palette=PICO8)
hash_bytes = encode("photo.jpg", codec)
```

### TypeScript（wasm-bindgen，浏览器 / Node）

```ts
import { encode, decode, toSvg, codec, Preset, encodeImage } from "arthash";

// Wasm 首次调用时自动加载；如需提前加载可 `await init()`。

// 命名预设
const c = codec.preset(Preset.DetailTriangle);   // triangle, n=64
const hash = await encode(rgbBytes, width, height, c);
const { w, h, rgba } = await decode(hash, c);
const svg = await toSvg(hash, c, { baseSize: 512, blur: 8 });

// 浏览器便捷入口：加载图片 + 缩放 + 编码一步完成
const hash2 = await encodeImage(imageUrlOrBlob, c);
```

工厂方法：`codec.dct()` / `.circle({ n })` / `.triangle({ n })` / `.square({ n })` / `.rect({ n })` / `.rotatedRect({ n })` / `.pixel({ n })`。详见 [`packages/arthash-ts/README.md`](./packages/arthash-ts/README.md)。

**前端体积**：wasm 核心首次加载 ~67 KB brotli / ~93 KB gzip（HTTP 缓存后免下载），SDK 进 bundle ~6 KB。wasm 是单体的——所有 codec 模式 + hill-climb 编码器都打包在一起，不是 decode-only 构建（如做 decode-only 还可再省 ~15–20 KB brotli，有需求请 [开 issue](https://github.com/Jannchie/arthash/issues)）。完整拆分见 [`packages/arthash-ts/README.md`](./packages/arthash-ts/README.md#footprint)。

## 模式 & 字节数

| 模式           | 视觉效果                     | n=12 字节 | n=64 字节 |
| -------------- | ---------------------------- | --------: | --------: |
| `DCT`          | 模糊缩略图（thumbhash 同类） |     17–24 |         — |
| `PIXEL`        | 调色板像素马赛克             |        25 |       129 |
| `CIRCLE`       | 叠加圆，输出 SVG             |        53 |       267 |
| `SQUARE`       | 轴对齐方块，输出 SVG         |        53 |       267 |
| `RECT`         | 轴对齐矩形，输出 SVG         |        59 |       299 |
| `ROTATED_RECT` | 旋转矩形，输出 SVG           |        66 |       339 |
| `TRIANGLE`     | 三角形马赛克，输出 SVG       |        77 |       395 |

playground 默认 `TRIANGLE n=64 / baseSize 512 / RGB-565`，是一个比较合理的起点。对大小敏感就调低 `n`（n=24 → 150 B，n=12 → 77 B）；想要特定视觉风格就换调色板；想要极致小体积加模糊感就上 DCT（≤ 24 B）。

## 与 thumbhash / sqip 的关系

**thumbhash**（Evan Wallace，2023）—— blurhash 之后的演化版本，同样用 DCT 编码模糊缩略图，编码更紧凑、~24 字节，纯 JS 实现。arthash 的 `DCT` 模式直接对标它。

**sqip**（Tobias Baldauf，2017）—— 一个 Node 插件框架，用得最多的是 `sqip-plugin-primitive`（调用 Go [primitive](https://github.com/fogleman/primitive)，爬山法叠加 N 个几何原语拟合原图），输出 SVG 字符串。典型用法是构建期生成、内联到 HTML 当 LQIP。arthash 的 shape 模式对标 primitive 这一支。

### 特性对比

| 特性                         |     arthash     |     thumbhash      |          sqip           |
| ---------------------------- | :-------------: | :----------------: | :---------------------: |
| DCT 模糊缩略图（17–24 B）    |        ✅        |         ✅          |            ❌            |
| 几何原语 SVG                 |     ✅ 5 种      |         ❌          |       ✅ 多种插件        |
| 像素马赛克                   |        ✅        |         ❌          |            ❌            |
| 外部调色板（颜色压到 4 bit） |        ✅        |         ❌          |            ❌            |
| Potrace 风格描边 SVG         |        ❌        |         ❌          | ✅ `sqip-plugin-potrace` |
| WebP 输出                    |        ❌        |         ❌          |     ✅ 部分插件支持      |
| 解码到任意尺寸               |        ✅        |   ⚠️ 默认 ~32 px    |      ✅（SVG 矢量）      |
| Web / 浏览器 wasm            |        ✅        |      ✅ 纯 JS       |   ❌（依赖 Go 子进程）   |
| Python 绑定                  |  ✅ PyO3 wheel   | ⚠️ 纯 Python 慢 80× |            ❌            |
| Rust crate                   |        ✅        |         ✅          |            ❌            |
| 部署形态                     | 请求期 / 构建期 |  请求期 / 构建期   |        仅构建期         |

arthash 当前**不覆盖** sqip 的 Potrace 描边模式（位图轮廓化 → SVG path），也没做 WebP / data-URI 输出。如果你的场景需要这些，sqip 仍然是更合适的选择。

## Benchmarks

同一台机器、100×100 输入。JS 数据由 [`bench/js-cross/`](./bench/js-cross/) 在 Node 22 下实测，NDJSON 在 [`docs/benchmarks/js_cross_*.ndjson`](./docs/benchmarks/)；原生数据见 [`docs/benchmarks/CROSS_IMPL.md`](./docs/benchmarks/CROSS_IMPL.md)。所有性能表按速度升序排列，倍率列 = baseline 时间 / 当前实现时间。

### encode：DCT 24 B 输出

JS（baseline = thumbhash-js）：

| 实现                | median |     vs baseline | 字节 |
| ------------------- | -----: | --------------: | ---: |
| arthash · ts (wasm) | 279 µs |     **1.9× 快** |   24 |
| thumbhash · JS      | 532 µs | 1.0× (baseline) |   24 |

原生（baseline = thumbhash-rust，最快的非 arthash 实现）：

| 实现                      | median |     vs baseline | 字节 |
| ------------------------- | -----: | --------------: | ---: |
| arthash · Python (PyO3)   | 242 µs |    **1.27× 快** |   24 |
| arthash · Rust            | 243 µs |    **1.27× 快** |   24 |
| thumbhash · Rust (evanw)  | 308 µs | 1.0× (baseline) |   24 |
| thumbhash · Go (n16f.net) | 415 µs |        0.74× 慢 |   24 |
| thumbhash · Python (PyPI) |  25 ms |       0.012× 慢 |   24 |

arthash Rust 和 PyO3 速度持平（min ~228 µs / median ~243 µs）——PyO3 只多一层 GIL/PyBytes 封装，开销在 µs 量级、被批量测量噪声盖掉。

### decode：DCT 24 B → RGBA

JS（baseline = thumbhash-js 在它默认的 ~32 px 输出）：

| 实现                | 输出尺寸 |     median |     vs baseline |
| ------------------- | -------: | ---------: | --------------: |
| arthash · ts (wasm) |   ~32 px |     116 µs |     **1.4× 快** |
| thumbhash · JS      |   ~32 px |     165 µs | 1.0× (baseline) |
| arthash · ts (wasm) |   256 px |    6.69 ms |  *(对方不支持)* |
| arthash · ts (wasm) |   512 px |   26.22 ms |  *(对方不支持)* |
| thumbhash · JS      |     256+ | API 不支持 |               — |

thumbhash JS 解码 API 只输出 ~32 px，要变大只能 CSS 拉伸（拉糊）。arthash 直接 IDCT 到任意尺寸，省掉前端上采样这一步。

原生 @ 256 px（baseline = thumbhash-go @ 256）：

| 实现                    |  median |     vs baseline |
| ----------------------- | ------: | --------------: |
| arthash · Rust          | 2.06 ms |     **5.9× 快** |
| arthash · Python (PyO3) | 2.60 ms |     **4.7× 快** |
| thumbhash · Go @ 256    | 12.2 ms | 1.0× (baseline) |

thumbhash 的 Rust crate 在它自己的默认 ~32 px 输出下比 arthash 快；一旦要求显示尺寸缓冲（占位图实际场景），arthash 反超 6×。

### encode：shape 模式 vs sqip

JS（baseline = sqip-node 同 n 值）。arthash 越大的 n 优势越明显——sqip 是线性 + 子进程 IPC，每多一个原语都要再爬山一遍；arthash 用积分图 / SSE 增量评估，搜索成本亚线性。

**编码时间**：

| 实现                           |      n=12 (倍率) |      n=24 (倍率) |       n=64 (倍率) |
| ------------------------------ | ---------------: | ---------------: | ----------------: |
| arthash · ts TRIANGLE          | 5.1 ms (**56×**) | 7.9 ms (**56×**) | 15.2 ms (**67×**) |
| arthash · ts CIRCLE            |     5.3 ms (54×) |     7.2 ms (62×) |     15.5 ms (66×) |
| sqip · primitive-triangle @0.3 |           284 ms |           446 ms |           1015 ms |

**输出大小**：

| 实现                           |       n=12 (倍率) |        n=24 (倍率) |        n=64 (倍率) |
| ------------------------------ | ----------------: | -----------------: | -----------------: |
| arthash · ts CIRCLE            | 53 B (**16× 小**) | 102 B (**15× 小**) | 267 B (**14× 小**) |
| arthash · ts TRIANGLE          |     77 B (11× 小) |     150 B (10× 小) |      395 B (9× 小) |
| sqip · primitive-triangle @0.3 |             842 B |             1482 B |             3650 B |

sqip 每次调用都要 spawn Go primitive 子进程，根本不能在浏览器跑；所以它适合**构建期一次性生成**。arthash 走 wasm-bindgen，**请求期实时编码**也轻松。

### 画质 (256 长边, PSNR，按 PSNR 降序)

| 输出                  |    字节 |    PSNR |
| --------------------- | ------: | ------: |
| sqip · 12 原语 SVG    | ~1100 B | 24.4 dB |
| arthash · DCT         |    17 B | 23.3 dB |
| thumbhash             |    17 B | 22.9 dB |
| arthash · TRIANGLE 12 |    77 B | 21.4 dB |
| arthash · CIRCLE 12   |    53 B | 20.7 dB |

同 17 B 预算下 arthash DCT 比 thumbhash 高 0.4 dB；arthash TRIANGLE 12 用 77 B 就拿到 21.4 dB，相比 sqip 的 ~1100 B / 24.4 dB，是用 1/14 字节换 3 dB 画质。

## 怎么省 bit 的

arthash 把每一个 bit 都花在图像信息上，没有任何 header 浪费。具体分四层：

**1. 不要 header —— 两端共识的 Codec。**
hash 字节流本身不自描述，不带 magic number、不带模式标签、不带 bit width。模式、形状数、量化位宽、调色板这些都由 Codec 同时配给 encode 和 decode。换走"自描述"，换来"每一个 bit 都是图像信息"。

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

字节格式定在 [`docs/SPEC.md`](./docs/SPEC.md)。

## 仓库结构

```
packages/
├── arthash-rs/          Rust SDK（标准实现）
├── arthash-py/          Python SDK —— PyO3 binding
├── arthash-ts/          TypeScript SDK —— wasm-bindgen binding
└── arthash-playground/  Vue playground

bench/
├── js-cross/            JS 跨实现 bench（arthash wasm vs thumbhash-js vs sqip）
├── sqip/                sqip 调用脚本
└── thumbhash-js/        thumbhash JS bench

docs/
├── SPEC.md              字节格式定义
└── benchmarks/          RESULTS.md, CROSS_IMPL.md, NDJSON
```

## License

MIT.

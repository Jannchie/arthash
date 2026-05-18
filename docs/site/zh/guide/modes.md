# 模式与 Codec

arthash 共 7 种 codec 模式，每个在字节预算、编码成本、视觉风格之间取了不同折中。

## 模式总览

| 模式           | 视觉效果                     | n=12 字节 | n=64 字节 | 可输出 SVG |
| -------------- | ---------------------------- | --------: | --------: | :--------: |
| `DCT`          | 模糊缩略图（thumbhash 同类） |     17–24 |         — |     ✗      |
| `PIXEL`        | 调色板像素马赛克             |        25 |       129 |     ✗      |
| `CIRCLE`       | 叠加圆                       |        53 |       267 |     ✓      |
| `SQUARE`       | 轴对齐方块                   |        53 |       267 |     ✓      |
| `RECT`         | 轴对齐矩形                   |        59 |       299 |     ✓      |
| `ROTATED_RECT` | 旋转矩形                     |        66 |       339 |     ✓      |
| `TRIANGLE`     | 三角形马赛克                 |        77 |       395 |     ✓      |

`toSvg` 支持 `circle` / `triangle` / `square` / `rect` / `rotrect`。`DCT` 和 `PIXEL` 没有天然的 SVG 原语形式，调用会抛错。

## 命名预设

最快上手的方式——这些就是 playground 默认提供的几档，按字节预算排序。

| 预设             | 模式     | n   | 大致字节 |
| ---------------- | -------- | --- | -------: |
| `Dct`            | DCT      | —   |     ~21  |
| `SmallCircle`    | CIRCLE   | 12  |       53 |
| `SmallSquare`    | SQUARE   | 12  |       53 |
| `SmallRect`      | RECT     | 12  |       59 |
| `SmallTriangle`  | TRIANGLE | 12  |       77 |
| `SmallPixel`     | PIXEL    | 16  |       33 |
| `MediumCircle`   | CIRCLE   | 24  |      102 |
| `MediumSquare`   | SQUARE   | 24  |      102 |
| `MediumRect`     | RECT     | 24  |      114 |
| `MediumTriangle` | TRIANGLE | 24  |      150 |
| `MediumPixel`    | PIXEL    | 24  |       49 |
| `LargeCircle`    | CIRCLE   | 64  |      267 |
| `LargeSquare`    | SQUARE   | 64  |      267 |
| `LargeRect`      | RECT     | 64  |      299 |
| `LargeTriangle`  | TRIANGLE | 64  |      395 |
| `LargePixel`     | PIXEL    | 64  |      129 |

0.3 之前的名字（`TinyDct` / `Placeholder*` / `Detail*`）作为已弃用的别名保留——详见
[`docs/MIGRATION.md`](https://github.com/Jannchie/arthash/blob/main/docs/MIGRATION.md#02--03)。

```ts
codec.preset(Preset.MediumTriangle)
```

```python
Codec.preset(Preset.MEDIUM_TRIANGLE)
```

```rust
Preset::MediumTriangle.codec()
```

## 工厂方法

想显式控制 `n` 和颜色模式时：

::: code-group

```ts [TypeScript]
codec.dct()
codec.circle({ n: 12 })
codec.triangle({ n: 64 })
codec.square({ n: 12 })
codec.rect({ n: 12 })
codec.rotatedRect({ n: 12, thetaBits: 5 })
codec.pixel({ n: 16 })

// 把 shape codec 套上调色板：
codec.withPalette(codec.triangle({ n: 24 }), { bytes: paletteBytes })

// 一致性测试用的低层入口：
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

# 调色板模式
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

## 怎么选字节预算

| 需求                       | 推荐                       |
| -------------------------- | -------------------------- |
| CSS / HTML 内联占位图      | DCT (~21 B) 或 PIXEL n=16  |
| 邮件友好的 LQIP            | TRIANGLE n=12 (77 B)       |
| 首屏 Hero 占位             | TRIANGLE n=24 (~150 B)     |
| 细节保真的画廊缩略         | TRIANGLE n=64 (~400 B)     |

降低 `n` 缩小 hash，提高 `n` 找回细节，开销大体随 `n` 线性。

## 颜色模式

shape 模式默认存 **RGB-565**（每形状 16 bit），另外两个可选：

| 模式      | 颜色位宽    | 什么时候用                                              |
| --------- | ----------- | ------------------------------------------------------- |
| `rgb565`  | 16 bit      | 默认。感知质量已经很好，比 RGB-888 省一半               |
| `rgb888`  | 24 bit      | 字节不是瓶颈、要求高保真时用                            |
| `palette` | `log₂(K)` bit | K=16 → 每色 4 bit；给画面打上一致的品牌 / 复古风格      |

切换到调色板就一行：

```ts
import { codec, palette } from "arthash";

const brand = palette.fromHex(["#0a0a0a", "#ffffff", "#0284c7", "#22c55e", ...]); // K 必须是 2 的幂
const c = codec.withPalette(codec.triangle({ n: 24 }), brand);
```

```python
from arthash.palettes import PICO8
codec = Codec.triangle(n=24, palette=PICO8)
```

更多见 [调色板](./palettes)。

## 字节总长公式

hash 长度完全由 codec 决定——没有 header、没有长度前缀。

```text
bytes = ceil((header_bits + n_shapes × per_shape_bits) / 8)
```

| 形状           | `per_shape_bits`（颜色 `C`、alpha `A`）          |
| -------------- | ------------------------------------------------ |
| `CIRCLE`       | `cx + cy + r + C + A`                            |
| `SQUARE`       | `cx + cy + r + C + A`                            |
| `RECT`         | `cx + cy + 2·r + C + A`                          |
| `ROTATED_RECT` | `cx + cy + 2·r + θ + C + A`                      |
| `TRIANGLE`     | `3·(cx + cy) + C + A`                            |
| `PIXEL`        | `C`（几何由 aspect 推出，不进 hash）             |

默认值：`cx = cy = 5`、`r = 4`、`α = 3`、`θ = 5`、`C = 16`（调色板时为 `log₂(K)`）。字节布局定在 [`docs/SPEC.md`](https://github.com/Jannchie/arthash/blob/main/docs/SPEC.md)。

运行时查长度：

```ts
codec.bytesTotal(c)             // TS
```

```python
codec.bytes_total()             # Python
```

```rust
codec.bytes_total()             // Rust
```

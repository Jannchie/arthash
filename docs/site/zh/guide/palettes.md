# 调色板

给 shape codec 套一个外部调色板，可以把每形状的颜色字段从 16 / 24 bit 压成 `log₂(K)` bit（K=16 → 4 bit），同时给整张图打上一致的视觉风格——品牌色、复古游戏调色板、莫兰迪色、随你定。

## 为什么用调色板

| 不用调色板                    | 用调色板（K = 16）               |
| ----------------------------- | -------------------------------- |
| 每形状颜色 16 bit             | 每形状颜色 4 bit                 |
| n=24 时 hash ~150 B           | n=24 时 hash ~114 B（省 ~36 B）  |
| 自由 RGB-565                  | 所有图共享一套视觉风格           |
| 看起来"计算机生成"            | 看起来像有意为之的设计           |

编码器在 Oklab 空间里把每个形状的颜色量化到最近的调色板项，所以颜色映射是感知均匀的，不是 RGB 距离的盲映射。

## 构造调色板

调色板就是一段长度为 `K × 3` 的扁平 sRGB 字节数组，其中 `K ∈ {2, 4, 8, 16, 32, 64, 128, 256, 512, 1024}`（2 到 1024 之间任意 2 的幂）。

::: code-group

```ts [TypeScript]
import { palette, codec } from "arthash";

// 从 hex 字符串构造——最方便
const brand = palette.fromHex([
  "#0a0a0a", "#ffffff", "#0284c7", "#22c55e",
  "#f59e0b", "#ef4444", "#a855f7", "#14b8a6",
  "#1e293b", "#f8fafc", "#0369a1", "#16a34a",
  "#d97706", "#dc2626", "#9333ea", "#0d9488",
]);                                                  // K = 16

// 从 [r, g, b] 三元组构造
const grayscale = palette.fromRgb([
  [0, 0, 0], [64, 64, 64], [128, 128, 128], [255, 255, 255],
]);                                                  // K = 4

// 套在任意 shape codec 上
const c = codec.withPalette(codec.triangle({ n: 24 }), brand);
```

```python [Python]
from arthash import Codec
from arthash.palettes import PICO8, GAMEBOY, MORANDI

codec = Codec.triangle(n=24, palette=PICO8)

# 也可以自己构造
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

::: warning K 必须是 2 的幂
位打包算法假定 `K ∈ {2, 4, 8, …, 1024}`。传入其他大小会在构造 codec 时直接报错，而不是在 encode 时才挂掉。
:::

## 内置调色板

TypeScript 和 Python SDK 自带一组常用调色板：

| 名称       | K  | 风格                          |
| ---------- | -- | ----------------------------- |
| `PICO8`    | 16 | PICO-8 主机调色板             |
| `GAMEBOY`  | 4  | 初代 Game Boy DMG 绿          |
| `NES`      | 64 | NES Famicom 调色板            |
| `MORANDI`  | 16 | 莫兰迪风格的中性低饱和色      |
| `MONO`     | 4  | 黑 / 深灰 / 浅灰 / 白         |

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

## 设计建议

- **K = 16 通常是甜蜜点。** 4 bit 一个颜色，足以覆盖一套有辨识度的风格，又不会浪费太多 bit。
- **极值要给到。** 一个好的调色板至少要有一个接近黑、一个接近白、以及饱和度合适的中间色；否则编码器没有高对比可用，画面会糊成一片。
- **按感知聚类，不要按 RGB 等距。** RGB 上等距取 16 个颜色看起来发灰。[Coolors](https://coolors.co/) / [Lospec](https://lospec.com/palette-list/) 这类平台精选的调色板用起来效果更好。
- **解码端必须有同一份调色板。** 它是 `Codec` 契约的一部分，**不会被打进 hash 字节流**。把它作为共享常量在 encode / decode 两端复用；如果调色板会随业务变化、必须随 hash 传输，请自己设计一个外层帧（例如把调色板字节拼在 hash 前面）——SDK 不内建这种打包格式。

## 调色板在各模式中的可用性

调色板颜色对所有 shape 模式（`CIRCLE`、`SQUARE`、`RECT`、`ROTATED_RECT`、`TRIANGLE`）和 `PIXEL` 都生效。`DCT` 模式忽略调色板——它在 Oklab DCT 系数空间里量化，没有按形状的颜色可重新映射。

# Python API

PyPI 包：[`arthash`](https://pypi.org/project/arthash/)。PyO3 wheel——重活仍由 Rust 干。

```python
from arthash import (
    Codec, Preset, Palette, RenderStyle,
    encode, decode, to_svg,
)
from arthash import palettes
```

## 顶层函数

### `encode(image, codec=None, *, seed=0, search=None) -> bytes`

把图片编成 hash。`image` 接受：

| 类型             | 行为                                                              |
| ---------------- | ----------------------------------------------------------------- |
| `str` / `Path`   | 文件路径；用 PIL 打开                                             |
| `bytes`          | 编码过的图片字节（PNG / JPEG / …）；用 PIL 打开                   |
| `PIL.Image`      | 直接使用                                                          |
| `numpy.ndarray`  | `H×W×3`（RGB）或 `H×W×4`（RGBA），`uint8`                         |

`codec` 为 `None` 时默认 `Codec.dct()`。

### `decode(hash_bytes, codec=None, *, base_size=256, override_aspect=None, aa=1, pixel_smooth="nearest", style=None)`

返回 `(width, height, rgba)`，其中 `rgba` 是 `(h, w, 4)` 形状、`uint8` 类型的 `numpy.ndarray`。`codec` 为 `None` 时默认 `Codec.dct()`。`style` 传 `RenderStyle` 控制模糊和圆角（见下）。

### `to_svg(hash_bytes, codec, *, base_size=256, override_aspect=None, style=None, blur=None) -> str`

把 shape 模式的 hash 渲染成 SVG 字符串。只支持 `CIRCLE` / `TRIANGLE` / `SQUARE` / `RECT` / `ROTATED_RECT`。DCT 和 PIXEL 抛 `ValueError`。`style` 控制模糊和圆角；`blur` kwarg **自 0.3.0 起弃用**——请改用 `style=RenderStyle(blur=...)`。1.0 移除。

## `Codec`

字节格式契约。encode 和 decode 必须传入同一个 `Codec`。

### 工厂方法

```python
Codec.dct()
Codec.circle(n=12, palette=None)
Codec.triangle(n=12, palette=None)
Codec.square(n=12, palette=None)
Codec.rect(n=12, palette=None)
Codec.rotated_rect(n=12, theta_bits=5, palette=None)
Codec.pixel(n=12, grid_aspect=None, palette=None)
```

所有工厂方法都接受可选的 `palette` 关键字以切换到调色板颜色模式。`Codec.dct()` 忽略调色板。

### `Codec.preset(p)`

```python
Codec.preset(Preset.LARGE_TRIANGLE)
```

### `Codec.raw(...)`

暴露 SPEC 每个字段的低层入口。

```python
Codec.raw(
    shape="triangle",
    n_shapes=12,
    cx_bits=5, cy_bits=5,
    r_bits=4, alpha_bits=3, color_bits=16,
    theta_bits=5,
    palette=None, palette_k=None,
    grid_aspect=None,
)
```

### 方法

| 方法                         | 描述                                                  |
| ---------------------------- | ----------------------------------------------------- |
| `codec.bytes_total()`        | 此 codec 编出的 hash 总字节数                         |
| `codec.is_palette_mode()`    | `bool`——颜色是否来自调色板                            |
| `codec.with_palette(p)`      | 返回切到调色板颜色模式的副本                          |

## `Preset`

```python
# 尺寸轴：small (n=12, pixel n=16) / medium (n=24) / large (n=64)
Preset.DCT
Preset.SMALL_TRIANGLE
Preset.SMALL_CIRCLE
Preset.SMALL_PIXEL
Preset.SMALL_RECT
Preset.SMALL_SQUARE
Preset.MEDIUM_TRIANGLE
Preset.MEDIUM_CIRCLE
Preset.MEDIUM_PIXEL
Preset.MEDIUM_RECT
Preset.MEDIUM_SQUARE
Preset.LARGE_TRIANGLE
Preset.LARGE_CIRCLE
Preset.LARGE_PIXEL
Preset.LARGE_RECT
Preset.LARGE_SQUARE

# 0.3 之前的别名——为 source 兼容保留。
# Preset.TINY_DCT, Preset.PLACEHOLDER_*, Preset.DETAIL_*
```

## `Palette`

```python
Palette.from_rgb([(r, g, b), ...])   # K 必须是 [2, 1024] 中 2 的幂
Palette.from_hex(["#aabbcc", ...])

# 内置常量
from arthash.palettes import PICO8, GAMEBOY, NES, MORANDI, MONO
```

## `RenderStyle`

```python
@dataclass
class RenderStyle:
    blur: float = 0.0          # 高斯 stdDeviation（viewBox 单位）；0 = 锐利
    corner_radius: float = 0.0 # 仅 rect / square / rotrect；0 = 直角
```

独立于 codec 的字节格式——同一 `(hash, codec)` 配不同 `style` 会产生视觉不同
但字节不变的输出。默认值（两个字段都为 0）走零成本快路径。

在非 rect 家族 codec（circle / triangle / pixel / DCT）上设置 `corner_radius`
会发出 `UserWarning` 并被静默忽略——TS SDK 在编译期通过条件类型拦截，Python
退回到运行时警告，意图一致。

```python
from arthash import RenderStyle, decode, to_svg, Codec

style = RenderStyle(blur=2.0, corner_radius=4.0)
w, h, rgba = decode(hash_bytes, Codec.rect(n=32), style=style)
svg = to_svg(hash_bytes, Codec.rect(n=32), style=style)
```

## `EncodeOptions` / 搜索预算

`encode` 的 `search` 关键字接受一个 dict，对应 Rust struct：

```python
encode(
    img, codec,
    seed=0,
    search={
        "strategy": "primitive",       # 或 "topk_uniform"
        "n_random": 64,
        "n_topk": 8,
        "hill_climb_steps": 100,
        "hill_climb_max_age": None,
        "n_attempts": 3,
    },
)
```

与 TS 绑定一样——这些只影响编码成本和质量，字节格式完全相同。

## 示例

```python
from arthash import Codec, Preset, encode, decode, to_svg
from arthash.palettes import PICO8

# 1. 极致小——DCT，~21 字节
hash_bytes = encode("photo.jpg")
print(len(hash_bytes))                              # ~21

# 2. 命名预设
codec = Codec.preset(Preset.LARGE_TRIANGLE)
hash_bytes = encode("photo.jpg", codec)
svg = to_svg(hash_bytes, codec, base_size=512, blur=8.0)

# 3. 调色板模式——复古风
codec = Codec.triangle(n=24, palette=PICO8)
hash_bytes = encode("photo.jpg", codec)

# 4. 按显示尺寸解码
w, h, rgba = decode(hash_bytes, codec, base_size=512)
# rgba 形状 (512, 512, 4)
```

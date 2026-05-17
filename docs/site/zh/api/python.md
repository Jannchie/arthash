# Python API

PyPI 包：[`arthash`](https://pypi.org/project/arthash/)。PyO3 wheel——重活仍由 Rust 干。

```python
from arthash import (
    Codec, Preset, Palette,
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

### `decode(hash_bytes, codec=None, *, base_size=256, override_aspect=None, aa=None, pixel_smooth=None)`

返回 `(width, height, rgba)`，其中 `rgba` 是 `(h, w, 4)` 形状、`uint8` 类型的 `numpy.ndarray`。`codec` 为 `None` 时默认 `Codec.dct()`。

### `to_svg(hash_bytes, codec, *, base_size=256, override_aspect=None, blur=0.0) -> str`

把 shape 模式的 hash 渲染成 SVG 字符串。只支持 `CIRCLE` / `TRIANGLE` / `SQUARE` / `RECT` / `ROTATED_RECT`。DCT 和 PIXEL 抛 `ValueError`。

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
Codec.preset(Preset.DETAIL_TRIANGLE)
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
Preset.TINY_DCT
Preset.PLACEHOLDER_TRIANGLE
Preset.PLACEHOLDER_CIRCLE
Preset.PLACEHOLDER_PIXEL
Preset.MEDIUM_TRIANGLE
Preset.MEDIUM_CIRCLE
Preset.MEDIUM_PIXEL
Preset.DETAIL_TRIANGLE
Preset.DETAIL_CIRCLE
Preset.DETAIL_PIXEL
```

## `Palette`

```python
Palette.from_rgb([(r, g, b), ...])   # K 必须是 [2, 1024] 中 2 的幂
Palette.from_hex(["#aabbcc", ...])

# 内置常量
from arthash.palettes import PICO8, GAMEBOY, NES, MORANDI, MONO
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
codec = Codec.preset(Preset.DETAIL_TRIANGLE)
hash_bytes = encode("photo.jpg", codec)
svg = to_svg(hash_bytes, codec, base_size=512, blur=8.0)

# 3. 调色板模式——复古风
codec = Codec.triangle(n=24, palette=PICO8)
hash_bytes = encode("photo.jpg", codec)

# 4. 按显示尺寸解码
w, h, rgba = decode(hash_bytes, codec, base_size=512)
# rgba 形状 (512, 512, 4)
```

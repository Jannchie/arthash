# Python API

PyPI パッケージ：[`arthash`](https://pypi.org/project/arthash/)。PyO3 wheel—重い処理は引き続き Rust で行われます。

```python
from arthash import (
    Codec, Preset, Palette,
    encode, decode, to_svg,
)
from arthash import palettes
```

## トップレベル関数

### `encode(image, codec=None, *, seed=0, search=None) -> bytes`

画像をハッシュにエンコード。`image` は以下を受け取ります：

| 型               | 動作                                                          |
| ---------------- | ------------------------------------------------------------- |
| `str` / `Path`   | ファイルパス；PIL で開く                                       |
| `bytes`          | エンコード済み画像バイト（PNG / JPEG / …）；PIL で開く          |
| `PIL.Image`      | 直接使用                                                       |
| `numpy.ndarray`  | `H×W×3`（RGB）または `H×W×4`（RGBA）、`uint8`                  |

`codec` が `None` のときデフォルトは `Codec.dct()`。

### `decode(hash_bytes, codec=None, *, base_size=256, override_aspect=None, aa=None, pixel_smooth=None)`

`(width, height, rgba)` を返します。`rgba` は形状 `(h, w, 4)`、dtype `uint8` の `numpy.ndarray`。`codec` が `None` のときデフォルトは `Codec.dct()`。

### `to_svg(hash_bytes, codec, *, base_size=256, override_aspect=None, blur=0.0) -> str`

shape モードのハッシュを SVG 文字列としてレンダリング。`CIRCLE` / `TRIANGLE` / `SQUARE` / `RECT` / `ROTATED_RECT` のみサポート。DCT と PIXEL は `ValueError` を投げます。

## `Codec`

バイト形式の契約。エンコードとデコードに同じ `Codec` を使う必要があります。

### ファクトリ

```python
Codec.dct()
Codec.circle(n=12, palette=None)
Codec.triangle(n=12, palette=None)
Codec.square(n=12, palette=None)
Codec.rect(n=12, palette=None)
Codec.rotated_rect(n=12, theta_bits=5, palette=None)
Codec.pixel(n=12, grid_aspect=None, palette=None)
```

すべてのファクトリは任意の `palette` キーワードを受け取り、パレットカラーモードに切り替えられます。`Codec.dct()` はパレットを無視します。

### `Codec.preset(p)`

```python
Codec.preset(Preset.DETAIL_TRIANGLE)
```

### `Codec.raw(...)`

SPEC の全フィールドを公開する低レベル入口。

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

### メソッド

| メソッド                     | 説明                                                  |
| ---------------------------- | ----------------------------------------------------- |
| `codec.bytes_total()`        | この codec のハッシュ総バイト数                        |
| `codec.is_palette_mode()`    | `bool`—色がパレットから来ているか                      |
| `codec.with_palette(p)`      | パレットカラーモードに切り替えたクローンを返す          |

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
Palette.from_rgb([(r, g, b), ...])   # K は [2, 1024] の 2 の冪
Palette.from_hex(["#aabbcc", ...])

# 同梱定数
from arthash.palettes import PICO8, GAMEBOY, NES, MORANDI, MONO
```

## `EncodeOptions` / 検索予算

`encode` の `search` キーワードは Rust struct を反映した dict を受け取ります：

```python
encode(
    img, codec,
    seed=0,
    search={
        "strategy": "primitive",       # または "topk_uniform"
        "n_random": 64,
        "n_topk": 8,
        "hill_climb_steps": 100,
        "hill_climb_max_age": None,
        "n_attempts": 3,
    },
)
```

TS バインディングと同様、これらはエンコードコストと品質にのみ影響します—バイト形式は予算に関わらず同一です。

## 例

```python
from arthash import Codec, Preset, encode, decode, to_svg
from arthash.palettes import PICO8

# 1. 最小—DCT、~21 バイト
hash_bytes = encode("photo.jpg")
print(len(hash_bytes))                              # ~21

# 2. 名前付きプリセット
codec = Codec.preset(Preset.DETAIL_TRIANGLE)
hash_bytes = encode("photo.jpg", codec)
svg = to_svg(hash_bytes, codec, base_size=512, blur=8.0)

# 3. パレットモード—レトロな見た目
codec = Codec.triangle(n=24, palette=PICO8)
hash_bytes = encode("photo.jpg", codec)

# 4. 表示サイズでデコード
w, h, rgba = decode(hash_bytes, codec, base_size=512)
# rgba 形状 (512, 512, 4)
```

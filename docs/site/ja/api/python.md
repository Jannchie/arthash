# Python API

PyPI パッケージ：[`arthash`](https://pypi.org/project/arthash/)。PyO3 wheel—重い処理は引き続き Rust で行われます。

```python
from arthash import (
    Codec, Preset, Palette, RenderStyle,
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

### `decode(hash_bytes, codec=None, *, base_size=256, override_aspect=None, aa=1, pixel_smooth="nearest", style=None)`

`(width, height, rgba)` を返します。`rgba` は形状 `(h, w, 4)`、dtype `uint8` の `numpy.ndarray`。`codec` が `None` のときデフォルトは `Codec.dct()`。`style` は `RenderStyle` でぼかしと角丸を制御します（後述）。

### `to_svg(hash_bytes, codec, *, base_size=256, override_aspect=None, style=None, blur=None) -> str`

shape モードのハッシュを SVG 文字列としてレンダリング。`CIRCLE` / `TRIANGLE` / `SQUARE` / `RECT` / `ROTATED_RECT` のみサポート。DCT と PIXEL は `ValueError` を投げます。`style` はぼかしと角丸を制御します。`blur` kwarg は **0.3.0 以降非推奨**—`style=RenderStyle(blur=...)` を使ってください。1.0 で削除予定。

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
Codec.preset(Preset.LARGE_TRIANGLE)
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
# サイズ軸：small (n=12, pixel n=16) / medium (n=24) / large (n=64)
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

# 0.3 以前の非推奨エイリアス—ソース互換のため保持。
# Preset.TINY_DCT, Preset.PLACEHOLDER_*, Preset.DETAIL_*
```

## `Palette`

```python
Palette.from_rgb([(r, g, b), ...])   # K は [2, 1024] の 2 の冪
Palette.from_hex(["#aabbcc", ...])

# 同梱定数
from arthash.palettes import PICO8, GAMEBOY, NES, MORANDI, MONO
```

## `RenderStyle`

```python
@dataclass
class RenderStyle:
    blur: float = 0.0          # ガウスの stdDeviation（viewBox 単位）；0 = シャープ
    corner_radius: float = 0.0 # rect / square / rotrect のみ；0 = 鋭角
```

codec のバイト形式と独立—同じ `(hash, codec)` に異なる `style` を渡すと、
ハッシュバイトを変えずに視覚的に異なる出力が得られます。デフォルト（両
フィールド 0）はゼロコストの fast path。

非 rect ファミリーの codec（circle / triangle / pixel / DCT）で
`corner_radius` を設定すると `UserWarning` が出て値は静かに無視されます—
TS SDK はコンパイル時に条件型でこれを捕捉しますが、Python は意図を揃え
るために実行時警告にフォールバックします。

```python
from arthash import RenderStyle, decode, to_svg, Codec

style = RenderStyle(blur=2.0, corner_radius=4.0)
w, h, rgba = decode(hash_bytes, Codec.rect(n=32), style=style)
svg = to_svg(hash_bytes, Codec.rect(n=32), style=style)
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
codec = Codec.preset(Preset.LARGE_TRIANGLE)
hash_bytes = encode("photo.jpg", codec)
svg = to_svg(hash_bytes, codec, base_size=512, blur=8.0)

# 3. パレットモード—レトロな見た目
codec = Codec.triangle(n=24, palette=PICO8)
hash_bytes = encode("photo.jpg", codec)

# 4. 表示サイズでデコード
w, h, rgba = decode(hash_bytes, codec, base_size=512)
# rgba 形状 (512, 512, 4)
```

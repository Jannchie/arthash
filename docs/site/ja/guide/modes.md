# モードと Codec

arthash には 7 つの codec モードがあります。それぞれバイト予算、エンコードコスト、視覚スタイルのトレードオフが異なります。

## モード概要

| モード         | 見た目                              | n=12 バイト | n=64 バイト | SVG 出力 |
| -------------- | ----------------------------------- | ----------: | ----------: | :------: |
| `DCT`          | ぼやけたサムネイル（thumbhash 風）  |       17–24 |           — |    ✗     |
| `PIXEL`        | パレットピクセルモザイク            |          25 |         129 |    ✗     |
| `CIRCLE`       | 重なる円                            |          53 |         267 |    ✓     |
| `SQUARE`       | 軸並行の正方形                      |          53 |         267 |    ✓     |
| `RECT`         | 軸並行の長方形                      |          59 |         299 |    ✓     |
| `ROTATED_RECT` | 回転長方形                          |          66 |         339 |    ✓     |
| `TRIANGLE`     | 三角形モザイク                      |          77 |         395 |    ✓     |

`toSvg` は `circle` / `triangle` / `square` / `rect` / `rotrect` をサポート。`DCT` と `PIXEL` には自然な SVG プリミティブ表現がなく、呼び出すとスローします。

## 名前付きプリセット

最速の選び方—これらは playground のデフォルトで、バイト予算順に並んでいます。

| プリセット       | モード   | n   | おおよそのバイト |
| ---------------- | -------- | --- | ---------------: |
| `Dct`            | DCT      | —   |             ~21  |
| `SmallCircle`    | CIRCLE   | 12  |               53 |
| `SmallSquare`    | SQUARE   | 12  |               53 |
| `SmallRect`      | RECT     | 12  |               59 |
| `SmallTriangle`  | TRIANGLE | 12  |               77 |
| `SmallPixel`     | PIXEL    | 16  |               33 |
| `MediumCircle`   | CIRCLE   | 24  |              102 |
| `MediumSquare`   | SQUARE   | 24  |              102 |
| `MediumRect`     | RECT     | 24  |              114 |
| `MediumTriangle` | TRIANGLE | 24  |              150 |
| `MediumPixel`    | PIXEL    | 24  |               49 |
| `LargeCircle`    | CIRCLE   | 64  |              267 |
| `LargeSquare`    | SQUARE   | 64  |              267 |
| `LargeRect`      | RECT     | 64  |              299 |
| `LargeTriangle`  | TRIANGLE | 64  |              395 |
| `LargePixel`     | PIXEL    | 64  |              129 |

0.3 以前の名前（`TinyDct` / `Placeholder*` / `Detail*`）は非推奨エイリアスとして
保持されます—詳しくは [`docs/MIGRATION.md`](https://github.com/Jannchie/arthash/blob/main/docs/MIGRATION.md#02--03) を参照。

```ts
codec.preset(Preset.MediumTriangle)
```

```python
Codec.preset(Preset.MEDIUM_TRIANGLE)
```

```rust
Preset::MediumTriangle.codec()
```

## ファクトリビルダー

`n` と色モードを明示的に制御したい場合：

::: code-group

```ts [TypeScript]
codec.dct()
codec.circle({ n: 12 })
codec.triangle({ n: 64 })
codec.square({ n: 12 })
codec.rect({ n: 12 })
codec.rotatedRect({ n: 12, thetaBits: 5 })
codec.pixel({ n: 16 })

// shape codec をパレットでラップ：
codec.withPalette(codec.triangle({ n: 24 }), { bytes: paletteBytes })

// 適合性テスト用の低レベル入口：
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

# パレットモード
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

## バイト予算の選び方

| ニーズ                                 | おすすめ                     |
| -------------------------------------- | ---------------------------- |
| CSS / HTML にインラインのプレースホルダー | DCT (~21 B) または PIXEL n=16 |
| メール向け LQIP                        | TRIANGLE n=12 (77 B)         |
| ファーストビューのヒーロー画像         | TRIANGLE n=24 (~150 B)       |
| 詳細を保ったギャラリーサムネイル       | TRIANGLE n=64 (~400 B)       |

`n` を下げるとハッシュが縮み、上げると詳細が戻ります。コストはおおむね `n` に比例します。

## 色モード

shape モードはデフォルトで **RGB-565**（形状ごとに 16 bit）で色を保存します。代替が 2 つあります：

| モード    | 色ビット幅      | いつ使うか                                                            |
| --------- | --------------- | --------------------------------------------------------------------- |
| `rgb565`  | 16 bit          | デフォルト。知覚品質は十分強く、RGB-888 の半分のサイズ                 |
| `rgb888`  | 24 bit          | バイトが制約でなく、高忠実度な再現が欲しいとき                         |
| `palette` | `log₂(K)` bit   | K=16 → 色 4 bit。一貫したブランド / レトロな見た目を画面に与える       |

パレットへの切り替えはワンライナー：

```ts
import { codec, palette } from "arthash";

const brand = palette.fromHex(["#0a0a0a", "#ffffff", "#0284c7", "#22c55e", ...]); // K は 2 の冪
const c = codec.withPalette(codec.triangle({ n: 24 }), brand);
```

```python
from arthash.palettes import PICO8
codec = Codec.triangle(n=24, palette=PICO8)
```

詳細は [パレット](./palettes) を参照。

## バイト合計の計算式

ハッシュ長は codec で完全に決定されます—ヘッダーも長さプレフィックスもありません。

```text
bytes = ceil((header_bits + n_shapes × per_shape_bits) / 8)
```

| 形状           | `per_shape_bits`（色 `C`、alpha `A`）            |
| -------------- | ------------------------------------------------ |
| `CIRCLE`       | `cx + cy + r + C + A`                            |
| `SQUARE`       | `cx + cy + r + C + A`                            |
| `RECT`         | `cx + cy + 2·r + C + A`                          |
| `ROTATED_RECT` | `cx + cy + 2·r + θ + C + A`                      |
| `TRIANGLE`     | `3·(cx + cy) + C + A`                            |
| `PIXEL`        | `C`（幾何は aspect から導出、ハッシュには入らない） |

デフォルト：`cx = cy = 5`、`r = 4`、`α = 3`、`θ = 5`、`C = 16`（パレットの場合は `log₂(K)`）。正確なバイトレイアウトは [`docs/SPEC.md`](https://github.com/Jannchie/arthash/blob/main/docs/SPEC.md) に固定されています。

ランタイムで取得：

```ts
codec.bytesTotal(c)             // TS
```

```python
codec.bytes_total()             # Python
```

```rust
codec.bytes_total()             // Rust
```

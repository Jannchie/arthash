# パレット

shape codec を外部パレットでラップすると、形状ごとの色フィールドが 16 / 24 bit から `log₂(K)` bit（K=16 → 4 bit）に縮みます。さらに出力全体に一貫した視覚スタイルが付きます—ブランドカラー、レトロゲームのパレット、モランディトーンなど、お好みで。

## なぜパレットを使うのか

| パレットなし                       | パレットあり（K = 16）                    |
| ---------------------------------- | ----------------------------------------- |
| 形状ごとの色 16 bit                | 形状ごとの色 4 bit                        |
| n=24 でハッシュ ~150 B             | n=24 でハッシュ ~114 B（~36 B 節約）       |
| 自由な RGB-565                     | 全画像で視覚的に一貫                      |
| 「コンピューター生成」っぽく見える | 意図したスタイルに見える                  |

エンコーダーは各形状の色を Oklab 空間で最も近いパレットエントリに量子化します。視覚的マッピングは RGB 距離ナイーブではなく、知覚的に均一です。

## パレットの構築

パレットは `K × 3` バイトのフラットな row-major sRGB バイト配列です。`K ∈ {2, 4, 8, 16, 32, 64, 128, 256, 512, 1024}`（2 から 1024 の 2 の冪）。

::: code-group

```ts [TypeScript]
import { palette, codec } from "arthash";

// hex 文字列から構築 — 最も簡単
const brand = palette.fromHex([
  "#0a0a0a", "#ffffff", "#0284c7", "#22c55e",
  "#f59e0b", "#ef4444", "#a855f7", "#14b8a6",
  "#1e293b", "#f8fafc", "#0369a1", "#16a34a",
  "#d97706", "#dc2626", "#9333ea", "#0d9488",
]);                                                  // K = 16

// [r, g, b] 三組から構築
const grayscale = palette.fromRgb([
  [0, 0, 0], [64, 64, 64], [128, 128, 128], [255, 255, 255],
]);                                                  // K = 4

// 任意の shape codec をラップ
const c = codec.withPalette(codec.triangle({ n: 24 }), brand);
```

```python [Python]
from arthash import Codec
from arthash.palettes import PICO8, GAMEBOY, MORANDI

codec = Codec.triangle(n=24, palette=PICO8)

# 自分で構築する場合
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

::: warning K は 2 の冪でなければなりません
ビットパッキングの計算は `K ∈ {2, 4, 8, …, 1024}` を前提としています。他のサイズを渡すと、エンコード時ではなく codec 構築時に例外が投げられます。
:::

## ビルトインパレット

TypeScript と Python SDK には、よく知られたパレットが小さなセットとして同梱されています：

| 名前       | K  | スタイル                              |
| ---------- | -- | ------------------------------------- |
| `PICO8`    | 16 | PICO-8 コンソールのパレット            |
| `GAMEBOY`  | 4  | 初代 Game Boy DMG のグリーン           |
| `NES`      | 64 | NES Famicom のパレット                 |
| `MORANDI`  | 16 | モランディ風の落ち着いたニュートラル系  |
| `MONO`     | 4  | 黒 / ダークグレー / ライトグレー / 白  |

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

## デザインのヒント

- **K = 16 はスイートスポット。** 形状あたり 4 bit、意味のあるスタイル範囲を覆うのに十分な大きさで、識別性を保つには十分な小ささ。
- **両極を含める。** 良いパレットには少なくとも 1 つの黒に近い色、1 つの白に近い色、適度に彩度のある中間色が必要です。これらがないと、エンコーダーは鋭い境界に使える高コントラストの選択肢を持ちません。
- **RGB ではなく知覚的にクラスタリング。** RGB で等間隔に 16 色を選ぶと洗いざらしのように見えます。[Coolors](https://coolors.co/) や [Lospec](https://lospec.com/palette-list/) が厳選したパレットが効果的です。
- **デコード時に同じパレットが必要。** これは `Codec` 契約の一部であり、**ハッシュバイトストリームには格納されません**。エンコード / デコード両端で共有定数として使い回してください。パレットが画像ごとに変わり、ハッシュと共に運ぶ必要がある場合は、外側のフレームを自分で設計してください（例：パレットバイトをハッシュの先頭に連結する）—SDK はこの種のパッケージング形式を内蔵していません。

## モード間でのパレットの動作

パレットカラーはすべての shape モード（`CIRCLE`、`SQUARE`、`RECT`、`ROTATED_RECT`、`TRIANGLE`）と `PIXEL` に適用されます。`DCT` はパレットを無視します—Oklab DCT 係数空間で量子化を行い、再マッピングするピクセル単位の色を持たないためです。

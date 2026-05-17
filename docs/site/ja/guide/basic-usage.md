# 基本的な使い方

すべてのバインディングが同じ 3 つのプリミティブを公開しています：

- `encode(...)` — ピクセルをハッシュ BLOB に変換
- `decode(...)` — ハッシュを任意のターゲットサイズの RGBA ピクセルに戻す
- `toSvg(...)` — shape モードのハッシュをコンパクトな SVG 文字列としてレンダリング

`Codec` オブジェクトはバイト形式の契約です。同じ `Codec` 値をエンコードとデコード両方に渡す必要があります—バイトストリームに交渉用のヘッダーはありません。

## TypeScript（ブラウザ / Node）

```ts
import { encode, decode, toSvg, codec, Preset, encodeImage, init } from "arthash";

// wasm は初回呼び出し時に自動ロード。オプションでプリロード：
await init();

// 名前付きプリセット — 最も簡単な入口
const c = codec.preset(Preset.DetailTriangle);   // triangle, n=64
const hash = await encode(rgbBytes, width, height, c);
const { w, h, rgba } = await decode(hash, c);
const svg = await toSvg(hash, c, { baseSize: 512, blur: 8 });

// ブラウザの便利関数 — 画像ロード + リサイズ + エンコードを一度に
const hash2 = await encodeImage(imageUrlOrBlob, c);
```

### 同期版

タイトなレンダーループの中で、既に `await init()` を済ませている場合：

```ts
import { encodeSync, decodeSync, toSvgSync, init } from "arthash";

await init();
const hash = encodeSync(rgbBytes, w, h, c);
```

### URL / Blob からのエンコード（ブラウザ専用）

`encodeImage` がパイプライン全体を処理します—fetch、ビットマップへのデコード、codec のサムネイルターゲット（shape モードは長辺 48 px、DCT は ≤ 100 px）へのリサイズ、RGB 抽出、エンコード。

```ts
const hash = await encodeImage("https://example.com/photo.jpg", c);
```

Node では `sharp` / `jimp` / `@napi-rs/canvas` で自分で画像をデコードし、生の RGB バイトを渡してください：

```ts
import sharp from "sharp";
import { encode, codec } from "arthash";

const c = codec.triangle({ n: 64 });
const { data, info } = await sharp("photo.jpg")
  .resize({ width: 48, height: 48, fit: "inside" })
  .removeAlpha()
  .raw()
  .toBuffer({ resolveWithObject: true });

const hash = await encode(new Uint8Array(data), info.width, info.height, c);
```

## Python

```python
from arthash import Codec, Preset, encode, decode, to_svg

# DCT — thumbhash 風のぼやけたプレースホルダー
hash_bytes = encode("photo.jpg")
w, h, rgba = decode(hash_bytes, base_size=256)   # rgba 形状 (h, w, 4)

# 名前付きプリセット
codec = Codec.preset(Preset.DETAIL_TRIANGLE)
hash_bytes = encode("photo.jpg", codec)
svg = to_svg(hash_bytes, codec, base_size=512, blur=8.0)

# ファクトリ + パレット
from arthash.palettes import PICO8
codec = Codec.triangle(n=24, palette=PICO8)
hash_bytes = encode("photo.jpg", codec)
```

`encode()` は `str` パス、`bytes`、`numpy.ndarray`（H×W×3 または H×W×4）、`PIL.Image` インスタンスを受け取ります。

## Rust

```rust
use arthash::{Codec, Preset, encode_rgb, decode, EncodeOptions, DecodeOptions};

// 名前付きプリセット（推奨）
let codec = Preset::DetailTriangle.codec();
let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
let out = decode(&hash, &codec, DecodeOptions::default());

// またはファクトリで構築
let codec = Codec::triangle(64);
// Codec::dct(), Codec::circle(n), Codec::square(n), Codec::rect(n),
// Codec::rotated_rect(n), Codec::pixel(n)
```

## フロントエンドパターン：プログレッシブ画像

典型的な LQIP の配線—SVG を即座にレンダリングし、本物の画像のロードが完了したら入れ替えます。

```vue
<script setup lang="ts">
import { ref, onMounted } from "vue";
import { toSvg, codec, Preset, init } from "arthash";

const props = defineProps<{ src: string; hash: Uint8Array }>();
const placeholder = ref("");
const loaded = ref(false);

onMounted(async () => {
  await init();
  placeholder.value = await toSvg(props.hash, codec.preset(Preset.DetailTriangle), {
    baseSize: 512,
    blur: 8,
  });
});
</script>

<template>
  <div class="img-wrap">
    <div class="placeholder" v-html="placeholder" />
    <img :src="src" @load="loaded = true" :class="{ loaded }" />
  </div>
</template>

<style scoped>
.img-wrap { position: relative; }
.placeholder, img { position: absolute; inset: 0; }
img { opacity: 0; transition: opacity .25s; }
img.loaded { opacity: 1; }
</style>
```

静的サイトのビルド時には、Python または Rust SDK でハッシュをビルド時に生成し、データ層にインラインで埋め込んでください。

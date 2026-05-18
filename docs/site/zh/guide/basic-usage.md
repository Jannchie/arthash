# 基础用法

每种绑定都提供同样的三个原语：

- `encode(...)`：把像素编成一段 hash
- `decode(...)`：把 hash 还原成任意尺寸的 RGBA 像素
- `toSvg(...)`：把 shape 模式的 hash 渲染成紧凑的 SVG 字符串

`Codec` 是字节格式契约。encode 与 decode 必须传入同一个 `Codec`——字节流里没有 header 来协商。

## TypeScript（浏览器 / Node）

```ts
import { encode, decode, toSvg, codec, Preset, encodeImage, init } from "arthash";

// wasm 首次调用时自动加载，也可以提前预加载：
await init();

// 命名预设——最简单的入口
const c = codec.preset(Preset.LargeTriangle);    // triangle, n=64
const hash = await encode(rgbBytes, width, height, c);
const { w, h, rgba } = await decode(hash, c);
const svg = await toSvg(hash, c, { baseSize: 512, blur: 8 });

// 浏览器便捷入口：加载图片 + 缩放 + 编码一步完成
const hash2 = await encodeImage(imageUrlOrBlob, c);
```

### 同步变体

在渲染热路径里、并且已经 `await init()` 过了：

```ts
import { encodeSync, decodeSync, toSvgSync, init } from "arthash";

await init();
const hash = encodeSync(rgbBytes, w, h, c);
```

### 从 URL / Blob 编码（仅浏览器）

`encodeImage` 处理完整链路——fetch、decode 成 bitmap、缩放到 codec 的缩略图目标尺寸（shape 模式 48 px 长边、DCT ≤ 100 px）、抽 RGB、编码。

```ts
const hash = await encodeImage("https://example.com/photo.jpg", c);
```

Node 环境请自己用 `sharp` / `jimp` / `@napi-rs/canvas` 解码图片，然后传原始 RGB：

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

# DCT —— thumbhash 风格的模糊占位图
hash_bytes = encode("photo.jpg")
w, h, rgba = decode(hash_bytes, base_size=256)   # rgba 形状 (h, w, 4)

# 命名预设
codec = Codec.preset(Preset.LARGE_TRIANGLE)
hash_bytes = encode("photo.jpg", codec)
svg = to_svg(hash_bytes, codec, base_size=512, blur=8.0)

# 工厂方法 + 调色板
from arthash.palettes import PICO8
codec = Codec.triangle(n=24, palette=PICO8)
hash_bytes = encode("photo.jpg", codec)
```

`encode()` 接受 `str` 路径、`bytes`、`numpy.ndarray`（H×W×3 或 H×W×4）、`PIL.Image` 实例。

## Rust

```rust
use arthash::{Codec, Preset, encode_rgb, decode, EncodeOptions, DecodeOptions};

// 命名预设（推荐）
let codec = Preset::LargeTriangle.codec();
let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
let out = decode(&hash, &codec, DecodeOptions::default());

// 或者用工厂方法
let codec = Codec::triangle(64);
// Codec::dct(), Codec::circle(n), Codec::square(n), Codec::rect(n),
// Codec::rotated_rect(n), Codec::pixel(n)
```

## 前端模式：渐进式图片

典型的 LQIP 接线——立即渲染 SVG，真图加载完毕后切换。

```vue
<script setup lang="ts">
import { ref, onMounted } from "vue";
import { toSvg, codec, Preset, init } from "arthash";

const props = defineProps<{ src: string; hash: Uint8Array }>();
const placeholder = ref("");
const loaded = ref(false);

onMounted(async () => {
  await init();
  placeholder.value = await toSvg(props.hash, codec.preset(Preset.LargeTriangle), {
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

静态站点构建场景，可以在构建期用 Python 或 Rust SDK 生成 hash，再内联到数据层。

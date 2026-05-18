# Basic Usage

Every binding exposes the same three primitives:

- `encode(...)` — turn pixels into a hash blob
- `decode(...)` — turn the hash back into RGBA pixels at any target size
- `toSvg(...)` — render a shape-mode hash as a compact SVG string

The `Codec` object is the byte-format contract. The same `Codec` value MUST be
passed to both encode and decode — there's no header to negotiate.

## TypeScript (browser / Node)

```ts
import { encode, decode, toSvg, codec, Preset, encodeImage, init } from "arthash";

// Wasm loads automatically on first call. Optionally preload:
await init();

// Named preset — easiest entry
const c = codec.preset(Preset.LargeTriangle);    // triangle, n=64
const hash = await encode(rgbBytes, width, height, c);
const { w, h, rgba } = await decode(hash, c);
const svg = await toSvg(hash, c, { baseSize: 512, blur: 8 });

// Browser convenience — load image, resize, encode in one call
const hash2 = await encodeImage(imageUrlOrBlob, c);
```

### Synchronous variants

If you're in a tight render loop and you've already `await init()`'d:

```ts
import { encodeSync, decodeSync, toSvgSync, init } from "arthash";

await init();
const hash = encodeSync(rgbBytes, w, h, c);
```

### Encoding from a URL / Blob (browser only)

`encodeImage` handles the full pipeline — fetch, decode to bitmap, resize to
the codec's thumbnail target (48 px long-edge for shape modes, ≤ 100 px for
DCT), extract RGB, encode.

```ts
const hash = await encodeImage("https://example.com/photo.jpg", c);
```

For Node, decode the image yourself with `sharp` / `jimp` / `@napi-rs/canvas`
and pass the raw RGB bytes:

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

# DCT — thumbhash-style blurry placeholder
hash_bytes = encode("photo.jpg")
w, h, rgba = decode(hash_bytes, base_size=256)   # rgba shape (h, w, 4)

# Named preset
codec = Codec.preset(Preset.LARGE_TRIANGLE)
hash_bytes = encode("photo.jpg", codec)
svg = to_svg(hash_bytes, codec, base_size=512, blur=8.0)

# Factory + palette
from arthash.palettes import PICO8
codec = Codec.triangle(n=24, palette=PICO8)
hash_bytes = encode("photo.jpg", codec)
```

`encode()` accepts `str` paths, `bytes`, `numpy.ndarray` (H×W×3 or H×W×4), and
`PIL.Image` instances.

## Rust

```rust
use arthash::{Codec, Preset, encode_rgb, decode, EncodeOptions, DecodeOptions};

// Named preset (recommended)
let codec = Preset::LargeTriangle.codec();
let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
let out = decode(&hash, &codec, DecodeOptions::default());

// Or build by factory
let codec = Codec::triangle(64);
// Codec::dct(), Codec::circle(n), Codec::square(n), Codec::rect(n),
// Codec::rotated_rect(n), Codec::pixel(n)
```

## Frontend pattern: progressive image

A typical LQIP wire-up — render the SVG immediately, swap to the real image
once it loads.

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

For static-site builds, generate hashes at build time with the Python or Rust
SDK and inline them into your data layer.

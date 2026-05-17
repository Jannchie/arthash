# Installation

arthash ships three first-party SDKs from a single Rust core. All bindings
share the same byte format — a hash produced by one decodes on every other.

## TypeScript (npm)

Runs in the browser and Node ≥ 18. The wasm-bindgen package is what most
users want.

::: code-group

```sh [pnpm]
pnpm add arthash
```

```sh [npm]
npm install arthash
```

```sh [yarn]
yarn add arthash
```

```sh [bun]
bun add arthash
```

:::

```ts
import { encode, codec, Preset } from "arthash";
const c = codec.preset(Preset.PlaceholderTriangle);
const hash = await encode(rgbBytes, w, h, c);
```

The wasm module loads automatically on first call. If you want to control
timing (e.g. preload before a render loop), call `await init()` explicitly.

### Footprint

| Artifact                              |    raw |  gzip | brotli |
| ------------------------------------- | -----: | ----: | -----: |
| `arthash_wasm_bg.wasm` (core)         | 218 KB | 82 KB |  67 KB |
| `arthash_wasm.js` (wasm-bindgen glue) |  14 KB |  4 KB |   3 KB |
| `dist/index.js` + `palettes.js` (SDK) |  21 KB |  7 KB |   6 KB |
| **Total over the wire**               | 253 KB | 93 KB |  76 KB |

The wasm is fetched once and HTTP-cached. The JS portion is tree-shakeable —
importing only `decode` drops the encode / `toSvg` / `encodeImage` paths from
your bundle (wasm exports stay intact). For frontends that only render
placeholders, this is **~67 KB brotli of wasm up front and ~6 KB of SDK in
your bundle**.

## Python (PyPI)

A PyO3 wheel — no Python rewrite, the heavy lifting still happens in Rust.

::: code-group

```sh [uv]
uv add arthash
```

```sh [pip]
pip install arthash
```

:::

```python
from arthash import Codec, encode, decode, to_svg
hash_bytes = encode("photo.jpg")       # DCT default
w, h, rgba = decode(hash_bytes, base_size=256)
```

The `encode()` helper accepts `str` paths, `bytes`, `numpy` arrays, and
`PIL.Image` instances. Decode returns plain numpy if available.

## Rust (crates.io)

The canonical implementation, used as the source of truth for the byte format
spec.

```sh
cargo add arthash
```

```rust
use arthash::{Codec, Preset, encode_rgb, decode, EncodeOptions, DecodeOptions};

let codec = Preset::DetailTriangle.codec();
let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
let out = decode(&hash, &codec, DecodeOptions::default());
// out.width / out.height / out.rgba
```

## From source

The repository is a pnpm + cargo + uv polyglot workspace.

```sh
git clone https://github.com/Jannchie/arthash.git
cd arthash

pnpm install
pnpm run build              # builds TS + wasm + Python wheel + Rust crate

pnpm run build:ts           # TypeScript SDK only
pnpm run build:py           # Python wheel only (requires uv + maturin)
pnpm run build:rs           # Rust crate only
```

The TS build depends on the Rust toolchain (`wasm32-unknown-unknown` target) and
`wasm-bindgen-cli`. The CLI version MUST match the `wasm-bindgen` library
version in `packages/arthash-ts/wasm/Cargo.toml`:

```sh
rustup target add wasm32-unknown-unknown
cargo install -f --locked wasm-bindgen-cli --version 0.2.121
```

## Compatibility matrix

| Binding    | Minimum runtime            | Native dependency                 |
| ---------- | -------------------------- | --------------------------------- |
| TypeScript | Node ≥ 18, modern browsers | none (wasm bundled)               |
| Python     | CPython 3.9+               | none (manylinux wheels pre-built) |
| Rust       | edition 2021, MSRV 1.75    | none                              |

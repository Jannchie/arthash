# 安装

arthash 共享同一份 Rust 内核，提供三个一等公民 SDK。所有绑定遵循同一份字节规范——任何一端产出的 hash 都能被其他端解码。

## TypeScript（npm）

适配浏览器与 Node ≥ 18，绝大多数用户应该选这个包。

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

wasm 模块在首次调用时自动加载。如想自己控制时机（比如渲染循环开始前预加载），可显式 `await init()`。

### 体积

| 资源                                  |    raw |  gzip | brotli |
| ------------------------------------- | -----: | ----: | -----: |
| `arthash_wasm_bg.wasm`（核心）        | 218 KB | 82 KB |  67 KB |
| `arthash_wasm.js`（wasm-bindgen glue） |  14 KB |  4 KB |   3 KB |
| `dist/index.js` + `palettes.js`（SDK） |  21 KB |  7 KB |   6 KB |
| **总计（网络传输）**                  | 253 KB | 93 KB |  76 KB |

wasm 一次性下载并被 HTTP 缓存。SDK 是 tree-shakable 的——如果只 import `decode`，打包器会把 encode / `toSvg` / `encodeImage` 全部丢掉（wasm 导出不受影响）。对只渲染占位图的前端来说，**首次 ~67 KB brotli 的 wasm + ~6 KB SDK 进 bundle** 是常见结果。

## Python（PyPI）

PyO3 wheel——没有任何 Python 重写，重活仍由 Rust 干。

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
hash_bytes = encode("photo.jpg")       # DCT 默认
w, h, rgba = decode(hash_bytes, base_size=256)
```

`encode()` 接受 `str` 路径、`bytes`、`numpy` 数组、`PIL.Image` 实例。decode 在 numpy 可用时返回纯 numpy 数组。

## Rust（crates.io）

标准实现，字节规范以 Rust 实现为准。

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

## 从源码构建

仓库是 pnpm + cargo + uv 多语言 workspace。

```sh
git clone https://github.com/Jannchie/arthash.git
cd arthash

pnpm install
pnpm run build              # 同时构建 TS + wasm + Python wheel + Rust crate

pnpm run build:ts           # 只构建 TypeScript SDK
pnpm run build:py           # 只构建 Python wheel（需要 uv + maturin）
pnpm run build:rs           # 只构建 Rust crate
```

TS 构建需要 Rust 工具链（`wasm32-unknown-unknown` target）以及 `wasm-bindgen-cli`。CLI 版本必须与 `packages/arthash-ts/wasm/Cargo.toml` 中的 `wasm-bindgen` 库版本一致：

```sh
rustup target add wasm32-unknown-unknown
cargo install -f --locked wasm-bindgen-cli --version 0.2.121
```

## 兼容矩阵

| 绑定       | 最低运行时                 | 原生依赖                           |
| ---------- | -------------------------- | ---------------------------------- |
| TypeScript | Node ≥ 18，主流浏览器       | 无（wasm 已打包）                  |
| Python     | CPython 3.9+               | 无（已预编 manylinux wheel）       |
| Rust       | edition 2021, MSRV 1.75    | 无                                 |

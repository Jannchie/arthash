# インストール

arthash は単一の Rust コアから 3 つのファーストパーティ SDK を提供します。すべてのバインディングは同じバイト形式を共有—1 つで作ったハッシュは他のすべてでデコードできます。

## TypeScript（npm）

ブラウザと Node ≥ 18 で動作。ほとんどのユーザーはこのパッケージで OK です。

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
const c = codec.preset(Preset.SmallTriangle);
const hash = await encode(rgbBytes, w, h, c);
```

wasm モジュールは初回呼び出し時に自動ロードされます。タイミングを制御したい場合（例：レンダーループ前にプリロード）は `await init()` を明示的に呼び出してください。

### フットプリント

| アーティファクト                      |    raw |  gzip | brotli |
| ------------------------------------- | -----: | ----: | -----: |
| `arthash_wasm_bg.wasm`（コア）         | 218 KB | 82 KB |  67 KB |
| `arthash_wasm.js`（wasm-bindgen glue） |  14 KB |  4 KB |   3 KB |
| `dist/index.js` + `palettes.js`（SDK） |  21 KB |  7 KB |   6 KB |
| **合計（ネットワーク転送）**          | 253 KB | 93 KB |  76 KB |

wasm は一度フェッチされて HTTP キャッシュされます。JS 部分は tree-shake 可能—`decode` だけインポートすれば、エンコード / `toSvg` / `encodeImage` は SDK バンドルから削除されます（wasm エクスポートはそのまま）。プレースホルダーだけ描画するフロントエンドなら、**初回 ~67 KB brotli の wasm + ~6 KB の SDK** が典型です。

## Python（PyPI）

PyO3 wheel—Python の書き直しは無く、重い処理は引き続き Rust で行われます。

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
hash_bytes = encode("photo.jpg")       # DCT デフォルト
w, h, rgba = decode(hash_bytes, base_size=256)
```

`encode()` は `str` パス、`bytes`、`numpy` 配列、`PIL.Image` インスタンスを受け取ります。decode は numpy が利用可能なら numpy 配列を返します。

## Rust（crates.io）

正規実装。バイト形式仕様の真実の源として使われます。

```sh
cargo add arthash
```

```rust
use arthash::{Codec, Preset, encode_rgb, decode, EncodeOptions, DecodeOptions};

let codec = Preset::LargeTriangle.codec();
let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
let out = decode(&hash, &codec, DecodeOptions::default());
// out.width / out.height / out.rgba
```

## ソースからビルド

リポジトリは pnpm + cargo + uv のマルチ言語 workspace です。

```sh
git clone https://github.com/Jannchie/arthash.git
cd arthash

pnpm install
pnpm run build              # TS + wasm + Python wheel + Rust crate を全てビルド

pnpm run build:ts           # TypeScript SDK のみ
pnpm run build:py           # Python wheel のみ（uv + maturin 必要）
pnpm run build:rs           # Rust crate のみ
```

TS ビルドには Rust ツールチェーン（`wasm32-unknown-unknown` ターゲット）と `wasm-bindgen-cli` が必要です。CLI のバージョンは `packages/arthash-ts/wasm/Cargo.toml` の `wasm-bindgen` ライブラリのバージョンと一致しなければなりません：

```sh
rustup target add wasm32-unknown-unknown
cargo install -f --locked wasm-bindgen-cli --version 0.2.121
```

## 互換性マトリクス

| バインディング | 最低ランタイム                | ネイティブ依存                       |
| -------------- | ----------------------------- | ------------------------------------ |
| TypeScript     | Node ≥ 18、モダンブラウザ      | なし（wasm 同梱）                    |
| Python         | CPython 3.9+                  | なし（manylinux wheel をビルド済み）  |
| Rust           | edition 2021、MSRV 1.75       | なし                                 |

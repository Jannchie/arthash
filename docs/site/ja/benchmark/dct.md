# DCT vs thumbhash

arthash の `DCT` モードは [thumbhash](https://evanw.github.io/thumbhash/) と [blurhash](https://blurha.sh/) と同じニッチを直接狙います：とても小さい（~20 B）ぼやけたプレースホルダー。同じバイト予算で、画質が高く、エンコード / デコードともに高速。

## エンコード @ 24 B 出力

### JS（baseline = thumbhash-js）

| 実装                | median |     vs baseline | バイト |
| ------------------- | -----: | --------------: | -----: |
| arthash · ts (wasm) | 279 µs |   **1.9× 高速** |     24 |
| thumbhash · JS      | 532 µs | 1.0× (baseline) |     24 |

### ネイティブ（baseline = thumbhash-rust）

| 実装                      | median |     vs baseline | バイト |
| ------------------------- | -----: | --------------: | -----: |
| arthash · Python (PyO3)   | 242 µs |  **1.27× 高速** |     24 |
| arthash · Rust            | 243 µs |  **1.27× 高速** |     24 |
| thumbhash · Rust (evanw)  | 308 µs | 1.0× (baseline) |     24 |
| thumbhash · Go (n16f.net) | 415 µs |        0.74× 遅 |     24 |
| thumbhash · Python (PyPI) |  25 ms |       0.012× 遅 |     24 |

arthash Rust と PyO3 は実質同速（min ~228 µs / median ~243 µs）—PyO3 は薄い GIL/PyBytes ラッパーを追加するだけで、その µs オーダーのオーバーヘッドはバッチ計測ノイズに埋もれます。

## デコード @ DCT 24 B → RGBA

### JS（baseline = thumbhash-js のデフォルト ~32 px 出力）

| 実装                | 出力サイズ |       median |     vs baseline |
| ------------------- | ---------: | -----------: | --------------: |
| arthash · ts (wasm) |     ~32 px |       116 µs |   **1.4× 高速** |
| thumbhash · JS      |     ~32 px |       165 µs | 1.0× (baseline) |
| arthash · ts (wasm) |     256 px |      6.69 ms |   *(相手不対応)* |
| arthash · ts (wasm) |     512 px |     26.22 ms |   *(相手不対応)* |
| thumbhash · JS      |       256+ | API 非対応   |               — |

::: tip 本番でこれが重要な理由
thumbhash の JS デコード API は ~32 px しか出力しません。大きくしたければ CSS で拡大する（ぼやける）しかありません。arthash は IDCT で直接任意のサイズに落とし、クライアント側のアップサンプリングを省きます。
:::

### ネイティブ @ 256 px（baseline = thumbhash-go @ 256）

| 実装                    |  median |     vs baseline |
| ----------------------- | ------: | --------------: |
| arthash · Rust          | 2.06 ms |   **5.9× 高速** |
| arthash · Python (PyO3) | 2.60 ms |   **4.7× 高速** |
| thumbhash · Go @ 256    | 12.2 ms | 1.0× (baseline) |

thumbhash の Rust crate は自分のデフォルト ~32 px 出力では arthash より速い；しかし表示サイズのバッファ（プレースホルダーの実シナリオ）を要求した瞬間、arthash が約 6× で逆転します。

## arthash DCT が高速な理由

- **二段階アップサンプリングなし。** arthash は IDCT で目標サイズに直接落とし、thumbhash が 32 px デフォルトから脱するために必要な双線形アップサンプルを省きます。
- **シングルパスの Oklab 逆量子化。** 別途の sRGB → リニア段階なし。IDCT は知覚空間で実行され、最終の sRGB クリップは出力ライターに融合されています。
- **可能なら wasm SIMD を有効化。** 同梱の wasm は `wasm32-unknown-unknown` ターゲットフラグで SIMD を有効化；モダンブラウザと Node 22 は自動的に高速パスを取ります。

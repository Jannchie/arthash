# ベンチマーク

以下のすべての数字は同一マシン、100×100 RGB 入力で計測しています。

- **JS** の数値は [`bench/js-cross/`](https://github.com/Jannchie/arthash/tree/main/bench/js-cross) を Node 22 で実測。生 NDJSON は [`docs/benchmarks/js_cross_*.ndjson`](https://github.com/Jannchie/arthash/tree/main/docs/benchmarks)。
- **ネイティブ** の数値は [`docs/benchmarks/CROSS_IMPL.md`](https://github.com/Jannchie/arthash/blob/main/docs/benchmarks/CROSS_IMPL.md)。
- 表は速度順、*vs baseline* 列 = baseline 時間 / 行の時間—1 を超えれば行の方が速い。

## ハイライト

| シナリオ                          | 最良結果                                                      |
| --------------------------------- | ------------------------------------------------------------- |
| 24 B DCT エンコード (JS)          | thumbhash-js より **1.9× 高速**                               |
| 24 B DCT エンコード (ネイティブ)  | 最速の非 arthash 実装より **1.27× 高速**                      |
| DCT デコード @ 256 px (ネイティブ) | thumbhash-go @ 256 より **5.9× 高速**                         |
| TRIANGLE n=64 vs sqip (JS)        | エンコード **67× 高速**、出力 **9× 小**                       |
| 17 B プレースホルダー PSNR        | DCT @ 17 B = 23.3 dB — thumbhash @ 17 B より 0.4 dB 高い      |

詳細：

- [**DCT vs thumbhash**](./dct) — 同じバイト予算で arthash が優位な理由。
- [**Shape vs sqip**](./shape) — shape モードがスピードとサイズの両方で勝つ仕組み。
- [**画質比較**](./quality) — モード別の PSNR vs バイト数。

## arthash が勝つ理由

- **ヘッダーなしのバイトストリーム。** マジックナンバー、モードタグ、ビット幅なし—すべての bit が画像情報。
- **チャンネル別適応 AC スケール。** 各画像が独自の量子化テーブルを持つが、ヘッダーで bit を消費しない。
- **積分画像 + SSE 増分探索。** shape モードの探索コストは `n` に対して亜線形；sqip は線形 + プリミティブごとに IPC バウンド。
- **直接ターゲットサイズへデコード。** thumbhash-js は ~32 px しか出力しない；arthash は IDCT で直接表示バッファに落とす。

完整なアルゴリズムの物語は [arthash が bit を節約する仕組み](../guide/introduction#arthash-が-bit-を節約する仕組み) を参照。

## 再現方法

```sh
# JS bench (Node 22)
cd bench/js-cross
pnpm install
pnpm run bench           # docs/benchmarks/js_cross_*.ndjson を出力

# ネイティブ bench
pnpm run bench:rs        # Rust
pnpm run bench:py        # Python via PyO3
pnpm run bench:binding   # クロスバインディング
```

以下の数値は同マシン再計測で ±5% 以内に安定します。

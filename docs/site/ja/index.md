---
layout: home

hero:
  name: arthash
  text: コンパクトなプレースホルダー画像ハッシュ
  tagline: 1 枚あたり 17 B 〜 400 B。本物の画像が読み込まれる間、認識可能なプレビューを表示するのに十分なサイズ。コアは Rust 製、Python / TypeScript で同じバイト形式を共有します。
  actions:
    - theme: brand
      text: はじめる
      link: /ja/guide/introduction
    - theme: alt
      text: GitHub で見る
      link: https://github.com/Jannchie/arthash

features:
  - icon: 🔤
    title: 小さなハッシュ、鮮明なプレビュー
    details: 17 B の DCT プレースホルダーは thumbhash より PSNR +0.4 dB。シェイプモードは sqip の 1/9 〜 1/16 のサイズで SVG プレビューを生成します。
  - icon: ⚡
    title: あらゆるランタイムで高速
    details: JS のエンコード 1.9× / デコード 1.4×（thumbhash-js 比）。ネイティブ Rust + PyO3 は表示サイズでのデコードで thumbhash-go より 5.9× / 4.7× 高速。
  - icon: 🎨
    title: 7 種類の codec モード
    details: DCT、PIXEL、CIRCLE、SQUARE、RECT、ROTATED_RECT、TRIANGLE。外部パレットで色を 4 bit まで圧縮し、一貫した視覚スタイルを付与可能。
  - icon: 📦
    title: 共有された一つの仕様
    details: Rust crate、PyO3 wheel、wasm-bindgen パッケージが生成するハッシュはバイト単位で互換。どのバインディングで作っても他のすべてでデコード可能。
  - icon: 🌐
    title: ブラウザ対応
    details: 初回ロードは wasm ~67 KB brotli（以降は HTTP キャッシュ）、バンドルに乗る SDK は ~6 KB。リクエスト時にブラウザで直接エンコードできます。
  - icon: 🔧
    title: ヘッダーなしのバイトストリーム
    details: マジックナンバーもモードタグもありません。すべての bit が画像情報—Codec はエンコードとデコード間の合意です。
---

## 30 秒で味見

```ts
import { encode, decode, toSvg, codec, Preset } from "arthash";

const c = codec.preset(Preset.LargeTriangle);    // triangle, n=64
const hash = await encode(rgbBytes, width, height, c);
//   → Uint8Array(395)  — 画像全体が 395 バイトの BLOB に

const svg = await toSvg(hash, c, { baseSize: 512, blur: 8 });
//   → '<svg xmlns="..." viewBox="...">...</svg>'  — そのまま LQIP として埋め込める
```

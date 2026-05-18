# プログレッシブ・ディゾルブアニメーション

hash プレースホルダーを形状ごとにフェードアウトさせて、実画像のロード完了
に合わせて溶かすパターン。HTML / CSS / JS のみ—フレームワーク依存なし、
追加の SDK メソッドも不要。必要な部分だけコピーして使ってください。

## レシピ

```js
import { toSvg, codec, init } from "arthash";

// 1. アプリ起動時に wasm をプリウォーム。最初のタイル decode がモジュール
//    ロードのコストを払わずに済む。
await init();

// 2. プレースホルダーを描画。サムネイル表示では角丸が見えるので、
//    rect / square / rotrect codec には cornerRadius を渡す。
const c = codec.rect({ n: 32 });
const svg = await toSvg(hashBytes, c, { style: { cornerRadius: 1 } });

const wrapper = document.querySelector(".placeholder");
wrapper.innerHTML = svg;

// 3. 子要素をサイズ別 4 ティアにグループ化。Chromium では「形状ごとの
//    opacity アニメーション」が形状ごとの合成レイヤーを作る—64 形状 × 50
//    タイル = 3200+ レイヤーで画面が固まる。4 ティアにまとめて 16 分の 1。
const TIERS = 4;
const REVEAL_TOTAL_MS = 560;

function groupShapesByArea(svgEl, tiers) {
  const shapes = [...svgEl.children].filter(
    (n) => n.tagName !== "filter" && n.tagName !== "path"  // 背景 path 除外
  );
  shapes.sort((a, b) => {
    const ab = a.getBBox(), bb = b.getBBox();
    return bb.width * bb.height - ab.width * ab.height;
  });
  const groups = [];
  const frag = new DocumentFragment();
  for (let i = 0; i < tiers; i++) {
    const g = document.createElementNS("http://www.w3.org/2000/svg", "g");
    g.classList.add("tier");
    g.style.setProperty("--d", `${(i / tiers) * REVEAL_TOTAL_MS}ms`);
    groups.push(g);
    frag.appendChild(g);
  }
  // 4. DocumentFragment で <g> をまとめて移動—生きた <svg> に個別に
  //    appendChild するとレイアウト無効化が毎回走る。
  shapes.forEach((s, i) => {
    groups[Math.min(tiers - 1, Math.floor((i / shapes.length) * tiers))]
      .appendChild(s);
  });
  svgEl.appendChild(frag);
  return groups;
}

const svgEl = wrapper.querySelector("svg");
groupShapesByArea(svgEl, TIERS);
let svgGrouped = true;

// 5. 競合状態：**画像ロード完了 AND SVG グループ化完了** の両方を待ってか
//    らフェード開始。さもないと CSS アニメーションの実開始がグループ化に
//    かかった時間ぶんずれるが「N ms 後に wrapper を削除」タイマーは元の
//    スケジュールで発火 → フェード途中で wrapper unmount → 視覚的なポップ。
let imageLoaded = false;

img.addEventListener("load", () => {
  imageLoaded = true;
  if (svgGrouped) startDissolve();
});

function startDissolve() {
  wrapper.classList.add("dissolving");
  setTimeout(() => wrapper.remove(), REVEAL_TOTAL_MS + 200);
}
```

```css
.placeholder.dissolving .tier {
  animation: tier-fade 140ms ease-out forwards;
  animation-delay: var(--d, 0ms);
}
@keyframes tier-fade {
  from { opacity: 1 }
  to { opacity: 0 }
}
```

## 落とし穴

1. **「形状ごとの opacity アニメーション」= Chromium で形状ごとの合成
   レイヤー**。50 タイル × 64 形状 = 3200+ レイヤーで画面が動かなくなる。
   小さなティア数（デフォルト 4）にバケットする。
2. **`steps(1, end)` ではレイヤー昇格を回避できない**—実測済み。timing
   function は compositor の判断に影響しない。レイヤー数を減らすには
   アニメーション対象の要素数を減らすしかない。
3. **競合状態**：SVG マウント＆グループ化が終わる前に `dissolving` を立
   てると、CSS アニメーション開始がグループ化に取られた時間だけ遅れるの
   に「N ms 後 wrapper 削除」タイマーは予定通り発火 → フェード途中で
   wrapper が外れて視覚的にポップ。**`image-loaded` と `svg-grouped` の
   両方が成立してからトリガーする。**
4. **スクロール中はアニメーション一時停止**で仮想スクロールを滑らかに：
   ```js
   let t;
   window.addEventListener("scroll", () => {
     document.documentElement.classList.add("scrolling");
     clearTimeout(t);
     t = setTimeout(() => {
       document.documentElement.classList.remove("scrolling");
     }, 150);
   }, { capture: true, passive: true });
   ```
   ```css
   html.scrolling .placeholder.dissolving .tier {
     animation-play-state: paused;
   }
   ```
5. **アプリ起動時に wasm をプリウォーム**：`init()` を一度呼べば、最初の
   タイル decode が wasm モジュールロードを待たずに済む。
6. **`DocumentFragment` でノード移動をバッチ化**—生きた `<svg>` に個別に
   appendChild するとレイアウト無効化が毎回走る。
7. **実画像が wasm decode 完了より前に到着したら、アニメーションをスキ
   ップする**：decode コールバックの中で `revealed` フラグを立て、すでに
   立っていれば `startDissolve()` を呼ばない。

## プレースホルダーのサイズ選び

| ユースケース | 推奨 |
|---|---|
| ギャラリーサムネイル、50+ 同時表示 | `codec.rect({ n: 32 })` + `cornerRadius: 1` |
| Hero / Above-the-fold 画像 | `codec.triangle({ n: 24 })` |
| 最小可読プレースホルダー | `codec.dct()`（~21 B） |

`n=32` の rect（~150 B、33 SVG エレメント）が高密度ギャラリーのスイート
スポット—playground デフォルトの `n=48` より小さく、`n=64` とは視覚的に
区別がつく。

## なぜ SDK に `<ArthashPlaceholder>` コンポーネントを入れないのか

arthash SDK はフレームワーク非依存を意図的に維持しています。ティア数、フ
ェード時間、スクロール一時停止のヒューリスティック、高速ロード時のスキ
ップ閾値—こうした UX の判断はアプリ層の責務であって encode/decode ライブ
ラリの責務ではありません。このレシピを自前のコンポーネントにコピーして
パラメータをチューニングしてください—完全な制御権を持ったまま、また一つ
のフレームワーク依存パッケージを増やさずに済みます。

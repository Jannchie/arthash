# 渐进溶解动画

把 hash 占位图按形状一片片淡出，等真图加载完。纯 HTML / CSS / JS，不绑定框
架、不需要新增 SDK 方法。按需复用。

## 配方

```js
import { toSvg, codec, init } from "arthash";

// 1. 应用启动时预热 wasm，第一块瓦片不用付模块加载的成本。
await init();

// 2. 渲染占位 SVG。缩略图尺寸下圆角能看出来——rect / square / rotrect codec
//    可以传 cornerRadius。
const c = codec.rect({ n: 32 });
const svg = await toSvg(hashBytes, c, { style: { cornerRadius: 1 } });

const wrapper = document.querySelector(".placeholder");
wrapper.innerHTML = svg;

// 3. 把子节点按大小分到 4 个 tier。Chromium 下「每形状一个 opacity 动画」
//    会给每形状创建一个合成层——64 形状 × 50 瓦片 = 3200+ 层，页面卡死。
//    按 4 tier 分桶，层数除以 16。
const TIERS = 4;
const REVEAL_TOTAL_MS = 560;

function groupShapesByArea(svgEl, tiers) {
  const shapes = [...svgEl.children].filter(
    (n) => n.tagName !== "filter" && n.tagName !== "path"  // 跳过背景 path
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
  // 4. DocumentFragment 批量挪 <g>——逐个 appendChild 到活的 <svg> 会触发
  //    每次挪动一次 layout 失效。
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

// 5. 竞态：必须 **图片加载完 AND SVG 分组完** 两个条件都满足才能触发淡出。
//    否则 CSS 动画起跑被分组时间推迟，但「N ms 后移除 wrapper」的定时器按
//    原计划触发 → wrapper 在淡出中途被 unmount → 视觉跳变。
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

## 坑

1. **「每形状一个 opacity 动画」= Chromium 每形状一个合成层**。屏幕上 50 个
   瓦片 × 64 形状 = 3200+ 层，页面卡到没法用。一定要按 tier 分组（默认 4
   个就够）。
2. **`steps(1, end)` 不能阻止合成层创建**——实测过，timing function 不影响
   compositor 决策。要降低层数，必须降低被动画的元素数。
3. **竞态**：如果在 SVG 还没挂载好就把 `dissolving` 加上，CSS 动画的实际起
   跑会被分组耗时推迟，但「N ms 后移除 wrapper」的定时器按原计划触发，
   wrapper 在淡出途中被 unmount → 视觉跳变。**等 `image-loaded` 和
   `svg-grouped` 都就绪再触发。**
4. **滚动期间暂停动画**让虚拟滚动更顺：
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
5. **应用启动时预热 wasm**：调一次 `init()`，首块瓦片解码就不用等 wasm
   模块加载。
6. **用 `DocumentFragment` 批量挪节点**——逐个 appendChild 到活的 `<svg>`
   每次都触发 layout 失效。
7. **如果真图在 wasm 解码完成之前就到了，跳过动画**：在解码回调里翻一个
   `revealed` flag，如果它已经被翻过就别再 `startDissolve()`。

## 占位图大小怎么选

| 场景 | 推荐 |
|---|---|
| 画廊缩略图，50+ 同屏 | `codec.rect({ n: 32 })` + `cornerRadius: 1` |
| Hero / 首屏图 | `codec.triangle({ n: 24 })` |
| 最小可读占位 | `codec.dct()`（~21 B） |

`n=32` 的 rect（~150 B、33 个 SVG 元素）是高密度画廊的甜点——比 playground
默认的 `n=48` 小，又比 `n=64` 视觉上有区分度。

## 为什么 SDK 不内置 `<ArthashPlaceholder>` 组件？

arthash SDK 故意保持 framework-free。tier 数量、淡出时长、滚动暂停启发式、
快加载跳过阈值这些 UX 决策属于应用层，不属于一个 encode/decode 库。把这套
配方拷到你自己的组件里、按自己的需求调参——你拿到全部控制权，不用引入又一
个框架绑定的依赖。

# Progressive dissolve animation

A pattern for fading a hash placeholder shape-by-shape as the real image
finishes loading. Pure HTML / CSS / JS — no framework lock-in, no extra
SDK methods. Copy what you need.

## The recipe

```js
import { toSvg, codec, init } from "arthash";

// 1. Pre-warm wasm at app boot so the first tile decode doesn't pay the
//    module load cost.
await init();

// 2. Render the placeholder. Rounded corners are visible at thumbnail
//    sizes — pass cornerRadius for rect / square / rotrect codecs.
const c = codec.rect({ n: 32 });
const svg = await toSvg(hashBytes, c, { style: { cornerRadius: 1 } });

const wrapper = document.querySelector(".placeholder");
wrapper.innerHTML = svg;

// 3. Group children into 4 size tiers. Per-shape opacity animations
//    create per-shape GPU layers in Chromium — at 64 shapes × 50 tiles
//    that's 3200+ layers and the page stutters. Bucketing into 4 tier
//    groups cuts layer count by 16×.
const TIERS = 4;
const REVEAL_TOTAL_MS = 560;

function groupShapesByArea(svgEl, tiers) {
  const shapes = [...svgEl.children].filter(
    (n) => n.tagName !== "filter" && n.tagName !== "path"  // skip bg path
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
  // 4. Batch DOM commits with DocumentFragment — moving each <g> directly
  //    into the live <svg> causes a layout invalidation per move.
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

// 5. Race condition: don't start the fade until BOTH the image is loaded
//    AND the SVG is grouped. Otherwise the CSS animation start is delayed
//    by whatever grouping took, but the "remove wrapper after N ms" timer
//    fires on the original schedule → wrapper unmounts mid-fade → pop.
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

## Gotchas

1. **Per-shape opacity animation = per-shape GPU layer in Chromium.** At 64
   shapes × 50 tiles on screen, that's 3200+ compositor layers and the
   page becomes unusable. Always bucket into a small tier count (4 is a
   good default).
2. **`steps(1, end)` does NOT avoid layer promotion** — verified
   empirically. The compositor reserves a layer regardless of timing
   function. Only way to cut layer count is to cut animated element count.
3. **Race condition**: if `dissolving` is set before the SVG is mounted
   and grouped, the CSS animation start gets pushed back by however long
   grouping took, while any "remove wrapper after N ms" timer fires on
   the original schedule → wrapper unmounts mid-fade → visible pop.
   **Wait for both `image-loaded` AND `svg-grouped` before triggering.**
4. **Pause animations during scroll** for smooth virtualised lists:
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
5. **Pre-warm wasm at app boot** via `init()` so first-tile decode isn't
   blocked by the wasm module load.
6. **Use `DocumentFragment` to batch group moves** — appending each `<g>`
   directly to the live `<svg>` causes a layout invalidation per append.
7. **Skip the animation if the real image arrives during wasm decode.**
   For the fastest-loading images, the placeholder isn't worth painting
   at all — flip a `revealed` flag inside the decode callback and skip
   `startDissolve()` if it's already set.

## Sizing the placeholder

| Use case | Recommendation |
|---|---|
| Gallery thumbnails, 50+ on screen | `codec.rect({ n: 32 })` + `cornerRadius: 1` |
| Hero image / above-the-fold | `codec.triangle({ n: 24 })` |
| Smallest readable placeholder | `codec.dct()` (~21 B) |

Rect with `n=32` (~150 B hash, 33 SVG elements) is a sweet spot for
high-density gallery layouts — smaller than the playground default `n=48`
but visibly distinct from `n=64`.

## Why not a `<ArthashPlaceholder>` component?

The arthash SDK is intentionally framework-free. UX decisions like tier
count, fade duration, scroll-pause heuristics, and fast-load skip
thresholds belong in the app layer, not in the encode/decode library.
Copy this recipe into your own component and tune the constants — that
gives you full control without taking a dependency on yet another
framework-locked package.

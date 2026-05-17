<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, shallowRef, watch } from "vue";
import type { Shape as ShapeType } from "arthash";
import { awaitEncodeSlot, fmtMs, loadImage, runPipeline, toHex } from "../pipeline";

interface Props {
  src: string;
  alt?: string;
  shape: ShapeType;
  nShapes: number;
  baseSize: number;
  blur: number;
  seed: number;
  useSvg: boolean;
  ready: boolean;
  // Aspect-ratio hint so the tile reserves the right height before the image
  // loads (vue-wf only sets width on slot children).
  aspectW?: number;
  aspectH?: number;
  label?: string;
  // cx/cy/r bit widths are derived from baseSize in runPipeline.
  alphaBits?: number;
  colorId?: string;
}

const props = defineProps<Props>();
const emit = defineEmits<{ (e: "metrics", v: { encodeMs: number; decodeMs: number; hashBytes: number } | null): void }>();

const canvas = ref<HTMLCanvasElement | null>(null);
const imgEl = ref<HTMLImageElement | null>(null);
const svgWrapper = ref<HTMLDivElement | null>(null);
const isHovering = ref(false);
const status = ref<"idle" | "loading" | "encoding" | "ready" | "error">("idle");
const error = ref<string>("");
const encodeMs = ref(0);
const decodeMs = ref(0);
const hashLen = ref(0);
const hashHex = ref("");
const svg = ref("");
const decodedSize = ref<{ w: number; h: number } | null>(null);

const sourceImage = shallowRef<HTMLImageElement | null>(null);

let runToken = 0;

async function ensureImage(): Promise<HTMLImageElement> {
  if (sourceImage.value) return sourceImage.value;
  status.value = "loading";
  const im = await loadImage(props.src, true);
  sourceImage.value = im;
  return im;
}

function paintDecoded(dec: { w: number; h: number; rgba: Uint8Array }) {
  const c = canvas.value;
  if (!c) return;
  c.width = dec.w;
  c.height = dec.h;
  const ctx = c.getContext("2d");
  if (!ctx) return;
  const im = new ImageData(new Uint8ClampedArray(dec.rgba), dec.w, dec.h);
  ctx.putImageData(im, 0, 0);
}

async function run() {
  if (!props.ready) return;
  const token = ++runToken;
  try {
    const img = await ensureImage();
    if (token !== runToken) return;
    status.value = "encoding";
    // Reserve a slot in the global encode queue — runs at most one tile per
    // animation frame so the progress UI gets a chance to paint between calls.
    await awaitEncodeSlot();
    if (token !== runToken) return;
    const res = runPipeline(img, {
      shape: props.shape,
      nShapes: props.nShapes,
      baseSize: props.baseSize,
      blur: props.blur,
      seed: props.seed,
      alphaBits: props.alphaBits,
      colorId: props.colorId,
      useSvg: props.useSvg,
    });
    if (token !== runToken) return;
    encodeMs.value = res.encodeMs;
    decodeMs.value = res.decodeMs;
    hashLen.value = res.hash.length;
    hashHex.value = toHex(res.hash);
    svg.value = res.svg ?? "";
    if (res.decoded) {
      decodedSize.value = { w: res.decoded.w, h: res.decoded.h };
      paintDecoded(res.decoded);
    } else {
      decodedSize.value = null;
    }
    status.value = "ready";
    emit("metrics", {
      encodeMs: res.encodeMs,
      decodeMs: res.decodeMs,
      hashBytes: res.hash.length,
    });
  } catch (e) {
    if (token !== runToken) return;
    error.value = e instanceof Error ? e.message : String(e);
    status.value = "error";
    emit("metrics", null);
  }
}

watch(
  () => [
    props.ready,
    props.src,
    props.shape,
    props.nShapes,
    props.baseSize,
    props.blur,
    props.seed,
    props.useSvg,
    props.alphaBits,
    props.colorId,
  ],
  () => {
    void run();
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  runToken++;
});

const showSvg = computed(() => props.useSvg && svg.value);

const rootStyle = computed(() => {
  if (props.aspectW && props.aspectH) {
    return { aspectRatio: `${props.aspectW} / ${props.aspectH}` };
  }
  return {};
});

// SVG mode bakes blur into the SVG via <feGaussianBlur>. In raster (canvas)
// mode the decoded pixels are painted directly, so blur has nowhere to apply
// — fall back to a CSS filter on the canvas element so the knob still works.
const canvasStyle = computed(() =>
  props.blur > 0 ? { filter: `blur(${props.blur}px)` } : {},
);

// On hover (SVG mode), reveal the original image instantly behind the SVG
// and fade out each SVG shape over ~1s — biggest first (the background path
// is by definition the largest), so the cover dissolves from broad strokes
// down to small details.
const REVEAL_TOTAL_MS = 1000;
const REVEAL_PER_SHAPE_MS = 220;

function svgChildren(): SVGGraphicsElement[] {
  const wrap = svgWrapper.value;
  if (!wrap) return [];
  const svgEl = wrap.querySelector("svg");
  if (!svgEl) return [];
  const out: SVGGraphicsElement[] = [];
  for (const node of Array.from(svgEl.children)) {
    const tag = node.tagName.toLowerCase();
    if (tag === "defs" || tag === "filter") continue;
    // Blur path wraps the shapes in <g filter="url(#b)">. Recurse one level.
    if (tag === "g") {
      for (const inner of Array.from((node as SVGGElement).children)) {
        out.push(inner as SVGGraphicsElement);
      }
    } else {
      out.push(node as SVGGraphicsElement);
    }
  }
  return out;
}

function shapeArea(el: SVGGraphicsElement): number {
  try {
    const b = el.getBBox();
    return b.width * b.height;
  } catch {
    return 0;
  }
}

/** Children sorted largest first, so index 0 is the background path and
 *  index N-1 is the smallest detail shape. */
function rankedChildren(): SVGGraphicsElement[] {
  return svgChildren()
    .map((el) => ({ el, area: shapeArea(el) }))
    .sort((a, b) => b.area - a.area)
    .map((x) => x.el);
}

/** Stagger opacity across `ranked`. `reverse=false` plays biggest → smallest
 *  (used to fade the cover out on hover-in); `reverse=true` plays smallest →
 *  biggest (used to fade it back on hover-out, so the background is the very
 *  last thing to reappear). */
function applyStagger(ranked: SVGGraphicsElement[], opacity: "0" | "1", reverse: boolean) {
  const n = ranked.length;
  if (n === 0) return;
  const span = Math.max(0, REVEAL_TOTAL_MS - REVEAL_PER_SHAPE_MS);
  for (let i = 0; i < n; i++) {
    const slot = reverse ? n - 1 - i : i;
    const delay = n <= 1 ? 0 : (span * slot) / (n - 1);
    const el = ranked[i];
    el.style.transition = `opacity ${REVEAL_PER_SHAPE_MS}ms ease-out`;
    el.style.transitionDelay = `${delay.toFixed(1)}ms`;
    el.style.opacity = opacity;
  }
}

function applyReveal() {
  applyStagger(rankedChildren(), "0", false);
}

function clearReveal() {
  applyStagger(rankedChildren(), "1", true);
}

function onMouseEnter() {
  isHovering.value = true;
  if (!showSvg.value) return;
  applyReveal();
}

function onMouseLeave() {
  isHovering.value = false;
  if (!showSvg.value) return;
  clearReveal();
}

// If the SVG content updates while hovered, re-apply the staggered fade.
// v-html re-renders blow away any inline styles we set, so we need to
// reapply once Vue has flushed the new DOM.
watch(svg, () => {
  if (isHovering.value && showSvg.value) {
    nextTick(applyReveal);
  }
});
</script>

<template>
  <figure
    class="tile"
    :data-status="status"
    :data-mode="showSvg ? 'svg' : 'raster'"
    :style="rootStyle"
    @mouseenter="onMouseEnter"
    @mouseleave="onMouseLeave"
  >
    <div v-if="label" class="badge">{{ label }}</div>
    <div class="tile-layers">
      <!-- decoded placeholder -->
      <div v-if="showSvg" ref="svgWrapper" class="layer placeholder svg-layer" v-html="svg" />
      <canvas v-else ref="canvas" class="layer placeholder canvas-layer" :style="canvasStyle" />

      <!-- full-resolution original revealed on hover -->
      <img
        ref="imgEl"
        class="layer original"
        :src="src"
        :alt="alt || ''"
        loading="lazy"
        decoding="async"
        crossorigin="anonymous"
      />

      <!-- subtle status / error overlay -->
      <div v-if="status === 'loading' || status === 'encoding'" class="layer pending" />
      <div v-else-if="status === 'error'" class="layer error-pane">
        <span>{{ error }}</span>
      </div>
    </div>

    <figcaption class="stats">
      <span class="row">
        <span class="k">enc</span><span class="v">{{ fmtMs(encodeMs) }}</span>
        <span class="k">dec</span><span class="v">{{ fmtMs(decodeMs) }}</span>
        <span class="k">{{ hashLen }}B</span>
      </span>
      <span class="hex">{{ hashHex }}</span>
    </figcaption>
  </figure>
</template>

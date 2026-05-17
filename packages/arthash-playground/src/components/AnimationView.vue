<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, shallowRef, watch } from "vue";
import { refDebounced } from "@vueuse/core";
import { Shape, type Shape as ShapeType } from "arthash";
import {
  ImageUp,
  Image as ImageIcon,
  Ruler,
  Play,
  Pause,
  Dice5,
  Gauge,
  Zap,
  ChevronLeft,
  ChevronRight,
  X,
} from "@lucide/vue";
import AdvancedPanel, { type AdvancedConfig } from "./AdvancedPanel.vue";
import SearchControls from "./SearchControls.vue";
import { DEMO_IMAGES } from "../demo";
import {
  COLOR_OPTIONS,
  DEFAULT_SEARCH,
  fmtMs,
  loadImage,
  runPipeline,
  supportsSvg,
  type SearchConfig,
} from "../pipeline";

interface Props {
  ready: boolean;
}
const props = defineProps<Props>();

const shape = ref<ShapeType>(Shape.RECT);
const nShapes = ref(48);
const baseSize = ref(512);
const blur = ref(0);
const useSvg = ref(true);
const fps = ref(12);
const playing = ref(true);
const colorId = ref<string>("rgb-565");
const advanced = ref<AdvancedConfig>({ alphaBits: 3 });
const search = ref<SearchConfig>({ ...DEFAULT_SEARCH });

const seed = ref(0);

// `nShapes` / `baseSize` etc. shouldn't trigger an encode on every keystroke
// while the loop is also bumping seed every frame — debounce the heavyweight
// knobs and leave `seed` raw so its updates land in real time.
const DEBOUNCE_MS = 200;
const nShapesD = refDebounced(nShapes, DEBOUNCE_MS);
const baseSizeD = refDebounced(baseSize, DEBOUNCE_MS);
const blurD = refDebounced(blur, DEBOUNCE_MS);
const advancedD = refDebounced(advanced, DEBOUNCE_MS);
const searchD = refDebounced(search, DEBOUNCE_MS);

const svgPossible = computed(() => supportsSvg(shape.value));
const renderSvg = computed(() => useSvg.value && svgPossible.value);

const shapeOptions: Array<{ value: ShapeType; label: string }> = [
  { value: Shape.RECT, label: "Rect" },
  { value: Shape.ROTATED_RECT, label: "RotRect" },
  { value: Shape.SQUARE, label: "Square" },
  { value: Shape.CIRCLE, label: "Circle" },
  { value: Shape.TRIANGLE, label: "Triangle" },
  { value: Shape.PIXEL, label: "Pixel" },
  { value: Shape.DCT, label: "DCT" },
];

// ---- image source ---------------------------------------------------------
//
// Two source modes:
//   * "demo"   — pick from the bundled DEMO_IMAGES via index (prev/next or
//                dropdown). Cheap to swap; aspect dims known upfront.
//   * "custom" — user-supplied via drop / paste / file picker. Lives at an
//                object URL we own and revoke when replaced or cleared.

const demoIdx = ref(0);
const fileName = ref(DEMO_IMAGES[0]?.alt ?? "demo");
const objectUrl = ref<string>(DEMO_IMAGES[0]?.src ?? "");
const ownedUrl = ref(""); // tracks a URL we created via createObjectURL, so we can revoke it
const isCustom = computed(() => ownedUrl.value !== "");
const dims = ref<{ w: number; h: number } | null>(
  DEMO_IMAGES[0] ? { w: DEMO_IMAGES[0].w, h: DEMO_IMAGES[0].h } : null,
);
const dragging = ref(false);
const fileInput = ref<HTMLInputElement | null>(null);
const error = ref("");

const sourceImage = shallowRef<HTMLImageElement | null>(null);

function selectDemo(idx: number) {
  if (DEMO_IMAGES.length === 0) return;
  const n = DEMO_IMAGES.length;
  const wrapped = ((idx % n) + n) % n;
  const d = DEMO_IMAGES[wrapped];
  demoIdx.value = wrapped;
  if (ownedUrl.value) {
    URL.revokeObjectURL(ownedUrl.value);
    ownedUrl.value = "";
  }
  fileName.value = d.alt;
  objectUrl.value = d.src;
  dims.value = { w: d.w, h: d.h };
}

watch(
  () => objectUrl.value,
  async (src) => {
    sourceImage.value = null;
    if (!src) return;
    try {
      const im = await loadImage(src, true);
      sourceImage.value = im;
      dims.value = { w: im.naturalWidth, h: im.naturalHeight };
    } catch (e) {
      error.value = `failed to load image: ${e instanceof Error ? e.message : String(e)}`;
    }
  },
  { immediate: true },
);

async function setFile(f: File) {
  error.value = "";
  if (!f.type.startsWith("image/")) {
    error.value = `not an image: ${f.type || "unknown"}`;
    return;
  }
  fileName.value = f.name;
  if (ownedUrl.value) URL.revokeObjectURL(ownedUrl.value);
  const url = URL.createObjectURL(f);
  ownedUrl.value = url;
  objectUrl.value = url;
}

function onDrop(ev: DragEvent) {
  ev.preventDefault();
  dragging.value = false;
  const f = ev.dataTransfer?.files?.[0];
  if (f) void setFile(f);
}
function onPick(ev: Event) {
  const f = (ev.target as HTMLInputElement).files?.[0];
  if (f) void setFile(f);
}
function onPaste(ev: ClipboardEvent) {
  const item = Array.from(ev.clipboardData?.items ?? []).find((i) =>
    i.type.startsWith("image/"),
  );
  if (!item) return;
  const f = item.getAsFile();
  if (f) void setFile(f);
}
function clearImage() {
  if (ownedUrl.value) {
    URL.revokeObjectURL(ownedUrl.value);
    ownedUrl.value = "";
  }
  // Fall back to the currently selected demo instead of going empty, so the
  // animation loop has something to render.
  if (DEMO_IMAGES.length > 0) {
    selectDemo(demoIdx.value);
  } else {
    objectUrl.value = "";
    fileName.value = "";
    dims.value = null;
  }
}

// ---- render loop ----------------------------------------------------------

const previewSvg = ref("");
const canvasEl = ref<HTMLCanvasElement | null>(null);
const encodeMs = ref(0);
const decodeMs = ref(0);
const hashBytes = ref(0);

function renderOnce() {
  const img = sourceImage.value;
  if (!props.ready || !img) return;
  try {
    const res = runPipeline(img, {
      shape: shape.value,
      nShapes: nShapesD.value,
      baseSize: baseSizeD.value,
      blur: blurD.value,
      seed: seed.value,
      alphaBits: advancedD.value.alphaBits,
      colorId: colorId.value,
      useSvg: renderSvg.value,
      search: searchD.value,
    });
    encodeMs.value = res.encodeMs;
    decodeMs.value = res.decodeMs;
    hashBytes.value = res.hash.length;
    if (renderSvg.value && res.svg) {
      previewSvg.value = res.svg;
    } else {
      previewSvg.value = "";
      if (res.decoded && canvasEl.value) {
        const c = canvasEl.value;
        c.width = res.decoded.w;
        c.height = res.decoded.h;
        const ctx = c.getContext("2d");
        if (ctx) {
          ctx.putImageData(
            new ImageData(
              new Uint8ClampedArray(res.decoded.rgba),
              res.decoded.w,
              res.decoded.h,
            ),
            0,
            0,
          );
        }
      }
    }
    error.value = "";
  } catch (e) {
    error.value = `render failed: ${e instanceof Error ? e.message : String(e)}`;
  }
}

// Re-render on any non-seed knob change. Seed has its own loop driver below;
// going through this watcher too would cause double renders per frame.
watch(
  [
    () => props.ready,
    sourceImage,
    shape,
    nShapesD,
    baseSizeD,
    blurD,
    renderSvg,
    colorId,
    advancedD,
    searchD,
  ],
  () => {
    void nextTick(renderOnce);
  },
  { immediate: true, deep: true },
);

let rafId: number | null = null;
let lastFrameTs = 0;

function loop(ts: number) {
  if (!playing.value) {
    rafId = null;
    return;
  }
  const interval = 1000 / Math.max(1, fps.value);
  if (ts - lastFrameTs >= interval) {
    lastFrameTs = ts;
    seed.value = (seed.value + 1) >>> 0;
    renderOnce();
  }
  rafId = requestAnimationFrame(loop);
}

function startLoop() {
  if (rafId !== null) return;
  lastFrameTs = 0;
  rafId = requestAnimationFrame(loop);
}
function stopLoop() {
  if (rafId !== null) {
    cancelAnimationFrame(rafId);
    rafId = null;
  }
}

watch(
  [playing, () => props.ready, sourceImage],
  ([on, ready, img]) => {
    if (on && ready && img) startLoop();
    else stopLoop();
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  stopLoop();
  if (ownedUrl.value) URL.revokeObjectURL(ownedUrl.value);
});

function nudgeSeed(delta: number) {
  seed.value = (seed.value + delta) >>> 0;
  if (!playing.value) renderOnce();
}

const previewStyle = computed(() => {
  if (!dims.value) return {};
  return { aspectRatio: `${dims.value.w} / ${dims.value.h}` };
});
</script>

<template>
  <div class="view animate" @paste="onPaste" tabindex="0">
    <div class="controls-row">
      <label class="ctl">
        <span>shape</span>
        <select v-model="shape">
          <option v-for="o in shapeOptions" :key="o.value" :value="o.value">{{ o.label }}</option>
        </select>
      </label>
      <label class="ctl">
        <span>n_shapes</span>
        <input type="number" min="1" max="128" step="1" v-model.number="nShapes" :disabled="shape === 'dct'" />
      </label>
      <label class="ctl">
        <span>base</span>
        <input type="number" min="64" max="1024" step="32" v-model.number="baseSize" />
      </label>
      <label class="ctl" v-if="svgPossible">
        <span>blur</span>
        <input type="number" min="0" max="32" step="1" v-model.number="blur" />
      </label>
      <label class="ctl toggle" v-if="svgPossible">
        <input type="checkbox" v-model="useSvg" />
        <span>SVG</span>
      </label>
      <label class="ctl">
        <span>color</span>
        <select v-model="colorId">
          <option v-for="o in COLOR_OPTIONS" :key="o.id" :value="o.id">{{ o.label }}</option>
        </select>
      </label>
      <label class="ctl">
        <span>fps</span>
        <input type="number" min="1" max="60" step="1" v-model.number="fps" />
      </label>
      <button class="link icon-btn" @click="playing = !playing" :title="playing ? 'pause' : 'play'">
        <component :is="playing ? Pause : Play" :size="14" :stroke-width="2" />
        <span>{{ playing ? "pause" : "play" }}</span>
      </button>
      <button class="link icon-btn" @click="nudgeSeed(1)" :disabled="playing" title="step one frame">
        <Dice5 :size="14" :stroke-width="2" />
        <span>step</span>
      </button>
      <SearchControls v-model="search" :disabled="shape === 'dct'" />
      <div class="spacer" />
      <AdvancedPanel v-model="advanced" />
      <button class="link icon-btn" v-if="isCustom" @click="clearImage" title="back to demo set">
        <X :size="14" :stroke-width="2" />
        <span>clear</span>
      </button>
    </div>

    <div class="controls-row" v-if="objectUrl">
      <button class="link icon-btn" @click="selectDemo(demoIdx - 1)" :disabled="isCustom" title="previous demo image">
        <ChevronLeft :size="14" :stroke-width="2" />
      </button>
      <label class="ctl">
        <span>image</span>
        <select :value="isCustom ? '' : String(demoIdx)" :disabled="isCustom" @change="selectDemo(+($event.target as HTMLSelectElement).value)">
          <option v-if="isCustom" value="">{{ fileName }} (custom)</option>
          <option v-for="(d, i) in DEMO_IMAGES" :key="d.src" :value="i">{{ i + 1 }} · {{ d.alt }}</option>
        </select>
      </label>
      <button class="link icon-btn" @click="selectDemo(demoIdx + 1)" :disabled="isCustom" title="next demo image">
        <ChevronRight :size="14" :stroke-width="2" />
      </button>
      <button class="link icon-btn" @click="fileInput?.click()" :disabled="false" title="upload a custom image">
        <ImageUp :size="14" :stroke-width="2" />
        <span>upload</span>
      </button>
      <input ref="fileInput" type="file" accept="image/*" hidden @change="onPick" />
      <div class="spacer" />
    </div>

    <div class="metrics-row" v-if="objectUrl">
      <div class="metric" v-if="fileName">
        <ImageIcon class="metric-icon" :size="13" :stroke-width="1.75" />
        <span class="k">image</span><span class="v">{{ fileName }}</span>
      </div>
      <div class="metric" v-if="dims">
        <Ruler class="metric-icon" :size="13" :stroke-width="1.75" />
        <span class="k">size</span><span class="v">{{ dims.w }}×{{ dims.h }}</span>
      </div>
      <div class="metric">
        <Gauge class="metric-icon" :size="13" :stroke-width="1.75" />
        <span class="k">seed</span><span class="v">{{ seed }}</span>
      </div>
      <div class="metric">
        <Zap class="metric-icon" :size="13" :stroke-width="1.75" />
        <span class="k">enc</span><span class="v">{{ fmtMs(encodeMs) }}</span>
      </div>
      <div class="metric">
        <span class="k">dec</span><span class="v">{{ fmtMs(decodeMs) }}</span>
      </div>
      <div class="metric">
        <span class="k">hash</span><span class="v">{{ hashBytes }} B</span>
      </div>
      <div class="spacer" />
      <span class="muted">drop or paste an image to swap the source</span>
    </div>

    <p v-if="error" class="error">{{ error }}</p>

    <div
      v-if="objectUrl && dims"
      class="animate-stage"
      @dragover.prevent="dragging = true"
      @dragleave.prevent="dragging = false"
      @drop="onDrop"
    >
      <figure class="animate-frame" :style="previewStyle">
        <figcaption class="animate-cap">hash · seed {{ seed }}</figcaption>
        <div v-if="renderSvg && previewSvg" class="animate-canvas svg-layer" v-html="previewSvg" />
        <canvas v-else ref="canvasEl" class="animate-canvas canvas-layer" />
      </figure>
      <figure class="animate-frame" :style="previewStyle">
        <figcaption class="animate-cap">original</figcaption>
        <img
          class="animate-canvas original-img"
          :src="objectUrl"
          :alt="fileName"
          crossorigin="anonymous"
          loading="lazy"
          decoding="async"
        />
      </figure>
    </div>
  </div>
</template>

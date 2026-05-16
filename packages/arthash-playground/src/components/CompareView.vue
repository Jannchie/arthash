<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { refDebounced } from "@vueuse/core";
import { Shape, type Shape as ShapeType } from "arthash";
import Tile from "./Tile.vue";
import AdvancedPanel, { type AdvancedConfig } from "./AdvancedPanel.vue";
import { COLOR_OPTIONS } from "../pipeline";

interface Props {
  ready: boolean;
}
defineProps<Props>();

interface Variant {
  id: string;
  label: string;
  shape: ShapeType;
  nShapes: number;
  useSvg: boolean;
}

// A curated comparison set covering all four shape families plus a small
// n_shapes sweep within Circle (the cheapest, most-used mode).
const VARIANTS: Variant[] = [
  { id: "dct",          label: "DCT",              shape: Shape.DCT,          nShapes: 0,  useSvg: false },
  { id: "circle-12",    label: "Circle · n=12",    shape: Shape.CIRCLE,       nShapes: 12, useSvg: true  },
  { id: "circle-24",    label: "Circle · n=24",    shape: Shape.CIRCLE,       nShapes: 24, useSvg: true  },
  { id: "triangle-12",  label: "Triangle · n=12",  shape: Shape.TRIANGLE,     nShapes: 12, useSvg: true  },
  { id: "triangle-24",  label: "Triangle · n=24",  shape: Shape.TRIANGLE,     nShapes: 24, useSvg: true  },
  { id: "square-12",    label: "Square · n=12",    shape: Shape.SQUARE,       nShapes: 12, useSvg: true  },
  { id: "rect-12",      label: "Rect · n=12",      shape: Shape.RECT,         nShapes: 12, useSvg: true  },
  { id: "rotrect-12",   label: "RotRect · n=12",   shape: Shape.ROTATED_RECT, nShapes: 12, useSvg: true  },
  { id: "pixel-12",     label: "Pixel · n=12",     shape: Shape.PIXEL,        nShapes: 12, useSvg: true  },
];

const cols = ref(4);
const baseSize = ref(512);
const blur = ref(0);
const seed = ref(0);
const colorId = ref<string>("rgb-565");

const advanced = ref<AdvancedConfig>({
  alphaBits: 3,
  overrideAspectEnabled: false,
  overrideAspect: 1,
});

const DEBOUNCE_MS = 300;
const baseSizeD = refDebounced(baseSize, DEBOUNCE_MS);
const blurD = refDebounced(blur, DEBOUNCE_MS);
const seedD = refDebounced(seed, DEBOUNCE_MS);
const advancedD = refDebounced(advanced, DEBOUNCE_MS);

const effectiveAspect = computed(() =>
  advancedD.value.overrideAspectEnabled ? advancedD.value.overrideAspect : undefined,
);

watch([baseSizeD, blurD, seedD, advancedD, colorId], () => {
  tileMetrics.value = new Map();
}, { deep: true });

const file = ref<File | null>(null);
const fileName = ref("");
const objectUrl = ref("");
const dims = ref<{ w: number; h: number } | null>(null);
const dragging = ref(false);
const fileInput = ref<HTMLInputElement | null>(null);
const error = ref("");

const tileMetrics = ref(new Map<string, { encodeMs: number; decodeMs: number; hashBytes: number }>());

const aggregate = computed(() => {
  const ms = Array.from(tileMetrics.value.values());
  if (!ms.length) return null;
  const fastest = ms.reduce((a, b) => (b.encodeMs < a.encodeMs ? b : a));
  const smallest = ms.reduce((a, b) => (b.hashBytes < a.hashBytes ? b : a));
  return {
    n: ms.length,
    fastest: fastest.encodeMs,
    smallest: smallest.hashBytes,
  };
});

function onMetrics(id: string, v: { encodeMs: number; decodeMs: number; hashBytes: number } | null) {
  if (v) tileMetrics.value.set(id, v);
  else tileMetrics.value.delete(id);
  tileMetrics.value = new Map(tileMetrics.value);
}

async function setFile(f: File) {
  error.value = "";
  if (!f.type.startsWith("image/")) {
    error.value = `not an image: ${f.type || "unknown"}`;
    return;
  }
  file.value = f;
  fileName.value = f.name;
  if (objectUrl.value) URL.revokeObjectURL(objectUrl.value);
  objectUrl.value = URL.createObjectURL(f);
  tileMetrics.value = new Map();
  // Probe dimensions so each variant tile can lock in aspect-ratio early.
  try {
    const img = new Image();
    img.src = objectUrl.value;
    await img.decode();
    dims.value = { w: img.naturalWidth, h: img.naturalHeight };
  } catch (e) {
    error.value = `failed to load image: ${e instanceof Error ? e.message : String(e)}`;
    dims.value = null;
  }
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
  const item = Array.from(ev.clipboardData?.items ?? []).find((i) => i.type.startsWith("image/"));
  if (!item) return;
  const f = item.getAsFile();
  if (f) void setFile(f);
}

function fmtMs(ms: number) {
  if (!ms) return "—";
  return ms >= 100 ? `${ms.toFixed(0)} ms` : ms >= 10 ? `${ms.toFixed(1)} ms` : `${ms.toFixed(2)} ms`;
}

const gridStyle = computed(() => ({
  display: "grid",
  gridTemplateColumns: `repeat(${cols.value}, minmax(0, 1fr))`,
  gap: "14px",
}));
</script>

<template>
  <div class="view compare" @paste="onPaste" tabindex="0">
    <div class="controls-row">
      <label class="ctl">
        <span>base</span>
        <input type="number" min="64" max="1024" step="32" v-model.number="baseSize" />
      </label>
      <label class="ctl">
        <span>blur</span>
        <input type="number" min="0" max="32" step="1" v-model.number="blur" />
      </label>
      <label class="ctl">
        <span>seed</span>
        <input type="number" min="0" max="999999" step="1" v-model.number="seed" />
      </label>
      <label class="ctl">
        <span>cols</span>
        <input type="number" min="2" max="6" step="1" v-model.number="cols" />
      </label>
      <label class="ctl">
        <span>color</span>
        <select v-model="colorId">
          <option v-for="o in COLOR_OPTIONS" :key="o.id" :value="o.id">{{ o.label }}</option>
        </select>
      </label>
      <div class="spacer" />
      <AdvancedPanel v-model="advanced" />
      <button class="link" v-if="objectUrl" @click="file = null; objectUrl = ''; dims = null; fileName = ''; tileMetrics = new Map();">clear</button>
    </div>

    <div class="metrics-row" v-if="objectUrl">
      <div class="metric">
        <span class="k">image</span><span class="v">{{ fileName }}</span>
      </div>
      <div class="metric" v-if="dims">
        <span class="k">size</span><span class="v">{{ dims.w }}×{{ dims.h }}</span>
      </div>
      <div class="metric" v-if="aggregate">
        <span class="k">fastest enc</span><span class="v">{{ fmtMs(aggregate.fastest) }}</span>
      </div>
      <div class="metric" v-if="aggregate">
        <span class="k">smallest hash</span><span class="v">{{ aggregate.smallest }} B</span>
      </div>
      <div class="spacer" />
      <span class="muted">hover a variant to reveal the original</span>
    </div>

    <!-- empty / dropzone state -->
    <label
      v-if="!objectUrl"
      class="drop"
      :class="{ dragging }"
      @dragover.prevent="dragging = true"
      @dragleave.prevent="dragging = false"
      @drop="onDrop"
      @click="fileInput?.click()"
    >
      <div class="big">drop an image here</div>
      <div class="sub">or click to pick · paste from clipboard also works</div>
      <input ref="fileInput" type="file" accept="image/*" hidden @change="onPick" />
    </label>
    <p v-if="error" class="error">{{ error }}</p>

    <!-- variant grid -->
    <div v-if="objectUrl && dims" class="variants" :style="gridStyle">
      <Tile
        v-for="v in VARIANTS"
        :key="v.id"
        :src="objectUrl"
        :alt="v.label"
        :label="v.label"
        :aspect-w="dims.w"
        :aspect-h="dims.h"
        :shape="v.shape"
        :n-shapes="v.nShapes"
        :base-size="baseSizeD"
        :blur="blurD"
        :seed="seedD"
        :use-svg="v.useSvg"
        :ready="ready"
        :alpha-bits="advancedD.alphaBits"
        :color-id="colorId"
        :override-aspect="effectiveAspect"
        @metrics="(m) => onMetrics(v.id, m)"
      />
    </div>
  </div>
</template>

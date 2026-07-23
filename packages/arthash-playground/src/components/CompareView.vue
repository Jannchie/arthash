<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { refDebounced } from "@vueuse/core";
import { Preset, Shape, type Shape as ShapeType } from "arthash";
import { ImageUp, Image as ImageIcon, Ruler, Zap, Package2, X } from "@lucide/vue";
import Tile from "./Tile.vue";
import AdvancedPanel, { type AdvancedConfig } from "./AdvancedPanel.vue";
import SearchControls from "./SearchControls.vue";
import { COLOR_OPTIONS, DEFAULT_SEARCH, type SearchConfig } from "../pipeline";

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

// One column per built-in `Preset` from `arthash`. Triangle / Circle / Pixel /
// Rect / Square each have three size tiers; DCT is the single frequency-domain
// mode (no SVG path). Kept in Preset declaration order so the byte budget
// grows left → right within each size tier.
const VARIANTS: Variant[] = [
  { id: Preset.Dct,            label: "DCT",            shape: Shape.DCT,      nShapes: 0,  useSvg: false },
  { id: Preset.SmallTriangle,  label: "Small · △12",    shape: Shape.TRIANGLE, nShapes: 12, useSvg: true  },
  { id: Preset.SmallCircle,    label: "Small · ○12",    shape: Shape.CIRCLE,   nShapes: 12, useSvg: true  },
  { id: Preset.SmallPixel,     label: "Small · ▦16",    shape: Shape.PIXEL,    nShapes: 16, useSvg: true  },
  { id: Preset.SmallRect,      label: "Small · ▭12",    shape: Shape.RECT,     nShapes: 12, useSvg: true  },
  { id: Preset.SmallSquare,    label: "Small · □12",    shape: Shape.SQUARE,   nShapes: 12, useSvg: true  },
  { id: Preset.MediumTriangle, label: "Medium · △24",   shape: Shape.TRIANGLE, nShapes: 24, useSvg: true  },
  { id: Preset.MediumCircle,   label: "Medium · ○24",   shape: Shape.CIRCLE,   nShapes: 24, useSvg: true  },
  { id: Preset.MediumPixel,    label: "Medium · ▦24",   shape: Shape.PIXEL,    nShapes: 24, useSvg: true  },
  { id: Preset.MediumRect,     label: "Medium · ▭24",   shape: Shape.RECT,     nShapes: 24, useSvg: true  },
  { id: Preset.MediumSquare,   label: "Medium · □24",   shape: Shape.SQUARE,   nShapes: 24, useSvg: true  },
  { id: Preset.LargeTriangle,  label: "Large · △64",    shape: Shape.TRIANGLE, nShapes: 64, useSvg: true  },
  { id: Preset.LargeCircle,    label: "Large · ○64",    shape: Shape.CIRCLE,   nShapes: 64, useSvg: true  },
  { id: Preset.LargePixel,     label: "Large · ▦64",    shape: Shape.PIXEL,    nShapes: 64, useSvg: true  },
  { id: Preset.LargeRect,      label: "Large · ▭64",    shape: Shape.RECT,     nShapes: 64, useSvg: true  },
  { id: Preset.LargeSquare,    label: "Large · □64",    shape: Shape.SQUARE,   nShapes: 64, useSvg: true  },
];

const baseSize = ref(512);
const blur = ref(0);
// Applies only to rect / square / rotrect variants; other shapes ignore it.
const cornerRadius = ref(4);
const seed = ref(0);
const dither = ref(false);
// Dot pitch in output px; 0 = auto (baseSize/128).
const ditherScale = ref(0);
const colorId = ref<string>("rgb-565");

const advanced = ref<AdvancedConfig>({ alphaBits: 3 });
const search = ref<SearchConfig>({ ...DEFAULT_SEARCH });

const DEBOUNCE_MS = 300;
const baseSizeD = refDebounced(baseSize, DEBOUNCE_MS);
const blurD = refDebounced(blur, DEBOUNCE_MS);
const cornerRadiusD = refDebounced(cornerRadius, DEBOUNCE_MS);
const seedD = refDebounced(seed, DEBOUNCE_MS);
const ditherScaleD = refDebounced(ditherScale, DEBOUNCE_MS);
const advancedD = refDebounced(advanced, DEBOUNCE_MS);
const searchD = refDebounced(search, DEBOUNCE_MS);

watch([baseSizeD, blurD, cornerRadiusD, seedD, dither, ditherScaleD, advancedD, colorId, searchD], () => {
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

// Show every preset side-by-side on a single row. Tiles shrink to fit the
// viewport width — at narrow widths each tile gets tiny but the comparison
// stays apples-to-apples (no wrapping ⇒ no visual reordering).
const gridStyle = computed(() => ({
  display: "grid",
  gridTemplateColumns: `repeat(${VARIANTS.length}, minmax(0, 1fr))`,
  gap: "8px",
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
        <span>corner_r</span>
        <input type="number" min="0" max="32" step="0.5" v-model.number="cornerRadius" title="applied to rect / square / rotrect variants" />
      </label>
      <label class="ctl">
        <span>seed</span>
        <input type="number" min="0" max="999999" step="1" v-model.number="seed" />
      </label>
      <label class="ctl toggle" title="Bayer dithering at 8-bit quantization — affects DCT and blurred raster output">
        <input type="checkbox" v-model="dither" />
        <span>dither</span>
      </label>
      <label class="ctl" v-if="dither" title="dot pitch in px; 0 = auto (base/128)">
        <span>dot_px</span>
        <input type="number" min="0" max="32" step="1" v-model.number="ditherScale" />
      </label>
      <label class="ctl">
        <span>color</span>
        <select v-model="colorId">
          <option v-for="o in COLOR_OPTIONS" :key="o.id" :value="o.id">{{ o.label }}</option>
        </select>
      </label>
      <SearchControls v-model="search" />
      <div class="spacer" />
      <AdvancedPanel v-model="advanced" />
      <button class="link icon-btn" v-if="objectUrl" @click="file = null; objectUrl = ''; dims = null; fileName = ''; tileMetrics = new Map();">
        <X :size="14" :stroke-width="2" />
        <span>clear</span>
      </button>
    </div>

    <div class="metrics-row" v-if="objectUrl">
      <div class="metric">
        <ImageIcon class="metric-icon" :size="13" :stroke-width="1.75" />
        <span class="k">image</span><span class="v">{{ fileName }}</span>
      </div>
      <div class="metric" v-if="dims">
        <Ruler class="metric-icon" :size="13" :stroke-width="1.75" />
        <span class="k">size</span><span class="v">{{ dims.w }}×{{ dims.h }}</span>
      </div>
      <div class="metric" v-if="aggregate">
        <Zap class="metric-icon" :size="13" :stroke-width="1.75" />
        <span class="k">fastest enc</span><span class="v">{{ fmtMs(aggregate.fastest) }}</span>
      </div>
      <div class="metric" v-if="aggregate">
        <Package2 class="metric-icon" :size="13" :stroke-width="1.75" />
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
      <ImageUp class="drop-icon" :size="48" :stroke-width="1.25" />
      <div class="drop-text">
        <div class="big">drop an image here</div>
        <div class="sub">or click to pick · paste from clipboard also works</div>
      </div>
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
        :corner-radius="cornerRadiusD"
        :dither="dither"
        :dither-scale="ditherScaleD"
        :seed="seedD"
        :use-svg="v.useSvg"
        :ready="ready"
        :alpha-bits="advancedD.alphaBits"
        :color-id="colorId"
        :search="v.shape === 'dct' ? undefined : searchD"
        @metrics="(m) => onMetrics(v.id, m)"
      />
    </div>
  </div>
</template>

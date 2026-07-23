<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { refDebounced } from "@vueuse/core";
import { Waterfall } from "vue-wf";
import { Shape, type Shape as ShapeType } from "arthash";
import { Zap, Cpu, Package2, LayoutGrid, Loader2 } from "@lucide/vue";
import Tile from "./Tile.vue";
import AdvancedPanel, { type AdvancedConfig } from "./AdvancedPanel.vue";
import SearchControls from "./SearchControls.vue";
import { DEMO_IMAGES } from "../demo";
import {
  COLOR_OPTIONS,
  DEFAULT_SEARCH,
  supportsCornerRadius,
  supportsSvg,
  type SearchConfig,
} from "../pipeline";

interface Props {
  ready: boolean;
}
defineProps<Props>();

const shape = ref<ShapeType>(Shape.RECT);
const nShapes = ref(64);
const baseSize = ref(512);
const blur = ref(0);
// Default cornerRadius=4 — visible softening at thumbnail size, gives rect
// placeholders a deliberate UI look. Only honored for rect / square /
// rotrect; other shapes silently ignore.
const cornerRadius = ref(4);
const seed = ref(0);
const dither = ref(false);
// Dot pitch in output px; 0 = auto (baseSize/128).
const ditherScale = ref(0);
const useSvg = ref(true);
const cols = ref(8);
const colorId = ref<string>("rgb-565");

const advanced = ref<AdvancedConfig>({ alphaBits: 3 });
const search = ref<SearchConfig>({ ...DEFAULT_SEARCH });

const DEBOUNCE_MS = 300;
const nShapesD = refDebounced(nShapes, DEBOUNCE_MS);
const baseSizeD = refDebounced(baseSize, DEBOUNCE_MS);
const blurD = refDebounced(blur, DEBOUNCE_MS);
const cornerRadiusD = refDebounced(cornerRadius, DEBOUNCE_MS);
const seedD = refDebounced(seed, DEBOUNCE_MS);
const ditherScaleD = refDebounced(ditherScale, DEBOUNCE_MS);
const advancedD = refDebounced(advanced, DEBOUNCE_MS);
const searchD = refDebounced(search, DEBOUNCE_MS);

const items = computed(() => DEMO_IMAGES.map((d) => ({ width: d.w, height: d.h })));

const tileMetrics = ref(new Map<number, { encodeMs: number; decodeMs: number; hashBytes: number }>());
// "settled" = encode finished (success OR error). Used to gate the waterfall:
// nothing shows until every tile has reported once, so users don't see a
// half-encoded grid mid-render.
const tileSettled = ref(new Set<number>());

const svgPossible = computed(() => supportsSvg(shape.value));
const renderSvg = computed(() => useSvg.value && svgPossible.value);
const cornerRadiusVisible = computed(() => supportsCornerRadius(shape.value));

const aggregate = computed(() => {
  const ms = Array.from(tileMetrics.value.values());
  if (!ms.length) return null;
  const avg = (k: "encodeMs" | "decodeMs") => ms.reduce((a, b) => a + b[k], 0) / ms.length;
  const sizes = new Set(ms.map((m) => m.hashBytes));
  const sizeStr = sizes.size === 1 ? `${[...sizes][0]} B` : `${Math.min(...sizes)}–${Math.max(...sizes)} B`;
  return { n: ms.length, avgEncode: avg("encodeMs"), avgDecode: avg("decodeMs"), size: sizeStr };
});

const settledCount = computed(() => tileSettled.value.size);
const allEncoded = computed(() => settledCount.value >= DEMO_IMAGES.length);
const progressPct = computed(() =>
  Math.round((settledCount.value / DEMO_IMAGES.length) * 100),
);

function onTileMetrics(i: number, v: { encodeMs: number; decodeMs: number; hashBytes: number } | null) {
  if (v) tileMetrics.value.set(i, v);
  else tileMetrics.value.delete(i);
  tileMetrics.value = new Map(tileMetrics.value);
  tileSettled.value = new Set(tileSettled.value).add(i);
}

watch(
  [shape, nShapesD, baseSizeD, blurD, cornerRadiusD, seedD, dither, ditherScaleD, useSvg, advancedD, colorId, searchD],
  () => {
    tileMetrics.value = new Map();
    tileSettled.value = new Set();
  },
  { deep: true },
);

const shapeOptions: Array<{ value: ShapeType; label: string }> = [
  { value: Shape.RECT, label: "Rect" },
  { value: Shape.ROTATED_RECT, label: "RotRect" },
  { value: Shape.SQUARE, label: "Square" },
  { value: Shape.CIRCLE, label: "Circle" },
  { value: Shape.TRIANGLE, label: "Triangle" },
  { value: Shape.PIXEL, label: "Pixel" },
  { value: Shape.DCT, label: "DCT" },
];

function fmtMs(ms: number) {
  if (!ms) return "—";
  return ms >= 100 ? `${ms.toFixed(0)} ms` : ms >= 10 ? `${ms.toFixed(1)} ms` : `${ms.toFixed(2)} ms`;
}
</script>

<template>
  <div class="view gallery">
    <div class="controls-row">
      <label class="ctl">
        <span>shape</span>
        <select v-model="shape">
          <option v-for="o in shapeOptions" :key="o.value" :value="o.value">{{ o.label }}</option>
        </select>
      </label>
      <label class="ctl">
        <span>n_shapes</span>
        <input type="number" min="1" max="64" step="1" v-model.number="nShapes" :disabled="shape === 'dct'" />
      </label>
      <label class="ctl">
        <span>seed</span>
        <input type="number" min="0" max="999999" step="1" v-model.number="seed" :disabled="shape === 'dct'" />
      </label>
      <label class="ctl">
        <span>base</span>
        <input type="number" min="64" max="1024" step="32" v-model.number="baseSize" />
      </label>
      <label class="ctl" v-if="svgPossible">
        <span>blur</span>
        <input type="number" min="0" max="32" step="1" v-model.number="blur" />
      </label>
      <label class="ctl" v-if="cornerRadiusVisible">
        <span>corner_r</span>
        <input type="number" min="0" max="32" step="0.5" v-model.number="cornerRadius" />
      </label>
      <!-- Dither only bites where 8-bit quantization bands: DCT gradients and
           blurred raster output. Hidden elsewhere (it would be a no-op). -->
      <label class="ctl toggle" v-if="shape === 'dct' || blur > 0">
        <input type="checkbox" v-model="dither" />
        <span>dither</span>
      </label>
      <label class="ctl" v-if="dither && shape === 'dct'" title="dot pitch in px; 0 = auto (base/128)">
        <span>dot_px</span>
        <input type="number" min="0" max="32" step="1" v-model.number="ditherScale" />
      </label>
      <label class="ctl toggle" v-if="svgPossible">
        <input type="checkbox" v-model="useSvg" />
        <span>SVG</span>
      </label>
      <label class="ctl">
        <span>cols</span>
        <input type="number" min="1" max="8" step="1" v-model.number="cols" />
      </label>
      <label class="ctl">
        <span>color</span>
        <select v-model="colorId">
          <option v-for="o in COLOR_OPTIONS" :key="o.id" :value="o.id">{{ o.label }}</option>
        </select>
      </label>
      <SearchControls v-model="search" :disabled="shape === 'dct'" />
      <div class="spacer" />
      <AdvancedPanel v-model="advanced" />
    </div>

    <div class="metrics-row">
      <div class="metric">
        <Zap class="metric-icon" :size="13" :stroke-width="1.75" />
        <span class="k">avg encode</span><span class="v">{{ aggregate ? fmtMs(aggregate.avgEncode) : "—" }}</span>
      </div>
      <div class="metric">
        <Cpu class="metric-icon" :size="13" :stroke-width="1.75" />
        <span class="k">avg decode</span><span class="v">{{ aggregate ? fmtMs(aggregate.avgDecode) : "—" }}</span>
      </div>
      <div class="metric">
        <Package2 class="metric-icon" :size="13" :stroke-width="1.75" />
        <span class="k">hash size</span><span class="v">{{ aggregate ? aggregate.size : "—" }}</span>
      </div>
      <div class="metric">
        <LayoutGrid class="metric-icon" :size="13" :stroke-width="1.75" />
        <span class="k">tiles</span><span class="v">{{ aggregate ? `${aggregate.n}/${DEMO_IMAGES.length}` : `0/${DEMO_IMAGES.length}` }}</span>
      </div>
      <div class="spacer" />
      <span class="muted">hover a tile to reveal the original</span>
    </div>

    <div v-if="!allEncoded" class="encoding-progress">
      <Loader2 class="spin" :size="22" :stroke-width="1.5" />
      <div class="encoding-progress-text">
        encoding {{ settledCount }} / {{ DEMO_IMAGES.length }}
      </div>
      <div class="encoding-progress-bar">
        <div class="encoding-progress-fill" :style="{ width: `${progressPct}%` }" />
      </div>
    </div>

    <div v-show="allEncoded" class="wf">
      <!-- range-expand forces vue-wf to render every slot child even when the
           wrapper is display:none (during encoding) — otherwise its viewport
           virtualization only mounts tiles in the visible band, and the rest
           never encode, stalling the global progress at <total>. -->
      <Waterfall
        :items="items"
        :cols="cols"
        :gap="14"
        :item-padding="0"
        :range-expand="100000"
        layout="waterfall"
      >
        <Tile
          v-for="(img, i) in DEMO_IMAGES"
          :key="img.src"
          :src="img.src"
          :alt="img.alt"
          :aspect-w="img.w"
          :aspect-h="img.h"
          :shape="shape"
          :n-shapes="nShapesD"
          :base-size="baseSizeD"
          :blur="blurD"
          :corner-radius="cornerRadiusD"
          :dither="dither"
          :dither-scale="ditherScaleD"
          :seed="seedD"
          :use-svg="renderSvg"
          :ready="ready"
          :alpha-bits="advancedD.alphaBits"
          :color-id="colorId"
          :search="searchD"
          @metrics="(v) => onTileMetrics(i, v)"
        />
      </Waterfall>
    </div>
  </div>
</template>

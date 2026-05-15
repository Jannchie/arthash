<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { refDebounced } from "@vueuse/core";
import { Waterfall } from "vue-wf";
import { Shape, type Shape as ShapeType } from "@pfhash/ts";
import Tile from "./Tile.vue";
import AdvancedPanel, { type AdvancedConfig } from "./AdvancedPanel.vue";
import { DEMO_IMAGES } from "../demo";
import { COLOR_OPTIONS, supportsSvg } from "../pipeline";

interface Props {
  ready: boolean;
}
defineProps<Props>();

const shape = ref<ShapeType>(Shape.CIRCLE);
const nShapes = ref(12);
const baseSize = ref(256);
const blur = ref(0);
const seed = ref(0);
const useSvg = ref(true);
const cols = ref(8);
const colorId = ref<string>("rgb-565");

const advanced = ref<AdvancedConfig>({
  alphaBits: 3,
  overrideAspectEnabled: false,
  overrideAspect: 1,
});

const DEBOUNCE_MS = 300;
const nShapesD = refDebounced(nShapes, DEBOUNCE_MS);
const baseSizeD = refDebounced(baseSize, DEBOUNCE_MS);
const blurD = refDebounced(blur, DEBOUNCE_MS);
const seedD = refDebounced(seed, DEBOUNCE_MS);
const advancedD = refDebounced(advanced, DEBOUNCE_MS);

const effectiveAspect = computed(() =>
  advancedD.value.overrideAspectEnabled ? advancedD.value.overrideAspect : undefined,
);
const items = computed(() => DEMO_IMAGES.map((d) => ({ width: d.w, height: d.h })));

const tileMetrics = ref(new Map<number, { encodeMs: number; decodeMs: number; hashBytes: number }>());

const svgPossible = computed(() => supportsSvg(shape.value));
const renderSvg = computed(() => useSvg.value && svgPossible.value);

const aggregate = computed(() => {
  const ms = Array.from(tileMetrics.value.values());
  if (!ms.length) return null;
  const avg = (k: "encodeMs" | "decodeMs") => ms.reduce((a, b) => a + b[k], 0) / ms.length;
  const sizes = new Set(ms.map((m) => m.hashBytes));
  const sizeStr = sizes.size === 1 ? `${[...sizes][0]} B` : `${Math.min(...sizes)}–${Math.max(...sizes)} B`;
  return { n: ms.length, avgEncode: avg("encodeMs"), avgDecode: avg("decodeMs"), size: sizeStr };
});

function onTileMetrics(i: number, v: { encodeMs: number; decodeMs: number; hashBytes: number } | null) {
  if (v) tileMetrics.value.set(i, v);
  else tileMetrics.value.delete(i);
  tileMetrics.value = new Map(tileMetrics.value);
}

watch(
  [shape, nShapesD, baseSizeD, blurD, seedD, useSvg, advancedD, colorId],
  () => {
    tileMetrics.value = new Map();
  },
  { deep: true },
);

const shapeOptions: Array<{ value: ShapeType; label: string }> = [
  { value: Shape.DCT, label: "DCT" },
  { value: Shape.CIRCLE, label: "Circle" },
  { value: Shape.TRIANGLE, label: "Triangle" },
  { value: Shape.PIXEL, label: "Pixel" },
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
      <div class="spacer" />
      <AdvancedPanel v-model="advanced" />
    </div>

    <div class="metrics-row">
      <div class="metric">
        <span class="k">avg encode</span><span class="v">{{ aggregate ? fmtMs(aggregate.avgEncode) : "—" }}</span>
      </div>
      <div class="metric">
        <span class="k">avg decode</span><span class="v">{{ aggregate ? fmtMs(aggregate.avgDecode) : "—" }}</span>
      </div>
      <div class="metric">
        <span class="k">hash size</span><span class="v">{{ aggregate ? aggregate.size : "—" }}</span>
      </div>
      <div class="metric">
        <span class="k">tiles</span><span class="v">{{ aggregate ? `${aggregate.n}/${DEMO_IMAGES.length}` : `0/${DEMO_IMAGES.length}` }}</span>
      </div>
      <div class="spacer" />
      <span class="muted">hover a tile to reveal the original</span>
    </div>

    <div class="wf">
      <Waterfall :items="items" :cols="cols" :gap="14" :item-padding="0" layout="waterfall">
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
          :seed="seedD"
          :use-svg="renderSvg"
          :ready="ready"
          :alpha-bits="advancedD.alphaBits"
          :color-id="colorId"
          :override-aspect="effectiveAspect"
          @metrics="(v) => onTileMetrics(i, v)"
        />
      </Waterfall>
    </div>
  </div>
</template>

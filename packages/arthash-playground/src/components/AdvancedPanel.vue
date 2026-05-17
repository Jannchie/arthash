<script setup lang="ts">
import { computed, ref } from "vue";
import { SlidersHorizontal, ChevronDown, ChevronRight, RotateCcw } from "@lucide/vue";

export interface AdvancedConfig {
  alphaBits: number;
}

const model = defineModel<AdvancedConfig>({ required: true });

const open = ref(false);

function reset() {
  model.value = { alphaBits: 3 };
}

const alphaLevels = computed(() => 1 << model.value.alphaBits);

function clamp(n: number, lo: number, hi: number) {
  return Math.min(hi, Math.max(lo, n));
}
function set<K extends keyof AdvancedConfig>(k: K, v: AdvancedConfig[K]) {
  model.value = { ...model.value, [k]: v };
}
</script>

<template>
  <div class="advanced" :class="{ open }">
    <button class="adv-toggle" @click="open = !open" :aria-expanded="open">
      <SlidersHorizontal :size="13" :stroke-width="1.75" />
      <span>Advanced</span>
      <span class="muted summary">α{{ model.alphaBits }}</span>
      <component :is="open ? ChevronDown : ChevronRight" :size="12" :stroke-width="2" class="caret" />
    </button>
    <div v-show="open" class="adv-body">
      <div class="adv-grid">
        <label class="adv-cell">
          <span class="adv-k">alpha_bits</span>
          <span class="adv-row">
            <input type="range" min="0" max="5" step="1"
                   :value="model.alphaBits"
                   @input="set('alphaBits', clamp(+($event.target as HTMLInputElement).value, 0, 5))" />
            <span class="adv-v">{{ model.alphaBits }} ({{ alphaLevels }} levels)</span>
          </span>
        </label>
      </div>
      <div class="adv-actions">
        <button class="link icon-btn" @click="reset">
          <RotateCcw :size="12" :stroke-width="1.75" />
          <span>reset to defaults</span>
        </button>
      </div>
    </div>
  </div>
</template>

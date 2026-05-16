<script setup lang="ts">
import { computed, ref } from "vue";

export interface AdvancedConfig {
  alphaBits: number;
  overrideAspectEnabled: boolean;
  overrideAspect: number;
}

const model = defineModel<AdvancedConfig>({ required: true });

const open = ref(false);

function reset() {
  model.value = {
    alphaBits: 3,
    overrideAspectEnabled: false,
    overrideAspect: 1,
  };
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
      <span class="caret">{{ open ? "▾" : "▸" }}</span>
      <span>Advanced</span>
      <span class="muted summary">
        α{{ model.alphaBits }}<template v-if="model.overrideAspectEnabled"> ar={{ model.overrideAspect }}</template>
      </span>
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
        <label class="adv-cell">
          <span class="adv-k">override aspect</span>
          <span class="adv-row">
            <input type="checkbox"
                   :checked="model.overrideAspectEnabled"
                   @change="set('overrideAspectEnabled', ($event.target as HTMLInputElement).checked)" />
            <input type="number" min="0.1" max="10" step="0.05"
                   :disabled="!model.overrideAspectEnabled"
                   :value="model.overrideAspect"
                   @input="set('overrideAspect', clamp(+($event.target as HTMLInputElement).value, 0.1, 10))" />
            <span class="muted" v-if="!model.overrideAspectEnabled">use stored</span>
          </span>
        </label>
      </div>
      <div class="adv-actions">
        <button class="link" @click="reset">reset to defaults</button>
      </div>
    </div>
  </div>
</template>

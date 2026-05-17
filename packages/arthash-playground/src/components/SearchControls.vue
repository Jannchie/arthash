<script setup lang="ts">
import type { SearchConfig } from "../pipeline";

const model = defineModel<SearchConfig>({ required: true });

// `disabled` lets callers grey these out when the shape doesn't run a
// hill-climb (e.g. DCT, which has no search at all).
defineProps<{ disabled?: boolean }>();

function set<K extends keyof SearchConfig>(k: K, v: SearchConfig[K]) {
  model.value = { ...model.value, [k]: v };
}

function clamp(n: number, lo: number, hi: number) {
  if (!Number.isFinite(n)) return lo;
  return Math.min(hi, Math.max(lo, Math.round(n)));
}
</script>

<template>
  <label class="ctl" :class="{ muted: disabled }">
    <span>search</span>
    <select :value="model.strategy" :disabled="disabled" @change="set('strategy', ($event.target as HTMLSelectElement).value as SearchConfig['strategy'])">
      <option value="primitive">primitive</option>
      <option value="topk_uniform">topk_uniform</option>
    </select>
  </label>
  <label class="ctl" :class="{ muted: disabled }">
    <span>n_random</span>
    <input
      type="number"
      min="1"
      max="512"
      step="1"
      :value="model.nRandom"
      :disabled="disabled"
      @input="set('nRandom', clamp(+($event.target as HTMLInputElement).value, 1, 512))"
    />
  </label>
  <label class="ctl" :class="{ muted: disabled }">
    <span>max_age</span>
    <input
      type="number"
      min="1"
      max="500"
      step="1"
      :value="model.hillClimbMaxAge"
      :disabled="disabled"
      :title="'Stop a hill-climb after this many consecutive non-improving steps.'"
      @input="set('hillClimbMaxAge', clamp(+($event.target as HTMLInputElement).value, 1, 500))"
    />
  </label>
  <label class="ctl" :class="{ muted: disabled }">
    <span>attempts</span>
    <input
      type="number"
      min="1"
      max="32"
      step="1"
      :value="model.nAttempts"
      :disabled="disabled"
      @input="set('nAttempts', clamp(+($event.target as HTMLInputElement).value, 1, 32))"
    />
  </label>
</template>

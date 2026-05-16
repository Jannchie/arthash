<script setup lang="ts">
import { onMounted, ref } from "vue";
import { init as initArthash } from "arthash";
import GalleryView from "./components/GalleryView.vue";
import CompareView from "./components/CompareView.vue";

type TabId = "gallery" | "compare";
const tab = ref<TabId>("gallery");

const ready = ref(false);
const initError = ref("");

onMounted(async () => {
  try {
    await initArthash();
    ready.value = true;
  } catch (e) {
    initError.value = `wasm init failed: ${e instanceof Error ? e.message : String(e)}`;
  }
});
</script>

<template>
  <div class="app">
    <header class="bar">
      <div class="brand">
        <h1>arthash</h1>
        <span class="tag">placeholder image hash playground</span>
      </div>
      <nav class="tabs">
        <button :class="{ active: tab === 'gallery' }" @click="tab = 'gallery'">Gallery</button>
        <button :class="{ active: tab === 'compare' }" @click="tab = 'compare'">Compare</button>
      </nav>
      <div class="status">
        <span v-if="initError" class="err">{{ initError }}</span>
        <span v-else-if="!ready" class="muted">loading wasm…</span>
        <span v-else class="muted">wasm ready</span>
      </div>
    </header>

    <GalleryView v-show="tab === 'gallery'" :ready="ready" />
    <CompareView v-show="tab === 'compare'" :ready="ready" />
  </div>
</template>

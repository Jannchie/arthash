<script setup lang="ts">
import { onMounted, ref } from "vue";
import { init as initArthash } from "arthash";
import { Hash, LayoutGrid, Columns2, Loader2, CircleCheck, TriangleAlert } from "@lucide/vue";
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
        <Hash class="brand-icon" :size="18" :stroke-width="2" />
        <h1>arthash</h1>
      </div>
      <nav class="tabs">
        <button :class="{ active: tab === 'gallery' }" @click="tab = 'gallery'">
          <LayoutGrid :size="14" :stroke-width="1.75" />
          <span>Gallery</span>
        </button>
        <button :class="{ active: tab === 'compare' }" @click="tab = 'compare'">
          <Columns2 :size="14" :stroke-width="1.75" />
          <span>Compare</span>
        </button>
      </nav>
      <div class="status">
        <span v-if="initError" class="err status-pill">
          <TriangleAlert :size="13" :stroke-width="2" />
          <span>{{ initError }}</span>
        </span>
        <span v-else-if="!ready" class="muted status-pill">
          <Loader2 :size="13" :stroke-width="2" class="spin" />
          <span>loading wasm…</span>
        </span>
        <span v-else class="muted status-pill">
          <CircleCheck :size="13" :stroke-width="2" />
          <span>wasm ready</span>
        </span>
        <a
          class="gh-link"
          href="https://github.com/Jannchie/arthash"
          target="_blank"
          rel="noopener noreferrer"
          aria-label="View on GitHub"
          title="View on GitHub"
        >
          <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
            <path fill="currentColor" fill-rule="evenodd" d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0 0 16 8c0-4.42-3.58-8-8-8z" clip-rule="evenodd" />
          </svg>
        </a>
      </div>
    </header>

    <GalleryView v-show="tab === 'gallery'" :ready="ready" />
    <CompareView v-show="tab === 'compare'" :ready="ready" />
  </div>
</template>

import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

declare const process: { env: Record<string, string | undefined> };

export default defineConfig({
  base: process.env.BASE_URL ?? "/",
  plugins: [vue()],
  server: {
    port: 7631,
    fs: {
      // wasm + glue JS live one level up in the workspace.
      allow: [".."],
    },
  },
  optimizeDeps: {
    // wasm-bindgen glue contains a `new URL('...wasm', import.meta.url)`
    // pattern that Vite must keep intact.
    exclude: ["arthash"],
  },
});

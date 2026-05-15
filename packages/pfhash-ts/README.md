# @pfhash/ts

TypeScript SDK for pfhash. Backed by `pfhash-rs` compiled to WebAssembly via
wasm-bindgen — same byte-format contract, same algorithms, no Node native
modules. Works in browser and Node ≥ 18.

```ts
import { init, encode, decode, toSvg, Shape } from "@pfhash/ts";

await init();  // load the wasm module (~70 KB gzip)

const hash = encode(rgbBytes, width, height, {
  shape: Shape.CIRCLE,
  nShapes: 12,
});

const { w, h, rgba } = decode(hash, { shape: Shape.CIRCLE, nShapes: 12 });

const svg = toSvg(hash, { shape: Shape.CIRCLE, nShapes: 12, blur: 12 });
// → inline-ready: '<svg xmlns="..." viewBox="...">...</svg>'
```

## Build

Requires the Rust toolchain + [`wasm-pack`](https://rustwasm.github.io/wasm-pack/).

```sh
pnpm --filter @pfhash/ts run build
```

This runs in two phases:

1. `wasm-pack build --target web --out-dir pkg wasm` → emits the wasm
   artifact + JS glue into `wasm/pkg/`.
2. `tsc -p tsconfig.json` → emits typed ESM into `dist/`.

The published npm package bundles `dist/` and `wasm/pkg/`.

## Layout

```
packages/pfhash-ts/
├── src/index.ts        TS public API
├── wasm/
│   ├── Cargo.toml      wasm-bindgen crate (depends on ../pfhash-rs)
│   ├── src/lib.rs      Rust → JS shim
│   └── pkg/            wasm-pack output (gitignored)
└── dist/               tsc output (gitignored)
```

## Status

**Wasm wiring complete, byte-format conformance not yet cross-validated.**
The same Rust core is byte-conformance-tested in `pfhash-rs`. SVG output
is verified byte-identical to the Python reference.

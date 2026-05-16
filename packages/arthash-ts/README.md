# arthash

TypeScript SDK for arthash. Backed by `arthash-rs` compiled to WebAssembly via
wasm-bindgen — same byte-format contract, same algorithms, no Node native
modules. Works in browser and Node ≥ 18.

```ts
import { init, encode, decode, toSvg, Shape } from "arthash";

await init();  // load the wasm module (~70 KB gzip)

const hash = encode(rgbBytes, width, height, {
  shape: Shape.CIRCLE,
  nShapes: 12,
});

const { w, h, rgba } = decode(hash, { shape: Shape.CIRCLE, nShapes: 12 });

const svg = toSvg(hash, { shape: Shape.CIRCLE, nShapes: 12, blur: 12 });
// → inline-ready: '<svg xmlns="..." viewBox="...">...</svg>'
```

## Shape modes

All variants in `Shape` are wired up to the wasm core:

| Mode                | Look                                                |
|---------------------|-----------------------------------------------------|
| `Shape.DCT`         | thumbhash-style blurry placeholder (default codec). |
| `Shape.CIRCLE`      | SQIP-style overlapping circles.                     |
| `Shape.TRIANGLE`    | Primitive-style triangle mosaic.                    |
| `Shape.SQUARE`      | Axis-aligned squares (cx, cy, side).                |
| `Shape.RECT`        | Axis-aligned rectangles (cx, cy, w, h).             |
| `Shape.ROTATED_RECT`| Rotated rectangles — `thetaBits` tunes angle steps. |
| `Shape.PIXEL`       | Retro-palette pixel mosaic.                         |

`toSvg` supports CIRCLE / TRIANGLE / SQUARE / RECT / ROTATED_RECT;
DCT and PIXEL have no natural SVG primitive form and throw.

## Build

Requires the Rust toolchain + [`wasm-pack`](https://rustwasm.github.io/wasm-pack/).

```sh
pnpm --filter arthash run build
```

This runs in two phases:

1. `wasm-pack build --target web --out-dir pkg wasm` → emits the wasm
   artifact + JS glue into `wasm/pkg/`.
2. `tsc -p tsconfig.json` → emits typed ESM into `dist/`.

The published npm package bundles `dist/` and `wasm/pkg/`.

## Layout

```
packages/arthash-ts/
├── src/index.ts        TS public API
├── wasm/
│   ├── Cargo.toml      wasm-bindgen crate (depends on ../arthash-rs)
│   ├── src/lib.rs      Rust → JS shim
│   └── pkg/            wasm-pack output (gitignored)
└── dist/               tsc output (gitignored)
```

## Status

**Wasm wiring complete, byte-format conformance not yet cross-validated.**
The same Rust core is byte-conformance-tested in `arthash-rs`. SVG output
is verified byte-identical to the Python reference.

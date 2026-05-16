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

Requires the Rust toolchain (with the `wasm32-unknown-unknown` target) and
`wasm-bindgen-cli`. The CLI version MUST match the `wasm-bindgen` library
version in [`wasm/Cargo.toml`](./wasm/Cargo.toml):

```sh
rustup target add wasm32-unknown-unknown
cargo install -f --locked wasm-bindgen-cli --version 0.2.121
```

Then:

```sh
pnpm --filter arthash run build
```

This runs in four phases:

1. `cargo build --release --target wasm32-unknown-unknown --manifest-path wasm/Cargo.toml`
   → produces `wasm/target/wasm32-unknown-unknown/release/arthash_wasm.wasm`.
2. `wasm-bindgen --target web --out-dir wasm/pkg <wasm-file>`
   → emits the JS glue + typed bindings into `wasm/pkg/`. Unlike `wasm-pack`,
   `wasm-bindgen` writes ONLY the four artifacts the runtime needs
   (`arthash_wasm.{js,d.ts,_bg.wasm,_bg.wasm.d.ts}`), so the tree is
   directly safe to ship through `files` in `package.json`.
3. `wasm-opt -Oz` (from the `binaryen` devDependency) shrinks the wasm
   binary in place — typically ~20% smaller than the unoptimized output.
4. `tsc -p tsconfig.json` → emits typed ESM into `dist/`.

The published npm package bundles `dist/`, `wasm/pkg/`, `LICENSE`, and
`README.md`. The `prepack` script runs the full build before tarball
creation, so a `pnpm publish` straight from a clean checkout is safe.

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

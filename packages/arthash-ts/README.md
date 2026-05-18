# arthash

TypeScript SDK for arthash. Backed by `arthash-rs` compiled to WebAssembly via
wasm-bindgen — same byte-format contract, same algorithms, no Node native
modules. Works in browser and Node ≥ 18.

```ts
import { encode, decode, toSvg, codec, Preset, encodeImage } from "arthash";

// Wasm loads automatically on first call. Pre-load with `await init()` if you
// want to control timing (e.g. before a tight render loop).

// Named preset
const c = codec.preset(Preset.LargeTriangle);        // triangle, n=64
const hash = await encode(rgbBytes, width, height, c);
const { w, h, rgba } = await decode(hash, c);
const svg = await toSvg(hash, c, { blur: 12 });
// → inline-ready: '<svg xmlns="..." viewBox="...">...</svg>'

// Browser convenience — load + resize + encode in one call
const hash2 = await encodeImage(imageUrlOrBlob, c);

// Hot path? Synchronous variants assume `await init()` has completed.
import { encodeSync, decodeSync, toSvgSync, init } from "arthash";
await init();
const fastHash = encodeSync(rgbBytes, width, height, c);
```

## Codec factories

```ts
codec.dct()
codec.circle({ n: 12 })
codec.triangle({ n: 64 })
codec.square({ n: 12 })
codec.rect({ n: 12 })
codec.rotatedRect({ n: 12, thetaBits: 5 })
codec.pixel({ n: 16 })

// Switch to palette color
codec.withPalette(codec.triangle({ n: 24 }), { bytes: paletteBytes })

// Low-level escape hatch for advanced control
codec.raw({ shape: "triangle", nShapes: 12, alphaBits: 2, colorBits: 24 })
```

`toSvg` supports `circle` / `triangle` / `square` / `rect` / `rotrect`;
DCT and PIXEL have no natural SVG primitive form and throw.

## Footprint

A single monolithic wasm carries every codec (encode + decode + hill-climb
search + SVG render). The wire cost on a cold load:

| Artifact                              |    raw |  gzip | brotli |
| ------------------------------------- | -----: | ----: | -----: |
| `arthash_wasm_bg.wasm` (core)         | 218 KB | 82 KB |  67 KB |
| `arthash_wasm.js` (wasm-bindgen glue) |  14 KB |  4 KB |   3 KB |
| `dist/index.js` + `palettes.js` (SDK) |  21 KB |  7 KB |   6 KB |
| **Total over the wire**               | 253 KB | 93 KB |  76 KB |

The wasm is fetched once and HTTP-cached across sessions; the JS portion
is tree-shakeable, so importing only `decode` lets your bundler drop the
`encode` / `toSvg` / `encodeImage` paths from the SDK (wasm exports stay
intact). For frontends that only render placeholders, this means **one
~67 KB brotli download up-front, then ~6 KB of SDK code in your bundle**.

A separate decode-only wasm (hill-climb search + encoder feature-gated out,
estimated ~45–55 KB brotli) would shave another ~15–20 KB off the first
paint. Not implemented yet — [open an issue](https://github.com/Jannchie/arthash/issues)
if your use case needs it.

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

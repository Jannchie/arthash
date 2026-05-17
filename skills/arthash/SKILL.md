---
name: arthash
description: Use arthash to add tiny placeholder-image hashes (17–400 bytes) to an application — the artistic alternative to blurhash / thumbhash / sqip. Trigger this skill when the user wants to render a low-quality image preview (LQIP) while a real image loads, replace blurhash or thumbhash, generate SVG placeholders without spawning a Go subprocess (sqip), or produce a stylised mosaic from a palette. arthash ships three SDKs (Rust crate, Python `arthash`, npm `arthash`) that all read and write the same byte format, so a hash made on the server decodes identically in the browser. This skill covers picking the right SDK, the right mode (default `RECT` and other shape modes `TRIANGLE` / `CIRCLE` / `SQUARE` / `ROTATED_RECT` for SVG, `PIXEL` for mosaic, `DCT` only when you specifically want a blurhash/thumbhash-style blurry look), the right palette, the encode → store → decode → render lifecycle, and the small pitfalls (codecs are not in the bytes; inputs must be small; SVG modes vs raster modes). Per-language API details live in `typescript.md`, `python.md`, and `rust.md` next to this file.
---

# arthash — integration skill

arthash is a placeholder-image-hash family. You feed it an image, get back 17–400 bytes, store those bytes alongside the image record, and later decode them on the client into an RGBA preview or an inline SVG that's "good enough" to show while the real image loads. Compared to blurhash/thumbhash it produces a different look (geometric shapes, palette mosaic, or — when you opt into it — a DCT-blur that beats thumbhash by ~0.4 dB at the same byte budget). Compared to sqip it's ~50× faster, ~1/15 the size, and runs in the browser via WebAssembly.

## Per-language API details

For runtime requirements, full encode/store/decode examples, factories, palettes, sync vs async, and language-specific pitfalls, read the SDK file for the language you're working in:

- [`typescript.md`](./typescript.md) — npm `arthash`. Browser + Node ≥ 22.
- [`python.md`](./python.md) — PyPI `arthash`. CPython ≥ 3.9, NumPy / PIL aware.
- [`rust.md`](./rust.md) — `cargo add arthash`. Canonical implementation.

Hashes produced by one SDK decode identically in any other. Pick the encoder by where the source image already lives (server, pipeline, or upload) and decode wherever you render.

## Recommended starting point

**Start with the default rectangle codec** — `Codec::rect(24)` / `Codec.rect(n=24)` / `codec.rect({ n: 24 })`. It is the recommended baseline for new integrations:

- ~120 bytes per hash — small enough to ship inline.
- Outputs SVG you can write straight into HTML or paint into a canvas.
- No palette setup required to look presentable.
- Works the same in all three SDKs with no surprises.

Move off the default only when you have a reason to:

| Goal | Switch to |
|---|---|
| Smaller bytes, same SVG vibe | `Codec.rect(n=12)` (~59 B) |
| More detail, hero-image slot | `Codec.rect(n=64)` (~299 B) or `Codec.triangle(n=64)` |
| Softer / rounded look | `circle` / `rotated_rect` |
| Retro pixel-art / lo-fi mosaic | `pixel` mode + a palette (e.g. `PICO8`, `NES`) |
| Brand-coloured aesthetic | any shape or `pixel` mode + a custom palette |
| Specifically a **blurhash / thumbhash-style blurry** placeholder, smallest possible bytes | `Codec.dct()` (~21 B) — see "When to choose DCT" below |
| Content addressing or perceptual dedup | **NOT arthash** — use BLAKE3 / pHash. arthash is a lossy placeholder. |

The [live playground](https://arthash.jannchie.com) is the fastest way to compare modes on your own images before committing to one.

## When to choose DCT

`DCT` is the only mode that produces a smoothly-blurred raster placeholder (no visible shapes). Use it **only** when:

- You explicitly want a blurhash / thumbhash-style blurry look.
- You need the absolute minimum byte budget (~17–24 B).
- You don't need SVG output (DCT cannot render to SVG).

For everything else, prefer one of the shape modes — they cost a few more bytes but inline as SVG, scale crisply, and tend to look more deliberate next to the loaded image.

## The codec contract — the one thing that bites people

arthash hashes are **not self-describing**. The bytes contain no magic number, no mode tag, no bit widths. You MUST decode with the same `Codec` that encoded. Practically:

- Pick one codec per use-case slot (e.g. "avatars use `rect(n=12)`, hero images use `triangle(n=64)`") and write it down. Configure encode and decode with that codec, not with library defaults.
- If you support multiple codecs in the same data set, store a small codec identifier alongside the hash (e.g. a `codec` enum column in your DB, a `Preset` name, or a 1-byte prefix you strip before decode). Don't try to sniff the codec from the bytes — there's nothing to sniff.
- A few SDKs expose a library-level default (e.g. Rust's `Codec::default()` returns `Codec::Dct` for backwards compatibility). Don't rely on it in new code — always construct the codec explicitly so the choice is visible at the call site.

## Palettes (stylised look + smaller bytes)

Any shape or PIXEL codec can use an external palette. The per-shape colour field shrinks from 16/24 bit to `log₂(K)` bit (K=16 → 4 bit per shape), and the output adopts the palette's aesthetic.

Bundled palettes (`PICO8`, `NES`, `GAMEBOY`, `MORANDI`, …) are stable across versions and decode identically in any SDK. Custom palettes need to ship alongside the hash (or be derivable from your codec identifier) since the decoder needs the exact same palette bytes.

See the per-language SDK files for the import path and exact call syntax.

## Storage & wire format

- Hash bytes are arbitrary binary. For DBs without a binary column, base64-encode (`btoa(String.fromCharCode(...hashBytes))` / `base64.b64encode(hash).decode()` / `STANDARD.encode(&hash)`). At ~21–400 bytes, base64 overhead is negligible.
- If you serve placeholders via SSR, render `to_svg(...)` at build/request time and inline the resulting `<svg>` directly — no JS, no wasm load on the client. Use the wasm path only when you need to render at runtime or at multiple sizes.
- Decode runs on a base size you choose (`base_size: 256/512/1024`). arthash rasterises directly to that size, so you don't need CSS upscaling on top.

## Input requirements

The Rust core takes **raw pixel buffers at the encoder's target resolution**. The high-level entry points in each SDK (Python `encode("photo.jpg", codec)`, TS `encodeImage(urlOrBlob, codec)`, Rust `encode_image` behind the `image-io` feature) all resize the input for you. The low-level `encode` / `encode_rgb` calls do **not** resize — you must pass a pre-thumbnailed buffer:

- Shape modes (`RECT`/`SQUARE`/`ROTATED_RECT`/`CIRCLE`/`TRIANGLE`): 48 px long-edge.
- `DCT`: ≤ 100 px long-edge.
- `PIXEL`: 48–64 px long-edge.

Passing a 4K buffer to the low-level encoders still produces a hash but takes orders of magnitude longer for no quality gain. Resize first or use the high-level entry point.

## Common pitfalls (cross-SDK)

- **Decoding with a different codec than was used to encode.** Produces garbage. Always pair `(hash, codec)`. If you change the codec for new uploads, keep the old codec around for old records.
- **Calling `toSvg` / `to_svg` on a `DCT` or `PIXEL` hash.** Throws / returns `Err`. Both modes are inherently raster — render via `decode` and paint to canvas / write as PNG instead.
- **Encoding a full-resolution image with the low-level encoder.** Slow, no quality benefit. Use the high-level entry point or thumbnail first.
- **Expecting decode to upscale crisply.** It won't — arthash is a placeholder, not a super-resolution tool. The decode is intentionally smooth/stylised at any output size.
- **Treating arthash as a content hash.** Two visually different images can produce visually similar arthashes (that's the point). Use a real cryptographic / perceptual hash for dedup or addressing.
- **Forgetting palette bytes on decode.** A codec with a palette must be reconstructed with the same palette on the decoding side. Bundled palettes are stable; custom palettes must ship alongside the hash or be derivable from the codec identifier you stored.

## Where to look next

- Live playground for picking a mode visually: https://arthash.jannchie.com
- Mode/byte/quality comparison and feature matrix vs thumbhash/sqip: the project `README.md`.
- Byte-format details (only relevant if you're persisting raw bytes across implementations and want to be defensive): `docs/SPEC.md` in the source repo.
- TS bundle-size breakdown and `init()` semantics: `packages/arthash-ts/README.md` in the source repo.

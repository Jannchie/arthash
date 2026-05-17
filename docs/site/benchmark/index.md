# Benchmarks

All numbers below are measured on the same machine, with a 100×100 RGB input.

- **JS** numbers come from [`bench/js-cross/`](https://github.com/Jannchie/arthash/tree/main/bench/js-cross) on Node 22. Raw NDJSON lives in [`docs/benchmarks/js_cross_*.ndjson`](https://github.com/Jannchie/arthash/tree/main/docs/benchmarks).
- **Native** numbers come from [`docs/benchmarks/CROSS_IMPL.md`](https://github.com/Jannchie/arthash/blob/main/docs/benchmarks/CROSS_IMPL.md).
- Tables are sorted by speed; the *vs baseline* column is `baseline_time / row_time`, so values > 1 mean the row is faster.

## Headline numbers

| Scenario                          | Best result                                             |
| --------------------------------- | ------------------------------------------------------- |
| 24 B DCT encode (JS)              | **1.9× faster** than thumbhash-js                       |
| 24 B DCT encode (native)          | **1.27× faster** than the fastest thumbhash impl        |
| DCT decode @ 256 px (native)      | **5.9× faster** than thumbhash-go @ 256                 |
| TRIANGLE n=64 encode vs sqip (JS) | **67× faster** encode, **9× smaller** output            |
| 17 B placeholder PSNR             | DCT @ 17 B = 23.3 dB — beats thumbhash @ 17 B by 0.4 dB |

Drill into the comparisons:

- [**DCT vs thumbhash**](./dct) — same byte budget, where arthash gains its lead.
- [**Shape vs sqip**](./shape) — how shape modes beat sqip on both speed and size.
- [**Visual quality**](./quality) — PSNR vs bytes across modes.

## Why arthash wins

- **Headerless byte stream.** No magic number, no mode tag, no bit-widths — every bit goes to image data.
- **Per-channel adaptive AC scale.** Each image gets its own quant table without paying header bits for it.
- **Integral images + SSE incremental search.** Shape-mode search cost is sub-linear in `n`; sqip is linear and IPC-bound per primitive.
- **Decode straight to target size.** thumbhash-js only emits ~32 px output; arthash IDCTs directly to the display buffer.

See [How arthash saves bits](../guide/introduction#how-arthash-saves-bits) for the full algorithmic story.

## Reproducing

```sh
# JS bench (Node 22)
cd bench/js-cross
pnpm install
pnpm run bench           # emits docs/benchmarks/js_cross_*.ndjson

# Native bench
pnpm run bench:rs        # Rust
pnpm run bench:py        # Python via PyO3
pnpm run bench:binding   # cross-binding
```

The numbers below are stable to within ±5% across runs on the same machine.

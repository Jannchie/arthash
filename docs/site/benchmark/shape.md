# Shape modes vs sqip

arthash's shape modes (`CIRCLE`, `TRIANGLE`, `SQUARE`, `RECT`, `ROTATED_RECT`)
target the same use case as [sqip](https://github.com/axe312ger/sqip)'s
`primitive` plugin: hill-climb N geometric shapes onto an image and render the
result as SVG. arthash is faster, smaller, and runs in the browser.

## Encode time (JS, baseline = sqip-node)

arthash uses integral images + SSE incremental updates, so search cost is
sub-linear in `n`. sqip is linear and IPC-bound (re-hill-climb per primitive,
shelled out to a Go binary).

| Impl                           |     n=12 (ratio) |     n=24 (ratio) |      n=64 (ratio) |
| ------------------------------ | ---------------: | ---------------: | ----------------: |
| arthash · ts TRIANGLE          | 5.1 ms (**56×**) | 7.9 ms (**56×**) | 15.2 ms (**67×**) |
| arthash · ts CIRCLE            |     5.3 ms (54×) |     7.2 ms (62×) |     15.5 ms (66×) |
| sqip · primitive-triangle @0.3 |           284 ms |           446 ms |           1015 ms |

The gap **grows with `n`** — at n=12 arthash is 56× faster, at n=64 it's 67×.

## Output size

| Impl                           |           n=12 (ratio) |            n=24 (ratio) |            n=64 (ratio) |
| ------------------------------ | ---------------------: | ----------------------: | ----------------------: |
| arthash · ts CIRCLE            | 53 B (**16× smaller**) | 102 B (**15× smaller**) | 267 B (**14× smaller**) |
| arthash · ts TRIANGLE          |     77 B (11× smaller) |     150 B (10× smaller) |      395 B (9× smaller) |
| sqip · primitive-triangle @0.3 |                  842 B |                  1482 B |                  3650 B |

sqip emits a full SVG string with `<polygon points="..." fill="..." ... />`
markup for every primitive. arthash stores the geometry as a packed bit stream
with no markup overhead, and only converts to SVG at render time.

## Where each shines

|                       | sqip (`primitive`)        | arthash (shape modes)     |
| --------------------- | ------------------------- | ------------------------- |
| Runtime               | Node + Go subprocess      | Pure wasm (browser / Node) |
| Deployment            | **Build-time only**       | Build-time or request-time |
| Output                | Direct SVG string         | Packed hash → SVG on demand |
| Output size           | 800 B – 4 kB              | 50 B – 400 B               |
| Encode time @ n=64    | ~1 s                      | ~15 ms                     |
| Per-shape colour bits | 24 (truecolour) + alpha   | 16 (RGB-565) or 4 (palette) |

sqip is still a good fit when you want the SVG string directly with no
arthash-aware decoder. arthash wins when you control both ends and want either
a smaller payload or browser-side encoding.

## Notes on the comparison

- sqip was run with the `primitive-triangle` plugin at the default
  `progressive: 0.3` setting — this matches the plugin's published example
  configurations.
- arthash numbers use the default `Codec::triangle(n)` / `Codec::circle(n)`
  (RGB-565 colour, 3-bit alpha). Switching to palette mode shrinks them
  further; switching to RGB-888 grows them by ~8 bit per shape.
- All sqip / arthash output above is rendered to the same `viewBox` size for
  size-on-disk comparison.

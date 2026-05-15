// Serviced SQIP benchmark — long-running Node process that processes a
// batch of images via the SQIP npm API, reporting per-image timing.
//
// Usage:
//   node bench.js <mode> <n_shapes> <blur> <warmup> <path1> [path2 ...]
//
// Output (one line per image, after `warmup` runs are discarded):
//   <ms>\t<bytes>
//
// `ms` is wall-clock for the SQIP call only (does NOT include Node startup,
// since the process is already running by then). `bytes` is the resulting
// SVG length.
//
// This is what a "SQIP service" (long-running HTTP/RPC server using the SQIP
// API) would see per request: Node + module loads are paid once at boot,
// only the per-call cost remains. The Go `primitive` binary still spawns
// per call inside SQIP — there's no in-process API for it.

const sqip = require('sqip');

const args = process.argv.slice(2);
if (args.length < 5) {
  process.stderr.write(
    "Usage: node bench.js <mode> <n_shapes> <blur> <warmup> <path1> [path2 ...]\n"
  );
  process.exit(2);
}

const mode = parseInt(args[0], 10);
const n = parseInt(args[1], 10);
const blur = parseInt(args[2], 10);
const warmup = parseInt(args[3], 10);
const paths = args.slice(4);

// SQIP 0.3's package main is the node API itself (require('sqip') === fn).
// It's synchronous and returns { final_svg, svg_base64encoded, img_dimensions }.
const run = (p) =>
  sqip({ filename: p, numberOfPrimitives: n, mode, blur });

try {
  // Warmup runs (discarded; primes any per-process caches).
  for (let i = 0; i < Math.min(warmup, paths.length); i++) {
    run(paths[i]);
  }

  for (const p of paths) {
    const t0 = process.hrtime.bigint();
    const result = run(p);
    const t1 = process.hrtime.bigint();
    const ms = Number(t1 - t0) / 1e6;
    const svg = (result && result.final_svg) || '';
    process.stdout.write(`${ms.toFixed(3)}\t${svg.length}\n`);
  }
} catch (err) {
  process.stderr.write(`bench error: ${err && err.stack ? err.stack : err}\n`);
  process.exit(1);
}

// Run sqip on a single image, output SVG + a rendered PNG. Prints a JSON
// summary on stdout (bytes / dims) for the caller.
import { sqip } from "sqip";
import { resolve } from "node:path";
import { writeFileSync } from "node:fs";
import sharp from "sharp";

const inputPath = process.argv[2];
const outSvg = process.argv[3];
const outPng = process.argv[4];
const renderW = parseInt(process.argv[5] || "256", 10);
const numShapes = parseInt(process.argv[6] || "12", 10);

const result = await sqip({
  input: resolve(inputPath),
  plugins: [
    { name: "sqip-plugin-primitive", options: { numberOfPrimitives: numShapes, mode: 0 } },
    "sqip-plugin-svgo",
  ],
});

const rec = Array.isArray(result) ? result[0] : result;
const svg = rec.content.toString();
writeFileSync(outSvg, svg);

// Render to PNG at renderW long-edge (sharp renders SVG via librsvg).
const origW = rec.metadata?.originalWidth;
const origH = rec.metadata?.originalHeight;
const scale = renderW / Math.max(origW, origH);
const targetW = Math.round(origW * scale);
const targetH = Math.round(origH * scale);
await sharp(Buffer.from(svg))
  .resize(targetW, targetH)
  .png()
  .toFile(outPng);
console.log(JSON.stringify({
  bytes: svg.length,
  origW, origH,
  pngW: targetW, pngH: targetH,
}));

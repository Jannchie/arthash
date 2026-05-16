#!/usr/bin/env node
// Verify all manifests agree with VERSION (and optionally a release tag).
//
//   node scripts/check-versions.mjs            → assert all manifests match VERSION
//   node scripts/check-versions.mjs v0.2.0     → also assert tag matches VERSION
//
// Used by CI's version-check step to gate publishing. arthash's policy
// (see RELEASING.md) is that every manifest carries the same version; this
// script is the enforcement point. It reads the manifests as text using the
// same anchors as scripts/bump-version.mjs so the two stay in lockstep.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(__dirname, "..");

const TAG = process.argv[2]; // optional — pass "v0.2.0" or "0.2.0"

function extract(file, re, label) {
  const txt = fs.readFileSync(path.join(REPO, file), "utf8");
  const m = txt.match(re);
  if (!m) throw new Error(`${file}: could not find ${label}`);
  return m[1];
}

function tomlVersion(file, section) {
  return extract(
    file,
    new RegExp(`\\[${section}\\][\\s\\S]*?\\nversion\\s*=\\s*"([^"]+)"`),
    `[${section}] → version`,
  );
}

const sources = {
  VERSION: fs.readFileSync(path.join(REPO, "VERSION"), "utf8").trim(),
  "arthash-rs/Cargo.toml": tomlVersion("packages/arthash-rs/Cargo.toml", "package"),
  "arthash-py/Cargo.toml": tomlVersion("packages/arthash-py/Cargo.toml", "package"),
  "arthash-py/pyproject.toml": tomlVersion("packages/arthash-py/pyproject.toml", "project"),
  "arthash-py/__about__.py": extract(
    "packages/arthash-py/python/arthash/__about__.py",
    /__version__\s*=\s*"([^"]+)"/,
    "__version__",
  ),
  "arthash-ts/package.json": extract(
    "packages/arthash-ts/package.json",
    /"version"\s*:\s*"([^"]+)"/,
    '"version"',
  ),
  "arthash-ts/wasm/Cargo.toml": tomlVersion("packages/arthash-ts/wasm/Cargo.toml", "package"),
};

const canonical = sources["VERSION"];
const drift = Object.entries(sources).filter(([, v]) => v !== canonical);

console.log(`VERSION = ${canonical}\n`);
for (const [src, v] of Object.entries(sources)) {
  const mark = v === canonical ? "✓" : "✗";
  console.log(`  ${mark} ${src.padEnd(34)} ${v}`);
}

if (drift.length > 0) {
  console.error(`\nerror: ${drift.length} manifest(s) disagree with VERSION (${canonical}).`);
  console.error("Run: node scripts/bump-version.mjs " + canonical);
  process.exit(1);
}

if (TAG !== undefined) {
  const tagVer = TAG.replace(/^v/, "");
  if (tagVer !== canonical) {
    console.error(`\nerror: tag '${TAG}' (→ ${tagVer}) does not match VERSION (${canonical}).`);
    process.exit(1);
  }
  console.log(`\n✓ tag ${TAG} matches VERSION.`);
} else {
  console.log("\n✓ all manifests agree.");
}

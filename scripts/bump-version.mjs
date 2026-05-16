#!/usr/bin/env node
// Bump arthash's unified version across every manifest in lockstep.
//
//   node scripts/bump-version.mjs <new-version>
//
// arthash uses a single version number across the Rust crate, the Python
// package, the npm package, and the wasm wrapper crate — see RELEASING.md.
// This script is the only supported way to bump that number; it edits all
// seven sites atomically and prints the lockfile-refresh + tag commands
// the user should run next. CI's version-check job assumes they all agree.
//
// Edits the FIRST `version = "..."` after `[package]` (or `[project]`) in
// each TOML file — never a dependency's version line. Re-running with the
// current version is a no-op and exits 0.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(__dirname, "..");

const NEW = process.argv[2];
if (!NEW) {
  console.error("usage: node scripts/bump-version.mjs <new-version>");
  process.exit(2);
}
// PEP 440 + SemVer overlap: x.y.z with optional `-tag.N` or `aN`/`bN`/`rcN`.
if (!/^\d+\.\d+\.\d+([\-.][\w.]+)?$/.test(NEW)) {
  console.error(`error: '${NEW}' is not a valid version string`);
  process.exit(2);
}

/** Replace the first `version = "..."` inside the FIRST table in a TOML doc.
 *  Anchors on the section header so we never touch a dependency's
 *  `version = "..."` line lower in the file. */
function bumpToml(txt, section /* "package" | "project" */) {
  const re = new RegExp(
    `(\\[${section}\\][\\s\\S]*?\\n)version(\\s*=\\s*)"[^"]+"`,
    "m",
  );
  if (!re.test(txt)) {
    throw new Error(`could not find [${section}] → version = "..." block`);
  }
  return txt.replace(re, (_m, head, eq) => `${head}version${eq}"${NEW}"`);
}

/** Each target is one place we have to keep in sync. */
const targets = [
  {
    file: "VERSION",
    transform: () => `${NEW}\n`,
  },
  {
    file: "packages/arthash-rs/Cargo.toml",
    transform: (t) => bumpToml(t, "package"),
  },
  {
    file: "packages/arthash-py/Cargo.toml",
    transform: (t) => bumpToml(t, "package"),
  },
  {
    file: "packages/arthash-py/pyproject.toml",
    transform: (t) => bumpToml(t, "project"),
  },
  {
    file: "packages/arthash-py/python/arthash/__about__.py",
    transform: (t) => t.replace(/__version__\s*=\s*"[^"]+"/, `__version__ = "${NEW}"`),
  },
  {
    file: "packages/arthash-ts/package.json",
    transform: (t) => {
      // Edit JSON as text to preserve indentation and trailing newline
      // exactly. JSON.parse → JSON.stringify loses formatting.
      const re = /("version"\s*:\s*)"[^"]+"/;
      if (!re.test(t)) throw new Error('could not find "version" key');
      return t.replace(re, (_m, head) => `${head}"${NEW}"`);
    },
  },
  {
    file: "packages/arthash-ts/wasm/Cargo.toml",
    transform: (t) => bumpToml(t, "package"),
  },
];

let changed = 0;
for (const t of targets) {
  const abs = path.join(REPO, t.file);
  const before = fs.readFileSync(abs, "utf8");
  const after = t.transform(before);
  if (after === before) {
    console.log(`  ${t.file}: already at ${NEW}`);
    continue;
  }
  fs.writeFileSync(abs, after);
  console.log(`✓ ${t.file}`);
  changed += 1;
}

if (changed === 0) {
  console.log(`\nnothing to do — every manifest already says ${NEW}.`);
  process.exit(0);
}

console.log(`\nupdated ${changed} file(s). Next steps:\n`);
console.log("  # refresh lockfiles so they record the new version");
console.log("  cargo update -p arthash --manifest-path packages/arthash-rs/Cargo.toml");
console.log("  cargo update -p arthash-py --manifest-path packages/arthash-py/Cargo.toml");
console.log("  cargo update -p arthash-wasm --manifest-path packages/arthash-ts/wasm/Cargo.toml");
console.log("  pnpm install --lockfile-only");
console.log("");
console.log("  # review, commit, tag");
console.log("  git diff");
console.log(`  git commit -am 'release v${NEW}'`);
console.log(`  git tag v${NEW}`);
console.log("  git push && git push --tags");

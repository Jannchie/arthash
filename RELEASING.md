# Releasing arthash

arthash uses **fixed versioning**: the Rust crate, the Python package, the
npm package, and the wasm wrapper crate all share one version number.
A given `arthash@X.Y.Z` produces and decodes the same bytes regardless of
which SDK you call it from — that's the contract this policy enforces.

## Where the version lives

| File | Purpose |
|---|---|
| `VERSION` | Single source of truth (plain text, one line). |
| `packages/arthash-rs/Cargo.toml` | Rust crate `arthash` — published to crates.io. |
| `packages/arthash-py/Cargo.toml` | PyO3 binding crate `arthash-py` (path dep on arthash-rs). |
| `packages/arthash-py/pyproject.toml` | Python distribution `arthash` — published to PyPI. |
| `packages/arthash-py/python/arthash/__about__.py` | Runtime `arthash.__version__`. |
| `packages/arthash-ts/package.json` | npm package `arthash` — published to npmjs.com. |
| `packages/arthash-ts/wasm/Cargo.toml` | Internal wasm wrapper crate (not published). |

`scripts/check-versions.mjs` is the canonical asserter — CI runs it on every
release tag, and you can run it locally any time.

## Release flow

```sh
# 1. Bump every manifest in lockstep.
node scripts/bump-version.mjs 0.2.0

# 2. Refresh lockfiles so they record the new version.
cargo update -p arthash      --manifest-path packages/arthash-rs/Cargo.toml
cargo update -p arthash-py   --manifest-path packages/arthash-py/Cargo.toml
cargo update -p arthash-wasm --manifest-path packages/arthash-ts/wasm/Cargo.toml
pnpm install --lockfile-only

# 3. (optional) Verify nothing drifted.
node scripts/check-versions.mjs

# 4. Commit, tag, push. CI does the rest.
git commit -am 'release v0.2.0'
git tag v0.2.0
git push && git push --tags
```

The single `v0.2.0` tag fans out to three workflows in parallel:

- `.github/workflows/wheels.yml` builds Python wheels for every supported
  OS×arch×Python combo and publishes to PyPI via Trusted Publishing (OIDC).
- `.github/workflows/npm-publish.yml` builds the wasm + ESM artifacts and
  publishes to npm with provenance attestation.
- `.github/workflows/crates-publish.yml` runs `cargo test` then `cargo
  publish` for `arthash-rs`, using crates.io Trusted Publishing (OIDC).

All three start with a `version-check` job that calls
`scripts/check-versions.mjs "$GITHUB_REF_NAME"` — if any of the seven sites
disagrees with the tag, publishing aborts before anything reaches a registry.

The three publishes are independent: if (say) npm rejects the package, the
PyPI / crates.io publishes still complete. If you need to re-fire one
workflow after fixing a registry-side issue, use **workflow_dispatch** from
the Actions tab — it skips the tag-based version-check (because the tag
already exists) and proceeds straight to publish.

## Why fixed versioning

arthash's "product" is the byte-format contract in [`docs/SPEC.md`](./docs/SPEC.md);
all three SDKs are language wrappers around the same Rust core. Apache Arrow
and Protobuf do the same thing for the same reason — when the wire format IS
the public API, decoupled SDK versions just create cognitive overhead
("does Python 0.5.3 read what TS 0.6.1 writes?").

The cost: a fix that only touches one SDK still triggers a publish for the
other two. We accept that; for a small SDK family it's cheap, and consumers
get a clean "I'm on arthash 0.2.0" mental model.

## What gets published, what doesn't

| Crate / Package | Registry | Workflow |
|---|---|---|
| `arthash` (Rust) | crates.io | `crates-publish.yml` |
| `arthash` (Python) | PyPI | `wheels.yml` |
| `arthash` (npm) | npmjs.com | `npm-publish.yml` |
| `arthash-py` (PyO3 glue crate) | — | not published; build artifact of the Python wheel. |
| `arthash-wasm` (wasm-bindgen shim) | — | not published; baked into the npm tarball as `wasm/pkg/`. |

## First-time registry setup

Each of the three registries needs one-time configuration before the first
publish, since we're using OIDC / Trusted Publishing everywhere:

- **PyPI**: create a Pending Publisher at
  `https://pypi.org/manage/account/publishing/` — repo `Jannchie/arthash`,
  workflow `wheels.yml`, environment `pypi`.
- **npm**: provenance works automatically; if you opt out of OIDC and use a
  classic token, save it as `NPM_TOKEN` in the `npm` GitHub environment.
- **crates.io**: create a Pending Trusted Publisher at the crate's
  Settings page — repo `Jannchie/arthash`, workflow `crates-publish.yml`,
  environment `crates-io`.

After the first successful publish each can be flipped from "pending" to
"confirmed" and won't need re-config for subsequent releases.

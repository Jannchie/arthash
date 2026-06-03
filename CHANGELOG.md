## v0.5.0

[v0.4.0...v0.5.0](https://github.com/Jannchie/arthash/compare/v0.4.0...v0.5.0)

### :sparkles: Features

- **rs**: RGBA encode and fallible try_* API with typed errors - By [Jianqi Pan](mailto:jannchie@gmail.com) in [44926ab](https://github.com/Jannchie/arthash/commit/44926ab)
- **py**: expose encode_rgba and preset shortcuts, unify thumbnail loaders - By [Jianqi Pan](mailto:jannchie@gmail.com) in [0090f94](https://github.com/Jannchie/arthash/commit/0090f94)

### :zap: Performance

- **rs**: byte-compatible incremental rebuild, alpha-sweep sum reuse, fused colorspace - By [Jianqi Pan](mailto:jannchie@gmail.com) in [d0cf419](https://github.com/Jannchie/arthash/commit/d0cf419)
- **ts**: zero-copy RGBA extraction via intoRgba() - By [Jianqi Pan](mailto:jannchie@gmail.com) in [8ffd58e](https://github.com/Jannchie/arthash/commit/8ffd58e)

### :adhesive_bandage: Fixes

- **deps**: resolve thash from PyPI instead of the ../thumbhash-py sibling - By [Jianqi Pan](mailto:jannchie@gmail.com) in [9ef1c22](https://github.com/Jannchie/arthash/commit/9ef1c22)
- **ci**: use `uv sync --frozen` so the lock check never reads the thumbhash-py sibling - By [Jianqi Pan](mailto:jannchie@gmail.com) in [6a730c9](https://github.com/Jannchie/arthash/commit/6a730c9)
- **ci**: move research deps to opt-in group so `uv sync` works without thumbhash-py - By [Jianqi Pan](mailto:jannchie@gmail.com) in [8d9632e](https://github.com/Jannchie/arthash/commit/8d9632e)

### :memo: Documentation

- **perf**: document Opt 4 incremental Integral2D rebuild; freeze historical PoC table - By [Jianqi Pan](mailto:jannchie@gmail.com) in [5097db7](https://github.com/Jannchie/arthash/commit/5097db7)

### :white_check_mark: Tests

- add byte-compat regression harness (golden + properties + vectors) - By [Jianqi Pan](mailto:jannchie@gmail.com) in [a1c3b2d](https://github.com/Jannchie/arthash/commit/a1c3b2d)

### :construction_worker: CI

- add GitHub Actions workflow - By [Jianqi Pan](mailto:jannchie@gmail.com) in [165bcea](https://github.com/Jannchie/arthash/commit/165bcea)

### :wrench: Chores

- **bench**: extend coverage to rect/square/rotrect - By [Jianqi Pan](mailto:jannchie@gmail.com) in [92490c1](https://github.com/Jannchie/arthash/commit/92490c1)

## v0.4.0

[v0.3.1...v0.4.0](https://github.com/Jannchie/arthash/compare/v0.3.1...v0.4.0)

### :sparkles: Features

- **palette**: add support for non-power-of-two palettes - By [Jianqi Pan](mailto:jannchie@gmail.com) in [c630f3b](https://github.com/Jannchie/arthash/commit/c630f3b)

## v0.3.1

[v0.3.0...v0.3.1](https://github.com/Jannchie/arthash/compare/v0.3.0...v0.3.1)

### :adhesive_bandage: Fixes

- **ci**: ensure cargo publish and test use --locked - By [Jianqi Pan](mailto:jannchie@gmail.com) in [73e5d87](https://github.com/Jannchie/arthash/commit/73e5d87)

### :wrench: Chores

- **changelog**: remove outdated changelog information - By [Jianqi Pan](mailto:jannchie@gmail.com) in [3ec9b36](https://github.com/Jannchie/arthash/commit/3ec9b36)

## v0.3.0

[v0.2.0...v0.3.0](https://github.com/Jannchie/arthash/compare/v0.2.0...v0.3.0)

### :rocket: Breaking Changes

- **preset**: rename presets for clarity and add new shape types - By [Jianqi Pan](mailto:jannchie@gmail.com) in [367e759](https://github.com/Jannchie/arthash/commit/367e759)

### :sparkles: Features

- **readme**: add visual styling support with RenderStyle - By [Jianqi Pan](mailto:jannchie@gmail.com) in [57f76b7](https://github.com/Jannchie/arthash/commit/57f76b7)
- **render**: add render style for blur and corner rounding - By [Jianqi Pan](mailto:jannchie@gmail.com) in [0801f30](https://github.com/Jannchie/arthash/commit/0801f30)

### :memo: Documentation

- **readme**: update README with new image and styles - By [Jianqi Pan](mailto:jannchie@gmail.com) in [e7ab05a](https://github.com/Jannchie/arthash/commit/e7ab05a)

### :wrench: Chores

- **github-actions**: update node version to 24 in workflows - By [Jianqi Pan](mailto:jannchie@gmail.com) in [1927af5](https://github.com/Jannchie/arthash/commit/1927af5)

## v0.2.0

[v0.1.2...v0.2.0](https://github.com/Jannchie/arthash/compare/v0.1.2...v0.2.0)

### :rocket: Breaking Changes

- **docs**: add changelog and migration documentation - By [Jianqi Pan](mailto:jannchie@gmail.com) in [1143d99](https://github.com/Jannchie/arthash/commit/1143d99)

### :sparkles: Features

- **animation**: add animation view and search controls - By [Jianqi Pan](mailto:jannchie@gmail.com) in [616cdc7](https://github.com/Jannchie/arthash/commit/616cdc7)
- **docs**: initialize documentation and add user guide - By [Jianqi Pan](mailto:jannchie@gmail.com) in [94bc8da](https://github.com/Jannchie/arthash/commit/94bc8da)
- **docs**: add comprehensive documentation and benchmarks - By [Jianqi Pan](mailto:jannchie@gmail.com) in [fc2300a](https://github.com/Jannchie/arthash/commit/fc2300a)
- **logo**: add logo generation script and svg file - By [Jianqi Pan](mailto:jannchie@gmail.com) in [61add74](https://github.com/Jannchie/arthash/commit/61add74)
- **ui**: add medium circle and pixel presets - By [Jianqi Pan](mailto:jannchie@gmail.com) in [9fa070b](https://github.com/Jannchie/arthash/commit/9fa070b)

### :adhesive_bandage: Fixes

- **ci**: bump all workflows to Node 22 - By [Jianqi Pan](mailto:jannchie@gmail.com) and [Claude Opus 4.7 (1M context)](mailto:noreply@anthropic.com) in [0780e72](https://github.com/Jannchie/arthash/commit/0780e72)
- **ci**: pin pnpm to 11.1.2 for npm OIDC Trusted Publishing - By [Jianqi Pan](mailto:jannchie@gmail.com) and [Claude Opus 4.7 (1M context)](mailto:noreply@anthropic.com) in [d53761e](https://github.com/Jannchie/arthash/commit/d53761e)
- **ci**: upgrade pnpm to 10 for npm OIDC Trusted Publishing - By [Jianqi Pan](mailto:jannchie@gmail.com) and [Claude Opus 4.7 (1M context)](mailto:noreply@anthropic.com) in [73df796](https://github.com/Jannchie/arthash/commit/73df796)

### :memo: Documentation

- **readme**: update footprint information for wasm - By [Jianqi Pan](mailto:jannchie@gmail.com) in [16d3517](https://github.com/Jannchie/arthash/commit/16d3517)
- **readme**: update readme with project details and benchmarks - By [Jianqi Pan](mailto:jannchie@gmail.com) in [2045704](https://github.com/Jannchie/arthash/commit/2045704)

### :wrench: Chores

- **build**: update build scripts and ignore patterns - By [Jianqi Pan](mailto:jannchie@gmail.com) in [dd2b7f6](https://github.com/Jannchie/arthash/commit/dd2b7f6)
- **deps**: update dependencies and workspace configuration - By [Jianqi Pan](mailto:jannchie@gmail.com) in [117e955](https://github.com/Jannchie/arthash/commit/117e955)
- **deps**: allow esbuild postinstall script - By [Jianqi Pan](mailto:jannchie@gmail.com) and [Claude Opus 4.7 (1M context)](mailto:noreply@anthropic.com) in [0b435c8](https://github.com/Jannchie/arthash/commit/0b435c8)
- **docs**: update readme with new icon and layout - By [Jianqi Pan](mailto:jannchie@gmail.com) in [59da81c](https://github.com/Jannchie/arthash/commit/59da81c)

## v0.1.2

[v0.1.1...v0.1.2](https://github.com/Jannchie/arthash/compare/v0.1.1...v0.1.2)

### :adhesive_bandage: Fixes

- **ci**: work around pnpm 9 --filter pack/publish bug - By [Jianqi Pan](mailto:jannchie@gmail.com) and [Claude Opus 4.7 (1M context)](mailto:noreply@anthropic.com) in [e74fbe3](https://github.com/Jannchie/arthash/commit/e74fbe3)

## v0.1.1

[py-v0.1.0...v0.1.1](https://github.com/Jannchie/arthash/compare/py-v0.1.0...v0.1.1)

### :sparkles: Features

- **docs**: update README and SPEC for new shape modes - By [Jianqi Pan](mailto:jannchie@gmail.com) in [619c87d](https://github.com/Jannchie/arthash/commit/619c87d)
- **release**: add bump-version and check-versions scripts - By [Jianqi Pan](mailto:jannchie@gmail.com) and [Claude Opus 4.7 (1M context)](mailto:noreply@anthropic.com) in [e354818](https://github.com/Jannchie/arthash/commit/e354818)
- **workflow**: add ci workflows for npm and crates publishing - By [Jianqi Pan](mailto:jannchie@gmail.com) in [ecc917c](https://github.com/Jannchie/arthash/commit/ecc917c)

### :lipstick: Styles

- **styles**: remove transition from encoding progress fill - By [Jianqi Pan](mailto:jannchie@gmail.com) in [796639f](https://github.com/Jannchie/arthash/commit/796639f)

### :wrench: Chores

- **ci**: update base url in workflow && remove sqip-bench package lock - By [Jianqi Pan](mailto:jannchie@gmail.com) in [3b2dcab](https://github.com/Jannchie/arthash/commit/3b2dcab)
- **ci**: update node and pnpm actions - By [Jianqi Pan](mailto:jannchie@gmail.com) in [083822f](https://github.com/Jannchie/arthash/commit/083822f)

## py-v0.1.0

[75421de685c7d026ca80f91dcf1feddc6322d9e7...py-v0.1.0](https://github.com/Jannchie/arthash/compare/75421de685c7d026ca80f91dcf1feddc6322d9e7...py-v0.1.0)

### :sparkles: Features

- **bench**: add hill-climb benchmarking and performance optimizations - By [Jianqi Pan](mailto:jannchie@gmail.com) in [39a3dac](https://github.com/Jannchie/arthash/commit/39a3dac)
- **canvas**: add blurring support for canvas element - By [Jianqi Pan](mailto:jannchie@gmail.com) in [ed32176](https://github.com/Jannchie/arthash/commit/ed32176)
- **gallery**: add encoding progress UI for image gallery - By [Jianqi Pan](mailto:jannchie@gmail.com) in [00f84c7](https://github.com/Jannchie/arthash/commit/00f84c7)
- **optimization**: add residual-driven init and drift-free Gaussian step - By [Jianqi Pan](mailto:jannchie@gmail.com) in [1d3203b](https://github.com/Jannchie/arthash/commit/1d3203b)
- **shapes**: add square and rotated rectangle support - By [Jianqi Pan](mailto:jannchie@gmail.com) in [a035086](https://github.com/Jannchie/arthash/commit/a035086)

### :adhesive_bandage: Fixes

- **py**: sync DEFAULT_SEARCH n_random with Rust default, regenerate test vectors - By [Jianqi Pan](mailto:jannchie@gmail.com) in [9fe1969](https://github.com/Jannchie/arthash/commit/9fe1969)

### :memo: Documentation

- update readme with project highlights and benchmarks - By [Jianqi Pan](mailto:jannchie@gmail.com) in [461b41d](https://github.com/Jannchie/arthash/commit/461b41d)

### :construction_worker: CI

- **github-actions**: add github actions for playground deployment - By [Jianqi Pan](mailto:jannchie@gmail.com) in [eb8c15c](https://github.com/Jannchie/arthash/commit/eb8c15c)

### :wrench: Chores

- **ci**: update workflow for arthash package - By [Jianqi Pan](mailto:jannchie@gmail.com) in [ee4dfca](https://github.com/Jannchie/arthash/commit/ee4dfca)
- **deps**: update lock files - By [Jianqi Pan](mailto:jannchie@gmail.com) in [be900c6](https://github.com/Jannchie/arthash/commit/be900c6)
- **modules**: rename @arthash/ts to arthash across the project - By [Jianqi Pan](mailto:jannchie@gmail.com) in [a6e1e1d](https://github.com/Jannchie/arthash/commit/a6e1e1d)

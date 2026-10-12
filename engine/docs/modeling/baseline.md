# G0 baseline (measured 2026-10-12)

Reference point for every Track 2 gate: what exists, what was measured, and on which machine.
Figures here are facts from this checkout; anything labelled *target* is not.

## Checkout and toolchain
| Item | Value |
|---|---|
| Commit | `c8de121` (lens warp), on `b0c2b75` viewer, `f24d463` audit hardening, `2ad18b5` M11b |
| Rust | 1.97.1, edition 2024, pinned in `rust-toolchain.toml`; targets wasm32-unknown-unknown and wasm32-wasip1 |
| C++ | CMake 4.0.3, Ninja 1.11.1, wasi-sdk 34.0 (arm64 macOS), C++17, `-fno-exceptions` |
| JS runtimes | Node 26.7.0 (engine tests), Bun 1.3.10 (root app). Engine `xtask/src/web.rs` still uses npm: ADR 0007 moves it to Bun |
| Vendored | Bevy 0.20.0 (38 crates), wgpu-hal 30.0.1 (2 patches), Manifold 3.5.4 (`ce50d78`), Clipper2 2.0.1 (`46f6391`), meshoptimizer 1.3 (`9e1f07b`) |

## Gates
`cargo xtask check --strict` passed at `c8de121`: fmt, clippy `-D warnings`, 155 nextest tests, wasm
clippy, cargo deny, C++ checks, parity, web build, the Playwright suites (engine, xr, xr-layer,
xr-lifecycle, xr-resolution, interact, xr-interact, viewer, quality, perf, bench-session) and rules.
`cargo xtask advisories` is a separate gate. Skips: none under `--strict` (a skip would fail it).
Not covered by any gate: real headset behaviour, frame time, hand tracking, viewport-scale support
on a real Quest, pointer lock in a real browser.

## Build artifacts (release profile: opt 3, thin LTO, line tables)
| Artifact | Raw | Brotli q9 |
|---|---|---|
| `ve_runtime_bg.wasm` as built | 33,168,092 B | 3,861,086 B |
| same, custom sections stripped (names and debug) | 18,472,141 B | 3,240,417 B |
| dev build (`web-dev`, opt 1) | 61,330,801 B | not measured |

`wasm-opt` was not run, so the shippable size is probably lower; measure it in G13. Track 1's target
(4 MB brotli) is met by the stripped figure. Gzip of the as-built file is 5.84 MB.
Track 2 budget: every gate records the size delta against 3.24 MB; ADR 0006 sets the worker split.

## Benchmark environment (named for every performance claim)
Apple M1, 16 GB, macOS 26.1; Chromium from the installed Playwright (1.64) with SwiftShader WebGL2
(software rendering: correct for goldens, **not** a performance reference); release build; warm runs
after 20 discarded frames or evaluations; cold runs reported separately. Headset numbers need a
Quest 3 and are never inferred from this machine.

## Existing capabilities Track 2 builds on
- Kernel (`ve-kernel`): cube, sphere, cylinder (Y axis), union, difference, intersect, translate,
  rotate, scale, volume, area, bounds, vertex and triangle counts, `from_mesh`, `to_mesh`, meshopt
  `simplify`; limits `MAX_EXTENT` 1e6 and a 2^28 element cap; typed errors.
- Vendored but **not exposed**: `CrossSection` (offset, booleans, hull), `Extrude` (twist, top scale),
  `Revolve`, `Warp`, `LevelSet`, `Smooth`, `Refine`, `MinkowskiSum`, `MinkowskiDifference`, `Hull`,
  `TrimByPlane`, `CalculateNormals`, `CalculateCurvature`, `ExecutionContext` (cancellation).
  `Compose` is deprecated upstream; use `BatchBoolean` with Add.
- Runtime: instanced forward renderer, shadows, quality tiers, stereo, WebXR, interaction, desktop
  viewer with lenses. `Interactable` objects carry a `Label` that selection can report.
- Registry (`ve-core`): ordered `Registry<T>` keyed by `Name`; no operator spec yet.
- Language spike (`spikes/geom-lang/lang.mjs`, 215 lines JS): about 50 node kinds, line-per-node text,
  validator with typo suggestions; 13 run files under `runs/` (the 53 programs). It is a spike:
  classify, never import.
- Root app (`src/`, React 19, three.js, R3F, zod): 574-line `App.tsx`, `Inspector`, a three.js
  viewport, `shared/commands.ts` (create, update, delete of box, cylinder, plane; zod schemas) and
  `shared/history.ts`. Replaced by Rust-generated commands after V0.

## Known gaps that affect Track 2 (from the audit, still open)
R09 culling and extraction cost; stable input-source IDs; instanced-mesh benchmark workload; B02
worker and cancellation (G2); B05 release-size measurement (this file starts it); B07 certificate
renewal; mirrored instances are not drawn (documented in `ve-render::extract`); `ve-kernel::Mesh`
holds positions and triangle indices only.

## Open items for G0 (all in this directory unless noted)
Done: this baseline, `commands.md`, `diagnostics.md`, `limits.md`, `fixtures.md`, `cases.md`,
ADRs 0004-0007. **Draft only:** case lists for G5 onward are short and must be expanded before each
gate starts (rule: a gate whose cases are not listed is not ready). **Not done:** X1 and X6, which
G0 names as pre-G2 and pre-V0 experiments, and the decision that follows each.

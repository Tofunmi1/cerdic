# V0 editor shell evidence (2026-10-12)

Status: V0 implemented; `cargo xtask check --strict` passed, including the editor browser tests.
Evidence log: `target/v0-check.log`; separate advisories log: `target/v0-advisories.log`.

## Contract and comparison

ADR 0009 chooses Rust-derived TypeScript (`ts-rs`, native optional feature). The build checks the
fresh output against the tracked module without rewriting it. A temporary serde rename of
`Command::SetLabel.label` to `display_label` made `bun run schema:check` fail with schema drift;
the mutation was reverted. `bun run typecheck` calls this check before TypeScript compilation.
The generator reports upstream warnings for serde conversion/unknown-field attributes; string wire
overrides explicitly cover conversions, and Rust still enforces unknown-field rejection.

The comparison scene uses the legacy starter's four authored objects, dimensions, transforms and
colors: floor, pedestal, lavender box and coral cylinder. Both views use a 40-degree camera at
(6, 4.8, 8), looking at (0, 0.75, 0). The comparison rubric is composition, silhouette, relative
placement/scale, distinguishable materials, depth ordering and visible nonblank geometry. It does
not require identical tone mapping, lighting, decorative grid, background or UI between renderers.

Both screenshots were opened and reviewed: the block sits left of the taller column over the round
pedestal; colors and relative proportions are recognizable in both; the engine panel renders cleanly.
The engine has a blue clear background and finite floor, while the legacy app has a pale background,
grid and extra ground. The old app remains reachable at `?legacy=1` for comparison; it has not been
deleted and its more extensive editor features are not claimed as implemented by V0.
Screenshots: `target/reports/v0-editor.png`, `target/reports/v0-legacy.png` (generated artifacts).

## Targeted checks

`bun web/tests/editor.mjs` passes against the real engine: one boot/canvas under StrictMode; a UI
command reflected in the Rust document view; resizing; DPR 2; text focus; separate named documents
in two tabs; save/reload; owned input/poll/worker counts zero on pagehide; fresh boot after reload;
nonblank gallery pixels and legacy comparison screenshots. No editor console errors were observed.
V0 changes the document view; evaluating that document into viewport geometry remains G2 work.

Actual pointer-lock permission in a headed browser, headset entry, controller selection and headset
frame time remain untested here. Headless Chromium does not establish those hardware behaviors.

## Release measurements

`bun run build` passed. `bun scripts/measure-engine.ts dist/engine/ve_runtime_bg.wasm` uses Brotli q9
to match the prior baseline: runtime 33,758,336 B raw / 4,010,623 B Brotli; stripped in memory
18,972,220 B / **3,370,251 B Brotli**, +12,114 B against the G1b baseline of 3,358,137 B. Shipping
assets currently retain custom sections; the stripped measurement is not a claim that the copied
artifact is stripped. No wasm-opt. Main editor JS is 229.47 kB raw / 72.09 kB gzip. Legacy comparison
chunks are lazy but still included in the build, including larger XR emulation dependencies.
Build warnings: existing Zod annotation warnings and large legacy chunks; no build errors.
`cargo xtask advisories` and schema-feature clippy (`-D warnings`) passed.

## Commands

- `bun run dev`: builds/copies the engine dev assets, runs the editor and existing local server.
- `bun run build`: builds/copies release engine assets, checks schemas/TS, builds Vite.
- `bun run schema:generate`: explicitly regenerates checked-in Rust-derived TS.
- From `engine/`: `cargo xtask check --strict`; editor acceptance is included in the browser gate.
- Bun owns `engine/web/bun.lock`; the npm lockfile and npm test invocation were removed.

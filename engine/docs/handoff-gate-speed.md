# Handoff: making the full gate cheaper without making it weaker

Date: 2026-10-14. Read first: `handoff-common.md` section 3 (the two gate speeds), `xtask/src/main.rs`, `web.rs`,
`web_pool.rs`, `quick.rs`. The owner thinks the checks waste time. They also asked what removing them would cost, and
the answer was: it would hide regressions that the gate caught this month (rules and lint failures, a dense-tube
regression, native versus WASI differences, a worker crash). So the goal is the same coverage in less wall time, and no
test or view removed without the owner's say-so.

## 1. What was already done (do not redo)

- `cargo xtask quick [crate...]`: fmt, clippy and tests for changed crates and their dependents, plus the source rules.
- Every stage prints `-- <stage>: done in Ns`; every browser script prints its own time.
- Browser scripts run in parallel (`web_pool.rs`): up to four at once (`VE_WEB_JOBS` overrides), the gallery in three shards
  (`GALLERY_SHARD=i/n`, `GALLERY_PORT`), frame-pacing scripts (`quality`, `perf`, `bench-session`) alone at the end. The script
  list is `web/package.json`'s `test` chain plus an explicit list in `web.rs`.

## 2. Measurements (2026-10-14, 8 cores, machine shared with another agent, load average 8 to 10)

| Stage | Time |
|---|---|
| bridge-schema, fmt, clippy, wasm-clippy, deny | 1 to 10 s each |
| tests (nextest, includes compiling) | 97 to 108 s (about 30 s of test running) |
| cpp (format, ASan/UBSan, tidy) | 10 to 14 s |
| parity (native versus WASI) | 12 to 20 s |
| **web-test** | **849 s** (parallel, my run) and **1,132 s** (the agent's strict run); serial before: about 1,200 s |
| Full gate | about 16 to 22 minutes; web-test is more than 85 percent of it |

Slowest browser scripts (parallel run, so inflated by contention): `evaluation` 282 s, `gallery` shards 246 to 309 s, `panels`
225 s, `mcp` 143 s, `v3-scope` 125 s, `instances` 113 s, `v3-gizmo` 109 s, `panels-groups` 108 s. Alone, `gallery` took 428 s serial,
`evaluation` 94 s, `panels` 121 s. Contention (SwiftShader is CPU-bound) costs a lot; more workers is not free.

## 2a. Implemented in this handoff

- Full `check` stages now use `target/gate-stamps/<stage>`. Keys hash working-tree and untracked input files with Git,
  plus Rust/Bun/Node/C++/WASI tool versions and relevant flags. Root TypeScript and package inputs are included in the
  browser key.
- A fresh run clears that stage's old stamp first and writes a new one only after a successful result. Filtered browser
  runs never stamp `web-test`; `--no-cache` and `VE_NO_CACHE=1` force each available stage.
- `--strict` may use an unchanged successful stamp. Every hit prints its timestamp and the final report lists cached
  stages. A strict no-cache gate is the release-evidence command.
- `cargo xtask web-test dev <name...>` retains matching browser scripts after regular build/typecheck setup. Browser
  failures print script-specific rerun commands. Gallery view count and capture size remain unchanged.
- The full gate orders schema, format, rules and lint before the long tests, C++/parity, and browser suite. No gate or
  evidence view was removed.
- Fresh strict no-cache run on this workspace: 13m22s total, with 682s in `web-test`, 78s in workspace tests, 15s in
  C++, 11s in parity, 7s in wasm clippy, and under 5s in each remaining stage. The previous 16–22 minute baseline was
  measured on a shared machine, so this is not a controlled before/after comparison; the browser stage still dominates.
- Fresh full strict no-cache validation is recorded in `target/reports/g7-attributes-final-gate.log`; its final line is
  `exit=0`. Stage times total about 12m33s, including 620.8s in `web-test`, 86s in workspace tests, 15s in C++, and 14.8s
  in parity. The complete browser suite included the UV subdivided-grid gallery fixture and all three pacing scripts.
- A second `cargo xtask check --strict` passed with all ten stages reported `cached pass` and `Cached stages: ...`.
  Its exact output is in `target/reports/g7-attributes-cached-gate.log`; it ends with `exit=0`. The strict cache policy
  is therefore implemented as recommended: an unchanged successful stamp is accepted and named in the report, while
  `--strict --no-cache` remains the command for fresh release evidence.
- `cargo xtask web-test dev v3-transform-commands` passed after normal web build/schema/typecheck setup and reported one
  selected script. Filtered runs do not write the full `web-test` stamp.
- Stamp unit coverage verifies that a browser-scoped key ignores docs changes and changes when a page file changes;
  the fresh and cached strict logs verify the corresponding full-stage and cache-hit paths.

## 3. Work packages

### G1. Skip a stage when its inputs have not changed (largest saving, no coverage lost)
A stage that already passed on exactly the same inputs does not need to run again. Implement a stamp per stage:
- Key = a hash of (the stage's input files, tool versions). Compute it with git only, no new crates: list files with
  `git ls-files -z --cached --others --exclude-standard -- <paths>`, pipe the sorted list of `path` plus content blob (through
  `git hash-object --stdin-paths`) and the tool versions (`rustc -V`, `bun --version`, the wasm-bindgen version from `Cargo.lock`)
  into `git hash-object --stdin` to get one hex key.
- Store the key in `target/gate-stamps/<stage>` only after the stage passes. A failed or skipped stage writes nothing.
- Inputs per stage (start here, widen if a miss is found):
  - `cpp`: `cpp/`, `third_party/` (Manifold, OpenSubdiv, Clipper2, meshoptimizer), `crates/ve-kernel-sys/`.
  - `parity`: `crates/ve-kernel*`, `ve-geometry`, `ve-modeling`, `ve-graph`, `cpp/`, `tools/run-wasi.mjs`, `xtask/`.
  - `web-test`: everything that reaches the browser: `web/`, `src/`, `scripts/`, `package.json`, `bun.lock`, `crates/` (all), `cpp/`,
    `third_party/`, `Cargo.lock`. This is nearly the whole repo, so a Rust change will usually rerun it; the win is when only
    docs, handoffs, `tools/` or `xtask/` changed, or when the gate is re-run after a failure in an earlier stage (very common).
  - `deny`: `Cargo.lock`, `deny.toml`, every `Cargo.toml`.
- Printing: `-- web-test: cached pass (inputs unchanged since <time>)`, and `--no-cache` or `VE_NO_CACHE=1` forces everything.
- Policy decision for the owner (recommendation: allow caching): should `check --strict` use stamps? Recommendation: yes, but
  only stamps written by a successful run in this tree, always printed, and a final line that lists which stages were cached so
  nothing is hidden. If the owner wants release evidence, `check --strict --no-cache` runs everything.
- Tests: unit tests for the key (stable for the same tree, changes when a file changes, an untracked file counts, a stage that
  failed leaves no stamp), and one manual check that touching a docs file leaves `web-test` cached while touching
  `web/index.html` reruns it.

### G2. Filter and re-run only what failed
- `cargo xtask web-test dev [name...]`: run only the browser scripts whose file name contains one of the names (for
  `evaluation`, `gallery`, `v3-gizmo`), skipping the others and the pacing run unless named. Keep the build step.
- `cargo xtask check` prints, at the end of a failure, the exact command to re-run only that stage and, for browser scripts, the
  one-line `cargo xtask web-test dev <script>`. After a fix, re-running a single script is minutes cheaper than the whole gate.
- Stage order: put the cheap, most-likely-to-fail stages first (fmt, clippy, the rules) so a failure shows in under a minute; run
  `web-test` last (already so). Consider running `cpp` and `parity` concurrently with the browser build if they do not contend
  for the same cargo target lock (measure before keeping it; the wasm build and `parity` both use cargo).

### G3. Make the browser stage itself cheaper (measure each change, report before and after)
Do these in order and stop when the stage is fast enough; each is a measured experiment, not a given.
1. Find the long poles. `evaluation.mjs` is 94 s alone but 282 s under load; read what it waits for and whether fixed timeouts or
   repeated page loads can be shared. Same for `panels.mjs`, `mcp.mjs`, `v3-scope.mjs`.
2. Reuse the built page and server: scripts that each start a server and Chromium could share one Chromium per worker (an
   `xtask` pool worker that keeps one browser and runs several scripts) if the scripts are written for it; this is the biggest
   change and needs per-script isolation (separate docnames already exist).
3. The gallery test renders six views of each of about 28 pieces at 1200 by 800 on a CPU renderer. Rendering at a smaller size
   for the visibility check, or fewer frames per view, would cut time; **this reduces the evidence images, so it needs the owner's
   decision and must not be done by default**. Offer it as an option (`GALLERY_QUICK=1`) only.
4. Worker count: measure 2, 3 and 4 workers on the same tree and quiet machine and choose by wall time, not by core count. If the
   machine is shared, the right default may be 3.

### G4. Report
- At the end of every `check`, print a table of stage times and which were cached (so the owner and later agents see where time goes).
- Update `handoff-common.md` (how stamps work, `--no-cache`, `web-test <name>`), `AGENTS.md` (one line), and the `xtask` usage string.
- Record the new timings in a short section in this file, with the machine load noted, before and after.

## 4. Traps

- Never cache a stage that was skipped or failed. Never let a stamp survive a change in a tool version.
- Do not let a stamp say `web-test` passed when it only ran a filtered subset; filtered runs must not write the stamp.
- The Rust rules: `.rs` files under 400 lines (the `xtask` files are small, keep them small: put the hashing in `xtask/src/stamp.rs`),
  functions under 80 lines, no `#[allow]`, pedantic clippy, no `unwrap`/`expect`/`println!` in library code (`xtask` already
  opts into `print_stdout`/`print_stderr` with `#[expect]`).
- The other agent may be editing the tree during a run; stamps keyed by content will correctly miss then. Do not read that as a bug.
- Do not remove or weaken a gate stage, a browser script, a test or a view without the owner's explicit approval, and do not
  describe a cached pass as a fresh run in a report: say "cached" where it was.
- Edit tools only; formatters are fine; no commits and no attribution lines.

## 5. Done means

- A second `cargo xtask check` on an unchanged tree finishes in about the time of the cheap stages, with every stage reporting
  `cached pass`; a docs-only change leaves the browser stage cached; changing `web/index.html` reruns it; `--no-cache` runs all.
- `cargo xtask web-test dev <name>` runs only the named scripts and writes no stamp.
- The failure message names the one-line command that re-runs the failing stage.
- Before and after timings are recorded here; `cargo xtask check --strict --no-cache` still passes with the real `exit=` line.
- No test, view or stage was removed; any option that reduces evidence is opt-in and was approved by the owner.

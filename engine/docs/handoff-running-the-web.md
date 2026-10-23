# Handoff: running the web pages and servers

Date: 2026-10-15. For anyone (person or agent) who needs to see the engine in a browser, drive it from a model, or put it on a headset. Everything here is
local. Nothing is deployed or published; do not deploy or expose a server beyond your machine or LAN without the owner's approval. Commands run from the
repository root unless noted; the engine lives in `engine/`.

## 1. Four ways to run it

| What | Command | Where it opens | Use it for |
|---|---|---|---|
| **A. Engine viewer page** (static, no Node server logic) | build `cargo xtask web dev`, serve `node engine/tools/serve-lan.mjs --http --host 127.0.0.1 --port 8123` | `http://127.0.0.1:8123/?viewer=1&eval=1&scene=showcase` | looking at models, the Document panel, benchmark scenes, MCP, tests |
| **B. React editor shell** (older viewport, full editor panels) | `bun run dev` | `http://127.0.0.1:5173/` | the editor shell; starts the hosted-provider server on 3001 and Vite on 5173 |
| **C. Headset (Quest)** | same server as A, over HTTPS or USB (section 5) | the Quest browser | WebXR sessions and the M9 benchmark pages |
| **D. Model-driven (MCP)** | page A with `mcp=1` plus `bun engine/tools/mcp/server.mjs` | `http://127.0.0.1:8123/?viewer=1&mcp=1` | letting Claude Code or another MCP client build and edit |

A is the one you almost always want.

## 2. Prerequisites

- Rust through `rust-toolchain.toml` (rustup picks it up), CMake, Ninja, and a WASI SDK: set `WASI_SDK`, default
  `~/.local/share/toolchains/wasi-sdk-34.0-arm64-macos`. `cargo-nextest` and `cargo-deny` are used when installed.
- `bun` (the repository's package manager) and `node` (the LAN server and the WASI runner are Node scripts).
- The wasm-bindgen CLI is installed automatically into `engine/target/tools` at the version in `Cargo.lock`.
- For the browser test suites: `bun install` runs automatically in `engine/web` when `playwright` is missing.
- The first build compiles the C++ kernel (Manifold, OpenSubdiv, Clipper2) and Bevy and takes several minutes; later builds take tens of seconds.

## 3. Run page A step by step

1. **Build** the engine into `engine/web/dist` (git-ignored):
   - `cargo xtask web dev` for the fast profile. Use this while developing (about 20 s when little has changed).
   - `cargo xtask web` for the release profile (smaller and faster at runtime, much slower to build). Use it for size and timing measurements.
   The build also bundles the evaluation worker, `viewer-bridge.js` and copies the page files and the `panels/` folder.
2. **Serve** it: `node engine/tools/serve-lan.mjs --http --host 127.0.0.1 --port 8123`. Without `--http` it serves HTTPS with a self-signed certificate;
   without `--host` it binds to all interfaces (default `0.0.0.0`) and default port is 8443, so for local work always pass `--host 127.0.0.1`.
   It serves only `web/dist` and a write-only `/report` endpoint (benchmark reports go to `engine/target/reports/`). Responses use `cache-control: no-store`,
   so a normal reload picks up a rebuild.
3. **Open** `http://127.0.0.1:8123/?viewer=1&eval=1&scene=showcase` (`open <url>` on macOS).
4. **Stop** it with Ctrl-C in its terminal, or `pkill -f serve-lan.mjs`. If a port is busy: `lsof -iTCP:8123 -sTCP:LISTEN -n -P`.

Rebuild after any Rust, C++, TypeScript-bridge or page change; the server does not rebuild anything.

## 4. URL parameters on page A

| Parameter | Meaning |
|---|---|
| `viewer=1` | the desktop viewer: mouse look, WASD, orbit, field of view (default 45 degrees), split-screen headset, lenses, quality, the Document panel and editing panels |
| `eval=1` | evaluate documents into geometry (worker, staged edits, Preview then Final refinement); implied by `mcp` |
| `mcp=1` or `mcp=<port>` | also connect the MCP bridge (default WebSocket port 8765) |
| `doc=1` | the Document panel without the full viewer |
| `docname=<name>` | which saved document to open (local browser storage); use a unique name for a clean slate |
| `scene=<name>` | `stage` (empty lit room, default with `eval`), `showcase` (colourful props, studio lights), `gallery`, `xr`, `shadows`, `kernel`, `materials`, `lights`, `stereo`, `instance-probe`, `bench1` to `bench5` (50k, 200k, 400k, 1M, 2M triangles) |
| `quality=low\|medium\|high` | render tier (MSAA, shadow size, light count); `governor=1` lets the frame-time governor change it; `hz=<n>` the refresh target |
| `bench=1` with `seconds=<n>` | benchmark session; posts a report to the server when it ends |
| `xr=auto` | enter XR immediately (emulated runtimes) |

## 5. Headset (Quest)

WebXR needs a secure context.
- **USB (simplest):** run the server with `--http`, then `adb reverse tcp:8443 tcp:8443` (use the port you chose), and open `http://localhost:8443/` in the
  Quest browser. `localhost` counts as secure, so there is no certificate warning.
- **Wi-Fi:** run `node engine/tools/serve-lan.mjs` (HTTPS, all interfaces, port 8443). It prints one URL per LAN address; the Quest warns about the
  self-signed certificate, so proceed past it. Use only on a trusted network, never on a public one.
- Useful pages are printed at start: `?scene=xr&xr=auto` and `?scene=bench1..bench5&bench=1&seconds=60`. Press **Enter VR** on the page.
- The headset run (M9) is still pending; every frame-time number so far is software rendering and says nothing about the Quest.

## 6. Driving it from a model (MCP) and from the console

- Register the server once: `claude mcp add spatial-engine -- bun engine/tools/mcp/server.mjs`, then open `http://127.0.0.1:8123/?viewer=1&mcp=1` and ask
  Claude Code to build something with the spatial-engine tools. One page connects at a time; the server listens on `127.0.0.1:8765` (`VE_MCP_PORT` changes it).
- Tools: `get_guide`, `list_operators`, `get_text`, `get_nodes`, `get_status`, `apply_text`, `apply_commands`, `undo`, `redo`, `frame_view`, `set_view`.
  A batch is atomic, up to 10,000 commands and 4 MiB; `apply_text` also caps source text at 256 KiB.
- One-shot helper that serves, opens and runs a model: `bun engine/tools/mcp/try-with-claude.mjs "make a hollow vase, then widen its opening"` (add
  `--headed` to watch). It uses the `claude` CLI; a hosted router only accepts that route (generic API clients are refused).
- **Loading a text program by hand:** the Document panel's Text box is read-only. With `eval=1`, open the browser console and run
  `window.__ve.doc.sendText(`<program text>`, "label")` (a template literal handles the newlines). Programs bigger than the batch limits must be split.
  `window.__ve.read()` returns status (frames, render counts, errors); `window.__ve.evaluation.state` shows worker state and the last failure. A suggested
  improvement (not built): a `?program=<file>` loader or a small "load gallery model" menu.
- Gallery programs live in `engine/crates/ve-modeling/tests/gallery/*.txt` and `.../gallery/visual/*.txt`.

## 7. Page B (React editor shell)

`bun run dev` runs `bun run engine:build:dev` (engine build into `src/wasm`) then `scripts/dev.ts`, which starts the provider server (`server/index.ts`, port
3001, watched) and Vite on `127.0.0.1:5173` with `/api` proxied to 3001; `strictPort` is on, so a busy 5173 fails instead of moving. For a quick wasm refresh
without a full rebuild: `bun scripts/build-engine.ts --copy-only`. `bun start` runs the provider server alone. Typecheck with `bun run typecheck`. The editor
shell uses the older three.js viewport; the engine's own viewport is page A.

## 8. Tests and how they relate to running pages

- `cargo xtask web-test dev [name...]` builds `dist` and runs the Playwright scripts (all of them, or only those whose file name contains a name). Each script
  starts its own server on an ephemeral port and its own headless Chromium, so it does not conflict with your page on 8123, but the build replaces `dist`,
  so a page you have open will show the new build on reload. The gallery test starts a Vite server on 5189 (shards use 5190 and 5191).
- Headless Chromium renders with SwiftShader (software); launch flags are `--enable-unsafe-swiftshader --ignore-gpu-blocklist`. Expect slow frames; do not read
  performance from them.
- `cargo xtask quick` while working; `cargo xtask check --strict` once at the end (it caches stages by content; `--no-cache` forces a fresh run).

## 9. Troubleshooting

| Symptom | Likely cause and fix |
|---|---|
| Blank page or 404 | `engine/web/dist` is missing or stale: run `cargo xtask web dev`; check the server was started from the repo root and prints the `web/dist` line |
| `window.__ve.error` is set or the status panel shows a failure | read `window.__ve.error` and the browser console; a missing wasm usually means the build failed or an old `dist` is cached by another server |
| Model never appears | evaluation refused it: check `window.__ve.evaluation.state.lastFailure` (budget codes such as `budget.triangles` or `budget.commands`, a `constraint.*` refusal, a parse diagnostic) |
| Slow or low frame rate | software rendering (SwiftShader) or the dev profile; use a real GPU browser and `cargo xtask web` for measurements |
| "No page connected" from MCP | open the page with `mcp=1`; only one page connects; check port 8765 is free |
| `address already in use` | `lsof -iTCP:<port> -sTCP:LISTEN -n -P`, then stop that process or choose another port |
| WebXR button missing | not a secure context (use `localhost` over USB or HTTPS), or the browser has no WebXR; see section 5 |
| Wrong colours or no shadows after a change | rebuild; also check the `quality` tier in the Viewer panel |
| Build fails in C++ | `WASI_SDK` not set or not found; `cmake` or `ninja` missing |


## 10. Rules

Edit files only with the edit tools (formatters and `bun run schema:generate` are fine). No commits and no attribution lines unless the owner asks. Never put a
provider key in the repository, a log or a commit; pass it through an environment variable for one command and rotate keys that were pasted into chat.
The LAN server is for private testing: it has no authentication, so keep it on `127.0.0.1` or a trusted network and never expose it publicly.

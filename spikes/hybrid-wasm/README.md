# Spike: Rust + C++ in one WebAssembly module

Question: can the Rust engine call C++ libraries directly, sharing one linear memory, and run in a browser?

**Answer: yes** (tested 2026-10-09). One `wasm32-wasip1` module contains:

| Part | Language | What the spike does |
|---|---|---|
| Manifold 3.5.4 (+ Clipper2) | C++ | cube − sphere boolean, then reads mesh back |
| `cpp/geomproc.cpp` (ours) | C++ | vertex weld using libc++ `unordered_map` |
| meshoptimizer 1.3 | C++ | simplify 1,400 → 350 triangles (error 0.008) |
| Rapier 0.36 | Rust | CSG mesh as static trimesh; ball lands on it at y = 0.60 (top 0.5 + radius 0.1) |
| boolmesh 0.1.10 | Rust | same boolean as Manifold, for comparison |

Results matched in Bun 1.3.10 and headless Chromium 156 (cross-origin isolated):

- Size: 2.08 MB raw → 1.63 MB after `wasm-opt -Oz` → **522 KB brotli** (671 KB gzip).
- Instantiate < 1 ms after compile. First run 36–51 ms, second run 11–23 ms. Calling the module repeatedly works.
- WASI imports are declared (`fd_write`, `proc_exit`, …) but **none are called**. `web/wasi-shim.mjs` is about 50 lines.

## Benchmark (Apple M1, Chromium 156, mean of repeated runs, ms)

`bun web/bench-browser.mjs` / `bun web/bench-bun.mjs`. Cube − sphere; "64" means a 2,048-triangle sphere (result 1,400 tris), "128" means an 8,192-triangle sphere (result 3,848 tris). Both CSG libraries produce the same triangle counts.

| Stage | 64 | 128 |
|---|---|---|
| C++ Manifold boolean | 3.1–3.7 | 9.3–9.8 |
| Rust boolmesh boolean (same inputs) | 3.7–4.2 | 10.7–11.8 |
| C++ geomproc weld (ours) | 0.04 | 0.13 |
| C++ meshoptimizer simplify to 25% | 0.34 | 0.9 |
| Rust Rapier: build trimesh + 240 steps (4 s simulated) | 2.0–3.0 | 2.5–3.0 |
| Pipeline C++ CSG → weld → simplify → Rust physics | 5.3–5.9 | 13.1–14.2 |
| Pipeline Rust CSG → Rust physics (no weld/simplify) | 5.6–6.0 | 13.9–15.2 |

- Compile 11 ms (Chromium) / 19 ms (Bun); instantiate < 2 ms. The first call of each stage is slower (cold code and memory growth).
- A combined pipeline costs about the sum of its parts: calls between Rust and C++ in the same module add no measurable overhead.
- Headset CPUs are slower than an M1. Measure on the device before setting budgets.
- Module with boolmesh also linked: 1.82 MB after `wasm-opt`, 578 KB brotli.

## Things that were required

1. **Reactor model.** The module must export `_initialize` (calls `__wasm_call_ctors`). Without it, wasm-ld wraps every export with constructor *and destructor* calls, so C++ globals are torn down after the first call and the next call traps (`call_indirect to a signature that does not match`). The host calls `_initialize()` once after instantiation.
2. **Link libc++ from wasi-sdk's `noeh/` sysroot directory** and build all C++ with `-fno-exceptions`. An `eh/` variant exists for C++ exceptions. That's untested.
3. **Upstream header fixes:** Manifold/Clipper2 rely on transitive includes that libc++ doesn't provide. Build with `-include iterator -include cstdlib` and `MANIFOLD_NO_IOSTREAM=ON`.
4. **No `wasm-bindgen`.** It targets `wasm32-unknown-unknown`. The boundary is plain C ABI exports + JSON/buffers read from linear memory.

## Not covered yet

Recast/Detour (expected to work the same way as Manifold); the `cxx` crate (the spike uses raw `extern "C"`); C++ exceptions; threads; the headset browser; Rust panic → JS error reporting.

## Reproduce

Requires wasi-sdk 34 (`~/.local/share/toolchains/wasi-sdk-34.0-arm64-macos`), `rustup target add wasm32-wasip1`, CMake, Ninja, Bun.

```sh
git clone --depth 1 --branch v3.5.4 https://github.com/elalish/manifold third_party/manifold
git clone --depth 1 --branch v1.3 https://github.com/zeux/meshoptimizer third_party/meshoptimizer
./build.sh              # Manifold via CMake, then cargo build
bun web/run-bun.mjs     # Bun
bun web/browser-check.mjs  # headless Chromium (uses the app's Playwright install)
```

# Vendored Blender (selected files)
- Upstream: https://projects.blender.org/blender/blender, read through the GitHub mirror https://github.com/blender/blender
- Commit: 8ffe6881003c24ef66ef979cf9de6beffe7aae2f (2026-10-16, `Merge branch 'blender-v5.3-release'`)
- Licence: GPL-2.0-or-later (each file's `SPDX-License-Identifier`; full text in `COPYING` and `doc/license/GPL-license.txt`; the Apache-2.0
  text is `doc/license/Apache2-license.txt`). Owner decision: the GPL exclusion was removed, see `docs/adr/0024-gpl-exclusion-removed.md`.
  A build that includes these files is a GPL work: whoever receives it can ask for the complete source. Nothing is published without owner approval.
- What is here: only the files our builds need, 641 of them (about 16 MB), chosen as the include closure of the sources listed under "Built from".
  They were fetched with `git clone --filter=blob:none`, `git fetch --depth 1 <commit>`, `git sparse-checkout set --no-cone --stdin` and `git checkout`,
  then the nested `.git` was removed. Nothing was copied by hand.
- Built from (compiled for native and for wasm32-wasip1):
  - `source/blender/bmesh` (data structure, 45 operators incl. `tools/bmesh_bevel.cc`)
  - `source/blender/blenlib/intern` (utility library; OS, file I/O, JSON, zstd and fmt users are left out)
  - `source/blender/blenkernel/intern`: `customdata.cc`, `curveprofile.cc`, `deform.cc`, `attribute.cc`, `mesh_normals.cc`, `mesh_mapping.cc` and their headers
  - `source/blender/geometry/intern/uv_parametrizer.cc` (LSCM and ABF++ UV unwrapping; SLIM is not built)
  - `intern/guardedalloc`, `intern/eigen` (the C wrapper over Eigen), `intern/atomic`, `intern/iksolver` (inverse kinematics), `extern/wcwidth`
- Eigen is vendored separately in `../eigen` (MPL-2.0). MikkTSpace is in `../mikktspace` (Apache-2.0).

## Local patches (all marked `VE patch` in the files)
1. `source/blender/blenlib/BLI_resource_scope.hh`: `static_assert(sizeof(ResourceScope) == 16)` became `2 * sizeof(void *)` (upstream assumes 64-bit pointers; wasm32 has 4-byte ones).
2. `intern/iksolver/intern/IK_QJacobianSolver.cpp`: removed a `try`/`catch (...)` around `Invert()` (dead code when exceptions are off).

## How the engine builds it (`cpp/blender/`)
- `cpp/blender/CMakeLists.txt` and `sources.cmake` (an explicit list of the compiled files) build four static libraries: `ve_blender_core`, `ve_blender_bmesh`, `ve_blender_geometry`, `ve_blender_ops`.
  `ve_blender_ops` is our code over Blender's data structures (`ops/`: the polygon-mesh bridge, bevel, UV unwrap) behind the C ABI in `cpp/include/ve_kernel.h`.
- Flags: C++20, RTTI on, `-fno-exceptions -funsigned-char -ffp-contract=off -DEIGEN_DONT_VECTORIZE`, with `try`/`catch`/`throw` turned into plain control flow, and Blender's own warnings off.
- Deterministic maths: `sin cos tan asin acos atan atan2` (double and float, plus their `__builtin_` spellings) are renamed to `ve_det_*` in Blender's objects only (`shim/det_libm.cc`, built from
  `+ - * /` and `sqrt`), so native and wasm agree bit for bit while the rest of the engine keeps the platform libm.
- Shims (`cpp/blender/shim/`): a no-op profiler header, `blender::G`, silent `CLG_*` logging, and `memory_usage.cc` (replaces `guardedalloc/intern/memory_usage.cc`, whose 128-byte-aligned
  thread-local breaks sanitizer builds). `fmt` is vendored header-only in `../fmt`.
- Unresolved functions: `cpp/blender/unresolved.txt` names the Blender functions the engine deliberately does not link (file I/O, undo, editors). Native links generated trap stubs for them; for
  WebAssembly the linker makes a typed trap for each (`--unresolved-symbols=ignore-all` in `.cargo/config.toml`). A listed function that is reached stops the program instead of jumping to nothing.
- The browser shim `web/wasi_snapshot_preview1.js` answers the wasi imports the Blender objects add (`fd_prestat_get`, `fd_prestat_dir_name`, `proc_exit`).

## Evidence in the engine (2026-10-16)
- `cpp/tests/bevel_test.cpp` and `uv_test.cpp` (C ABI, ASan + UBSan) and `crates/ve-kernel/tests/{bevel,uv}.rs`: the spike's bevel cases reproduce its vertex hashes exactly; a cross-net cube unfolds with
  every face keeping its area; six packed islands stay in the unit square without overlap; bad input is refused with typed codes.
- Native and wasm32-wasip1 give identical output for the kernel corpus (31 lines), which now includes 4 bevel and 4 UV cases (packing, rotation, LSCM and ABF++ on a sphere).
- `poly.bevel_network@1` (graph node over `ve_kernel::bevel_edges`) bevels edge layouts the older `poly.bevel_edges@1` refuses; tests in `crates/ve-geometry/tests/g10_bevel_network.rs` and
  `crates/ve-modeling/tests/g10_bevel_network.rs`.
- `uv_pack.cc` was added to the vendored tree by `git fetch` of the pinned commit (byte-identical).

## Evidence from the feasibility spike (scratch, 2026-10-16)
- Bevel: 8 box cases (all edges at 1/2/3/4 segments, a top loop, two adjacent edges, one edge, an open quad refused) are identical native vs WASI down to a hash of every vertex bit; Euler number 2;
  the one-edge volume (7.96875) equals the closed form. Brotli size of the linked bevel closure: about 310 KB.
- UV unwrap: a cube cut into a cross net unfolds with area ratio exactly 1.0; a UV sphere unwraps with no flipped triangles; all 5 cases identical native vs WASI with the deterministic libm.
- IK solver: converges (error 4.4e-5) and the main values match natively and under WASI, but entries near zero (about 1e-17) differ in the last bits. Open: cause narrowed (not libm, not Eigen's cache-size
  probing, not uninitialised memory; native output also changes with the optimisation level and builtin handling). Needs a bit-exact fix before IK enters the parity corpus.
- Not yet tested: the browser page with the Blender code linked, cancellation inside Blender loops, large meshes, the other bmesh operators, UV node in the graph.

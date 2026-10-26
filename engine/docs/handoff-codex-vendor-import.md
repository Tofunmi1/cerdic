# Handoff: vendor RedGPU and Trompe as reference copies (ADR 0070)

Read `docs/adr/0070-code-taken-from-redgpu-and-trompe.md`. This job has no Rust code; it copies, documents and indexes.

## Do
1. **RedGPU** (MIT). Staging copy (sparse checkout, already fetched, commit `95aaf9a9ecb3c5ccfcfc824ccad1a6489392cf68`): `/private/tmp/claude-501/-Users-mac-Work-victoria-vr-hack/9d098cda-d740-48c5-ac1e-a0a723c575c1/scratchpad/ext/redgpu/`. Its licence file is `LICENCE.md` upstream at the repository root but is NOT in the staging copy: write `engine/third_party/redgpu/LICENCE.md` with the standard MIT text, copyright line `Copyright (c) 2019~ Redcamel` (that is the line upstream uses; verify with `gh api repos/redcamel/RedGPU/contents/LICENCE.md --jq .content | base64 -d` if network allows, otherwise use the line given here and say so in Status). Copy `src/` and `README.md` from staging into `engine/third_party/redgpu/` (same relative paths), without changes.
2. **Trompe** (Apache-2.0). Staging copy (full clone, commit `fd81fe467b1e29659597109f14bcf4e7054afa7f`): `.../scratchpad/ext/trompe/` (same parent folder). Copy `src/core`, `src/api.ts`, `test`, `scripts`, `LICENSE`, `README.md` into `engine/third_party/trompe/` (same relative paths). Do NOT copy `src/providers`, `src/ui`, `src/demo.ts`, `samples`, `video`, `spike`, `docs` or `node_modules`.
3. `engine/third_party/redgpu/VENDOR.md` and `engine/third_party/trompe/VENDOR.md` in the format of `engine/third_party/meshoptimizer/VENDOR.md` (upstream URL, commit, licence, what is kept, "Local changes: none", a line saying it is a read-only reference and nothing builds from it). Add `engine/third_party/NOTICE` (create or extend) with the two upstreams, their licences and copyright lines (Trompe's Apache-2.0 requires the notice; the Patch name, mark and typeface are not licensed: say we do not use them).
4. `engine/third_party/redgpu/INDEX.md` and `engine/third_party/trompe/INDEX.md`: for each folder or important file, one or two lines on what it does (read the code, do not guess) and the port decision from ADR 0070 (ported by item number, referenced only, or not taken, with the reason). For RedGPU cover at least: `src/landscape` (tile streamer, VHT/VNT/VBT bake, GPU culler, scatter, stochastic sampling, grass, foliage, impostors), `src/water` (lake shader, wave simulator, capture pass), `src/display/instancingMesh`, `LODManager.ts`, `src/systemCodeManager` (shader library and how chunks are assembled), `src/postEffect`, `src/light` and `src/shadow`. For Trompe: `planner.ts`, `views.ts`, `clay.ts`, `atlas.ts`, `composite.ts`, `semantics.ts`, `sky.ts`, `export.ts`. For each WGSL file you index, note the WebGPU features it needs (compute, indirect draw, storage textures, atomics) and whether it would run on a WebGL2 fallback (no compute).
5. Check `engine/deny.toml` / `cargo deny` licence config or any `rules` that scan `third_party` (look in `xtask/src/rules.rs`): vendored non-Rust folders must not break them; if one does, report it in Status (do not run `cargo xtask`; read the code).
6. Add the third_party folders to nothing else (no workspace membership, no build scripts).

## Rules
- No commits or staging. No changes to anything outside `engine/third_party/` and this file's Status section. Do not edit the copied files.
- Report the sizes (`du -sh`) and file counts of both copies.

## Status

- Copied the requested RedGPU and Trompe reference subsets at their pinned commits. Added vendor notes, indexes, and the shared NOTICE. No workspace or build files were changed, and no commit or staging action was performed.
- RedGPU: 2.8M, 344 files. Trompe: 228K, 27 files (`du -sh`; regular-file counts).
- RedGPU's staged subset omitted its upstream `LICENCE.md`; per the handoff, wrote the standard MIT text using the provided `Copyright (c) 2019~ Redcamel` line. Network verification was not performed.
- `deny.toml` checks Rust dependency metadata/licenses; `xtask/src/rules.rs` scans Rust source under `crates/` for file size and crate dependency boundaries. Neither scans non-Rust `third_party` files, so these reference copies do not affect those checks. Per instruction, no `cargo xtask` command was run.
- Trompe NOTICE copyright attribution is recorded as Patch-gg and contributors based on the upstream project identity; the README says the Patch name, mark/wordmark, and Patch Pixel typeface are not covered, and the copies do not use them.

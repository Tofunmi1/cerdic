# Vendored MikkTSpace (from Blender)
- Upstream: https://projects.blender.org/blender/blender (read through the GitHub mirror https://github.com/blender/blender), directory `intern/mikktspace`
- Commit: 8ffe6881003c24ef66ef979cf9de6beffe7aae2f (2026-10-16)
- Licence: Apache-2.0 (each file's `SPDX-License-Identifier` says so; the full text is the standard Apache-2.0 text also kept at `../manifold/LICENSE`).
  Copyright: Morten S. Mikkelsen 2011, Blender Authors 2020-2023, Meta Platforms 2012-2021 (the hash set derives from Folly).
- Files: `mikktspace.hh`, `mikk_float3.hh`, `mikk_util.hh`, `mikk_atomic_hash_set.hh`, copied unmodified (checked identical to upstream).
  Upstream's `CMakeLists.txt` and `README.md` are not copied.
- Local changes: none to the files. The headers expect their includer to define `uint`, `LIKELY` and `UNLIKELY` and to include
  `<array>`, `<atomic>`, `<cstdint>` and `<type_traits>` first; our wrapper `cpp/src/tangents.cpp` does that. `WITH_TBB` is never defined, so the code is
  single-threaded and has no dependency beyond the C++ standard library.
- Use: tangent-space generation (the standard behind normal-map baking in game assets) behind the C ABI function `ve_tangents`.
- Determinism notes: the only maths are `+ - * /`, `sqrtf`, `fabsf` and `floorf` (all exactly rounded, so native and WASI agree) plus a polynomial `fast_acosf`.
  The wrapper compiles with `-ffp-contract=off -fno-strict-aliasing` (the hash helpers type-pun floats).
- This is not GPL code; it is listed here because it came from the Blender extraction effort (see docs/adr/0024-gpl-exclusion-removed.md).

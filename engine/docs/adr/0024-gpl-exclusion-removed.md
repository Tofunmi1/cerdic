# 0024: The GPL exclusion is removed
- Status: accepted (owner direction, 2026-10-16)
- Date: 2026-10-16

## Context
ADR 0003 excluded GPL code (Blender and derivatives) from the engine and ADR 0005 kept that as a "standing rule from earlier owner decisions". The owner
has said they never wrote that rule: it began as a default recorded in the original plan ("change on request"), and was later repeated as if it were an
owner decision. The owner now directs that it be removed and that forking or pulling in Blender be evaluated.

## Decision
- GPL-family code may be used in the engine, including Blender source, subject to the consequences below. No GPL code is adopted by this ADR alone: each
  adoption (a Blender module, a library) still needs a feasibility note and a `third_party/<name>/VENDOR.md` entry (upstream, commit, licence file, patches).
- The no-imported-assets rule from ADR 0003 stands unchanged: all geometry and materials come from our own system. Blender's source code is not an asset;
  Blender's bundled models, brushes, textures, HDRIs and sample files are, and stay out.
- `deny.toml` licence allow-list is extended with a specific SPDX identifier (for example `GPL-2.0-or-later`) only when an actual dependency needs it,
  with a comment naming it.

## Consequences
- Blender's source files carry `SPDX-License-Identifier: GPL-2.0-or-later` (checked on `bmesh_bevel.cc`, 2026-10-16). A work that combines it with the engine is distributed under GPL terms: anyone who receives a build can demand the
  complete corresponding source of the combined work. While the project stays private and undistributed this costs nothing; publishing the hackathon
  entry, a hosted demo or a headset build that includes GPL code would trigger the obligation. The owner approves any publication (standing rule), and
  should treat this as part of that approval.
- Reusing GPL code also fixes the engine's own licence to GPL-compatible terms; the engine's current permissive dependencies remain compatible.
- Blender's source is large and tied to its own runtime (guarded allocator, blenlib, Python bindings, GMP for exact booleans). Taking pieces means
  extracting modules and building them for `wasm32` and WASI with our determinism rule (identical native and WASI output); see the feasibility
  note in the chat record for the first candidates.
- ADR 0003's GPL exclusion and ADR 0005's "standing rule kept" bullet are superseded by this ADR. The `AGENTS.md` and `handoff-common.md` statements are updated.

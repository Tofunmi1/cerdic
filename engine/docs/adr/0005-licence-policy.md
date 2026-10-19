# 0005: Licences are not a selection criterion
- Status: accepted (owner, 2026-10-12)
- Date: 2026-10-12

## Context
ADR 0003 says permissive code only, enforced by `cargo deny`, while `deny.toml` already allows
MPL-2.0. OpenSubdiv's file is the Tomorrow Open Source Technology License 1.0, Apache 2.0 with only
section 6 (trademarks) changed. The owner has decided licence terms should not steer library
choices.

## Decision
- Libraries are chosen on build fit under our toolchain, correctness and size. MPL-2.0 and the
  Tomorrow Open Source Technology License 1.0 are allowed.
- `deny.toml` is extended with an SPDX identifier whenever a Rust dependency needs it. C++ sources
  are vendored with a `VENDOR.md` that records upstream, commit and licence file, as now.
- (Superseded by ADR 0024, 2026-10-16: the owner removed the GPL exclusion; it had been recorded here as an owner decision in error.)
- RustSec advisory checks (`cargo xtask advisories`) and the dependency-review step stay; they are
  about security and provenance, not licence terms.

## Consequences
ADR 0003's "permissive only" sentence is superseded; its no-imported-assets and GPL-exclusion
decisions stand. A licence review is no longer a gate on adopting OpenSubdiv, PMP, Fidget or similar.

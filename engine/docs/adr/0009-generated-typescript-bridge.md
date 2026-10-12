# 0009: Generate bridge types from Rust serde contracts

- Status: accepted for V0 implementation, 2026-10-12
- Extends: ADR 0007's requirement for generated bridge types

Use `ts-rs` behind a native `typescript` feature in `ve-graph`. Derive types on the actual
serde command/document/diagnostic definitions. The native exporter produces a deterministic single
TypeScript module; no second command schema is maintained in the editor. String-backed validated
IDs explicitly export as strings. JSON integers export as numbers; the bridge rejects unsafe
integer revisions before submitting commands.

`bun run schema:check`, the TS build and `cargo xtask check` compare fresh generated output with
the checked-in file and fail on drift without modifying it. Regeneration is an explicit command.
The feature does not add a dependency to the browser runtime. Rust validation remains authoritative.
Reference: https://docs.rs/ts-rs/12.0.1/ts_rs/trait.TS.html

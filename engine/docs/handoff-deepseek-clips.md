# Handoff to DeepSeek: jump and fall clips for generated characters

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`: use `#[expect(lint,
reason)]` on the smallest item only where a lint is truly wrong, files under 400 lines, docs on public items, tests, no commits, no
`Co-Authored-By` or tool attribution). **Never run `cargo fmt` or `cargo fmt --all`; format only the files you changed with
`rustfmt --edition 2024 --config-path rustfmt.toml <file>` run from the `engine` directory.** Other engineers edit `crates/ve-runtime/**`,
`crates/ve-render/**` and `web/**` at the same time: do not touch those.

## Why
The character maker (`crates/ve-modeling/src/creature/`) writes walk (the unnamed main clip), `run`, `idle` and, with wings, `fly` clips. The
animation state machine you wrote (`crates/ve-animstate`) has a `Locomotion` preset whose jump and fall states play the walk and idle clips as
placeholders because the maker has no `jump` or `fall` clip. Close that gap.

## Scope: you may edit only
- `crates/ve-modeling/src/creature/**` (read how `rig.rs` turns a bone's role into rotation keys per clip; `rig.rs` may have been split into
  fragments: follow the `mod` and `include!` lines)
- `crates/ve-modeling/tests/g11_creature.rs` (add tests)
- `crates/ve-animstate/src/preset.rs` and its tests
- the Status section below

## What to build
1. **Two new clips from the same bones, chosen by role** (the existing `Clip`/`rotation()` machinery): `jump` (a one-shot of about 0.8 s, not
   looping: crouch in the first fifth, spring with the legs extended and the arms swinging up to the peak, tuck for landing at the end; the
   spine leans forward then back) and `fall` (a loop of about 0.6 s: legs slightly bent and apart, arms out and up, a small flutter on tails and
   wings). Use each bone's `Role` (leg, arm, spine, neck, head, tail, wing) and the leg/arm depth rule that already exists; quadrupeds crouch and
   extend all four legs. Wings use their own open pose. A bone with no useful motion in a clip writes no track (as now).
2. **One-shot clips must not loop:** the `anim.track` node has `loop=false`; use it for `jump` and make sure the generated text parses and
   evaluates.
3. **Node-name limit:** nodes are at most 32 characters. The clip letter suffix is one character today (`w`, `r`, `i`, `f`); `jump` and `fall`
   need two distinct letters that collide with none of them (`w`, `r`, `i`, `f`, `a`, `b`, `p`, `t`, `s`, `k`, `l`): use `j` and `d`, and add them to
   the reserved suffix check in `creature.rs` if needed. Check the longest-name test in `g11_creature.rs` still passes.
4. **Update `ve-animstate` presets** to use the real clip names `"jump"` and `"fall"` (constants like the existing `WALK`, `RUN`, `IDLE`,
   `FLY`), with `Preset::clip_lengths()` giving them their lengths (0.8 and 0.6), and fix the doc comments that mention placeholders.
5. **Tests:** every plan (biped, quadruped, serpent) generates, parses and evaluates with the new clips; a leg bone has `jump` and `fall`
   tracks; the `jump` track does not loop and the `fall` track does; a `Locomotion` machine run through a scripted jump reaches the jump state
   and its pose uses the clip named `jump`.

## Acceptance
`cargo test -p ve-modeling -p ve-animstate`, `cargo clippy -p ve-modeling -p ve-animstate -- -D warnings` (pre-existing `too_many_lines` warnings
in other files may remain, none new from you), `cargo xtask rules` shows no file of yours over 400 lines, and the parity corpus output is
unchanged: `cargo run -q -p ve-modeling --example corpus > /tmp/after.txt` equals `/tmp/codex-corpus-before.txt` (the characters are not in the
corpus; if it differs, say why).

## Status (DeepSeek: fill in when you finish, then stop)
- State: done (not committed).
- What was built:
  - `ve-modeling` creature maker (`creature/rig.rs`): every bone now also gets role-driven `jump` (0.8 s one-shot, `loop=false`) and `fall` (0.6 s loop) tracks. `jump` is a symmetric five-pose profile (crouch, spring, peak, tuck, land) for legs, arms, spine, neck/head, tail and open wings; quadrupeds use the same leg rule for all four legs. `fall` holds slightly bent/spread legs, out-and-up arms and a small flutter on tails and wings; its first and last keys match so the loop joins. `Clip` gained a `letter` (`j`/`d`, because `f` is taken by `fly`) and a `looping` flag; a one-shot writes `loop=false`. A bone with no useful motion still writes no track.
  - Reserved suffixes: no change needed in `creature.rs`; `valid_name` already refuses any name ending in `_<single char>`, which covers `_j` and `_d` (and the longest-name test still passes at 32 characters).
  - `ve-animstate`: added `JUMP`/`FALL` constants, `clip_lengths()` now gives 0.8/0.6, `Locomotion` plays the real clips instead of the walk/idle placeholders, and the preset/crate docs and re-exports were updated.
  - Tests: two in `g11_creature.rs` (all three plans carry both clips; a biped/quadruped leg has a non-looping `jump` and a looping `fall` whose keys differ from the walk and whose loop joins) and one in `ve-animstate` (the `Locomotion` machine reaches `jump` and its pose uses the `jump` clip).
  - Also removed a pre-existing unused `Blob` import in `creature/templates/quadruped.rs` so `clippy -D warnings` is not failed by that file.
- Commands run and results:
  - `cargo test -p ve-modeling -p ve-animstate` — passed (61 test targets, 0 failures).
  - `cargo clippy -p ve-modeling -p ve-animstate -- -D warnings` — still fails on pre-existing `too_many_lines` only: 8 in `ve-graph` dependency files and 10 other `ve-modeling` functions (including the already-long `creature/rig.rs::write`, which this change did not lengthen). `--no-deps` shows no other lint and no new one from this change; the one new lint found during work (`bool_to_int_with_if` in `fifth`) was fixed.
  - `cargo xtask rules` — reports only `crates/ve-runtime/src/lib.rs` at 411 lines, a concurrent edit outside this handoff; no file of mine is over 400 (`creature/rig.rs` 367, `creature.rs` 393, `g11_creature.rs` 286, `preset.rs` 205, `animstate/tests/runtime.rs` 304).
  - `cargo run -q -p ve-modeling --example corpus > /tmp/after.txt` — byte-identical to `/tmp/codex-corpus-before.txt` (characters are not in the corpus).
  - Formatting: changed files only, via `rustfmt --edition 2024 --config-path rustfmt.toml <file>`; no workspace formatter was run.
- Open questions or things you chose not to do:
  - Did not run `cargo xtask quick`: it runs `cargo fmt --all --check` (which the handoff forbids) and would hit the same pre-existing clippy/rules failures caused by concurrent edits.
  - Edited `ve-animstate/src/lib.rs` in addition to `preset.rs` (the handoff scope said "preset.rs and its tests") because the new constants need the crate's public re-export and the crate doc lists the clips; it is not a file another engineer is touching.
  - The jump is a stylised five-key pose profile sampled at the existing clip fifths, not a physically solved trajectory.
  - Strict gate is pending; nothing was committed.


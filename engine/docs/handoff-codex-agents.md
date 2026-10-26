# Handoff: creatures that move on their own, and the walkable level (ADR 0067)

Owner: Claude manages; Codex implements. **Crates you may edit: `ve-game` (src, Cargo.toml), `ve-graph` (`src/builtin/game.rs` and wherever its node specs/tests need a line), `ve-modeling` (`src/game_nodes.rs` and its collect/describe path).** Not `ve-runtime` and not `ve-nav` (Claude wires the runtime; read `ve-nav`'s API, do not change it).
Rules: never run `git checkout`, `git clean`, `git stash` or `git restore`; never `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`; use exactly `CARGO_TARGET_DIR=/tmp/codex-target-agents`; do not commit or stage; edit with your edit tools, not scripts; leave `[lints]` alone; files stay under 400 lines (new modules `src/world/state/agents.rs`, `src/parse/agent.rs`...). Tests are written: never weaken them. The `ve-game` throwing work (gravity, release velocity) just landed and is done: do not change it.

## Why

"Blackout 2" is a hands-first VR game: creatures patrol the rooms, notice you, chase, and you stun them by throwing things. `ve-nav` (walkable grid, A*, line of sight, pure Rust) exists but nothing uses it; `ve-game` has no creatures.

## What to build (the test headers hold the contract)

1. **`ve-game` depends on `ve-nav`** (path dependency, add to `Cargo.toml` and to `ve-game`'s `[dependencies]`). `World::set_nav(&mut self, Option<Arc<ve_nav::NavMesh>>)` and `World::nav()`; the navmesh is **not** part of the snapshot or hash. Without one, agents walk straight.
2. **Text lines** (parse, `GameDef::describe()` round trip, diagnostics with line, column, message naming the key, and a suggestion):
   - `agent KIND patrol=x,z;x,z;... speed=M chase_speed=M sight=M lose=M` (`KIND` must be a declared kind, else a diagnostic naming it; `patrol` needs at least two points; `speed` required, positive; `chase_speed` default `1.6 * speed`; `sight` default 5; `lose` default `1.5 * sight`; every number finite and positive).
   - `nav parts=a,b cell=M radius=M height=M step=M slope=DEG` (`parts` required; defaults cell 0.25, radius 0.3, height 1.8, step 0.4, slope 45; `cell` > 0, `radius` >= 0, `height` > 0, `step` >= 0, `slope` in (0, 90)); `GameDef::nav() -> Option<&NavDef>` with public fields `parts: Vec<String>, cell, radius, height, step, slope: f64` (tests in `tests/nav_parts.rs`).
3. **Agent behaviour**, every fixed step, for each entity of a kind with an `agent` line (deterministic: no hash-map iteration order, no time, no randomness; lowest entity id first):
   - States: `patrol` (walk the loop of points in order, at `speed`, advance when within 0.5 m horizontally), `chase` (go for the hero, the entity with the tag `hero`, at `chase_speed`), `stunned` (not moving).
   - **Notice:** in `patrol`, when the hero is within `sight` metres horizontally and in view (`NavMesh::line_of_sight` between the two points when a navmesh is set, else always in view), switch to `chase` and raise the event `sight` (entity `a` = the creature, `b` = the hero). Back to `patrol` (event `lost`) when the hero is farther than `lose` metres, or has been out of view for 3 seconds; it then walks to the nearest patrol point.
   - **Catch:** while chasing, when the horizontal distance to the hero is below the sum of both kinds' `size`, raise `catch` (`a` = creature, `b` = hero) once per contact: not again until the creature has been stunned or has been further away than that sum plus 0.5 m.
   - **Stun:** the rule action `stun:SELECTOR:SECONDS` (selector syntax as the other actions, `kind:creature`, `tag:x`, `a`, `b`) stuns each selected agent for that many seconds (a bad number is ignored as `setvel` does); it raises `stun` (`a` = the creature) on entering and `recover` when the time ends (back to `chase` if the hero is still in sight, else `patrol`). A stunned creature has zero velocity.
   - **Moving:** the agent sets the entity's velocity each step toward the next point of its path (`NavMesh::find_path` from where it is to its goal, recomputed every 15 steps or when the goal moved more than 0.5 m; follow the path's points, y from `NavMesh::height_at` when available); the world integrates the velocity as it does for `setvel:` entities, so `heading` follows by itself. It must not walk into a `solid` kind's box (the existing solid test the hero is held back by; it stops instead).
   - Agent state (mode, stun time, patrol index, catch latch, out-of-view counter, path cursor) is part of the snapshot and the hash **only when at least one `agent` line exists**; worlds without agents hash exactly as before.
4. **Nodes** (ve-graph specs + ve-modeling emit; the tests are `crates/ve-modeling/tests/game_agent_nodes.rs`):
   - `game.agent@1(kind, patrol=[[x,z],...], speed, chase_speed, sight, lose)` writes the `agent` line; an unknown kind is reported at evaluation naming it (like the other `game.*` nodes do).
   - `game.nav@1(parts=[names], cell, radius, height, step, slope)` writes the `nav` line.
   - `game.ground@1(y)` writes the `ground y=` line (the ground line exists in `ve-game`, from the throwing work).
   - `game.kind@1` gets two optional parameters, `fall` (bool) and `bounce` (0..0.95), written as `fall=1` and `bounce=F` (the kind keys exist in `ve-game`).
   - All new nodes appear in `ready()`, with help text (one working example each) in the node spec; `game.collect@1` accepts them.

## Tests (written by Claude; never weaken)

`crates/ve-game/tests/agents.rs` (11), `crates/ve-game/tests/nav_parts.rs` (5), `crates/ve-modeling/tests/game_agent_nodes.rs` (4). Also run: `cargo test -q -p ve-game` (all), `cargo test -q -p ve-graph`, `cargo test -q -p ve-modeling --test game_agent_nodes --test g14_game_nodes --test game_spawn_heading`, `cargo clippy -q -p ve-game -p ve-nav --all-targets -- -D warnings` (older `validate_action`/`effects` findings exist: do not silence them; do not add new ones).

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [ ] done / [x] blocked: implementation is in place; acceptance is blocked by two supplied movement assertions that conflict with the stated behaviour (numbers below). The supplied tests were not edited. The four pre-existing Clippy findings remain unsuppressed.

Implemented the agent/nav declarations and diagnostics, parseable `describe()` round trips, host navmesh access, deterministic patrol/chase/stun/catch/lost/recover behaviour, path following and height sampling, solid blocking, snapshot/restore/hash state, and the graph/modeling nodes and readiness entries. Added seven runtime contract tests and a 128-case parser fuzz property. Existing gravity and release-velocity behaviour is unchanged; the falling integration method moved verbatim into `movement.rs` to keep `state.rs` below 400 lines.

Tests run and their output (from `engine`, always with `CARGO_TARGET_DIR=/tmp/codex-target-agents`):

- `cargo test -q -p ve-game --test agents --test nav_parts`: initial implementation run exposed the missing parseable description and the two movement assertions. The description was fixed; final results are covered by the full run below.
- `cargo test -q -p ve-game --no-fail-fast`: **118 passed, 2 failed**. `agents.rs` actually contains 12 tests: 10 passed, 2 failed. `nav_parts.rs`: 5 passed. All other existing game suites passed, including throwing, hands, heading and snapshot tests. All 8 new unit tests passed (7 runtime tests plus the parser fuzz property).
- `cargo test -q -p ve-graph`: **138 passed, 0 failed**.
- `cargo test -q -p ve-modeling --test game_agent_nodes --test g14_game_nodes --test game_spawn_heading`: **20 passed, 0 failed** (4 agent nodes, 15 existing game nodes, 1 spawn heading). Existing warnings remain for `ve-geometry`'s unused `toward`, `world_terrain.rs`'s unused `origin`, and `result.rs`'s unreachable public export.
- `cargo clippy -q -p ve-game -p ve-nav --all-targets -- -D warnings`: **exit 101**, only the four older findings: `parse/helpers.rs:103` `validate_action` is 84/80 lines; `world/state/actions.rs:187` `effects` is 104/80 lines; nested `if` statements at `actions.rs:216` and `:217` are collapsible. No new Clippy findings remain.
- Used `rustfmt --edition 2024 --config-path rustfmt.toml <file>` on the changed module roots; all changed Rust source modules are below 400 lines. `git diff --check` passed for the task paths. No `cargo xtask` was run, as instructed here; the strict gate is pending.

Supplied tests that look physically/logically wrong:

- `agents.rs:104`, `it_patrols_its_loop_at_walking_speed`: 60 + 540 steps = **10 s**, at **1.5 m/s** = **15 m** of travel. The first leg is **12 m**, so even reaching the exact corner leaves about **3 m** along the next leg; advancing within the required **0.5 m** turns earlier, leaving about **3.5 m**. The assertion requires distance **< 3 m** from that corner. Actual position was **[5.698008135759962, 0, -2.4105318695588895]**, distance **3.6021494340906477 m** from **[6, 0, -6]**. The first-second speed assertion passes. Slowing or delaying the patrol to satisfy the second assertion would contradict the specified speed/advance behaviour.
- `agents.rs:146`, `it_goes_round_a_hole_to_reach_the_hero`: initial creature/hero separation is **6 m**, exceeding **sight=5**, and the hole blocks line of sight. The creature must therefore patrol. Its first destination is **[-6, -6]**, then it traverses the bottom leg near **z=-6**, still outside sight of the hero at **[3, 0]** during the test's **600 steps / 10 s**. Actual final position was **[3.036276228975294, 0, -5.801148818567041]**, distance **5.801262240232762 m** from the hero, while the assertion requires **< 1.5 m**. It did not cross the hole. A separate new test first acquires a visible hero, moves the hero behind a hole, and verifies that the creature reaches it while remaining on walkable cells; that test passes. The supplied fixture would need a visible acquisition or more patrol time, rather than bypassing the notice requirement.

Files changed (paths relative to `engine`; unrelated existing workspace changes were preserved):

- `Cargo.lock`: Cargo added the `ve-game` dependencies on `ve-nav` and the parser fuzz test's `proptest` to the existing lockfile.
- `crates/ve-game/Cargo.toml`, `src/lib.rs`, `src/parse.rs`.
- `crates/ve-game/src/parse/{agent.rs,agent/fuzz.rs,format.rs,helpers.rs,hud.rs,reader.rs}`.
- `crates/ve-game/src/world/state.rs` and `src/world/state/{actions.rs,agents.rs,agents/hash.rs,agents/tests.rs,events.rs,follow.rs,hash.rs,movement.rs}`.
- `crates/ve-graph/src/builtin/{game.rs,game/navigation.rs,ready.rs}`.
- `crates/ve-modeling/src/game_nodes.rs` and `src/game_nodes/{agents.rs,collect.rs}`.
- `docs/handoff-codex-agents.md`: this Status section.

Hash compatibility notes:

- Definitions with no `agent` declarations append no new state-hash or definition-fingerprint bytes. Their agent snapshot field is `None`; existing simulation ordering and hash encoding are preserved.
- Agent state is ordered by entity id and includes mode, remaining stun steps, patrol index, catch latch, unseen steps, cached goal, planning tick, path points and cursor. Snapshots restore this state, and agent declarations participate in snapshot compatibility checks.
- The host's `Arc<NavMesh>` is excluded from snapshots and hashes. Setting/replacing it does not itself change the hash; restoring a snapshot retains the current host mesh. The provided no-agent hash test and additional agent hash/restore tests pass.
- `describe()` now emits parseable canonical text, with variables sorted for deterministic descriptions. `to_text()` retains declaration order so existing definition round-trip tests continue to pass.

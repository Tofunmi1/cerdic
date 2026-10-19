# Handoff to DeepSeek: every refusal tells the caller what to change

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`, Rust files under 400 lines, docs on public items, tests, no
commits, no `Co-Authored-By` or tool attribution). **Never run `cargo fmt`/`cargo fmt --all`; format only your Rust files with `rustfmt --edition 2024 --config-path rustfmt.toml
<file>` from `engine`.** Use the edit tools. Do not build the browser bundle.

## Context
A model (or a person) that gets an error should be told what to change. `ve_graph::Diagnostic` has `suggestion: Option<String>` and `.suggest(text)`. The polygon operators already do
this through `crates/ve-modeling/src/poly_ops/errors.rs` (`suggestion(code)` table and `refuse()`). About 70 other error sites in `crates/ve-modeling/src` and `crates/ve-graph/src` build
a `Diagnostic::error(...)` with no suggestion. Count them: `grep -rn "Diagnostic::error(" crates/ve-modeling/src crates/ve-graph/src | wc -l` (112 total, about 40 already suggest).

## You may edit
Everything under `crates/ve-modeling/src` and `crates/ve-graph/src` **except** `crates/ve-modeling/src/poly_ops/**`, `crates/ve-modeling/src/creature/**`, and
`crates/ve-graph/src/builtin/poly*.rs` (another engineer works there at the same time). New test file `crates/ve-modeling/tests/g11_suggestions_more.rs`, and the Status below.

## Tasks
1. List every `Diagnostic::error(` site you may edit that has no `.suggest(`. For each, add a suggestion **only if one is actionable** and specific (what parameter to change, which
   operator to use, which limit applies, with the real numbers in the text where the site has them, e.g. the budget and the amount). If a site is internal (a state that cannot be fixed by the
   caller), leave it and list it in the Status with the reason. Prefer a shared table or helper when one code appears in several places (like `poly_ops/errors.rs`) over copying text.
2. Suggestions must be short (one sentence), concrete, and true. Do not invent operators or parameters: check the names exist (`grep` the builtin specs). Do not change any code, message
   or behaviour except adding the suggestion.
3. Tests in `g11_suggestions_more.rs`: at least 15 real refusing programs written in the text language (model them on `crates/ve-modeling/tests/g11_suggestions.rs`: it builds a session,
   evaluates, and reads `error.code` and `error.suggestion`), covering different families: a solid too large for its budget, a field mesh over budget, an unknown or invalid parameter, a
   missing input, a wrong input type, a group or instance problem, a physics or rig limit (bone count, parent depth), an animation clip name, a curve with too few points, a profile
   problem. For each: assert the code and that `suggestion` is `Some` and contains a word that names the fix (a parameter or operator name). Programs that do not refuse as expected must be
   reported, not dropped.

## Acceptance
`cargo test -p ve-modeling -p ve-graph`, `cargo clippy -p ve-modeling -p ve-graph --tests` shows nothing new from you, `cargo xtask rules`. In the Status give the before and after count of
sites without a suggestion, and the list of sites you left alone with reasons.

## Status (DeepSeek: fill in, then stop)
- State: done (uncommitted; the strict gate is pending, and the browser bundle was not built).
  Suggestions were added across `crates/ve-modeling/src` and `crates/ve-graph/src` where a fix was
  actionable, sharing a table/helper for codes that repeat: `crate::budget::suggestion` (all
  `budget.*`), `crate::fail_suggest` (rig bones, anim clip name, plan input/type/output),
  `field_ops::field_suggestion` (`field.*`), and a new `suggestion` argument on `curve_error`
  (`curves.rs`), `node_fail` (`curves/loft.rs`, `curves/sweep.rs`). New test file
  `crates/ve-modeling/tests/g11_suggestions_more.rs` has 17 programs, all refusing and all passing.

- Before and after counts:
  - `Diagnostic::error(` sites (`grep`): **109 at HEAD → 112 now**. The 3 extra are all
    suggestion-carrying and change no code or message: `ve-modeling/src/lib.rs` (new `fail_suggest`),
    `ve-modeling/src/field_ops.rs` (the field `to_solid` `map_err`), and `ve-graph/src/group_check.rs`
    (the `graph.port_type` site was inlined from the `error(...)` helper so it could carry a
    suggestion).
  - Sites where the diagnostic does **not** get a `.suggest(`: **84 at HEAD → 42 now**.
    Method: a site counts as having a suggestion if `.suggest(` appears on its builder chain or
    later in the same expression block, measured as up to 16 lines but stopped at the next
    `Diagnostic::error(` (this tree either chains `.suggest(`, applies it in a `match` on the code
    just below, or goes through the helpers named above). The window makes a few helper-vs-caller
    cases fuzzy, so the left-alone list below is the authoritative enumeration.
  - Of these, files I may not edit (`poly_ops/**`; also `creature/**`, `builtin/poly*.rs`, which
    have no such sites) hold **14 before and 14 now** (untouched).
  - Editable sites without a suggestion: **70 at HEAD → 28 now** (42 gained one).

- Sites left alone, with reasons (28):
  - `ve-modeling/src/lib.rs:53` (`fail`) — generic diagnostic constructor; callers that can name a
    fix now call the new `fail_suggest` instead. Internal constructor.
  - `ve-modeling/src/operations.rs:227` (`kernel_fail`) — relays the C++ kernel's own message for
    any kernel refusal; the cause is the kernel text and no single caller parameter is the fix.
  - `ve-graph/src/apply.rs:117` (`fail`) — generic constructor for the apply layer; sites that know
    the fix build their own suggestion. Internal constructor.
  - `ve-graph/src/group_check.rs:17` (`error`) — helper taking a code and message; each caller that
    can name a fix attaches one. Internal helper.
  - `ve-graph/src/links.rs:14` (`port_error`) — helper taking code, node and port; callers that can
    name a fix attach it. Internal helper.
  - `ve-graph/src/session.rs:156` (`rejected`) — helper building a `Rejected` outcome from a code;
    the call sites that know the fix add the suggestion. Internal helper.
  - `ve-graph/src/text/lex.rs:39` (`error`) — lexer helper taking a code and span; the parser adds
    suggestions where a fix exists. Internal helper.
  - `ve-graph/src/expand.rs:241` — `group.unknown` while expanding an instance: the stored document
    already names a group that is absent, so there is no caller action here (the write path is where
    a suggestion belongs). Broken document.
  - `ve-graph/src/expand.rs:327` — `parse.unknown_op` while copying a group body: reachable only
    from a corrupt stored document, not from a program the caller wrote. Unreachable.
  - `ve-graph/src/groups.rs:23` (`unknown_group`) — a document-level id lookup reached from several
    callers; the message already names the missing id and which group to use depends on the
    document, so a generic hint would be less concrete than the message.
  - `ve-graph/src/groups/instances.rs:195` — `group.up_to_date`: the instance already matches its
    group; there is nothing to change. No fix exists.
  - `ve-graph/src/links.rs:35` — `parse.unknown_op` from a bare registry `get` with no hint list in
    scope; the parser-level site already suggests the closest registered names. Internal lookup.
  - `ve-graph/src/nodes.rs:47,110,250,257,286,296,327,334,374` (9 sites: `param.range` string
    length, `budget.nodes` nodes/outputs/materials, `param.range` material/position, `graph.
    unknown_port` twice) — the file is 395 lines, at the 400-line limit; adding the text (plus the
    split and the tests it needs) is a change of its own, left for a follow-up.
  - `ve-graph/src/store.rs:98` — `store.unavailable` from a backend read failure: an environment
    (storage backend) problem, not something the document can change.
  - `ve-graph/src/store.rs:158` — `store.unavailable` from a backend write failure: same environment
    case; the sibling in-memory budget site just above already suggests removing nodes.
  - `ve-graph/src/store/envelope.rs:48,68` — `store.corrupt` from a serde encode failure: an
    internal invariant; no document input can make serialisation fail. Internal invariant.
  - `ve-graph/src/store/migrate.rs:54` — `store.migration_failed`: the stored schema is newer than
    this build knows; the fix (a newer build) is not a document change. Build limitation.
  - `ve-graph/src/text/print.rs:148` — an instance names a group missing from the document while
    printing it back out: a broken document, not a caller action.
  - `ve-graph/src/text/print.rs:217` — a node's op is not registered while printing: reachable only
    from a document built against a different registry. Unreachable.

- Commands run and results:
  - `cargo test -p ve-modeling -p ve-graph` — all pass; `--test g11_suggestions_more` is
    `17 passed; 0 failed`.
  - `cargo clippy -p ve-modeling -p ve-graph --tests` — nothing new from the edited files. The only
    warning that touches one is the pre-existing `too_many_lines` on `curves/sweep.rs:12`; all other
    warnings are pre-existing in files I did not edit (`ve-measure`, `rig_skin`, `sky`, ...).
  - `cargo xtask rules` — passes (prints only the rules header).
  - `cargo xtask quick ve-modeling ve-graph` — the build/test stages pass; the `fmt` stage fails on
    pre-existing unformatted files under `crates/ve-measure/tests/` (untracked-by-me, unmodified:
    `git status` shows no change there). Every file I edited passes
    `rustfmt --edition 2024 --config-path rustfmt.toml --check`.
  - Not run (owner-only): `cargo xtask check --strict` / `--no-cache`. The browser bundle was not
    built. Nothing was committed.

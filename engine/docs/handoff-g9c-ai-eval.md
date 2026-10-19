# Handoff: G9c AI composition workflow and the frozen 60-task evaluation

Date: 2026-10-16. Read [`handoff-common.md`](handoff-common.md), [`handoff-g9-remaining.md`](handoff-g9-remaining.md) (section 6),
[`handoff-mcp-tooling.md`](handoff-mcp-tooling.md), [`docs/modeling/mcp-test-provider.md`](modeling/mcp-test-provider.md) and
`crates/ve-provider/` (`assist.rs`, `patch.rs`, `mock.rs`, `tests/g3_provider.rs`). Start only after the tooling handoff's guide and error work is in,
because the evaluation measures that interface. Do not commit.

## Provider status (verified 2026-10-16, router AgentRouter)

| Model | Result with the owner's funded key |
|---|---|
| `deepseek-v4-flash` | works (`ok` returned) |
| `claude-opus-4-8`, `claude-opus-5` | `503 no channel available under group default`; a router group/channel problem, not balance |
| `gpt-6-astra` | `402 Budget pool quota has been exhausted` |

The router only accepts the `claude` CLI route; generic clients get `unauthorized client detected`. For a CLI call use inline env vars for that one
command (`ANTHROPIC_BASE_URL`, `ANTHROPIC_API_KEY`, `CLAUDE_CODE_MAX_OUTPUT_TOKENS`, `--bare --tools ""`). Never write a key to the repo, a log, a
report or a commit; delete scratch copies; keys pasted into chat should be rotated. Report which model produced every number. Do not call a
DeepSeek result an Opus result. If Opus never becomes reachable, the final report says so.

## Tasks

1. **Audit before building.** Read `ve-provider`, `server/` (hosted adapters), `src/engine/mcp-bridge.ts`, the editor pending-transaction flow
   and `docs/modeling/experiments.md`. List what exists (typed patches, one repair feed-back, stale-reply handling, mock provider, "no key reaches
   the browser" test) and extend it; do not write a second framework.
2. **Capability selection with dependency closure.** From the request, choose the operator families and kits needed, include the families they
   depend on, and send only those specs plus the guide. Bounded, deterministic, unit-tested; the selection is logged with the result.
3. **Bounded scene context.** Stable node IDs, relevant parameters, current diagnostics and limits (batch 1,000 commands / 1 MiB; planner caps),
   capped in size, with a test that a large document never exceeds the cap.
4. **One targeted repair attempt** that feeds back the typed diagnostic plus its suggestion (see the tooling handoff), then reports. Optional
   image critique, budgeted, off by default.
5. **Reviewable pending edit.** Graph diff plus viewport preview, explicit user accept; refusal paths covered: stale revision, impossible request,
   ambiguous edit, malformed reply, provider refusal, over-budget plan. Provider output stays untrusted and passes schema, graph validation and
   evaluator limits before anything is applied.
6. **Mock corpus first.** Deterministic scripted replies for every task, so the pipeline, scoring and report run with no network.
7. **Freeze 60 tasks** (creation, follow-up edits, impossible, ambiguous, stale revision, kits and scatter, rounding and fidelity features).
   Freeze rubric, thresholds, schemas, prompt version, model and provider version, budgets and the task-corpus hash in a file before any real run.
   Mock results and real results are stored and reported separately.
8. **Real run on DeepSeek V4 Flash** (and on Opus/GPT only if the router fixes them). Budget guard: stop at a stated spend; the router credit is small.
   Count timeouts and failures in the denominators.
9. **Report** (`docs/modeling/g9c-report.md` plus raw JSON under `target/reports/`): first-try validity, repaired validity, semantic success,
   visual rubric result, edit locality (unchanged IDs), latency, tokens and cost, failed and timed-out tasks, per model.

## Exit criteria

Mock corpus green in `quick` and the focused scripts; the frozen file hash recorded; real-model results labelled with the actual model; strict gate reported as pending; no secret in
any file (`grep -r "sk-" engine docs src server` returns nothing real). Update `cases.md`, `limits.md`, ADR 0023 and the plan with current facts only.

# Head study audit loop

`world.txt` is the only model-owned artifact. The brief, numerical targets, camera list, and pose coverage list are fixed audit inputs.

From `engine/`:

```sh
bash web/tests/scenes/characters/head_study/run-audit.sh
bun web/tests/scenes/characters/head_study/capture.mjs
```

The first command compiles/evaluates the graph into named OBJ outputs, then measures the resulting mesh with this workspace's read-only `audit.ts`. It exits nonzero if a measured rule fails and writes a machine-readable report to `target/reports/character-head/audit.json`. The second uses one headless Chromium session to render all fixed views through the Bevy viewer; images and browser error report go under `target/reports/character-head/shots/`.

Current audited scope is the neutral head and eyes. `poses.json` lists facial deformations that are not implemented yet; those are visible pipeline gaps, not passing tests.

## Edit protocol

Use `agent-protocol.md` for the model loop. Only `world.txt` is editable. For each iteration, state the failing audit IDs, change one cause, rerun `run-audit.sh`, and inspect the same fixed views after a geometry or material milestone. Do not move a target into range to manufacture a pass. Report the JSON audit path, the exact measured lines, and the remaining unmeasured items.

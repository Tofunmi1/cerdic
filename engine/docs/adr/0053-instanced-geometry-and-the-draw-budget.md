# ADR 0053: Instanced geometry is stored once; the draw load is the viewer's problem

- Status: accepted
- Date: 2026-10-19
- Amends: ADR 0014 (instance sets)

## Context

The triangle budget (200,000 preview, 2,000,000 final) multiplied every shape by its number of placements. Two thousand copies of a 128-triangle cone were refused at preview, and a single 100-tree forest of five-thousand-triangle
trees was refused outright. The multiplication made sense while every placement was assumed to cost memory and worker time, but a placement set stores its prototype once and a matrix per copy (`ADR 0014`); export writes one node per copy
sharing one mesh. The number that really grows with copies is how much the viewer draws each frame, and that is for the viewer to bound, by not drawing what cannot be seen and by drawing distant copies in less detail. A budget that
refuses the scene at evaluation can do neither, and made forests, grass and crowds impossible to author at all.

## Decision

1. **Two numbers.** *Stored* triangles (each part's mesh once, however many placements) stay limited to 200,000 preview and 2,000,000 final (`budget.triangles`). *Drawn* triangles (every placement of a shared mesh counted) have their own,
   much larger sanity bound, 20,000,000 preview and 200,000,000 final (`budget.drawn_triangles`), so an edit cannot ask for absurd amounts, checked in preflight before any node runs and again on the finished result.
2. **A warning, not a refusal, for heavy scenes.** Above 4,000,000 triangles at full detail an evaluation succeeds and carries `budget.drawn_heavy` on the output that draws the most, telling the author the viewer may struggle.
3. **The viewer is responsible for the draw load**, in steps that follow: culling what is outside the view, drawing far copies with fewer triangles (detail levels), and holding a per-frame triangle budget by degrading the farthest copies first.
   Until those land, a scene that draws many millions of triangles is heavy on a weak GPU; the warning says so.

## Consequences

- Preflight (`plan.rs`) and the evaluator (`budget.rs`, `Triangles`) apply the same two checks; `inst.*` operators stay pass-through in the estimator.
- Memory limits (`budget.memory`, the result cache) are unaffected: they were always about stored geometry and matrices (128 bytes a placement).
- The 16,384 placement cap per set and per result is unchanged by this decision; raising it is a separate step (the viewer spawns one entity per placement today).
- The glTF exporter is unaffected: its cost follows the number of nodes, not triangles.

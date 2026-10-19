# 0016: Viewport edits are authored command batches

- Date: 2026-10-13
- Status: draft implementation decision; owner may revise.

Viewport handles are input devices for the same typed graph commands used by the inspector and MCP.
They never mutate evaluated meshes or disposable interaction placements. A drag captures the document
revision and its target at pointer-down, previews against that immutable base, and submits one command
batch at pointer-up. Cancel, Escape, pointer loss, or a revision conflict discards the preview. This
avoids history coalescing: there is no committed document state during a drag.

Pointer-ray constraint, snapping and pivot math live in pure `ve-interact`; the viewer/runtime owns
camera rays, handle drawing and hit priority. During a drag the page shows a **projected ghost** (the
selection's bounds moved, or the candidate profile outline) computed from the drag delta: no geometry
is rebuilt, no document is touched. On release the batch goes through the normal staged-document path,
so the geometry is evaluated before the edit is committed and a failing result is rejected like any
other edit. *Deviation from the first draft of this ADR:* it planned throttled preview-tier evaluation of
a candidate document for geometry-changing handles; that is **not built**. The ghost is the preview, and
the cost of a real preview-tier rebuild per drag step has not been measured. This stays an open
improvement, not a claim.

A drag ends without a commit on: Escape, `pointercancel`, lost pointer capture, window blur, and any
change of the document revision while it is held (checked on every move and again on release). A press
while an earlier edit is still being evaluated is refused with that reason; the last good scene stays
selectable. The status line keeps such a message for a few seconds, because the idle redraw would
otherwise replace it at once.

For a selected non-transform node, translation/rotation/scale inserts a typed transform immediately
after it. The insertion and all consumer/output rewires are one graph command batch; undo removes the
transform and restores the original graph. Instance-set handles edit set parameters, never placement
matrices. Socket frame handles edit the authored frame operator through `set_param`.

The implementation uses **world axes only**, grid/angle/scale snapping, and radians in documents.
Part-centre, world-origin and named-socket pivots are explicit choices. A rotation is never decomposed
back into Euler angles: when the selected node is an `xform.rotate` whose angles are all zero its angle is
set directly, otherwise a new world-space rotation node is inserted after the selection (and a pivoted
edit is the chain move by minus the pivot, transform, move by the pivot). So there is no gimbal-lock
case to handle: a node already at 90 degrees about Y is left untouched and gets a new rotation. Because
every handle works in world space, a mirrored part (negative determinant) edits like any other; this is
tested through the real kernel. **Local-axis handles are not implemented.** A request for them needs a
decision about which frame a chain of transforms defines, and is open.

Overlays (handles, bounds, wire, normals, diagnostics, socket frames) are view state only. They do not
enter the document or the evaluation hash, and wire and normals are sampled from the accepted worker
mesh with per-part caps. **They are an SVG layer over the desktop viewer's canvas.** They do not render
inside the split-screen mock headset or WebXR: an engine-side overlay pass in `ve-render`, per eye, is
still to be built and is a prerequisite for V6 (an earlier draft of this ADR claimed stereo rendering;
that was wrong). Handles have their own pick priority and cannot trigger part selection or grabbing.

**Feature picking and `selection.ambiguous`.** The evaluator records which output a face belongs to, not
which operand made it, so a click cannot be attributed below the part. The inspector has an opt-in
"pick the authored feature" mode: a viewport click then follows single-input nodes (a move, a scale, a
material) from the clicked part down to the one feature that makes the shape and selects that feature;
where a node combines several operands (a union, a difference) the part stays selected and the
inspector reports `selection.ambiguous` with the operands as candidates to choose from. An instance of
a group is one feature. Nothing unrelated is ever selected, and no command is sent.

**Evaluation failures.** `lastFailure` in the evaluation state stays as history (the status panel keeps
showing a crash after recovery, G2 case g2_14). Whether it still describes the scene on screen is
`failureCurrent`; the Problems list, the document message line and the diagnostic overlay use that.

## Implementation status

See [the V3 report](../modeling/v3-report.md) for what is built, what was run and what was not. Open
against this ADR: local-axis handles, engine-side per-eye overlays (needed for V6), real preview-tier
evaluation while a geometry handle drags, handles for loft sections (G5 has not landed), and handles
that edit an instance's exposed-parameter override (a handle on a group member edits the definition).
Nothing here has been run on a headset or a real GPU.

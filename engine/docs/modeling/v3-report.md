# V3 viewport editing: implementation report

Status: every row of the V3 case table has evidence; several rows are met in a narrower form than written,
and those are listed under Gaps. Declaring the gate complete is the owner's call; my recommendation is to
accept it with the gaps recorded. Evidence is from the working tree on 2026-10-13 and is **not committed**.
All browser evidence is headless Chromium on SwiftShader: there has been no real-GPU run, no headset run and no
controller run. Design and corrections: [ADR 0016](../adr/0016-viewport-editing.md). Per-case evidence:
[`cases.md`](cases.md), section V3.

## What exists

- **Drag maths** (`ve-interact`, pure): axis, plane and ring constraints, grid/angle/scale snapping, camera
  projection and pointer rays, degenerate rays refused.
- **Transform handles** on the viewer page: move, rotate and single-axis scale in world space; pivot at the part
  centre, the world origin or a selected socket. Release sends one command batch: an existing transform node is
  edited, otherwise a transform is inserted after the selection with consumers, named outputs and a group's
  result rewired atomically. Undo removes the whole batch. A pivoted edit is the chain move by minus the pivot,
  transform, move by the pivot.
- **Parameter handles**: `profile.polygon` points, `profile.circle` radius, `xform.array` and `inst.array`
  spacing, `inst.repeat` step, `frame.add` position and rotation, `solid.revolve` angle. Each sets the same authored
  parameter the inspector shows. An instance set is edited through its pattern node; a click that picks one
  placement offers no handle and says so.
- **Drag safety**: the drag ends without a commit on Escape, `pointercancel`, lost pointer capture, window blur,
  or a changed document revision (checked on every move and on release). A press while an earlier edit is
  still evaluating is refused with that reason. The release refusal names its reason (revision, selection,
  pending), and the status line keeps such a message for six seconds instead of replacing it at once.
- **Selection**: `selection.lost` for a removed node; an opt-in "pick the authored feature" mode that resolves a
  viewport click down single-input chains to the one feature, and reports `selection.ambiguous` with the operands
  as candidates where a node combines several. No command is sent and nothing unrelated is selected.
- **Overlays** (view state only): bounds, sampled wire and normals, diagnostic bounds, socket frames. The
  diagnostic overlay no longer needs a selection.
- **Socket frame gizmo**: the sockets of the selected parts are drawn as oriented triads (X red, Y green,
  Z blue) with a disc to pick; picking is the inspector's socket selection; as the pivot, the move handles start
  at the socket.
- **Frame selected** (F or the button) frames the selected parts' bounds.

## Found and fixed in this pass

- The diagnostic overlay drew only inside the path that needs a selected part with bounds, so with nothing
  selected, or one placement selected, it never appeared.
- A status message about a discarded or refused edit was overwritten by the idle redraw almost at once.
- A release refused because an earlier edit was still evaluating was reported as "document changed".
- The Problems list, the document message line and the diagnostic overlay treated `lastFailure` as current
  forever, so a fixed error stayed listed. My first fix cleared `lastFailure` and broke G2 case g2_14 (a crash
  must stay visible in the status panel after recovery), which `evaluation.mjs` caught; the fix is now a separate
  `failureCurrent` flag that the "what is wrong now" views use, and `lastFailure` stays as history.
- Clicking the outliner row of the selected node cleared the selection even when one placement of an instance
  set was selected; it now widens the selection to the set.
- Instance sets had no spacing handle (`inst.array`, `inst.repeat`), only `xform.array`.
- ADR 0016 claimed things the code does not do: stereo rendering of overlays, preview-tier candidate evaluation,
  closest-equivalent Euler conversion, local axes. It now states what is built.

## Validation run

- `cargo xtask check --strict`: **passed**. 387 native tests (383 before this pass, plus 4 in
  `ve-modeling/tests/v3_pivot.rs`), workspace and WASM clippy, C++ checks, native/WASM parity, the web build,
  every browser suite listed in `xtask/src/web.rs` and the source rules. Log: `target/v3-strict-02.log`.
- New or extended in this pass: `v3_pivot.rs` (pivot fixed through the real kernel, with a control, and a
  mirrored part), `v3-transform-commands.mjs` (pivot invariance by composition, the gimbal rule, conflict
  reasons), `panels-graph.mjs` (`featureCandidates`), `v3-drag-guards.mjs`, `v3-scope.mjs`, `v3-frames.mjs`,
  and the shared helper `support-v3.mjs`. The five V3 suites that were already in the tree
  (`v3-transform-commands`, `v3-gizmo`, `v3-parameter-handles`, `v3-profile-points`, `v3-radius-handle`) plus
  `sockets` and `panels` all passed in a baseline run before this pass's edits, and again in the gate.
- Synthetic events: a browser cannot be told to send `pointercancel` or `blur`, so `v3-drag-guards.mjs`
  dispatches those two on the real elements. Everything else uses real mouse and keyboard input.
- Six fixed views are written to `target/reports/v3-view-{front,back,left,right,top,bottom}.png`. I looked at
  front and top: the gizmo axes and dashed bounds read correctly; the small box itself is low-contrast against
  the grey studio stage. The other four were not looked at in this pass.

## Gaps (open, not hidden)

- **Overlays are an SVG layer over the desktop canvas.** They do not render in the split-screen mock headset
  or WebXR. An engine-side per-eye overlay pass is still to be built and is a prerequisite for V6.
- **No preview-tier rebuild while a geometry handle drags.** The preview is a projected ghost. Whether a
  throttled real preview is affordable has not been measured.
- **World axes only.** No local-axis handles.
- **No handle for an instance's exposed-parameter override.** A handle on a group member edits the definition.
- **Loft section handles** wait for G5.
- Ambiguity is by construction (the operands of a combining node), not by face provenance, which the evaluator
  does not record.
- `inst.repeat` step handle has no browser case; framing a set member or a socket has no dedicated case;
  an undo or an MCP edit during a drag is driven through the document API rather than separately.
- By reading `drawDragPreview` (not by a test): the profile-point and radius ghost projects the profile as if it
  sat untransformed in the XY plane, so for a profile that a later node moves the ghost is probably misplaced.
  The committed edit sets the profile's own parameter and is unaffected.
- Picking cost is unchanged from G4c (linear in placements). No headset, controller or real-GPU timing exists.

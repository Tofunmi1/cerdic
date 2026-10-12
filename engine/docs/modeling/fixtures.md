# Release fixtures and rubrics (v1)

Each fixture is judged by the rubric below, from the fixed views listed. A fixture passes when every
rubric line is met and the follow-up edit sequence completes; a numeric pass alone does not count.

**Fixed views** (always the same): front, right, top, three-quarter (azimuth 35 degrees, elevation
20 degrees), a close detail view of the named feature, and the underside. Fixed lighting (the viewer's
default sun and ambient), neutral grey material unless the fixture says otherwise.

**Edit sequence** (every fixture): one edit in the node graph, one edit in the viewport (gizmo or
handle), undo, redo, save, reload. After each step, authored IDs and parameters must equal the
expected document and the picture must be explainable by the edit.

| Fixture | Gate | Rubric |
|---|---|---|
| Vase / pipe | G3, G5 | Closed solid; rim opening visible from top; wall at least the declared minimum everywhere sampled; widening the opening keeps node IDs; the pipe follows its curve with no twist flips and no pinched cross-section |
| Wheel assembly | G4 | Four wheels from one group; one instance override and one group-definition edit behave differently as specified; the body is lofted or subdivided; named parts selectable (wheel, body, hub); attachment sockets honoured |
| Creature | G8 | Silhouette reads as one organism from front, right and three-quarter views; symmetric within tolerance; limbs join smoothly; named regions (head, body, limb) selectable; no claim of animation-ready topology |
| Facade | G4, G6 | Parameterised floors and bays; windows from one group; one window edited without changing the others; profile details present; instance count change works within the budget |
| Bracket (optional) | G12 | Dimensioned sketch extrudes to the expected volume; changing a dimension regenerates; conflicting dimensions are named |
| Ink drawing (optional) | G11 | Multi-width strokes with correct occlusion and stereo projection |

**Unsupported requests** are part of the rubric: for each fixture a short list of requests the system
must refuse with a `geometry.unsupported_case` or `budget.*` diagnostic rather than produce a
wrong shape (for the vase: a wall thinner than the minimum; for the facade: ten thousand windows
realised as separate solids).

Screenshots are reviewed by eye and stored as goldens only after review; the tolerance rule is the
existing one (10 per channel, at most 0.2 percent of pixels) unless a fixture states otherwise.

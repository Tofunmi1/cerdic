# 0055: A human head and body from CC0 data, edited through the language
- Status: accepted (amends 0003: "no imported assets" gets one exception, named below)
- Date: 2026-10-20

## Context
Hand-sculpted human heads built from field primitives look wrong (tried 2026-10-20: ellipsoid skull, jaw, nose, lids; proportions drifted toward an ape's however long we tuned).
A convincing person needs real anthropometric proportions at the level of landmarks, which we cannot recreate from memory. Scan-based models (SMPL-X, FLAME, GNM) need research licences
or scans. The MakeHuman project's MPFB2 assets and NAVER's Anny model are different: artist-made, calibrated to WHO and survey data, not scans, and released **CC0** (data) with
Apache-2.0 code. Anny's repository carries the body (13,378 quads, 1,445 targets, about 68 MB of source), the 163-bone rig and weights, and a CC0 pack of 52 ARKit-style face shapes.

## Decision
- The owner's exception to 0003: we import **one** dataset, the CC0 MPFB2/Anny human (base mesh, targets, rig, weights, face units). It is converted once into our own compact `human.pack`
  by a converter in the repository, and ships as a separate lazily loaded asset. Nothing else is imported: no other model, no textures from outside (skin looks stay procedural), no
  retrieval at run time. The source commit and licences are recorded in `lib/VENDOR.md`; the raw data is not stored in the repository, only the pack and the converter.
- The human is a **node**, not a mesh file: `body.human@1` takes named phenotype parameters (age, gender, height, weight, muscle, proportions, local changes, face poses) and returns a
  skinned solid with its skeleton, like any other part. All of it is reachable from the text language, MCP tools and the editor, as everything else is.
- It is **editable**, with algorithms not sliders alone: (1) fitting, where measurements (stature, waist, head width, limb lengths, from the ANSUR II ranges) are solved to phenotype
  weights by bounded regularised least squares, since the model is linear in its targets; (2) region edits that keep the result inside the plausible shape space; (3) handle edits
  (move a landmark) solved the same way, with a smooth falloff for what the targets cannot reach.
- Hair, clothing and skin appearance stay procedural, built in the language on top of the body.

## Consequences
Realism of the base human is bounded by MakeHuman's artists, whose phenotype choices carry their preconceptions (Anny's paper says so); we expose parameters, not identity traits.
Download grows by the pack (target: under 12 MB brotli, measured and recorded). The engine gains explicit per-vertex skin weights and morph targets, which until now it did not have
(skinning was derived from distance to bones). Four limbs only. CC0 imposes no attribution duty; we credit the sources in `VENDOR.md` anyway.

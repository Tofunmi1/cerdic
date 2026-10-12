# 0007: The editor is a React/TypeScript shell over a typed bridge; Bun everywhere
- Status: accepted (owner, 2026-10-12); X6 and X7 may adjust the details. Its first decision (the editor
  is a React shell) is superseded by [ADR 0012](0012-viewer-page-is-the-editing-surface.md); the rest stands.
- Date: 2026-10-12

## Context
Two front ends exist: the root app (React 19, three.js, a TypeScript command slice, Bun) and the
engine page (`engine/web`, plain JavaScript, npm via xtask). The visual editor needs a node graph,
outliner, inspector and a viewport with gizmos, and must never own document state.

## Decision
- The editor is a React and TypeScript shell in the root app. It hosts the engine canvas and talks to
  the engine only through a typed bridge: submit `CommandBatch`, subscribe to document, evaluation
  and selection events, query read-only views. Types are generated from the Rust schemas, so a schema
  change breaks the TS build.
- The node canvas starts on `@xyflow/react` behind a thin adapter; if experiment X7 fails, a custom
  canvas replaces it behind the same adapter. Docking uses a small own split-pane component.
- The engine canvas replaces the three.js viewport after V0. X6 (2026-10-12) settled it: embed, no
  iframe. The canvas is created once outside React and moved into the panel, the engine boots once per
  page load behind a guard, and a panel is hidden rather than destroyed (the engine has no `stop()`).
  A canvas that React owns and re-creates cannot be followed by the engine and must not be used.
- Bun is the package manager and script runner everywhere; `xtask/src/web.rs` moves from npm to Bun in
  G0 follow-up work, with the lockfile and test chain updated together.

## Consequences
UI state (layout, panel sizes, hover, camera) may live in the UI or in the editor layer; semantics
live only in the Rust document. The old three.js viewport and `shared/commands.ts` are retired once
the engine canvas matches the agreed rubric.

# 0002: Geometry is a node-graph language written as text
- Status: accepted
- Date: 2026-10-11

## Context
The LLM must create and edit shapes. Spike (53 programs): the line-per-node language compiles
reliably; the real failures are geometric (touching or floating parts), not syntactic.

## Decision
A typed node graph (solid, sdf, curve, profile, instances), one node per line, validated by a
registry-driven compiler. Evaluation returns diagnostics (pieces, gaps, voids) so a repair loop
can fix programs. Parts and sockets give named, attachable sub-shapes.

## Consequences
New capability means a new registered node, not a new format. Attach/socket snapping, creature
and grammar nodes and a procedural material language are the next growth areas.

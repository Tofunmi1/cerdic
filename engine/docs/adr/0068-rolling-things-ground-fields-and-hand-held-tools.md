# ADR 0068: rolling things, ground fields and hand-held tools

Status: proposed (2026-10-25)

## Context

Game entities in `ve-game` were points that a rule moved; the only ground was a flat `ground y=` plane. Games with a ball (golf, billiards, bowling, hockey, tennis) need three general things: a ground with shape and surfaces, things that roll on it, and a way for a hand-held tool to hit them. A golf demo was the first game to need them; the owner's rule is that a game is an example, not a part of the engine, so the mechanisms are named and built generally.

## Decision

- **Ground field** (`ve_game::GroundField`): a grid of heights and surface ids, built by the host from the triangles of the authored parts a `terrain parts=... cell=M` line names (top surface wins; uncovered samples are void) and the footprints of the parts each `surface NAME parts=... decel=D bounce=B` line names (ids 1, 2, ... in order; surface 0 is decel 1.0 bounce 1.0). It is host data like the navmesh: not in snapshots or hashes. `game_ground.rs` in `ve-runtime` builds it.
- **Rolling things** (`roll=1`): fall under gravity, bounce by `kind bounce x surface bounce`, roll on the ground with a constant surface deceleration and a 5/7 g sin(slope) pull, rest when the pull does not beat the deceleration, raise `out` over a void. Fixed 1/60 s step, deterministic.
- **Pockets** (`pocket=1`): a rolling thing over one, slower than 1.6 m/s, on the ground raises `pocket`.
- **Swinging tools** (`swing=1 reach=M loft=DEG power=F`): follow a hand; the head is `reach` metres out of the hand's -Z axis (hand rotation comes from `Inputs::hand_rotations`, from controllers, from the view on the desktop, or from a test); the head's segment is swept against rolling things (a fast head cannot pass through a small ball between two steps), and a hit sets the ball's velocity to the head's speed times power (at most 45 m/s) along the swing, tilted up by loft; one `swing` event per ball until the head has been away or slow. `Entity::orientation` carries the hand's rotation so the tool is drawn turned with the hand.
- **Hands are fed smoothly**: the runtime interpolates hand poses across the fixed steps of a frame, so a swing or a throw has its real speed in every step.
- Nodes: `game.terrain`, `game.surface`, and `roll`, `swing`, `reach`, `loft`, `power`, `pocket` on `game.kind`.

## Relation to `ve-physics`

`ve-physics` (Rapier, ADR 0035) simulates authored rigid bodies. These game-entity mechanisms are a second, simpler path in `ve-game` (pure Rust, deterministic, replayable, testable natively, no host needed). Use `ve-game` kinematics for simple game entities (balls, thrown things, pickups) and `ve-physics` for authored rigid-body scenes (stacks, ragdolls, joints). Moving game entities onto `ve-physics` with a heightfield collider is possible later if two paths prove a burden.

## Not covered

Spin (a ball's rotation and its effect), friction that depends on speed, wind, collisions between two rolling things (billiard balls bouncing off each other), bank shots off walls (walls are not in the ground field), and a swing's face angle (only the swing direction and loft are used).

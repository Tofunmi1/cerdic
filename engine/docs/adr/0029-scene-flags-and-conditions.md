# ADR 0029: Scene flags and conditions

Status: proposed (owner confirmation pending)

## Context
Clicks and proximity can start clips (ADR 0027) and a switch can run another part (targets), but real logic needs
memory: a door that stays locked until a key is taken, a lamp that always matches its switch, a machine that needs two
levers down. The state must be typed, bounded, deterministic and authored through the same graph as everything else.

## Decision
1. **Flags** are named on/off values, scene-wide, off at the start of play mode and cleared when play mode stops.
   Names are lowercase letters, digits and underscores, start with a letter, at most 24 long. A scene uses at most
   32 flags; a behaviour names at most 8 in `sets` and 8 in `needs`.
2. `behavior.on` gains three text parameters: `sets` (`name=on|off|toggle`, bare name = on), `needs` (`name`,
   `!name`, `name=on|off`) and `watch` (one flag). Trigger `flag` fires whenever the watched flag changes; action
   `follow` (only with trigger `flag`) runs the part's own clip forward while the flag is on and backward while it
   is off, so a lamp cannot get out of step with its switch.
3. A fired behaviour acts only if every `need` holds. Then it applies its `sets`, then runs its own clip (if it
   has one) and its `target` listeners.
4. **Deterministic order.** Everything that fires within one 1/60 s step is handled in node-name order, so two
   switches changing one flag in the same step always give the same result. A change made in a step reaches
   `flag` watchers in the next step. A behaviour cannot watch a flag it sets, so a part cannot trigger itself
   forever; longer loops between parts would take a step per hop and never stall a frame.
5. A behaviour needs a clip, a target or at least one `sets`, so a part may be pure logic (an AND gate: watches
   one flag, needs another, sets a third).
6. The page can read flags (`viewer_behaviors().flags`) and set one (`viewer_flag`) for tests and tools. glTF
   export drops logic like it drops triggers.

## Grab triggers
7. Triggers `grab` (a hand or the mouse picks the part up in play mode), `release` (it is let go) and `hold` (both:
   `sets` apply on pick-up and reverse on release, and the clip runs forward while held and back when let go).
   A part is grabbable only in play mode and only if its behaviour uses one of these triggers; everything else
   authored stays put. A carried part keeps the pose it was dropped in until play mode stops, which puts it back
   where it was authored. Letting go always undoes what picking up did, even if the flags it `needs` have changed.
8. **Sockets**: trigger `socket` with `accepts` (the output name of a grabbable part) and `radius`. A part let go
   within `radius` of a socket that accepts it snaps onto it (its centre moves onto the socket's centre) and the socket
   fires like `hold`: `sets` as written and its clip forward. Picking the part up again fires the reverse. The nearest
   fitting socket wins; one part sits in one socket, and a socket holds one part (a newer part replaces the old one).
   Position only: a held part's rotation is not matched in this slice. Leaving play mode undocks everything.

## Not in this slice
Numeric or counting state, timers, OR conditions, flags in the document (initial values), per-instance flags.

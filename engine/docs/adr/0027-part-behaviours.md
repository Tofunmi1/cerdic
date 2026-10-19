# ADR 0027: Behaviours that start a part's clip

Status: proposed (owner confirmation pending)

## Context
ADR 0026 made parts move on a shared looping clock. Interactive scenes need things that respond: a door that
opens when clicked or approached. The same typed language must express this so a model and a person author it
through the same commands, and a replay of the same inputs must give the same poses.

## Decision
1. `behavior.on@1(in, trigger, radius, action)` is a pass-through node like `anim.track`. It attaches one
   `Behavior` to the part (data, not code): `trigger` is `select` (the part is clicked in play mode) or `near`
   (the viewer camera comes within `radius` metres of the part's centre); `action` is `play` (run the part's
   tracks once from the start, then hold the end) or `toggle` (run forward, and run backward on the next
   trigger). One behaviour per part; a second `behavior.on` on the same shape is refused.
2. A part with a behaviour does not follow the shared clock. It has its own clip time that starts at 0 (rest
   pose) and moves only when the behaviour runs. Its tracks are sampled one-shot (held at both ends), whatever
   their `loop` setting, since a response is a gesture, not a cycle.
3. Behaviours run at a fixed step of 1/60 s, accumulated from real time with a cap of 5 steps per frame, so a
   given sequence of triggers on the step counter gives the same clip times on every machine. Trigger events
   are queued and applied at step boundaries.
4. Play mode is explicit view state, off by default (like selection, never saved). In edit mode clicks select;
   in play mode a click on a part with a `select` behaviour triggers it and selects nothing (entering play mode
   clears the selection, since a selected part's gizmo would take the click). `near` triggers fire only in
   play mode: `play` on entering the radius; `toggle` on entering and again on leaving, so a door opens as
   you come up to it and closes as you walk away. Leaving play mode resets every behaviour part to rest.
5. glTF export ignores behaviours (a file has no triggers). A part with a behaviour exports its tracks as
   usual so the clip is still visible in other tools.

## Targets (second slice)
6. `behavior.on` gains `target`: the output name of another part. A behaviour with a target runs that part's
   clip instead of (or, if the sender has a clip of its own, as well as) its own. The named part is a
   listener: `trigger="signal"`, which nothing in the world fires and which needs an `anim.track`. A listener
   with a target is refused (no chains in this slice, so there are no loops to reason about), and a sender
   needs a track or a target. Several senders may name one listener. A target that matches no listener does
   nothing. The listener's own `action` decides what each signal does (toggle alternates, play runs once).

## Not in this slice
State variables and conditions (a lamp that is on or off, a door that needs two switches), sequences, chains
of listeners, grab and socket triggers, per-instance behaviours. These add fields, not a new model.

## Limits
Radius 0.1 to 100 m. At most 64 behaviour parts (clip time state is per part).

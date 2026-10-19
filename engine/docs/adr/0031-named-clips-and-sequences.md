# ADR 0031: Named clips and sequences

Status: proposed (owner confirmation pending). Built and tested 2026-10-17.

## Context
A part had one timeline. Characters and machines need separate motions on the same part (an arm that waves and
reaches, a gate that opens and shuts), chosen by different triggers, and motions that follow each other.

## Decision
1. `anim.track` and `anim.wave` take `clip`: a name (lowercase letters, digits, underscores, at most 24, starting with
   a letter; empty is the part's main clip). Tracks with the same clip form one clip; its length is its longest track.
2. A part with a behaviour keeps one running state **per clip** (time, direction, running). Each track is sampled at its
   own clip's time and the poses of all clips combine in track order, so a held "wave" and a playing "reach" move the
   arm together. A part with no behaviour plays every clip on the shared clock.
3. `behavior.on` takes `clip` (which of the part's clips it runs; must exist unless it is a listener) and `then`
   (up to 8 more clips of the part, run in order). A sender names a clip on its target with `target="name:clip"`.
4. **Sequences**: the clip a behaviour runs, then each clip in `then`, each starting in the step after the previous one
   reaches its end running forward. Starting a sequence forward rewinds the later clips, so it can play again.
   Backward runs do not chain.
5. glTF: each clip is its own animation (`clip` for the main one, the clip name otherwise), holding the node,
   pointer and visibility channels of that clip's tracks. Clips that move the same node are separate animations because
   glTF plays animations one at a time; the viewer combines them.
6. The page can read every clip's state in `viewer_behaviors().parts[].clips`.

## Not in this slice
Clip speed and delay (use timers and `every`), blending or cross-fading between clips, backward chaining, and per-clip
loops different from their tracks' own `loop`.

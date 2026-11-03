// The "effects" guide topic: particle effects in the language (fx.emitter and the nodes that refine it). The recipes are the real worlds
// `fx_campfire.txt` (fire, smoke and bouncing spark ribbons) and the ones below.

export const EFFECTS = {
  about: 'particle effects: fire, smoke, sparks, dust, rain, splashes, magic, trails, explosions (fx.emitter, fx.force, fx.collide, fx.trail, fx.sub)',
  text: `Particle effects. A shape that carries fx.emitter gives birth to small soft sprites that fly, fade and die; the effect sits at the middle of the
shape and follows it when the shape moves (anim.track, anim.parent, held in a hand). Put fx.* AFTER geometry nodes such as xform.move (they rebuild the
shape and drop it); material, anim.* and rig.* nodes keep it.

fx.emitter@1(shape, name, rate, burst, burst_at, lifetime=[a,b], speed=[a,b], direction, cone, shape, extent, size=[start,end], colour, colour_end,
alpha=[start,end], glow, blend, render, max_particles, delay, duration, loop) makes the emitter. Call it again on the result for a second emitter
(fire plus smoke plus sparks); give each a name. Then refine one by name (the last one when no name is given):
- fx.force(kind="turbulence"|"wind"|"gravity"|"drag"|"vortex"|"attractor", strength, vector, centre, radius, scale, speed): turbulence is swirling noise
  with no sources or sinks, the thing that makes smoke and fire look alive (strength 1 to 3, scale 0.5 to 2). Forces add up.
- fx.collide(reaction="bounce"|"stick"|"kill", restitution, friction, ground=<y>): particles meet the ground plane at height ground, and in play mode
  every phys.body of the scene too (sparks bounce off a crate and a wall, chips pile on a table).
- fx.trail(points, min_step) with emitter render="ribbon": each particle leaves a streak (sparks, comet tails, tracers, sword swooshes).
- fx.sub(parent, child, on="death"|"birth"|"collide", count): particles of one emitter spawn particles of another (a splash where each drop lands).

render="cube" makes each particle a small LIT box that tumbles (spin=radians per second): wood chips, stone shards, shell casings; set size to its edge
and colour to its paint. The world call fx.status lists every emitting part with how many particles each emitter has alive and the box they fill: use it to
check that an effect runs and where it goes.

Looks: blend="additive" adds light (fire, sparks, magic, lightning: with glow 3 to 8 and bright colours, the bloom does the rest); blend="alpha" is ordinary
transparency (smoke, dust, mist, rain). size=[start,end] and colour/colour_end/alpha=[start,end] change over each particle's life: fire shrinks and goes
from yellow-white to deep red; smoke grows and fades. glow multiplies the colour above 1.

Recipes (a small shape s at the place the effect comes from):
- Campfire: fx.emitter(s, name="fire", rate=90, lifetime=[0.5,1], speed=[0.6,1.4], cone=14, shape="disc", extent=[0.14,0,0], size=[0.34,0.06], colour=[1,0.7,0.2],
  colour_end=[0.7,0.08,0], alpha=[1,0], glow=3, blend="additive") then fx.force(kind="turbulence", strength=2.4, scale=1.4); add a smoke emitter (rate=24,
  lifetime=[2,3.2], size=[0.25,0.95], colour=[0.24,0.24,0.26], alpha=[0.55,0], blend="alpha") with a turbulence and a wind force.
- Sparks that bounce: fx.emitter(name="sparks", rate=16, lifetime=[1,2], speed=[2,4], cone=40, size=[0.05,0.05], colour=[1,0.8,0.3], glow=6, blend="additive",
  render="ribbon") then fx.force(kind="gravity"), fx.collide(reaction="bounce", restitution=0.45, ground=0), fx.trail(points=9).
- Explosion: rate=0, burst=200, lifetime=[0.4,1.2], speed=[3,9], cone=85, shape="sphere", extent=[0.2,0,0], blend="additive", glow=5, size=[0.5,0.1]; a second alpha
  emitter for smoke with a burst; fx.force(kind="drag", strength=2).
- Rain with splashes: a wide shape="box" emitter high up, direction=[0,-1,0], speed=[8,10], rate=400, render="ribbon" with fx.trail(points=3), fx.collide(reaction="kill",
  ground=0) and fx.sub(parent="rain", child="splash", on="collide", count=3); the splash emitter has rate=0, lifetime=[0.2,0.5], speed=[1,2], cone=70, fx.force gravity.
- A trail behind something that moves: put the emitter on the moving part (anim.track), rate=60, speed=[0,0.2], lifetime=[0.4,0.8], size=[0.15,0], blend="additive".
- Magic: shape="sphere", extent=[0.3,0,0], speed=[0,0.3], fx.force(kind="vortex", centre at the shape, strength=3, radius=1) and fx.force(kind="attractor", strength=2).

Limits: 16 emitters per effect, 64 emitting parts per scene, max_particles up to 20000 per emitter. Effects are drawn by the Bevy renderer (the viewer and the
headset path). They are visual: they do not push the player or physics bodies.`,
};

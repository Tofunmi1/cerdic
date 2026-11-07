// combat: melee timelines for any game with fighters. Needs the rpg library declared before it (`lib="rpg"`, then `lib="combat"`).
// An attack is data on a timeline in frames (60 a second): startup, active (the hit shape is live), recovery, with a cancel window,
// an input buffer, hitstop on contact, i-frames, knockback and combos. The simulation decides hits; animation follows through effects.
//
//   combat.attack_def('light1', { startup: 0.15, active: 0.1, recovery: 0.25, cancel_from: 0.12, next: 'light2', cost: { stamina: 10 },
//       shape: { range: 1.8, arc: 110 }, damage: { amount: 20, type: 'physical', poise: 12 }, hitstop: 0.07, knockback: 0.4, lunge: 0.3 })
//   combat.dodge_def('roll', { duration: 0.45, i_frames: [0.05, 0.25], distance: 2.5, cost: { stamina: 20 } })
//   combat.input(e, 'light1' | 'roll' | 'block_on' | 'block_off')   buffered ~8 frames; chains by `next` inside the cancel window
//   combat.busy(e)  combat.frozen(e)  combat.current(e)
//   combat.hostile(a, b) is true for actors whose 'team.*' tags differ; replace combat.hostile to change it.
// Hits call rpg.damage; effects (anim, particles, shake) go to the host; events: emit:combat_hit, emit:combat_parried (a = target/attacker, b = other).
(() => {
  'use strict';
  if (typeof rpg === 'undefined') throw new Error('the combat library needs the rpg library declared before it: game.script@1(lib="rpg")');
  const BUFFER = 8, PARRY = 8;
  const C = () => (state.combat ??= {});
  const attacks = {}, dodges = {};
  const idOf = (e) => (typeof e === 'number' ? e : e.id);
  const fr = (s) => Math.max(1, Math.round(s * 60));
  const st = (e) => (C()[idOf(e)] ??= { act: null, t: 0, hit: [], buf: null, hs: 0, block: 0 });
  const team = (e) => { const a = state.rpg.actors[idOf(e)]; const t = a && a.tags.find((x) => x.tag.startsWith('team.')); return t ? t.tag : ''; };
  const api = {
    attack_def: (id, d) => { attacks[id] = { startup: 0.15, active: 0.1, recovery: 0.25, cancel_from: 0, next: null, cost: {}, shape: { range: 1.6, arc: 100 }, damage: { amount: 10, type: 'physical', poise: 8 }, hitstop: 0.06, knockback: 0.3, lunge: 0, unblockable: false, ...d }; },
    dodge_def: (id, d) => { dodges[id] = { duration: 0.45, i_frames: [0.05, 0.25], distance: 2.5, cost: {}, ...d }; },
    hostile: (a, b) => { const x = team(a), y = team(b); return x !== y; },
    busy: (e) => !!st(e).act,
    frozen: (e) => st(e).hs > 0,
    current: (e) => st(e).act && st(e).act.id,
    input: (e, name) => { st(e).buf = { name, tick: game.tick }; },
  };
  globalThis.combat = api;

  const begin = (e, name) => {
    const s = st(e);
    const a = attacks[name], d = dodges[name];
    const def = a || d;
    if (!def || !rpg.is_actor(e) || rpg.is_dead(e) || rpg.has_tag(e, 'state.staggered')) return false;
    if (!rpg.use(e, 'combat:' + name, null)) return false;
    const ent = entity(idOf(e));
    s.act = { id: name, kind: a ? 'attack' : 'dodge', dir: ent.forward };
    s.t = 0; s.hit = []; s.buf = null;
    const total = a ? fr(a.startup) + fr(a.active) + fr(a.recovery) : fr(d.duration);
    rpg.add_tag(e, 'state.busy', 'combat', game.tick + total + 1);
    if (a) game.effect('anim', name, ent);
    if (d) { game.effect('anim', name, ent); const [i0, i1] = d.i_frames; rpg.add_tag(e, 'state.invulnerable', 'dodge', game.tick + fr(i1)); s.inv_from = fr(i0); }
    return true;
  };
  const defineAbility = (name, def) => rpg.ability_def('combat:' + name, { cost: def.cost || {}, cooldown: 0, blocked_by: ['state.dead', 'state.staggered'] });
  const origAttack = api.attack_def, origDodge = api.dodge_def;
  api.attack_def = (id, d) => { origAttack(id, d); defineAbility(id, attacks[id]); };
  api.dodge_def = (id, d) => { origDodge(id, d); defineAbility(id, dodges[id]); };

  const inShape = (u, t, shape) => {
    const p = u.pos, q = t.pos;
    const dx = q[0] - p[0], dz = q[2] - p[2];
    const dist = Math.hypot(dx, dz);
    if (dist > shape.range + t.size) return false;
    if (dist < 1e-6) return true;
    const f = u.forward;
    const cos = (f[0] * dx + f[2] * dz) / dist;
    const half = (shape.arc / 2) * Math.PI / 180;
    return Math.acos(Math.min(1, Math.max(-1, cos))) <= half + Math.atan2(t.size, Math.max(dist, 1e-6));
  };

  const hit = (u, t, a) => {
    const us = st(u), ts = st(t);
    const [dx, dz] = [t.pos[0] - u.pos[0], t.pos[2] - u.pos[2]];
    const len = Math.max(1e-6, Math.hypot(dx, dz));
    if (rpg.has_tag(t, 'state.parrying') && !a.unblockable) {
      rpg.add_tag(u, 'state.staggered', 'parried', game.tick + 90);
      us.act = null; us.hs = fr(0.18); ts.hs = fr(0.18);
      game.emit('combat_parried', t, u); game.effect('particles', 'parry', t); game.effect('shake', 'parry');
      return;
    }
    const lost = rpg.damage(t, { amount: a.damage.amount, type: a.damage.type, poise: a.damage.poise, source: u, unblockable: a.unblockable });
    const stop = fr(a.hitstop);
    us.hs = Math.max(us.hs, stop); ts.hs = Math.max(ts.hs, stop);
    t.pos = [t.pos[0] + dx / len * a.knockback, t.pos[1], t.pos[2] + dz / len * a.knockback];
    game.emit('combat_hit', t, u);
    // an actor with the 'hyper' tag (a boss) is not flinched by hits: its attacks play on; its stagger is shown when its poise breaks
    if (!rpg.has_tag(t, 'hyper')) game.effect('anim', 'hit_react', t);
    game.effect('particles', rpg.has_tag(t, 'state.blocking') ? 'block_sparks' : 'hit_sparks', t);
    game.effect('shake', 'hit', t, Math.min(1, lost / 40));
  };

  // a broken guard ends the attack in progress
  on('emit:rpg_staggered', (e) => { if (e.a) { const s = st(e.a.id); s.act = null; s.buf = null; } });
  on('tick', () => {
    const fighters =Object.keys(state.rpg ? state.rpg.actors : {}).map(Number);
    for (const id of fighters) {
      const s = st(id);
      if (rpg.is_dead(id)) { s.act = null; continue; }
      const e = entity(id);
      if (!e || !e.alive) continue;
      // block: held state; the first frames of it parry
      if (s.block > 0) { s.block++; if (s.block > PARRY) rpg.remove_tags(id, 'parry'); }
      if (s.hs > 0) { s.hs--; continue; }
      if (s.buf && game.tick - s.buf.tick > BUFFER) s.buf = null;
      if (s.buf && (s.buf.name === 'block_on' || s.buf.name === 'block_off')) {
        if (s.buf.name === 'block_on' && !s.block && !s.act) { s.block = 1; rpg.add_tag(id, 'state.blocking', 'block', 0); rpg.add_tag(id, 'state.parrying', 'parry', game.tick + PARRY); }
        if (s.buf.name === 'block_off') { s.block = 0; rpg.remove_tags(id, 'block'); rpg.remove_tags(id, 'parry'); }
        s.buf = null;
      }
      if (!s.act) { if (s.buf && !s.block) begin(id, s.buf.name); continue; }
      const act = s.act; const a = attacks[act.id]; const d = dodges[act.id];
      s.t++;
      if (a) {
        const su = fr(a.startup), ac = fr(a.active), total = su + ac + fr(a.recovery);
        if (s.t === 1 && a.lunge) { e.pos = [e.pos[0] + act.dir[0] * a.lunge, e.pos[1], e.pos[2] + act.dir[2] * a.lunge]; }
        if (s.t > su && s.t <= su + ac) {
          for (const tid of fighters) {
            if (tid === id || s.hit.includes(tid) || rpg.is_dead(tid) || !api.hostile(id, tid)) continue;
            const t = entity(tid);
            if (t && t.alive && inShape(e, t, a.shape)) { s.hit.push(tid); hit(e, t, a); if (s.act !== act) break; }
          }
        }
        if (s.act && s.t >= su + ac + fr(a.cancel_from) && s.buf && a.next && s.buf.name === a.next && s.t < total) { const next = s.buf.name; s.act = null; if (!begin(id, next)) s.act = null; continue; }
        if (s.t >= total) s.act = null;
      } else if (d) {
        const total = fr(d.duration), step = d.distance / total;
        e.pos = [e.pos[0] + act.dir[0] * step, e.pos[1], e.pos[2] + act.dir[2] * step];
        if (s.t >= total) s.act = null;
      }
    }
  });
})();

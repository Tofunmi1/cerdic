// rpg: attributes, tags, effects, abilities and a damage pipeline for any game with fighters, stats and status effects.
// Load it with `game.script@1(lib="rpg")` before the scripts that use it. All data lives in `state.rpg`, so saves, undo and replays keep it.
//
//   rpg.actor(entity, { attributes: { health: 100, max_health: 100, strength: 10 }, tags: ['team.player'], resist: { fire: 0.25 } })
//   rpg.get(e, 'strength')            current value: (base + sum of 'add' modifiers) x product of 'mul' modifiers, then limits
//   rpg.set_base(e, 'health', 50)     rpg.add_base(e, 'stamina', -10)
//   rpg.has_tag(e, 'state.stunned')   matches the tag itself and anything under it ('state.stunned.short')
//   rpg.effect_def('burn', { duration: 4, period: 1, stacking: 'refresh', modifiers: [{ attr: 'speed', op: 'mul', value: 0.8 }],
//                            tags: ['state.burning'], on_tick: (target, source) => rpg.damage(target, { amount: 5, type: 'fire', source }) })
//   rpg.apply(target, 'burn', source)  rpg.remove(target, 'burn')
//   rpg.ability_def('slash', { cost: { stamina: 15 }, cooldown: 0.6, blocked_by: ['state.stunned', 'state.dead'], run: (user, target) => {} })
//   rpg.use(user, 'slash', target)    true when it started (cost paid, cooldown running), false with rpg.last_refusal naming why
//   rpg.damage(target, { amount: 20, type: 'physical', source: attacker, poise: 10 })   returns the health lost
// Events (for `on('emit:...')` and rules): emit:rpg_damaged, emit:rpg_died, emit:rpg_staggered (a = target, b = source).
(() => {
  'use strict';
  const S = () => (state.rpg ??= { actors: {}, effects: [], cooldowns: {}, next: 1 });
  const limits = { health: { min: 0, max: 'max_health' }, stamina: { min: 0, max: 'max_stamina' }, poise: { min: 0, max: 'max_poise' } };
  const effectDefs = {};
  const abilityDefs = {};
  const idOf = (e) => (typeof e === 'number' ? e : e.id);
  const actor = (e) => S().actors[idOf(e)];
  const need = (e) => { const a = actor(e); if (!a) throw new Error('entity ' + idOf(e) + ' is not an rpg actor: call rpg.actor(entity, {...}) first'); return a; };
  const tagMatch = (tag, wanted) => tag === wanted || tag.startsWith(wanted + '.');
  const hasTag = (e, wanted) => { const a = actor(e); return !!a && a.tags.some((t) => tagMatch(t.tag, wanted)); };
  const addTag = (e, tag, source, expires) => { need(e).tags.push({ tag, source: source || '', expires: expires || 0 }); };
  const removeTags = (e, source) => { const a = actor(e); if (a) a.tags = a.tags.filter((t) => t.source !== source); };

  const get = (e, name) => {
    const a = need(e);
    let value = a.base[name] ?? 0;
    let add = 0, mul = 1, set = null;
    for (const m of a.mods) {
      if (m.attr !== name) continue;
      if (m.op === 'add') add += m.value; else if (m.op === 'mul') mul *= m.value; else if (m.op === 'set') set = m.value;
    }
    value = set !== null ? set : (value + add) * mul;
    const lim = limits[name];
    if (lim) {
      const hi = typeof lim.max === 'string' ? (a.base[lim.max] ?? Infinity) : lim.max;
      value = Math.min(Math.max(value, lim.min ?? -Infinity), hi ?? Infinity);
    }
    return value;
  };
  const setBase = (e, name, v) => { const a = need(e); const lim = limits[name]; if (lim) { const hi = typeof lim.max === 'string' ? (a.base[lim.max] ?? Infinity) : lim.max; v = Math.min(Math.max(v, lim.min ?? -Infinity), hi ?? Infinity); } a.base[name] = v; return v; };
  const addBase = (e, name, d) => setBase(e, name, (need(e).base[name] ?? 0) + d);

  const register = (e, spec) => {
    S().actors[idOf(e)] = { base: { ...(spec.attributes || {}) }, mods: [], tags: (spec.tags || []).map((tag) => ({ tag, source: 'base', expires: 0 })), resist: { ...(spec.resist || {}) }, dead: false };
    return idOf(e);
  };

  const effectDef = (id, def) => { effectDefs[id] = { duration: 0, period: 0, stacking: 'refresh', max_stacks: 5, modifiers: [], tags: [], ...def }; };
  const apply = (target, id, source) => {
    const def = effectDefs[id];
    if (!def) throw new Error("unknown effect '" + id + "': define it with rpg.effect_def first");
    const a = need(target); const s = S(); const t = idOf(target); const now = game.tick;
    let inst = s.effects.find((x) => x.target === t && x.def === id && (def.stacking !== 'separate' ? true : x.source === idOf(source ?? 0)));
    if (inst) {
      if (def.stacking === 'ignore') return false;
      if (def.stacking === 'stack') inst.stacks = Math.min(inst.stacks + 1, def.max_stacks);
      inst.expires = def.duration > 0 ? now + Math.round(def.duration * 60) : 0;
      a.mods = a.mods.filter((m) => m.source !== 'fx' + inst.id);
    } else {
      inst = { id: s.next++, target: t, def: id, source: source ? idOf(source) : -1, stacks: 1, expires: def.duration > 0 ? now + Math.round(def.duration * 60) : 0, next_tick: def.period > 0 ? now + Math.round(def.period * 60) : 0 };
      s.effects.push(inst);
      for (const tag of def.tags) addTag(target, tag, 'fx' + inst.id, 0);
      if (def.on_apply) def.on_apply(entity(t), source ?? null);
    }
    for (const m of def.modifiers) a.mods.push({ attr: m.attr, op: m.op, value: m.op === 'add' ? m.value * inst.stacks : m.op === 'mul' ? Math.pow(m.value, inst.stacks) : m.value, source: 'fx' + inst.id });
    return true;
  };
  const dropEffect = (inst, expired) => {
    const a = actor(inst.target); const def = effectDefs[inst.def];
    if (a) { a.mods = a.mods.filter((m) => m.source !== 'fx' + inst.id); removeTags(inst.target, 'fx' + inst.id); }
    S().effects = S().effects.filter((x) => x !== inst);
    if (def && def.on_expire && a) def.on_expire(entity(inst.target), expired);
  };
  const remove = (target, id) => { for (const inst of S().effects.filter((x) => x.target === idOf(target) && x.def === id)) dropEffect(inst, false); };

  const abilityDef = (id, def) => { abilityDefs[id] = { cost: {}, cooldown: 0, requires: [], blocked_by: [], grants: [], ...def }; };
  const refuse = (why) => { api.last_refusal = why; return false; };
  const use = (user, id, target) => {
    const def = abilityDefs[id];
    if (!def) throw new Error("unknown ability '" + id + "'");
    need(user);
    const key = idOf(user) + ':' + id;
    if ((S().cooldowns[key] ?? 0) > game.tick) return refuse('cooldown');
    for (const tag of def.blocked_by) if (hasTag(user, tag)) return refuse('blocked by ' + tag);
    for (const tag of def.requires) if (!hasTag(user, tag)) return refuse('needs ' + tag);
    for (const [attr, amount] of Object.entries(def.cost)) if (get(user, attr) < amount) return refuse('not enough ' + attr);
    for (const [attr, amount] of Object.entries(def.cost)) addBase(user, attr, -amount);
    if (def.cooldown > 0) S().cooldowns[key] = game.tick + Math.round(def.cooldown * 60);
    for (const tag of def.grants) addTag(user, tag, 'ab:' + id, game.tick + Math.max(1, Math.round((def.duration || 0.1) * 60)));
    if (def.run) def.run(typeof user === 'number' ? entity(user) : user, target ?? null);
    return true;
  };

  const damage = (target, spec) => {
    const a = need(target); const t = entity(idOf(target));
    if (a.dead || hasTag(target, 'state.invulnerable')) return 0;
    const type = spec.type || 'physical';
    let amount = Math.max(0, spec.amount) * (1 - Math.min(0.95, a.resist[type] ?? 0));
    if (type === 'physical') amount = amount * 100 / (100 + Math.max(0, get(target, 'armor')));
    if (hasTag(target, 'state.blocking') && !spec.unblockable) amount *= 1 - Math.min(0.95, get(target, 'block') || 0.7);
    const lost = Math.min(amount, get(target, 'health'));
    addBase(target, 'health', -amount);
    t.set('last_damage', lost);
    const source = spec.source === undefined || spec.source === null ? null : (typeof spec.source === 'number' ? entity(spec.source) : spec.source);
    game.emit('rpg_damaged', t, source);
    if (spec.poise && 'poise' in a.base) {
      addBase(target, 'poise', -spec.poise);
      if (get(target, 'poise') <= 0 && !hasTag(target, 'state.staggered')) {
        addTag(target, 'state.staggered', 'stagger', game.tick + Math.round((a.base.stagger_time ?? 1) * 60));
        a.base.poise = a.base.max_poise ?? 0;
        game.emit('rpg_staggered', t, source);
      }
    }
    if (get(target, 'health') <= 0 && !a.dead) { a.dead = true; addTag(target, 'state.dead', 'death', 0); game.emit('rpg_died', t, source); }
    return lost;
  };

  const api = { actor: register, get, set_base: setBase, add_base: addBase, has_tag: hasTag, add_tag: addTag, remove_tags: removeTags, effect_def: effectDef, apply, remove, ability_def: abilityDef, use, damage, limits, last_refusal: '', is_actor: (e) => !!actor(e), is_dead: (e) => !!actor(e)?.dead };
  globalThis.rpg = api;

  on('destroyed', (e) => { if (e.a) delete S().actors[e.a.id]; });
  on('tick', (e) => {
    const s = S(); const now = e.tick;
    for (const inst of s.effects.slice()) {
      const def = effectDefs[inst.def];
      if (!def) continue;
      if (def.period > 0 && inst.next_tick && now >= inst.next_tick) { inst.next_tick += Math.round(def.period * 60); if (def.on_tick) def.on_tick(entity(inst.target), inst.source >= 0 ? entity(inst.source) : null, inst.stacks); }
      if (inst.expires && now >= inst.expires) dropEffect(inst, true);
    }
    for (const [id, a] of Object.entries(s.actors)) {
      a.tags = a.tags.filter((t) => !t.expires || now < t.expires);
      if (a.dead) continue;
      const regen = a.base.health_regen; if (regen) addBase(+id, 'health', regen / 60);
      const sr = a.base.stamina_regen; if (sr && !a.tags.some((t) => tagMatch(t.tag, 'state.exhausted'))) addBase(+id, 'stamina', sr / 60);
    }
  });
})();

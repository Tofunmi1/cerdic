// THE ASHEN COLOSSUS: the fight, as one script over the engine's generic APIs (world calls, key events, ui, view). No game code lives in the engine.
// Player: left click or J slash, right click or Q fire bolt, E dodge (a brief invulnerability), X potion, Space jump, Enter to start or try again.
// Boss: telegraphed attacks (slam, sweep, stomp ring, fireballs, meteors), three phases, a glowing core that takes double damage from bolts.
const FOCUS = [0, 12, 0];
const LEGS = [[4.6, 0], [-4.6, 0]];
const LEG_R = 2.9;
const CORE = [0, 13.8, 4.3];
const MOUTH = [0, 18.6, 6.6];
const HIDE = [0, -300, 0];
const GRAVITY_EYE = 1.6; // eye height above the feet
const PHASE_AT = [0.66, 0.33];
const BOSS_BASE_HP = 3200;

const rnd = (a, b) => a + random() * (b - a);
const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));
const dist2 = (ax, az, bx, bz) => Math.hypot(ax - bx, az - bz);

const P = { pos: [0, 1.7, 27], fwd: [0, 0, -1], hp: 100, maxHp: 100, stam: 100, mana: 100, potions: 3, iframe: 0, dodgeCd: 0, slashCd: 0, boltCd: 0, regenAt: 0, level: 1, kills: 0, dmg: 1 };
const B = { hp: BOSS_BASE_HP, max: BOSS_BASE_HP, phase: 1, busy: 0, gap: 2, last: '', dying: 0, lastHit: 0, hitText: '' };
let mode = 'intro'; // intro | fight | dead | won
let now = 0;
const later = [];
const after = (sec, fn) => later.push({ at: now + sec, fn });

// ---- helpers over the world API ----
const ops = [];
const place = (entity, at, scale = [1, 1, 1]) => ops.push({ method: 'world.insert_components', params: { entity, components: { Transform: { translation: at, rotation: [0, 0, 0, 1], scale } } } });
const spawn = async (shape, position = HIDE) => (await world.call('world.spawn_entity', { shape, position })).entity;
const burst = (name, position, scale = 1) => { world.call('fx.burst', { name, position, scale }).catch(() => {}); };
const trigger = (name) => { world.call('animation.trigger', { name }).catch(() => {}); };
const lastUi = new Map();
const setUi = (id, spec) => { const key = JSON.stringify(spec); if (lastUi.get(id) !== key) { lastUi.set(id, key); ui.set(id, spec); } };
const dropUi = (id) => { lastUi.delete(id); ui.remove(id); };

function quatForward([x, y, z, w]) {
  // the view looks down -z: rotate (0, 0, -1) by the quaternion
  const vx = 0, vy = 0, vz = -1;
  const tx = 2 * (y * vz - z * vy), ty = 2 * (z * vx - x * vz), tz = 2 * (x * vy - y * vx);
  return [vx + w * tx + (y * tz - z * ty), vy + w * ty + (z * tx - x * tz), vz + w * tz + (x * ty - y * tx)];
}

// ---- pools of entities (made once, parked out of sight) ----
const pools = { discs: [], bars: [], ring: [], balls: [], bolts: [] };
async function makePools() {
  for (let i = 0; i < 8; i++) pools.discs.push({ e: await spawn({ kind: 'cylinder', radius: 1, height: 0.1, color: [0.9, 0.06, 0.02], emissive: [2.2, 0.15, 0.03], opacity: 0.55 }), on: false });
  for (let i = 0; i < 2; i++) pools.bars.push({ e: await spawn({ kind: 'box', size: [1, 0.1, 1], color: [0.9, 0.06, 0.02], emissive: [2.2, 0.15, 0.03], opacity: 0.5 }), on: false });
  for (let i = 0; i < 28; i++) pools.ring.push({ e: await spawn({ kind: 'box', size: [1.8, 0.7, 0.5], color: [1, 0.5, 0.1], emissive: [3, 1, 0.15] }), on: false });
  for (let i = 0; i < 10; i++) pools.balls.push({ e: await spawn({ kind: 'sphere', radius: 1, color: [1, 0.4, 0.05], emissive: [4, 1.3, 0.2] }), on: false });
  for (let i = 0; i < 6; i++) pools.bolts.push({ e: await spawn({ kind: 'sphere', radius: 0.3, color: [0.6, 0.8, 1], emissive: [2.5, 4, 7] }), on: false });
}
const take = (pool) => pool.find((x) => !x.on);
const free = (item) => { item.on = false; place(item.e, HIDE); };

// ---- the screen ----
function banner(text, sub, seconds, color = '#ffd9a0') {
  ui.set('banner', { kind: 'text', text, x: 0.5, y: 0.38, size: 64, color, weight: 'bold', font: 'serif' });
  if (sub) ui.set('banner_sub', { kind: 'text', text: sub, x: 0.5, y: 0.47, size: 22, color: '#e8cfa8', font: 'serif' });
  if (seconds) after(seconds, () => { dropUi('banner'); dropUi('banner_sub'); });
}
function note(text, color = '#ffffff') { ui.set('note', { kind: 'text', text, x: 0.5, y: 0.66, size: 26, color, weight: 'bold' }); const mine = now; after(0.9, () => { if (Math.abs(now - mine - 0.9) < 0.2) dropUi('note'); }); }
function hud() {
  setUi('boss', { kind: 'bar', x: 0.5, y: 0.055, w: 0.5, h: 20, value: B.hp / B.max, ghost: B.hp / B.max, color: B.phase === 3 ? '#e04010' : '#b02020', label: 'THE ASHEN COLOSSUS', align: 'center' });
  setUi('phase', { kind: 'text', text: ['I', 'II', 'III'].map((r, i) => (i + 1 === B.phase ? '[ ' + r + ' ]' : '  ' + r + '  ')).join(' '), x: 0.5, y: 0.095, size: 14, color: '#d9b48a', font: 'serif' });
  setUi('hp', { kind: 'bar', x: 0.03, y: 0.93, w: 0.26, h: 20, value: P.hp / P.maxHp, color: P.hp / P.maxHp < 0.3 ? '#ff3030' : '#c82828', label: 'HP ' + Math.ceil(P.hp), align: 'left' });
  setUi('stam', { kind: 'bar', x: 0.03, y: 0.962, w: 0.2, h: 11, value: P.stam / 100, color: '#d6b02a', label: '', align: 'left' });
  setUi('mana', { kind: 'bar', x: 0.03, y: 0.982, w: 0.2, h: 11, value: P.mana / 100, color: '#3a7be0', label: '', align: 'left' });
  setUi('potions', { kind: 'text', text: 'Potions ' + P.potions + '   [X]      Level ' + P.level, x: 0.3, y: 0.93, size: 15, color: '#e6d3b0', align: 'left' });
  setUi('cross', { kind: 'text', text: '+', x: 0.5, y: 0.5, size: 22, color: '#ffffff', opacity: 0.75 });
}

// ---- damage ----
function hurt(amount, why) {
  if (mode !== 'fight') return;
  if (P.iframe > 0) { note('DODGED', '#9fd8ff'); return; }
  P.hp = Math.max(0, P.hp - amount);
  log('hurt ' + amount + ' ' + why + ' hp ' + P.hp);
  ui.flash('#ff1010', 0.45, 0.5);
  ui.shake(Math.min(26, 8 + amount * 0.4), 0.4);
  P.regenAt = now + 1.2;
  if (P.hp <= 0) die(why);
}
function die() {
  mode = 'dead';
  banner('YOU DIED', 'press Enter to rise again', 0, '#c02020');
  ui.flash('#400000', 1.2, 0.8);
}
function hitBoss(amount, at, kind) {
  if (mode !== 'fight' || B.dying) return;
  const dealt = Math.round(amount * P.dmg * rnd(0.92, 1.08));
  B.hp = Math.max(0, B.hp - dealt);
  log('boss hit ' + kind + ' ' + dealt + ' hp ' + B.hp);
  B.lastHit = now;
  burst(kind === 'crit' ? 'explode' : 'sparks', at, kind === 'crit' ? 1.6 : 1.1);
  ui.shake(kind === 'crit' ? 7 : 4, 0.18);
  setUi('hit', { kind: 'text', text: (kind === 'crit' ? 'CRITICAL  ' : '') + '-' + dealt, x: 0.5, y: 0.125, size: kind === 'crit' ? 30 : 22, color: kind === 'crit' ? '#ffd040' : '#ffffff', weight: 'bold' });
  after(0.8, () => { if (now - B.lastHit > 0.75) dropUi('hit'); });
  if (B.hp <= 0) bossDies();
  else if (B.phase === 1 && B.hp / B.max < PHASE_AT[0]) B.phaseUp = 2;
  else if (B.phase === 2 && B.hp / B.max < PHASE_AT[1]) B.phaseUp = 3;
}

// ---- the player's actions ----
function slash() {
  if (mode !== 'fight' || P.slashCd > 0 || P.stam < 14) return;
  P.stam -= 14; P.slashCd = 0.45; P.regenAt = now + 0.7;
  const tipAt = [P.pos[0] + P.fwd[0] * 2.4, P.pos[1] - 0.2 + P.fwd[1] * 2.4, P.pos[2] + P.fwd[2] * 2.4];
  burst('sparks', tipAt, 0.5);
  ui.shake(2, 0.1);
  after(0.14, () => {
    for (const [lx, lz] of LEGS) {
      const d = dist2(P.pos[0], P.pos[2], lx, lz);
      const toLeg = [lx - P.pos[0], lz - P.pos[2]];
      const len = Math.hypot(toLeg[0], toLeg[1]) || 1;
      const facing = (P.fwd[0] * toLeg[0] + P.fwd[2] * toLeg[1]) / (len * Math.hypot(P.fwd[0], P.fwd[2]) || 1);
      if (d <= LEG_R + 3.4 && facing > 0.3) { hitBoss(58, [lx - (toLeg[0] / len) * LEG_R, 3, lz - (toLeg[1] / len) * LEG_R], 'hit'); break; }
    }
  });
}
function fireBolt() {
  if (mode !== 'fight' || P.boltCd > 0 || P.mana < 25) return;
  const b = take(pools.bolts);
  if (!b) return;
  P.mana -= 25; P.boltCd = 0.7;
  b.on = true; b.age = 0; b.v = P.fwd.map((c) => c * 44);
  b.p = [P.pos[0] + P.fwd[0] * 1.2, P.pos[1] - 0.25 + P.fwd[1] * 1.2, P.pos[2] + P.fwd[2] * 1.2];
  burst('magic', b.p, 0.6);
}
function dodge() {
  if (mode !== 'fight' || P.dodgeCd > 0 || P.stam < 25) return;
  P.stam -= 25; P.iframe = 0.6; P.dodgeCd = 0.95; P.regenAt = now + 0.8;
  ui.flash('#a8d8ff', 0.25, 0.3);
}
function drink() {
  if (mode !== 'fight' || P.potions <= 0 || P.hp >= P.maxHp) return;
  P.potions -= 1; P.hp = Math.min(P.maxHp, P.hp + 50);
  ui.flash('#50ff80', 0.4, 0.3);
  burst('magic', [P.pos[0], P.pos[1] - 0.8, P.pos[2]], 1.2);
  note('+50 HP', '#8dff9f');
}

// ---- the boss ----
function telegraph(kind, at, size, seconds) {
  const pool = kind === 'bar' ? pools.bars : pools.discs;
  const t = take(pool);
  if (!t) return null;
  t.on = true; t.at = at; t.size = size; t.born = now; t.life = seconds; t.kind = kind;
  return t;
}
function impact(x, z, r, dmg, label) {
  burst('explode', [x, 1, z], 2.2); burst('dust', [x, 0.5, z], 3); burst('embers', [x, 0.5, z], 2);
  ui.shake(label === 'meteor' ? 14 : 20, 0.55);
  const grounded = P.pos[1] < 2.4;
  if (dist2(P.pos[0], P.pos[2], x, z) <= r && (label !== 'slam' || grounded)) hurt(dmg, label);
}
const attacks = {
  slam() {
    const sx = P.pos[0] >= 0 ? 1 : -1;
    trigger(sx > 0 ? 'slam_p' : 'slam_n');
    const x = sx * 8.8, z = 10.2;
    telegraph('disc', [x, z], 7, 1.7);
    after(1.78, () => impact(x, z, 7.1, 38, 'slam'));
    B.busy = 3.0;
  },
  sweep() {
    trigger('sweep');
    telegraph('bar', [0, 9.6], [32, 11], 1.45);
    after(1.45, () => { burst('dust', [0, 1, 10], 4); ui.shake(12, 0.4); if (Math.abs(P.pos[0]) < 16 && P.pos[2] > 4 && P.pos[2] < 15.5 && P.pos[1] < 3.0) hurt(32, 'sweep'); });
    B.busy = 2.6;
  },
  stomp() {
    trigger('stomp');
    after(1.55, () => { ui.shake(22, 0.7); burst('dust', [0, 0.5, 6], 4); burst('explode', [0, 1, 7], 2.5); startRing(); });
    B.busy = 3.0;
  },
  fireballs() {
    trigger('roar');
    after(1.0, () => {
      const n = B.phase === 3 ? 5 : 3;
      for (let i = 0; i < n; i++) after(i * 0.28, () => launchBall(MOUTH, [P.pos[0] + rnd(-4, 4), 0.6, P.pos[2] + rnd(-4, 4)], 20, 1.0, 'fireball'));
    });
    B.busy = 3.4;
  },
  meteors() {
    trigger('roar');
    ui.flash('#ff6020', 0.5, 0.25);
    const targets = [[P.pos[0], P.pos[2]], [P.pos[0] + rnd(-5, 5), P.pos[2] + rnd(-5, 5)], [P.pos[0] + rnd(-6, 6), P.pos[2] + rnd(-6, 6)]];
    for (let i = 0; i < 4; i++) { const a = rnd(0, Math.PI * 2), r = rnd(6, 26); targets.push([Math.cos(a) * r, Math.sin(a) * r]); }
    targets.forEach(([x, z], i) => {
      const cx = clamp(x, -30, 30), cz = clamp(z, -30, 30), delay = 1.0 + i * 0.35;
      after(delay, () => { telegraph('disc', [cx, cz], 4.4, 1.5); });
      after(delay + 0.7, () => launchBall([cx, 48, cz], [cx, 0.5, cz], 60, 3.2, 'meteor'));
    });
    B.busy = 4.4;
  },
};
function launchBall(from, to, speed, size, kind) {
  const b = take(pools.balls);
  if (!b) return;
  const d = [to[0] - from[0], to[1] - from[1], to[2] - from[2]], len = Math.hypot(...d);
  b.on = true; b.age = 0; b.kind = kind; b.size = size; b.p = [...from]; b.v = d.map((c) => (c / len) * speed); b.to = to;
}
let ringState = null;
function startRing() {
  ringState = { r: 6, hit: false };
}
function chooseAttack() {
  const pool = ['slam', 'slam', 'sweep'];
  if (B.phase >= 2) pool.push('stomp', 'fireballs');
  if (B.phase >= 3) pool.push('meteors', 'stomp', 'slam');
  let pick = pool[Math.floor(random() * pool.length)];
  for (let i = 0; i < 4 && pick === B.last; i++) pick = pool[Math.floor(random() * pool.length)];
  B.last = pick;
  log('attack ' + pick + ' phase ' + B.phase);
  attacks[pick]();
  B.gap = [1.7, 1.2, 0.8][B.phase - 1];
}
function phaseRoar() {
  B.phase = B.phaseUp; B.phaseUp = 0;
  trigger('roar');
  banner(['', 'PHASE II', 'PHASE III'][B.phase - 1], B.phase === 2 ? 'the Colossus tears the ground open' : 'the Colossus burns with rage', 2.6, '#ff8a50');
  ui.shake(22, 1.2); ui.flash('#ff5010', 0.6, 0.35);
  after(1.0, () => { burst('explode', [0, 14, 5], 4); startRing(); });
  B.busy = 3.4; B.gap = 1.0;
}
function bossDies() {
  B.dying = now; mode = 'won';
  trigger('death');
  banner('THE COLOSSUS FALLS', '', 0, '#ffd070');
  for (let i = 0; i < 16; i++) after(i * 0.4, () => { burst(i % 3 === 0 ? 'explode' : 'fire', [rnd(-6, 6), rnd(2, 22), rnd(-4, 5)], rnd(2.5, 5)); ui.shake(14, 0.3); });
  after(6.8, () => {
    P.level += 1;
    banner('VICTORY', 'Level ' + P.level + ': more health and a stronger blade. The Colossus will return, harder.', 0, '#ffe08a');
    ui.set('again', { kind: 'text', text: 'press Enter to fight again', x: 0.5, y: 0.56, size: 22, color: '#ffffff' });
  });
}

// ---- starting and restarting ----
async function startFight() {
  mode = 'fight';
  log('fight starts, level ' + P.level);
  dropUi('banner'); dropUi('banner_sub'); dropUi('again'); dropUi('intro_help');
  P.maxHp = 100 + 20 * (P.level - 1); P.hp = P.maxHp; P.stam = 100; P.mana = 100; P.potions = 3; P.dmg = 1 + 0.12 * (P.level - 1);
  B.max = Math.round(BOSS_BASE_HP * (1 + 0.3 * (P.level - 1))); B.hp = B.max; B.phase = 1; B.phaseUp = 0; B.dying = 0; B.busy = 0; B.gap = 3.4; B.last = '';
  ringState = null;
  for (const group of Object.values(pools)) for (const item of group) { item.on = false; place(item.e, HIDE); }
  later.length = 0;
  trigger('idle');
  after(0.3, () => { trigger('roar'); ui.shake(18, 1.4); });
  banner('THE ASHEN COLOSSUS', '', 2.4, '#ffb070');
  B.busy = 3.4;
  view.teleport([0, 1.7, 27], [0, 12, 0]);
}

on('key', (e) => {
  if (!e.down || e.repeat) return;
  if (e.code === 'Enter') { if (mode === 'intro' || mode === 'dead' || (mode === 'won' && B.dying && now - B.dying > 7)) startFight(); return; }
  if (e.code === 'Mouse0' || e.code === 'KeyJ') slash();
  else if (e.code === 'Mouse2' || e.code === 'KeyQ') fireBolt();
  else if (e.code === 'KeyE') dodge();
  else if (e.code === 'KeyX') drink();
});

// ---- the frame ----
let busy = false;
on('frame', async (dt, t) => {
  now = t;
  if (busy || !ready) return;
  busy = true;
  try {
    const pose = await view.pose();
    if (pose && pose.position) { P.pos = pose.position; if (pose.rotation) P.fwd = quatForward(pose.rotation); }
    // scheduled things
    for (let i = later.length - 1; i >= 0; i--) if (later[i].at <= now) { const { fn } = later.splice(i, 1)[0]; fn(); }
    if (mode === 'fight') {
      P.iframe = Math.max(0, P.iframe - dt); P.dodgeCd = Math.max(0, P.dodgeCd - dt); P.slashCd = Math.max(0, P.slashCd - dt); P.boltCd = Math.max(0, P.boltCd - dt);
      if (now >= P.regenAt) P.stam = Math.min(100, P.stam + 30 * dt);
      P.mana = Math.min(100, P.mana + 5 * dt);
      B.busy -= dt;
      if (B.busy <= 0) {
        B.gap -= dt;
        if (B.gap <= 0) { if (B.phaseUp) phaseRoar(); else chooseAttack(); }
      }
    }
    // the stomp ring: a ring of glowing blocks running outward; it hurts what stands on the ground when it passes
    if (ringState) {
      ringState.r += 13 * dt;
      const pieces = pools.ring;
      pieces.forEach((piece, i) => { const a = (i / pieces.length) * Math.PI * 2; place(piece.e, [Math.cos(a) * ringState.r, 0.4, Math.sin(a) * ringState.r], [1, 1, 1].map((c, k) => c)); piece.on = true; });
      if (!ringState.hit && Math.abs(Math.hypot(P.pos[0], P.pos[2]) - ringState.r) < 1.1 && P.pos[1] < 2.5) { ringState.hit = true; hurt(30, 'shockwave'); }
      if (ringState.r > 38) { ringState = null; pieces.forEach(free); }
    }
    // telegraphs
    for (const group of [pools.discs, pools.bars]) for (const tg of group) if (tg.on) {
      const age = now - tg.born;
      if (age > tg.life) { free(tg); continue; }
      const pulse = 0.85 + 0.15 * Math.sin(age * 18);
      if (tg.kind === 'bar') place(tg.e, [tg.at[0], 0.07, tg.at[1]], [tg.size[0], 1, tg.size[1]]);
      else { const r = tg.size * (0.35 + 0.65 * Math.min(1, age / tg.life)) * pulse; place(tg.e, [tg.at[0], 0.07, tg.at[1]], [r, 1, r]); }
    }
    // fireballs and meteors
    for (const b of pools.balls) if (b.on) {
      b.age += dt;
      b.p = b.p.map((c, i) => c + b.v[i] * dt);
      const r = b.size;
      place(b.e, b.p, [r, r, r]);
      if (Math.random() < 0.5) burst('embers', b.p, 0.5);
      const toPlayer = Math.hypot(b.p[0] - P.pos[0], b.p[1] - P.pos[1], b.p[2] - P.pos[2]);
      if (b.kind === 'fireball' && toPlayer < 1.9) { burst('explode', b.p, 1.6); free(b); hurt(24, 'fireball'); continue; }
      if (b.p[1] <= 0.5 || b.age > 4) {
        const ground = [b.p[0], 0.5, b.p[2]];
        if (b.kind === 'meteor') impact(ground[0], ground[2], 4.6, 26, 'meteor');
        else { burst('explode', ground, 1.8); burst('dust', ground, 1.5); if (dist2(P.pos[0], P.pos[2], ground[0], ground[2]) < 3.2 && P.pos[1] < 2.4) hurt(14, 'fireball'); }
        free(b);
      }
    }
    // the player's bolts
    for (const b of pools.bolts) if (b.on) {
      b.age += dt;
      b.p = b.p.map((c, i) => c + b.v[i] * dt);
      place(b.e, b.p);
      if (Math.random() < 0.6) burst('embers', b.p, 0.3);
      const [x, y, z] = b.p;
      const core = Math.hypot(x - CORE[0], y - CORE[1], z - CORE[2]) < 2.4;
      const body = (x / 7.2) ** 2 + ((y - 13.2) / 6.2) ** 2 + (z / 4.9) ** 2 < 1 || (x / 3.7) ** 2 + ((y - 19.9) / 3.8) ** 2 + ((z - 1.9) / 4.1) ** 2 < 1
        || (y < 9.5 && (Math.hypot(x - 4.6, z) < 3 || Math.hypot(x + 4.6, z) < 3)) || (Math.abs(x) < 7.2 && y > 7.7 && y < 10.3 && Math.abs(z) < 3.8);
      if (core || body) { hitBoss(core ? 240 : 110, b.p, core ? 'crit' : 'hit'); free(b); continue; }
      if (y < 0.1 || b.age > 1.8 || Math.hypot(x, z) > 40) { burst('smoke', b.p, 0.4); free(b); }
    }
    hud();
  } catch (error) { log('frame error', String(error && error.message ? error.message : error)); }
  busy = false;
});

let ready = false;
(async () => {
  await makePools();
  ui.set('title', { kind: 'text', text: 'THE ASHEN COLOSSUS', x: 0.5, y: 0.3, size: 66, color: '#ffb070', weight: 'bold', font: 'serif' });
  ui.set('intro_help', { kind: 'text', text: 'Click the view to look around.  WASD walk   Space jump   Shift run\nLeft click or J: slash (hit the legs)    Right click or Q: fire bolt (aim at the glowing core)\nE: dodge, brief invulnerability    X: potion\n\nPress Enter to begin', x: 0.5, y: 0.52, size: 19, color: '#f0dcc0', font: 'serif' });
  after(0, () => {});
  ready = true;
  log('the fight is ready');
})();
on('key:Enter', () => { dropUi('title'); });

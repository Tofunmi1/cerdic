(() => {
  'use strict';
  // The script API. Everything a script can reach is defined here; the host function is captured and removed, so scripts cannot call it.
  const host = globalThis.__host;
  delete globalThis.__host;
  const call = (op, args) => JSON.parse(host(op, JSON.stringify(args === undefined ? {} : args)));
  const handlers = new Map();
  const timers = [];
  let current = '';
  let now = 0;
  const steps = (seconds) => Math.max(1, Math.round(seconds * 60));

  class Entity {
    constructor(id) { this.id = id; }
    get alive() { return call('ent', { id: this.id }) !== null; }
    get kind() { const e = call('ent', { id: this.id }); return e && e.kind; }
    get tags() { const e = call('ent', { id: this.id }); return e ? e.tags : []; }
    get props() { const e = call('ent', { id: this.id }); return e ? e.props : {}; }
    get pos() { const e = call('ent', { id: this.id }); return e && e.pos; }
    set pos(v) { call('set', { id: this.id, f: 'pos', v }); }
    get vel() { const e = call('ent', { id: this.id }); return e && e.vel; }
    set vel(v) { call('set', { id: this.id, f: 'vel', v }); }
    get heading() { const e = call('ent', { id: this.id }); return e && e.heading; }
    set heading(v) { call('set', { id: this.id, f: 'heading', v }); }
    get size() { const e = call('ent', { id: this.id }); return e ? e.size : 0; }
    /** The unit vector the entity faces (heading is a yaw about +Y; zero faces +Z). */
    get forward() { const h = this.heading; return [Math.sin(h), 0, Math.cos(h)]; }
    has(tag) { return this.tags.includes(tag); }
    get(name) { return call('prop', { id: this.id, name }); }
    set(name, v) { return call('prop', { id: this.id, name, v }); }
    add(name, d) { return call('prop', { id: this.id, name, add: d }); }
    destroy() { call('destroy', { id: this.id }); }
  }
  const entity = (id) => (id === null || id === undefined || id < 0 ? null : new Entity(id));

  const on = (event, a, b) => {
    const fn = typeof a === 'function' ? a : b;
    const filter = typeof a === 'function' ? null : a;
    if (typeof event !== 'string' || typeof fn !== 'function') throw new Error('on(event, [filter,] handler) needs an event name and a function');
    if (!handlers.has(event)) handlers.set(event, []);
    handlers.get(event).push({ fn, filter, script: current });
  };
  const every = (seconds, fn) => {
    if (!(seconds > 0) || typeof fn !== 'function') throw new Error('every(seconds, fn) needs a positive number and a function');
    timers.push({ next: now + steps(seconds), interval: steps(seconds), fn, once: false, script: current });
  };
  const after = (seconds, fn) => {
    if (!(seconds >= 0) || typeof fn !== 'function') throw new Error('after(seconds, fn) needs a number and a function');
    timers.push({ next: now + steps(seconds), interval: 0, fn, once: true, script: current });
  };
  const game = {
    get: (name) => call('var', { name }),
    set: (name, v) => call('var', { name, v }),
    add: (name, d) => call('var', { name, add: d }),
    emit: (name, a, b) => call('emit', { name, a: a ? a.id : -1, b: b ? b.id : -1 }),
    effect: (kind, name, target, value) => call('fx', { kind, name, target: target ? target.id : -1, value }),
    reset: () => call('reset', {}),
    win: () => call('end', { win: true }),
    lose: () => call('end', { win: false }),
    get tick() { return now; },
    get time() { return now / 60; },
  };
  // Game UI: elements are set by id and stay until removed; units are fractions of the screen height; text is drawn from procedural fonts
  // (presets sans, rounded, techno, pixel, hand, bold, italic, or {preset, weight, width, slant, spacing, wobble}). Anchors: tl tc tr cl c cr bl bc br.
  const theme = { text: '#ffffff', size: 0.03, font: 'sans', panel: 'rgba(20,24,32,0.7)', panel_border: 'rgba(255,255,255,0.18)', bar: '#d94a4a', back: 'rgba(0,0,0,0.55)', border: 'rgba(255,255,255,0.35)', accent: '#ffd76a' };
  const anchors = { tl: [0, 0], tc: [0.5, 0], tr: [1, 0], cl: [0, 0.5], c: [0.5, 0.5], cr: [1, 0.5], bl: [0, 1], bc: [0.5, 1], br: [1, 1] };
  const sizeOf = (s) => (s.t === 'text' ? [call('ui_measure', s), s.size || theme.size] : s.size || [0.3, s.t === 'bar' ? 0.025 : 0.2]);
  const ui = {
    theme,
    set: (id, spec) => call('ui_set', { id: String(id), spec }),
    text: (id, content, o) => ui.set(id, { t: 'text', text: String(content), size: theme.size, color: theme.text, font: theme.font, ...o }),
    bar: (id, value, max, o) => ui.set(id, { t: 'bar', value, max, color: theme.bar, back: theme.back, border: theme.border, ...o }),
    panel: (id, o) => ui.set(id, { t: 'panel', fill: theme.panel, border: theme.panel_border, ...o }),
    remove: (id) => call('ui_remove', { id: String(id) }),
    clear: () => call('ui_clear', {}),
    measure: (text, size, font) => call('ui_measure', { text: String(text), size: size || theme.size, font: font || theme.font }),
    /** Lays elements out in a column or row from an anchor: ui.stack('stats', [{ id: 'hp', t: 'bar', ... }, { id: 'name', t: 'text', text: 'Knight' }], { anchor: 'tl', at: [0, 0], dir: 'column', gap: 0.01 }). */
    stack: (name, items, o) => {
      const { anchor = 'tl', at = [0, 0], dir = 'column', gap = 0.01 } = o || {};
      const sizes = items.map((s) => sizeOf({ ...s, font: s.font || theme.font, size: s.size || (s.t === 'text' ? theme.size : undefined) }));
      const column = dir === 'column';
      const total = sizes.reduce((acc, s) => [column ? Math.max(acc[0], s[0]) : acc[0] + s[0] + gap, column ? acc[1] + s[1] + gap : Math.max(acc[1], s[1])], [0, 0]);
      if (items.length) total[column ? 1 : 0] -= gap;
      const pivot = anchors[anchor] || anchors.tl;
      let cursor = [at[0] - pivot[0] * total[0], at[1] - pivot[1] * total[1]];
      items.forEach((s, i) => {
        ui.set(name + '.' + (s.id || i), { color: s.t === 'text' ? theme.text : undefined, font: s.t === 'text' ? theme.font : undefined, size: s.t === 'text' ? theme.size : undefined, ...(s.t === 'bar' ? { back: theme.back, border: theme.border, color: theme.bar } : {}), ...s, anchor, pivot: 'tl', at: [...cursor] });
        cursor = column ? [cursor[0], cursor[1] + sizes[i][1] + gap] : [cursor[0] + sizes[i][0] + gap, cursor[1]];
      });
    },
  };
  // The host's input for the step: named axes (held keys and sticks, -1..1) and buttons pressed this step. The page names them.
  const input = {
    axis: (name) => call('inp', { name }),
    held: (name) => call('inp', { name }) > 0.5,
    pressed: (name) => call('inpp', { name }),
  };
  // Where the view stands: camera.look(eye, target) each step makes the game's camera (third person, fixed, cinematic).
  const camera = { look: (pos, target) => call('cam', { pos, target }) };
  Math.random = () => call('rand');
  for (const name of ['Date', 'performance']) delete globalThis[name];
  Object.assign(globalThis, {
    state: {},
    on, every, after, game, entity, ui, input, camera,
    find: (selector) => call('find', { s: selector === undefined ? '' : selector }).map((id) => new Entity(id)),
    spawn: (kind, pos, heading) => new Entity(call('spawn', { kind, pos, heading: heading || 0 })),
    log: (...args) => call('log', { text: args.map((v) => (typeof v === 'string' ? v : JSON.stringify(v))).join(' ') }),
    __load: (name, fn) => { current = name; fn(); current = ''; },
    __dispatch: (name, a, b, tick) => {
      now = tick;
      const fail = (script, error) => call('err', { script, event: name, message: String((error && error.message) || error) });
      const list = handlers.get(name);
      if (list) {
        const ea = entity(a);
        const eb = entity(b);
        for (const h of list.slice()) {
          if (h.filter && ((h.filter.a && !(a >= 0 && call('sel', { s: h.filter.a, id: a }))) || (h.filter.b && !(b >= 0 && call('sel', { s: h.filter.b, id: b }))))) continue;
          try { h.fn({ name, a: ea, b: eb, tick, dt: 1 / 60 }); } catch (error) { fail(h.script, error); }
        }
      }
      if (name === 'tick') {
        for (const t of timers.slice()) {
          if (now < t.next) continue;
          if (t.once) timers.splice(timers.indexOf(t), 1); else t.next += t.interval;
          try { t.fn(now); } catch (error) { fail(t.script, error); }
        }
      }
    },
    __state: () => JSON.stringify(globalThis.state),
    __restore: (text) => { globalThis.state = JSON.parse(text); },
  });
})();

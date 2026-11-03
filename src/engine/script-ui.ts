// The screen UI a script can draw over the view: bars, text, a colour flash and a screen shake. A script runs in a worker with no DOM, so it sends small specs and the page
// draws them here. Every spec is checked first (`normaliseUi`): only plain colours and plain text get through, numbers are clamped, and a mistake comes back as a sentence
// that says what to fix. Elements are updated in place, so a script may redraw a bar every frame.
export type UiItem =
  | { kind: 'text'; text: string; x: number; y: number; size: number; color: string; align: 'left' | 'center' | 'right'; weight: 'normal' | 'bold'; opacity: number; shadow: boolean; font: 'sans' | 'serif' | 'mono' }
  | { kind: 'bar'; x: number; y: number; w: number; h: number; value: number; ghost: number; color: string; back: string; border: string; label: string; align: 'left' | 'center' | 'right' }
  | { kind: 'shake'; amount: number; seconds: number }
  | { kind: 'flash'; color: string; seconds: number; opacity: number };

export type UiResult = { ok: true; item: UiItem } | { ok: false; error: string };

const COLOUR = /^(#[0-9a-fA-F]{3}|#[0-9a-fA-F]{6}|#[0-9a-fA-F]{8}|rgba?\(\s*\d{1,3}\s*,\s*\d{1,3}\s*,\s*\d{1,3}\s*(,\s*(0|1|0?\.\d+)\s*)?\)|[a-zA-Z]{3,20})$/;
const NAMED_BAD = new Set(['inherit', 'initial', 'unset', 'revert', 'expression', 'url', 'var', 'calc']);
const clamp = (v: unknown, lo: number, hi: number, fallback: number) => { const n = typeof v === 'number' && Number.isFinite(v) ? v : fallback; return Math.min(hi, Math.max(lo, n)); };
const pick = <T extends string>(v: unknown, options: readonly T[], fallback: T): T => (typeof v === 'string' && (options as readonly string[]).includes(v) ? (v as T) : fallback);

function colour(v: unknown, fallback: string): string | null {
  if (v === undefined) return fallback;
  if (typeof v !== 'string' || !COLOUR.test(v) || NAMED_BAD.has(v.toLowerCase())) return null;
  return v;
}

/** Checks one spec from a script. */
export function normaliseUi(spec: unknown): UiResult {
  if (!spec || typeof spec !== 'object' || Array.isArray(spec)) return { ok: false, error: 'a UI spec is an object like {kind: "text", text: "Hello"}' };
  const s = spec as Record<string, unknown>;
  const bad = (name: string) => ({ ok: false as const, error: `${name} must be a plain color such as "#ff8800", "rgb(255, 0, 0)" or "gold"` });
  if (s.kind === 'text') {
    const color = colour(s.color, '#ffffff');
    if (color === null) return bad('color');
    return { ok: true, item: {
      kind: 'text', text: String(s.text ?? '').slice(0, 200), x: clamp(s.x, 0, 1, 0.5), y: clamp(s.y, 0, 1, 0.5), size: clamp(s.size, 8, 160, 18), color,
      align: pick(s.align, ['left', 'center', 'right'], 'center'), weight: pick(s.weight, ['normal', 'bold'], 'normal'), opacity: clamp(s.opacity, 0, 1, 1),
      shadow: s.shadow !== false, font: pick(s.font, ['sans', 'serif', 'mono'], 'sans'),
    } };
  }
  if (s.kind === 'bar') {
    const color = colour(s.color, '#cc3333'), back = colour(s.back, 'rgba(0,0,0,0.55)'), border = colour(s.border, 'rgba(255,255,255,0.35)');
    if (color === null) return bad('color');
    if (back === null) return bad('back');
    if (border === null) return bad('border');
    const value = clamp(s.value, 0, 1, 0);
    return { ok: true, item: {
      kind: 'bar', x: clamp(s.x, 0, 1, 0.5), y: clamp(s.y, 0, 1, 0.9), w: clamp(s.w, 0.01, 1, 0.2), h: clamp(s.h, 2, 60, 14), value, ghost: clamp(s.ghost, 0, 1, value),
      color, back, border, label: String(s.label ?? '').slice(0, 40), align: pick(s.align, ['left', 'center', 'right'], 'left'),
    } };
  }
  if (s.kind === 'shake') return { ok: true, item: { kind: 'shake', amount: clamp(s.amount, 0, 40, 8), seconds: clamp(s.seconds, 0.05, 3, 0.4) } };
  if (s.kind === 'flash') {
    const color = colour(s.color, '#ff0000');
    if (color === null) return bad('color');
    return { ok: true, item: { kind: 'flash', color, seconds: clamp(s.seconds, 0.05, 3, 0.35), opacity: clamp(s.opacity, 0, 1, 0.45) } };
  }
  return { ok: false, error: `the kind of a UI spec is text, bar, shake or flash (got ${JSON.stringify(s.kind)})` };
}

const FONTS = { sans: 'system-ui, sans-serif', serif: 'Georgia, "Times New Roman", serif', mono: 'ui-monospace, Menlo, monospace' };
const MAX_ELEMENTS = 64;

/** The DOM side: draws and updates the elements of every running script's UI, and tidies them when a script stops. */
export function createScriptUi(canvas: HTMLElement | null, parent: HTMLElement = document.body) {
  const root = document.createElement('div');
  root.id = 've-script-ui';
  root.style.cssText = 'position:fixed;inset:0;pointer-events:none;z-index:6;overflow:hidden;font-family:system-ui,sans-serif';
  parent.appendChild(root);
  const elements = new Map<string, { el: HTMLElement; item: UiItem; fill?: HTMLElement; ghost?: HTMLElement; label?: HTMLElement }>();
  let shakeUntil = 0;
  let shakeAmount = 0;
  let shakeFrame = 0;

  function place(el: HTMLElement, x: number, y: number, align: 'left' | 'center' | 'right') {
    el.style.left = `${x * 100}%`;
    el.style.top = `${y * 100}%`;
    el.style.transform = `translate(${align === 'center' ? '-50%' : align === 'right' ? '-100%' : '0'}, -50%)`;
  }

  function draw(key: string, item: UiItem) {
    let entry = elements.get(key);
    if (item.kind === 'text') {
      if (!entry || entry.item.kind !== 'text') {
        entry?.el.remove();
        if (!entry && elements.size >= MAX_ELEMENTS) return { ok: false as const, error: `at most ${MAX_ELEMENTS} UI elements; remove one first` };
        const el = document.createElement('div');
        el.style.cssText = 'position:absolute;white-space:pre;line-height:1.1';
        root.appendChild(el);
        entry = { el, item };
        elements.set(key, entry);
      }
      const { el } = entry;
      el.textContent = item.text;
      Object.assign(el.style, { fontSize: `${item.size}px`, color: item.color, fontWeight: item.weight, opacity: String(item.opacity), fontFamily: FONTS[item.font], textShadow: item.shadow ? '0 2px 6px rgba(0,0,0,0.85)' : 'none' });
      place(el, item.x, item.y, item.align);
      entry.item = item;
    } else if (item.kind === 'bar') {
      if (!entry || entry.item.kind !== 'bar') {
        entry?.el.remove();
        if (!entry && elements.size >= MAX_ELEMENTS) return { ok: false as const, error: `at most ${MAX_ELEMENTS} UI elements; remove one first` };
        const el = document.createElement('div');
        el.style.cssText = 'position:absolute;box-sizing:border-box;border-radius:3px;overflow:hidden';
        const ghost = document.createElement('div');
        ghost.style.cssText = 'position:absolute;left:0;top:0;bottom:0;background:rgba(255,255,255,0.55);transition:width .5s ease-out';
        const fill = document.createElement('div');
        fill.style.cssText = 'position:absolute;left:0;top:0;bottom:0;transition:width .08s linear';
        const label = document.createElement('div');
        label.style.cssText = 'position:absolute;inset:0;display:flex;align-items:center;padding:0 8px;font:600 11px system-ui,sans-serif;color:#fff;text-shadow:0 1px 3px #000;letter-spacing:.06em';
        el.append(ghost, fill, label);
        root.appendChild(el);
        entry = { el, item, fill, ghost, label };
        elements.set(key, entry);
      }
      const { el, fill, ghost, label } = entry;
      Object.assign(el.style, { width: `${item.w * 100}%`, height: `${item.h}px`, background: item.back, border: `1px solid ${item.border}` });
      place(el, item.x, item.y, item.align);
      if (fill) { fill.style.width = `${item.value * 100}%`; fill.style.background = item.color; }
      if (ghost) ghost.style.width = `${Math.max(item.ghost, item.value) * 100}%`;
      if (label) label.textContent = item.label;
      entry.item = item;
    }
    return { ok: true as const };
  }

  function shake(amount: number, seconds: number) {
    if (!canvas) return;
    shakeAmount = Math.max(shakeAmount, amount);
    shakeUntil = performance.now() + seconds * 1000;
    const span = seconds * 1000;
    if (shakeFrame) return;
    const step = () => {
      const left = shakeUntil - performance.now();
      if (left <= 0) { canvas.style.transform = ''; shakeAmount = 0; shakeFrame = 0; return; }
      const k = Math.min(1, left / Math.max(span, 1)) * shakeAmount;
      canvas.style.transform = `translate(${(Math.random() * 2 - 1) * k}px, ${(Math.random() * 2 - 1) * k}px)`;
      shakeFrame = requestAnimationFrame(step);
    };
    shakeFrame = requestAnimationFrame(step);
  }

  function flash(color: string, seconds: number, opacity: number) {
    const el = document.createElement('div');
    el.style.cssText = `position:absolute;inset:0;background:${color};opacity:${opacity};transition:opacity ${seconds}s ease-out`;
    root.appendChild(el);
    requestAnimationFrame(() => requestAnimationFrame(() => { el.style.opacity = '0'; }));
    setTimeout(() => el.remove(), seconds * 1000 + 100);
  }

  return {
    /** One message from a script: {op: 'set' | 'remove' | 'clear' | 'shake' | 'flash', id?, spec?}. Returns an error sentence or null. */
    apply(script: string, message: { op: string; id?: unknown; spec?: unknown }): string | null {
      const prefix = `${script}:`;
      if (message.op === 'clear') { for (const [key, e] of [...elements]) if (key.startsWith(prefix)) { e.el.remove(); elements.delete(key); } return null; }
      if (message.op === 'remove') { const key = prefix + String(message.id); elements.get(key)?.el.remove(); elements.delete(key); return null; }
      if (message.op === 'set' || message.op === 'shake' || message.op === 'flash') {
        const spec = message.op === 'set' ? message.spec : { ...(message.spec as object), kind: message.op };
        const checked = normaliseUi(spec);
        if (!checked.ok) return checked.error;
        const { item } = checked;
        if (item.kind === 'shake') { shake(item.amount, item.seconds); return null; }
        if (item.kind === 'flash') { flash(item.color, item.seconds, item.opacity); return null; }
        if (!/^[a-z][a-z0-9_]{0,31}$/.test(String(message.id))) return 'a UI element id is lowercase letters, digits and underscores, starting with a letter (32 at most)';
        const drawn = draw(prefix + String(message.id), item);
        return drawn.ok ? null : drawn.error;
      }
      return 'ui.set, ui.remove, ui.clear, ui.shake and ui.flash are the UI calls';
    },
    /** Removes everything a script drew (it stopped or restarted). */
    clearScript(script: string) { this.apply(script, { op: 'clear' }); },
    elements: () => elements.size,
    root,
  };
}

export type ScriptUi = ReturnType<typeof createScriptUi>;

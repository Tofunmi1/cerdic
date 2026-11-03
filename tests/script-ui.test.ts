// The screen UI a script can draw (health bars, text, flashes, shake): what a spec may say, checked before anything touches the DOM, so a script cannot inject styles or
// markup and a typo gets an error that says what to fix. Written before the code; do not weaken.
import { expect, test } from 'bun:test';
import { normaliseUi } from '../src/engine/script-ui';

test('a text spec is completed with defaults and clamped to the screen', () => {
  const r = normaliseUi({ kind: 'text', text: 'VICTORY', x: 1.7, y: -3, size: 900 });
  expect(r.ok).toBe(true);
  if (!r.ok) return;
  expect(r.item).toMatchObject({ kind: 'text', text: 'VICTORY', x: 1, y: 0, size: 160, align: 'center', shadow: true });
});

test('a bar spec keeps its value between 0 and 1 and its width a share of the screen', () => {
  const r = normaliseUi({ kind: 'bar', x: 0.1, y: 0.9, w: 0.4, h: 18, value: 1.4, color: '#c33', label: 'HP' });
  expect(r.ok).toBe(true);
  if (!r.ok) return;
  expect(r.item).toMatchObject({ kind: 'bar', value: 1, w: 0.4, h: 18, color: '#c33', label: 'HP' });
  const low = normaliseUi({ kind: 'bar', value: -2 });
  expect(low.ok && low.item.kind === 'bar' && low.item.value).toBe(0);
});

test('only plain colours are accepted, so a script cannot smuggle in other CSS', () => {
  for (const color of ['red; position:absolute', 'url(javascript:alert(1))', 'expression(1)', '#12', '', 'var(--x)']) {
    const r = normaliseUi({ kind: 'text', text: 'x', color });
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.error).toMatch(/color/);
  }
  for (const color of ['#fff', '#ffcc00', '#ffcc0080', 'rgb(255, 0, 0)', 'rgba(0,0,0,0.5)', 'white', 'gold']) expect(normaliseUi({ kind: 'text', text: 'x', color }).ok).toBe(true);
});

test('text is plain text, capped in length, and an unknown kind says what the kinds are', () => {
  const long = normaliseUi({ kind: 'text', text: 'a'.repeat(500) });
  expect(long.ok && long.item.kind === 'text' && long.item.text.length).toBe(200);
  const html = normaliseUi({ kind: 'text', text: '<img src=x onerror=alert(1)>' });
  expect(html.ok && html.item.kind === 'text' && html.item.text).toBe('<img src=x onerror=alert(1)>'); // kept as text; the renderer uses textContent
  const bad = normaliseUi({ kind: 'sprite' });
  expect(bad.ok).toBe(false);
  if (!bad.ok) expect(bad.error).toMatch(/text.*bar|bar.*text/);
});

test('shake and flash are bounded in size and time', () => {
  const s = normaliseUi({ kind: 'shake', amount: 500, seconds: 60 });
  expect(s.ok && s.item).toMatchObject({ kind: 'shake', amount: 40, seconds: 3 });
  const f = normaliseUi({ kind: 'flash', color: '#f00', seconds: -1, opacity: 5 });
  expect(f.ok && f.item).toMatchObject({ kind: 'flash', color: '#f00', seconds: 0.05, opacity: 1 });
});

test('anything that is not an object is refused', () => {
  for (const spec of [null, 'text', 4, [], undefined]) expect(normaliseUi(spec).ok).toBe(false);
});

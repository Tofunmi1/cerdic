// Cast QA and model sheets. Evaluates cast.txt in the real page, then
//  1. rasterises every character's silhouette (front and side) from its own geometry, at true size and normalised to its height, and compares every pair
//     (two characters whose silhouettes overlap too much fail the "reads at a glance" test),
//  2. checks each character's measured height against the proportion canon of the style bible,
//  3. renders a four-view model sheet (front, three-quarter, side, back) per character into out/<name>.png and writes out/qa.json.
// Usage: bun engine/web/tests/scenes/characters/run-qa.mjs
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { chromium } from 'playwright';
import { PNG } from 'pngjs';

const here = new URL('./', import.meta.url).pathname;
const root = new URL('../../../../', import.meta.url).pathname; // engine/
const out = `${here}out/`;
mkdirSync(out, { recursive: true });

// name, x, canon height in metres (head to toe, ignoring horns, hats and crests), camera distance factor
const CAST = [
  { name: 'warrior', x: -9.1, canon: 1.8, dist: 3.0 },
  { name: 'wizard', x: -6.5, canon: 1.75, dist: 3.4 },
  { name: 'priest', x: -3.9, canon: 1.78, dist: 3.4 },
  { name: 'rogue', x: -1.3, canon: 1.75, dist: 3.0 },
  { name: 'ranger', x: 1.3, canon: 1.95, dist: 3.0 },
  { name: 'dwarf', x: 3.9, canon: 1.35, dist: 3.0 },
  { name: 'orc', x: 6.5, canon: 2.1, dist: 3.0 },
  { name: 'dragon', x: 13.5, canon: 3.4, dist: 3.4 },
];
const IOU_LIMIT = 0.72; // two silhouettes sharing more than this of their combined area read as the same character

const child = Bun.spawn(['node', 'tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5270', '--dir', 'web/dist-bevy-gpu'], { cwd: root, stdout: 'ignore', stderr: 'ignore' });
const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || 'chromium', args: ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
try {
  for (let i = 0; i < 100; i++) { try { if ((await fetch('http://127.0.0.1:5270')).ok) break; } catch {} await Bun.sleep(100); }
  const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('http://127.0.0.1:5270/?viewer=1&eval=1&docname=cast');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate((text) => window.__ve.doc.sendText(text, 'cast'), readFileSync(`${here}cast.txt`, 'utf8'));
  if (!staged.pending) throw new Error(`not staged: ${JSON.stringify(staged).slice(0, 500)}`);
  await page.waitForFunction(() => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent, null, { timeout: 600000 });
  const failure = await page.evaluate(() => window.__ve.evaluation.state.lastFailure);
  if (failure) throw new Error(`evaluation failed: ${JSON.stringify(failure).slice(0, 500)}`);

  // ---- 1 and 2: silhouettes and heights, computed in the page from the evaluated triangles ----
  const names = CAST.map((c) => c.name);
  const measured = await page.evaluate((names) => {
    const W = 640, Hpx = 256, PER_M = 56; // 56 px per metre: a 4.5 m silhouette fits the 256 px height
    const parts = window.__ve.evaluation.parts;
    const result = {};
    for (const name of names) {
      const mine = parts.filter((p) => p.name === `${name}_body` || p.name.startsWith(`${name}_`));
      const lo = [1e9, 1e9, 1e9], hi = [-1e9, -1e9, -1e9];
      for (const p of mine) for (let i = 0; i < p.positions.length; i += 3) for (let a = 0; a < 3; a++) { lo[a] = Math.min(lo[a], p.positions[i + a]); hi[a] = Math.max(hi[a], p.positions[i + a]); }
      // the body alone, for the proportion canon (hats, horns, crests and wings do not count)
      const blo = [1e9, 1e9, 1e9], bhi = [-1e9, -1e9, -1e9];
      for (const p of parts.filter((q) => q.name === `${name}_body`)) for (let i = 0; i < p.positions.length; i += 3) for (let a = 0; a < 3; a++) { blo[a] = Math.min(blo[a], p.positions[i + a]); bhi[a] = Math.max(bhi[a], p.positions[i + a]); }
      const draw = (axis, normalise) => {
        const canvas = new OffscreenCanvas(W, Hpx);
        const g = canvas.getContext('2d');
        g.fillStyle = '#fff'; g.fillRect(0, 0, W, Hpx);
        g.fillStyle = '#000';
        const horizontal = axis === 'front' ? 0 : 2; // x for the front view, z for the side
        const height = hi[1] - lo[1];
        const scale = normalise ? (Hpx - 8) / height : PER_M;
        const centre = (lo[horizontal] + hi[horizontal]) / 2;
        for (const p of mine) {
          for (let t = 0; t < p.indices.length; t += 3) {
            g.beginPath();
            for (let k = 0; k < 3; k++) {
              const v = p.indices[t + k] * 3;
              const px = W / 2 + (p.positions[v + horizontal] - centre) * scale * (axis === 'side' ? -1 : 1);
              const py = Hpx - 4 - (p.positions[v + 1] - lo[1]) * scale;
              if (k === 0) g.moveTo(px, py); else g.lineTo(px, py);
            }
            g.closePath(); g.fill();
          }
        }
        const data = g.getImageData(0, 0, W, Hpx).data;
        const bits = new Uint8Array(W * Hpx);
        for (let i = 0; i < bits.length; i++) bits[i] = data[i * 4] < 128 ? 1 : 0;
        return bits;
      };
      result[name] = { lo, hi, blo, bhi, true_front: draw('front', false), true_side: draw('side', false), norm_front: draw('front', true), norm_side: draw('side', true) };
    }
    // pairwise intersection-over-union of the dark pixels
    const iou = (a, b) => { let i = 0, u = 0; for (let k = 0; k < a.length; k++) { const x = a[k], y = b[k]; if (x & y) i++; if (x | y) u++; } return u ? i / u : 0; };
    const pairs = [];
    for (let a = 0; a < names.length; a++) for (let b = a + 1; b < names.length; b++) {
      const A = result[names[a]], B = result[names[b]];
      pairs.push({ a: names[a], b: names[b], true_front: iou(A.true_front, B.true_front), true_side: iou(A.true_side, B.true_side), norm_front: iou(A.norm_front, B.norm_front), norm_side: iou(A.norm_side, B.norm_side) });
    }
    return { boxes: Object.fromEntries(names.map((n) => [n, { lo: result[n].lo, hi: result[n].hi, body_height: result[n].bhi[1] - result[n].blo[1] }])), pairs };
  }, names);

  const report = { generated: 'cast.txt', iou_limit: IOU_LIMIT, characters: [], pairs: [], failures: [] };
  for (const c of CAST) {
    const { lo, hi } = measured.boxes[c.name];
    const size = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    const body = measured.boxes[c.name].body_height;
    report.characters.push({ name: c.name, canon_height: c.canon, body_height: +body.toFixed(3), overall_height: +size[1].toFixed(3), width: +size[0].toFixed(3), depth: +size[2].toFixed(3), on_floor: Math.abs(lo[1]) < 0.06 });
    // the body (without hats, horns and wings) should be within 12 percent of the canon
    if (Math.abs(body - c.canon) / c.canon > 0.12) report.failures.push(`${c.name}'s body is ${body.toFixed(2)} m, canon ${c.canon} m`);
    if (Math.abs(lo[1]) >= 0.06) report.failures.push(`${c.name} does not stand on the floor (lowest point y=${lo[1].toFixed(3)})`);
  }
  for (const p of measured.pairs) {
    const worst = Math.max(p.norm_front, p.norm_side);
    report.pairs.push({ ...p, worst_shape_overlap: +worst.toFixed(3), reads_apart: worst <= IOU_LIMIT && Math.max(p.true_front, p.true_side) <= IOU_LIMIT });
    if (worst > IOU_LIMIT) report.failures.push(`${p.a} and ${p.b} are too alike as shapes (overlap ${worst.toFixed(2)} > ${IOU_LIMIT})`);
    else if (Math.max(p.true_front, p.true_side) > IOU_LIMIT) report.failures.push(`${p.a} and ${p.b} are too alike at true size (overlap ${Math.max(p.true_front, p.true_side).toFixed(2)} > ${IOU_LIMIT})`);
  }
  report.pairs.sort((a, b) => b.worst_shape_overlap - a.worst_shape_overlap);

  // ---- 3: model sheets ----
  // Each sheet is its own document with only that character (the header of cast.txt: sky and floor, plus the character's own lines), so no neighbour is in the way.
  const source = readFileSync(`${here}cast.txt`, 'utf8').split('\n');
  const header = source.slice(0, source.findIndex((l) => l.startsWith('person('))).join('\n');
  const dragonAt = source.findIndex((l) => l.includes('the dragon:'));
  const variantOf = (name) => `${header}\n${name === 'dragon' ? source.slice(dragonAt).join('\n') : source.filter((l) => (l.startsWith('person(') || l.startsWith('kit(')) && l.includes(`"${name}"`)).join('\n')}\n`;
  for (const c of CAST) {
    const sp = await browser.newPage({ viewport: { width: 1280, height: 720 } });
    await sp.goto('http://127.0.0.1:5270/?viewer=1&eval=1&docname=sheet');
    await sp.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
    await sp.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
    const sent = await sp.evaluate((text) => window.__ve.doc.sendText(text, 'sheet'), variantOf(c.name));
    if (!sent.pending) throw new Error(`${c.name} sheet not staged: ${JSON.stringify(sent).slice(0, 300)}`);
    await sp.waitForFunction(() => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent, null, { timeout: 600000 });
    await sp.waitForTimeout(1200);
    await sp.addStyleTag({ content: 'body > *:not(canvas) { display: none !important; }' });
    const { lo, hi } = measured.boxes[c.name];
    const H = hi[1] - lo[1], cx = (lo[0] + hi[0]) / 2, cz = (lo[2] + hi[2]) / 2, mid = H * 0.5, d = Math.max(c.dist * H * 0.62, (hi[0] - lo[0]) * 0.8 + 1);
    const views = [
      ['front', [cx, mid, cz + d]],
      ['three-quarter', [cx + d * 0.72, mid * 1.15, cz + d * 0.72]],
      ['side', [cx + d, mid, cz]],
      ['back', [cx, mid, cz - d]],
    ];
    const panels = [];
    for (const [view, eye] of views) {
      await sp.evaluate(([e, t]) => window.__runtime.viewer_look_at(e[0], e[1], e[2], t[0], t[1], t[2]), [eye, [cx, mid, cz]]);
      await sp.waitForTimeout(900);
      panels.push(PNG.sync.read(await sp.screenshot()));
    }
    await sp.close();
    // a 2 x 2 sheet at half resolution
    const w = 640, h = 360, sheet = new PNG({ width: w * 2, height: h * 2 });
    panels.forEach((png, index) => {
      const ox = (index % 2) * w, oy = Math.floor(index / 2) * h;
      for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
        const s = ((y * 2) * png.width + x * 2) * 4, o = ((oy + y) * sheet.width + ox + x) * 4;
        sheet.data[o] = png.data[s]; sheet.data[o + 1] = png.data[s + 1]; sheet.data[o + 2] = png.data[s + 2]; sheet.data[o + 3] = 255;
      }
    });
    writeFileSync(`${out}${c.name}.png`, PNG.sync.write(sheet));
  }
  // the whole cast, front on
  await page.addStyleTag({ content: 'body > *:not(canvas) { display: none !important; }' });
  await page.evaluate(() => window.__runtime.viewer_look_at(2.2, 1.6, 15.5, 2.2, 1.05, 0));
  await page.waitForTimeout(900);
  writeFileSync(`${out}cast-lineup.png`, await page.screenshot());

  report.console_errors = errors;
  writeFileSync(`${out}qa.json`, JSON.stringify(report, null, 2));
  console.log('characters (canon -> measured height, m):');
  for (const c of report.characters) console.log(`  ${c.name.padEnd(8)} canon ${c.canon_height.toFixed(2)}  body ${c.body_height.toFixed(2)}  overall ${c.overall_height.toFixed(2)}  width ${c.width.toFixed(2)}  floor ${c.on_floor}`);
  console.log('most alike pairs (shape overlap, lower is more distinct):');
  for (const p of report.pairs.slice(0, 6)) console.log(`  ${p.a} / ${p.b}: ${p.worst_shape_overlap}  (true size front ${p.true_front.toFixed(2)} side ${p.true_side.toFixed(2)})`);
  console.log(report.failures.length ? `QA FAILURES:\n  ${report.failures.join('\n  ')}` : 'QA: every pair reads apart and everyone stands on the floor');
  console.log('console errors:', errors.length ? errors.slice(0, 3) : 'none');
} finally { await browser.close(); child.kill(); await child.exited; }

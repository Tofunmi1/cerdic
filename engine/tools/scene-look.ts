// One command for the model's edit-and-look loop: evaluate a scene file natively, check it for errors, and write a contact sheet (top, front, side, three-quarter,
// plus an optional close-up) at full resolution, with a part list and bounds. No browser, no page reload, a few seconds.
//
//   bun engine/tools/scene-look.ts web/tests/scenes/creatures/bird.txt /tmp/look                      (sheet at /tmp/look/sheet.png)
//   bun engine/tools/scene-look.ts web/tests/scenes/creatures/bird.txt /tmp/look --focus 0,0.1,0.37 --extent 0.3   (adds close-ups of that spot)
//
// Needs ./target/release/examples/dump_obj (cargo build --release --example dump_obj -p ve-evaluator, if missing).
import { mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { PNG } from '../web/node_modules/pngjs/lib/png.js';

const [scene, outDir, ...rest] = process.argv.slice(2);
if (!scene || !outDir) {
  console.error('usage: bun engine/tools/scene-look.ts <scene.txt> <out dir> [--focus x,y,z --extent m] [--size 700]');
  process.exit(2);
}
const opt = (name: string, fallback: string) => { const i = rest.indexOf(`--${name}`); return i >= 0 ? rest[i + 1] : fallback; };
const size = Number(opt('size', '700'));
const focus = opt('focus', '');
const extent = opt('extent', '0.3');
mkdirSync(outDir, { recursive: true });
const objDir = `${outDir}/obj`;

const run = (cmd: string[]) => {
  const r = Bun.spawnSync(cmd, { cwd: new URL('..', import.meta.url).pathname });
  return { ok: r.exitCode === 0, out: `${r.stdout}${r.stderr}` };
};
const dump = run(['./target/release/examples/dump_obj', scene, objDir]);
console.log(dump.out.split('\n').filter((l) => /error|warn|fail|rror/i.test(l)).join('\n') || 'evaluated: no errors');
if (!dump.ok) { console.error(dump.out.slice(-1500)); process.exit(1); }

const views: [string, string[]][] = [
  ['top', ['--az', '0', '--el', '90']],
  ['front', ['--az', '0', '--el', '4']],
  ['side', ['--az', '90', '--el', '4']],
  ['three-quarter', ['--az', '40', '--el', '22']],
];
if (focus) {
  views.push(['close side', ['--az', '90', '--el', '8', '--center', focus, '--extent', extent]]);
  views.push(['close three-quarter', ['--az', '35', '--el', '15', '--center', focus, '--extent', extent]]);
}
const tiles: PNG[] = [];
for (const [name, args] of views) {
  const file = `${outDir}/${name.replace(/ /g, '_')}.png`;
  const r = run(['bun', 'tools/render-obj.ts', objDir, file, ...args, '--size', String(size)]);
  if (!r.ok) { console.error(r.out); process.exit(1); }
  tiles.push(PNG.sync.read(readFileSync(file)));
}
// a grid, two columns; every tile padded to the tallest in its row
const cols = 2;
const rows = Math.ceil(tiles.length / cols);
const rowH = Array.from({ length: rows }, (_, r) => Math.max(...tiles.slice(r * cols, r * cols + cols).map((t) => t.height)));
const sheet = new PNG({ width: size * cols, height: rowH.reduce((a, b) => a + b, 0) });
sheet.data.fill(255);
let y0 = 0;
for (let r = 0; r < rows; r++) {
  for (let c = 0; c < cols; c++) {
    const t = tiles[r * cols + c];
    if (!t) continue;
    PNG.bitblt(t, sheet, 0, 0, t.width, t.height, c * size, y0);
  }
  y0 += rowH[r];
}
writeFileSync(`${outDir}/sheet.png`, PNG.sync.write(sheet));

// bounds and triangle counts per part
let total = 0;
const lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
const lines: string[] = [];
for (const f of readdirSync(objDir).filter((n) => n.endsWith('.obj')).sort()) {
  let tri = 0;
  for (const line of readFileSync(`${objDir}/${f}`, 'utf8').split('\n')) {
    if (line.startsWith('v ')) { const p = line.split(/\s+/).slice(1).map(Number); for (let i = 0; i < 3; i++) { lo[i] = Math.min(lo[i], p[i]); hi[i] = Math.max(hi[i], p[i]); } }
    else if (line.startsWith('f ')) tri += line.trim().split(/\s+/).length - 3;
  }
  total += tri;
  lines.push(`${f.replace('.obj', '').padEnd(20)} ${tri}`);
}
console.log(lines.join('\n'));
console.log(`total ${total} triangles; bounds x ${lo[0].toFixed(2)}..${hi[0].toFixed(2)}  y ${lo[1].toFixed(2)}..${hi[1].toFixed(2)}  z ${lo[2].toFixed(2)}..${hi[2].toFixed(2)}`);
console.log(`views: ${views.map((v) => v[0]).join(', ')}\nsheet: ${outDir}/sheet.png`);

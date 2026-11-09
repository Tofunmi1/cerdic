// Dimension audit for the frigate (after USS Constitution): reads the OBJ files `dump_obj` wrote and checks the measured mesh against the ship's real dimensions.
//
//   ./target/release/examples/dump_obj web/tests/scenes/ship/ship.txt /tmp/ship_obj
//   bun engine/tools/audit-ship.ts /tmp/ship_obj
//
// Real figures (Wikipedia, US Navy fact file): length on deck 62 m, waterline 53 m, beam 13.26 m, draft 6.4 m forward and 7.0 m aft, 15 long guns a side, masts 60.3 / 67.0 / 52.6 m,
// bowsprit to spanker boom 93 m, spar deck 4.9 m above the water (derived: 172 ft deck to main truck against 210 ft keel to truck). Mast heights and the yard lengths are
// reconstructed from model plans, so those rules carry wider tolerances. Each failing line names the value in the scene's parameter table or loft to change.
import { existsSync, readFileSync } from 'node:fs';

type V = [number, number, number];
const dir = process.argv[2];
if (!dir) {
  console.error('usage: bun engine/tools/audit-ship.ts <obj dir>');
  process.exit(2);
}
const has = (name: string) => existsSync(`${dir}/${name}.obj`);
function readObj(name: string): V[] {
  const out: V[] = [];
  for (const line of readFileSync(`${dir}/${name}.obj`, 'utf8').split('\n')) {
    if (line.startsWith('v ')) {
      const [, x, y, z] = line.trim().split(/\s+/);
      out.push([Number(x), Number(y), Number(z)]);
    }
  }
  return out;
}
const bounds = (pts: V[]) => {
  const lo = [Infinity, Infinity, Infinity];
  const hi = [-Infinity, -Infinity, -Infinity];
  for (const p of pts) for (let i = 0; i < 3; i++) { lo[i] = Math.min(lo[i], p[i]); hi[i] = Math.max(hi[i], p[i]); }
  return { lo, hi };
};

const hull = ['hull_copper', 'hull_black_low', 'hull_ochre', 'hull_black_high'].flatMap(readObj);
const deck = readObj('deck');
const hb = bounds(hull);
const length = hb.hi[2] - hb.lo[2];
const waterline = hull.filter((p) => Math.abs(p[1]) < 0.06 && Math.abs(p[0]) > 0.02);
const wlb = bounds(waterline);
const beam = hb.hi[0] - hb.lo[0];
const wlBeam = bounds(hull.filter((p) => Math.abs(p[1]) < 0.12 && Math.abs(p[2]) < 3));
const draftAft = -Math.min(...hull.filter((p) => p[2] < -10).map((p) => p[1]));
const draftFwd = -Math.min(...hull.filter((p) => p[2] > 10 && p[2] < 22).map((p) => p[1]));
const midDeck = deck.filter((p) => Math.abs(p[2]) < 1.5);
const deckY = Math.max(...midDeck.map((p) => p[1]));
const railMid = Math.max(...hull.filter((p) => Math.abs(p[2]) < 1.5).map((p) => p[1]));
const guns = readObj('guns').length / 20;

type Rule = { name: string; value: number; lo: number; hi: number; unit: string; hint: string };
const rules: Rule[] = [
  { name: 'length on deck (stem to taffrail)', value: length, lo: 61, hi: 64, unit: 'm', hint: 'the loft stations (-29 to 33) and the bow and stern keys' },
  { name: 'length at the waterline', value: wlb.hi[2] - wlb.lo[2], lo: 51, hi: 56, unit: 'm', hint: 'yK: where the keel line meets y = 0 at the stern (-26.5) and the bow (26.8)' },
  { name: 'maximum beam', value: beam, lo: 12.8, hi: 13.8, unit: 'm', hint: 'beam' },
  { name: 'beam at the waterline amidships', value: wlBeam.hi[0] - wlBeam.lo[0], lo: 12.6, hi: 13.4, unit: 'm', hint: 'beam and the waterline point of the section (S)' },
  { name: 'length to beam ratio', value: length / beam, lo: 4.4, hi: 5.0, unit: '', hint: 'beam or the loft length' },
  { name: 'draft aft', value: draftAft, lo: 6.7, hi: 7.3, unit: 'm', hint: 'draft_aft' },
  { name: 'draft forward', value: draftFwd, lo: 6.1, hi: 6.7, unit: 'm', hint: 'draft_fwd' },
  { name: 'spar deck above the water amidships', value: deckY, lo: 4.7, hi: 5.1, unit: 'm', hint: 'freeboard' },
  { name: 'bulwark height above the deck', value: railMid - deckY, lo: 1.0, hi: 1.4, unit: 'm', hint: 'railY offset (1.15) in the jsh code' },
  { name: 'long guns on the gun deck (both sides)', value: guns, lo: 30, hi: 30, unit: '', hint: 'the loop count (15 a side) in ports_body and guns_body' },
  { name: 'left-right symmetry error of the hull', value: Math.abs(hb.hi[0] + hb.lo[0]), lo: 0, hi: 0.02, unit: 'm', hint: 'a loft or cut is not mirrored' },
];

// ---- the rig, when it exists ----
if (has('mast_main')) {
  const top = (n: string) => Math.max(...readObj(n).map((p) => p[1]));
  const mz = (n: string) => { const b = bounds(readObj(n)); return (b.lo[2] + b.hi[2]) / 2; };
  rules.push(
    { name: 'main mast truck above the waterline', value: top('mast_main'), lo: 55.5, hi: 59, unit: 'm', hint: 'main_h' },
    { name: 'fore mast truck above the waterline', value: top('mast_fore'), lo: 49.8, hi: 53.4, unit: 'm', hint: 'fore_h (0.9 of the main)' },
    { name: 'mizzen mast truck above the waterline', value: top('mast_mizzen'), lo: 43.2, hi: 46.6, unit: 'm', hint: 'mizzen_h (0.784 of the main)' },
    { name: 'foremast to mainmast spacing', value: mz('mast_fore') - mz('mast_main'), lo: 16, hi: 22, unit: 'm', hint: 'mast z positions (estimated from plans)' },
    { name: 'mainmast to mizzenmast spacing', value: mz('mast_main') - mz('mast_mizzen'), lo: 14, hi: 20, unit: 'm', hint: 'mast z positions (estimated from plans)' },
  );
  if (has('bowsprit')) {
    const bs = bounds(readObj('bowsprit'));
    const boom = has('spanker_boom') ? bounds(readObj('spanker_boom')) : null;
    if (boom) rules.push({ name: 'bowsprit tip to spanker boom end', value: bs.hi[2] - boom.lo[2], lo: 88, hi: 98, unit: 'm', hint: 'bowsprit length or spanker boom length' });
  }
  if (has('yards')) {
    const yb = bounds(readObj('yards'));
    rules.push({ name: 'widest yard (main course) against the beam', value: (yb.hi[0] - yb.lo[0]) / beam, lo: 1.8, hi: 2.3, unit: '', hint: 'the course yard widths in the yard macro calls' });
  }
}

console.log(`measured hull: z ${hb.lo[2].toFixed(2)}..${hb.hi[2].toFixed(2)}, x ${hb.lo[0].toFixed(2)}..${hb.hi[0].toFixed(2)}, y ${hb.lo[1].toFixed(2)}..${hb.hi[1].toFixed(2)}`);
let failed = 0;
for (const r of rules) {
  const ok = r.value >= r.lo && r.value <= r.hi;
  if (!ok) failed++;
  const shown = r.unit === '' ? r.value.toFixed(3) : r.value.toFixed(2);
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${r.name.padEnd(48)} ${shown.padStart(8)}${r.unit}  target ${r.lo}-${r.hi}${r.unit}${ok ? '' : `  ${r.value < r.lo ? 'too low' : 'too high'}: change ${r.hint}`}`);
}
console.log(failed === 0 ? 'ALL RULES PASS' : `${failed} of ${rules.length} rules fail`);
process.exit(failed === 0 ? 0 : 1);

// Throwaway: classify the 53 spike programs against the G3 operator set and translate the ones
// that can run into the engine's text language. No spike code is imported into the engine; the
// output is plain text files plus a table, with the spike evaluator's volume and size recorded as
// the reference to compare against.
import fs from 'node:fs';
import { compile } from './lang.mjs';
import { evaluate } from './eval.mjs';

const outDir = process.argv[2];
fs.mkdirSync(outDir, { recursive: true });

const blocks = ['runs/batch1.txt', 'runs/batch2.txt'].flatMap((f) =>
  fs.readFileSync(f, 'utf8').split(/^=== /m).filter(Boolean).map((b) => {
    const nl = b.indexOf('\n');
    return { title: b.slice(0, nl).trim(), source: b.slice(nl + 1) };
  }));

const SUPPORTED = new Set(['box', 'sphere', 'cylinder', 'union', 'difference', 'intersect', 'move', 'rotate', 'scale', 'polygon', 'rect', 'circle', 'extrude', 'revolve']);
const CHANGED = new Set(['part', 'material']);
const DEFERRED_WHY = {
  mirror: 'mirror needs the instance and symmetry operators (G4)', array: 'grid instancing arrives with groups and instances (G4)', repeat: 'linear and spiral repetition is G4',
  radial: 'radial repetition is G4', scatter: 'scatter and merge are instance operators (G4)', merge: 'scatter and merge are instance operators (G4)', socket: 'sockets are semantic parts (G4)',
  cone: 'cone is not yet a conformance-tested primitive; add with the next primitive batch', torus: 'torus is not yet a conformance-tested primitive; revolve a circle profile instead',
  capsule: 'capsule is not yet a conformance-tested primitive', sweep: 'sweep needs curves and frames (G5)', loft: 'loft needs curves and frames (G5)', spline: 'splines are G5',
  sd_sphere: 'implicit surfaces and meshing are G8', sd_box: 'implicit surfaces and meshing are G8', sd_capsule: 'implicit surfaces and meshing are G8', sd_union: 'implicit surfaces and meshing are G8',
  sd_subtract: 'implicit surfaces and meshing are G8', sd_noise: 'implicit surfaces and meshing are G8', sd_expr: 'implicit surfaces and meshing are G8', mesh_sdf: 'implicit surfaces and meshing are G8',
  bevel: 'bevel and offset on solids are not in the kernel', smooth: 'smoothing is G7', simplify: 'simplify is not scheduled', hollow: 'shell and hollow solids are not scheduled; use difference of offset profiles',
};

const f = (n) => Number(n.toPrecision(15)).toString();
const list = (v) => `[${v.map(f).join(', ')}]`;
const rad = (d) => (d * Math.PI) / 180;

function translate(c) {
  const alias = new Map(); const lines = []; let outName = 'result';
  const name = (n) => alias.get(n) ?? n;
  for (const n of c.nodes) {
    const p = { ...Object.fromEntries(Object.entries(n.spec.params).map(([k, [, d]]) => [k, typeof d === 'symbol' ? undefined : d])), ...n.params };
    const ins = n.inputs.map(name);
    const stmt = (op, args) => lines.push(`${n.name} = ${op}(${args.join(', ')})`);
    switch (n.kind) {
      case 'part': alias.set(n.name, ins[0]); outName = p.name.replace(/[^A-Za-z0-9_]/g, '_'); break;
      case 'material': alias.set(n.name, ins[0]); break;
      case 'box': stmt('solid.box@1', [`size=${list(p.size)}`]); break;
      case 'sphere': stmt('solid.sphere@1', [`radius=${f(p.radius)}`, `segments=${p.segments}`]); break;
      case 'cylinder': {
        const base = p.axis === 'y' ? n.name : `${n.name}_y`;
        lines.push(`${base} = solid.cylinder@1(radius=${f(p.radius)}, height=${f(p.height)}, segments=${p.segments})`);
        if (p.axis === 'z') lines.push(`${n.name} = xform.rotate@1(${base}, angles=[90, 0, 0])`);
        if (p.axis === 'x') lines.push(`${n.name} = xform.rotate@1(${base}, angles=[0, 0, -90])`);
        break;
      }
      case 'union': stmt('solid.union@1', ins); break;
      case 'difference': stmt('solid.difference@1', ins); break;
      case 'intersect': stmt('solid.intersect@1', ins); break;
      case 'move': stmt('xform.move@1', [...ins, `by=${list(p.by)}`]); break;
      case 'rotate': stmt('xform.rotate@1', [...ins, `angles=${list(p.degrees)}`]); break;
      case 'scale': stmt('xform.scale@1', [...ins, `by=${list(p.by)}`]); break;
      case 'polygon': stmt('profile.polygon@1', [`points=[${p.points.map((q) => list(q)).join(', ')}]`]); break;
      case 'rect': stmt('profile.polygon@1', [`points=[${[[-p.size[0] / 2, -p.size[1] / 2], [p.size[0] / 2, -p.size[1] / 2], [p.size[0] / 2, p.size[1] / 2], [-p.size[0] / 2, p.size[1] / 2]].map(list).join(', ')}]`]); break;
      case 'circle': stmt('profile.polygon@1', [`points=[${Array.from({ length: p.segments }, (_, i) => list([p.radius * Math.cos((2 * Math.PI * i) / p.segments), p.radius * Math.sin((2 * Math.PI * i) / p.segments)])).join(', ')}]`]); break;
      case 'extrude': stmt('solid.extrude@1', [...ins, `height=${f(p.height)}`]); break;
      case 'revolve': stmt('solid.revolve@1', [...ins, `angle=${f(p.angle)}deg`, `segments=${p.segments}`]); break;
      default: throw new Error(`untranslated ${n.kind}`);
    }
  }
  lines.push(`output ${outName} = ${name(c.output)}`);
  return lines.join('\n') + '\n';
}

const rows = [];
blocks.forEach((b, i) => {
  const id = `${String(i + 1).padStart(2, '0')}_${b.title.replace(/[^A-Za-z0-9]+/g, '_').replace(/^_|_$/g, '').toLowerCase()}`;
  const c = compile(b.source);
  if (!c.ok) { rows.push({ id, title: b.title, status: 'deferred', reason: `does not compile in the spike language: ${c.errors[0].msg}` }); return; }
  const kinds = [...new Set(c.nodes.map((n) => n.kind))];
  const blockers = kinds.filter((k) => !SUPPORTED.has(k) && !CHANGED.has(k));
  const arrays = c.nodes.filter((n) => n.kind === 'rect' && n.params.corner);
  if (blockers.length || arrays.length) {
    rows.push({ id, title: b.title, status: 'deferred', kinds, reason: [...new Set(blockers.map((k) => DEFERRED_WHY[k] ?? k)), ...(arrays.length ? ['rounded rectangle corners are a profile.offset case, not a primitive'] : [])].join('; ') });
    return;
  }
  const changed = kinds.filter((k) => CHANGED.has(k));
  const r = evaluate(b.source);
  if (!r.ok) { rows.push({ id, title: b.title, status: 'deferred', kinds, reason: `the spike evaluator itself failed: ${r.errors[0].msg}` }); return; }
  fs.writeFileSync(`${outDir}/${id}.txt`, translate(c));
  rows.push({
    id, title: b.title, status: changed.length ? 'changed' : 'supported', kinds,
    reason: changed.includes('material') ? 'material slots dropped: appearance is G10' : changed.includes('part') ? 'part names become output names' : 'same operators and semantics',
    volume: r.volume, size: r.size, components: r.components,
  });
});
fs.writeFileSync(`${outDir}/index.json`, JSON.stringify(rows, null, 1) + '\n');
const count = (s) => rows.filter((r) => r.status === s).length;
if (process.argv[3]) {
  const table = rows.map((r) => `| ${r.id.slice(0, 2)} | ${r.title} | ${r.status} | ${r.reason} |`).join('\n');
  fs.writeFileSync(process.argv[3], `# Spike program classification (G3-21)

The 53 programs of the language spike (\`spikes/geom-lang/runs/batch1.txt\` and \`batch2.txt\`),
classified against the operators G3 ships. **supported**: translates one to one and runs. **changed**:
runs, with the stated difference. **deferred**: needs an operator that arrives in a later gate, or
the spike's own evaluator failed. Generated by \`spikes/geom-lang/classify.mjs\`; the translated
programs and the spike evaluator's reference volumes and sizes are in
\`crates/ve-modeling/tests/spike53/\`, and \`crates/ve-modeling/tests/spike53.rs\` runs every
supported and changed program and compares volume (1.5 %) and size (2 mm) with the reference. No spike
code was imported; the translation is plain text in the engine's own language.

${count('supported')} supported, ${count('changed')} changed, ${count('deferred')} deferred.

| # | Program | Class | Reason |
|---|---|---|---|
${table}
`);
}
console.log(`${rows.length} programs: ${count('supported')} supported, ${count('changed')} changed, ${count('deferred')} deferred`);

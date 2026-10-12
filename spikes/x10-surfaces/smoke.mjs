// Smoke test: new operators and multi-output materials through the extended evaluator.
import { evaluate, thinWallReport } from './eval.mjs';

const src = `
outer = polygon(points=[[0,0],[0.06,0],[0.11,0.12],[0.07,0.26],[0.05,0.30],[0,0.30]])
inner = polygon(points=[[0,0.01],[0.056,0.01],[0.104,0.12],[0.064,0.26],[0.040,0.31],[0,0.31]])
body = revolve(outer, angle=360, segments=64)
cav = revolve(inner, angle=360, segments=64)
vase = difference(body, cav)
vm = material(vase, name="glaze", color=[0.1,0.3,0.7], metallic=0, roughness=0.2)
vp = part(vm, name="vase")
sq = rect(size=[0.1,0.1])
st = offset(sq, delta=-0.01)
tw = extrude(st, height=0.3, twist=90, scale_top=0.6, divisions=24)
tm = material(tw, name="copper", color=[0.72,0.45,0.2], metallic=1, roughness=0.3)
tp = part(tm, name="tower")
tooth = rect(size=[0.01,0.02])
ring = circle(radius=0.03, segments=48)
bore = circle(radius=0.01, segments=24)
teeth = radial(tooth, count=12, radius=0.03)
gear2d = punion(ring, teeth)
gear2dh = pdifference(gear2d, bore)
gear = extrude(gear2dh, height=0.01)
gm = material(gear, name="steel", color=[0.6,0.6,0.6], metallic=1, roughness=0.4)
output vp
output tp
output gm
`;
const r = evaluate(src);
if (!r.ok) { console.log('FAIL', r.stage, r.errors); process.exit(1); }
for (const o of r.outputs) console.log(o.part, o.material?.name, 'tris', o.tris, 'vol', o.volume.toFixed(6), 'size', o.size.join('x'), 'genus', o.genus, 'comps', o.components);
const vase = r.outputs[0];
const heights = Array.from({ length: 14 }, (_, i) => 0.02 + i * 0.02);
console.log('vase wall bad (3mm):', JSON.stringify(thinWallReport(vase.m, 0.003, heights)));
console.log('vase wall bad (8mm):', JSON.stringify(thinWallReport(vase.m, 0.008, heights)).slice(0, 120));

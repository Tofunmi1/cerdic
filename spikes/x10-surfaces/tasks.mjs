// Sixteen creation tasks and eight follow-up edits. Each check returns [label, passed].
// Prompts name the parts so the checks can find them; everything else is the model's judgement.
import {
  near, within, out, lum, dist3, isMetal, isDielectric, isGlass, isEmissive, matte, glossy, bboxAll, center, hslice, vslice,
  sectionBounds, widthAt, lobes, boreRadius, openingLoss, holeAreas, componentBoxes, stddev,
} from './lib.mjs';

const need = (res, ...names) => names.map((n) => out(res, n));
const has = (...os) => os.every(Boolean);
const missing = (n) => [`part "${n}" exists`, false];
const guard = (os, names, fn) => (has(...os) ? fn() : names.filter((n, i) => !os[i]).map(missing));
const snapBasic = (res) => Object.fromEntries(res.outputs.map((o) => [o.part, { size: o.size, bbox: o.bbox, volume: o.volume, genus: o.genus, components: o.components, material: o.material, boxes: o.boxes }]));

export const TASKS = [
  {
    id: 'wine_glass',
    prompt: 'Model a stemmed wine glass: 200 mm tall overall, a bowl about 75 mm across at its widest with a thin 2 mm wall and an open top, a slim stem, and a flat round foot about 70 mm across. Clear glass. One part named `glass`.',
    checks(res) {
      const [g] = need(res, 'glass');
      return guard([g], ['glass'], () => [
        ['clear glass material', isGlass(g)], ['height 180-220 mm', within(g.size[1], 0.18, 0.22)],
        ['widest 65-85 mm', within(Math.max(g.size[0], g.size[2]), 0.065, 0.085)], ['one piece', g.components === 1],
        ['open top, no inner void (genus 0)', g.genus === 0], ['thin wall: volume 20-160 cm3', within(g.volume, 2e-5, 1.6e-4)],
        ['rests on the ground', near(g.bbox.min[1], 0, 0.002)],
      ]);
    },
    snap: (res) => { const g = out(res, 'glass'); return g ? { ...snapBasic(res), footWidth: widthAt(g, g.bbox.min[1] + 0.002), height: g.size[1], width: Math.max(g.size[0], g.size[2]) } : {}; },
    edit: {
      prompt: 'Make the bowl 25% wider, but keep the overall height, the stem and the foot exactly as they are.',
      checks(res, before) {
        const [g] = need(res, 'glass');
        return guard([g], ['glass'], () => [
          ['widest is 25% more (+-6%)', near(Math.max(g.size[0], g.size[2]) / before.width, 1.25, 0.06)], ['height unchanged (+-3 mm)', near(g.size[1], before.height, 0.003)],
          ['foot unchanged (+-2 mm)', near(widthAt(g, g.bbox.min[1] + 0.002), before.footWidth, 0.002)], ['still clear glass', isGlass(g)],
        ]);
      },
    },
  },
  {
    id: 'spur_gear',
    prompt: 'Model a spur gear: 14 teeth, module 5 mm (so a 70 mm pitch diameter, about 80 mm over the teeth), 12 mm thick, with a 10 mm round bore through the centre. Brushed steel. One part named `gear`.',
    checks(res) {
      const [g] = need(res, 'gear');
      return guard([g], ['gear'], () => {
        const y = g.bbox.min[1] + g.size[1] / 2;
        return [
          ['over-teeth diameter 72-86 mm', within(Math.max(g.size[0], g.size[2]), 0.072, 0.086)], ['12 mm thick (+-1)', near(g.size[1], 0.012, 0.001)],
          ['through bore (genus 1)', g.genus === 1], ['one piece', g.components === 1], ['14 teeth', lobes(g, y) === 14],
          ['bore diameter 9-11 mm', near((boreRadius(g, y) ?? 0) * 2, 0.01, 0.001)], ['steel (metallic, rough 0.2-0.7)', isMetal(g) && within(g.material.roughness, 0.2, 0.7)],
        ];
      });
    },
    snap: (res) => { const g = out(res, 'gear'); return g ? { ...snapBasic(res), thick: g.size[1] } : {}; },
    edit: {
      prompt: 'Change it to 18 teeth at the same module (pitch diameter 90 mm) and make the bore 12 mm. Keep the thickness and material.',
      checks(res, before) {
        const [g] = need(res, 'gear');
        return guard([g], ['gear'], () => {
          const y = g.bbox.min[1] + g.size[1] / 2;
          return [
            ['18 teeth', lobes(g, y) === 18], ['over-teeth diameter 92-106 mm', within(Math.max(g.size[0], g.size[2]), 0.092, 0.106)],
            ['bore diameter 11-13 mm', near((boreRadius(g, y) ?? 0) * 2, 0.012, 0.001)], ['thickness unchanged (+-1 mm)', near(g.size[1], before.thick, 0.001)], ['still steel', isMetal(g)],
          ];
        });
      },
    },
  },
  {
    id: 'bolt_nut',
    prompt: 'Model an M8 hex bolt and its nut, both standing upright on the ground. Bolt: 8 mm round shaft 50 mm long below a hex head 13 mm across the flats and 5.5 mm tall. Nut: hex, 13 mm across the flats, 6.5 mm thick, with an 8 mm hole. Place them side by side with about 20 mm clear between them. Both zinc-plated steel. Parts `bolt` and `nut`.',
    checks(res) {
      const [b, n] = need(res, 'bolt', 'nut');
      return guard([b, n], ['bolt', 'nut'], () => {
        const gap = Math.max(b.bbox.min[0] - n.bbox.max[0], n.bbox.min[0] - b.bbox.max[0], b.bbox.min[2] - n.bbox.max[2], n.bbox.min[2] - b.bbox.max[2]);
        return [
          ['bolt 53-58 mm tall', within(b.size[1], 0.053, 0.058)], ['nut 6-7 mm thick', within(n.size[1], 0.006, 0.007)], ['nut has a hole (genus 1)', n.genus === 1],
          ['bolt solid (genus 0)', b.genus === 0], ['head 12.5-15.5 mm wide', within(Math.max(b.size[0], b.size[2]), 0.0125, 0.0155)], ['nut 12.5-15.5 mm wide', within(Math.max(n.size[0], n.size[2]), 0.0125, 0.0155)],
          ['each one piece', b.components === 1 && n.components === 1], ['gap 10-30 mm', within(gap, 0.01, 0.03)], ['both on the ground', near(b.bbox.min[1], 0, 0.002) && near(n.bbox.min[1], 0, 0.002)],
          ['both metal', isMetal(b) && isMetal(n)],
        ];
      });
    },
    snap: (res) => snapBasic(res),
    edit: {
      prompt: 'Make the bolt 15 mm longer, and make the nut brass-coloured. Everything else stays as it is.',
      checks(res, before) {
        const [b, n] = need(res, 'bolt', 'nut');
        return guard([b, n], ['bolt', 'nut'], () => [
          ['bolt 15 mm taller (+-1.5)', near(b.size[1], before.bolt.size[1] + 0.015, 0.0015)], ['nut thickness unchanged', near(n.size[1], before.nut.size[1], 0.0005)], ['nut is brass (metal, red > blue)', isMetal(n) && n.material.color[0] > n.material.color[2] * 1.4],
          ['bolt colour unchanged', dist3(b.material?.color ?? [0, 0, 0], before.bolt.material?.color ?? [1, 1, 1]) < 0.05], ['nut still has its hole', n.genus === 1],
        ]);
      },
    },
  },
  {
    id: 'spiral_stairs',
    prompt: 'Model a spiral staircase: a central steel pole 80 mm in diameter and 14 treads. Each tread is a wedge-shaped oak slab 40 mm thick reaching 900 mm out from the pole axis, rising 180 mm per step and turning 22.5 degrees per step (counter-clockwise seen from above). The first tread sits on the ground. Parts `treads` and `pole`.',
    checks(res) {
      const [t, p] = need(res, 'treads', 'pole');
      return guard([t, p], ['treads', 'pole'], () => {
        const boxes = componentBoxes(t).sort((a, b) => a.c[1] - b.c[1]);
        const ang = boxes.map((b) => (Math.atan2(-b.c[2], b.c[0]) * 180) / Math.PI);
        const diffs = ang.slice(1).map((a, i) => { let d = a - ang[i]; while (d < -180) d += 360; while (d > 180) d -= 360; return d; });
        const reach = Math.max(t.bbox.max[0], -t.bbox.min[0], t.bbox.max[2], -t.bbox.min[2]);
        const rises = boxes.slice(1).map((b, i) => b.c[1] - boxes[i].c[1]);
        return [
          ['14 separate treads', t.components === 14], ['treads span ~2.38 m (+-0.1)', near(t.size[1], 13 * 0.18 + 0.04, 0.1)], ['reach 0.85-0.95 m', within(reach, 0.85, 0.95)],
          ['turns ~22.5 deg per step, one direction', diffs.length === 13 && within(Math.abs(diffs.reduce((s, d) => s + d, 0) / 13), 18, 27) && diffs.every((d) => Math.sign(d) === Math.sign(diffs[0]))],
          ['rise 0.17-0.19 per step', rises.length === 13 && rises.every((r) => within(r, 0.17, 0.19))], ['pole 70-90 mm wide', within(Math.max(p.size[0], p.size[2]), 0.07, 0.09)], ['pole at least as tall as the treads', p.size[1] >= t.size[1] - 0.01],
          ['pole is steel, treads are not metal', isMetal(p) && isDielectric(t)], ['first tread on the ground', near(t.bbox.min[1], 0, 0.01)],
        ];
      });
    },
    snap: (res) => snapBasic(res),
    edit: {
      prompt: 'Add a handrail on the outer ends of the treads: a 30 mm round steel rail about 1 m above the treads, following them all the way up, with a steel baluster post from the end of every other tread up to the rail. Put the new geometry in a new part `rail`. Do not change the treads or pole.',
      checks(res, before) {
        const [t, p, r] = need(res, 'treads', 'pole', 'rail');
        return guard([t, p, r], ['treads', 'pole', 'rail'], () => [
          ['treads unchanged (14, same height)', t.components === 14 && near(t.size[1], before.treads.size[1], 0.002)], ['pole unchanged', near(p.size[1], before.pole.size[1], 0.002)],
          ['rail is steel', isMetal(r)], ['rail rises at least 2 m', r.size[1] >= 2.0], ['rail one connected piece (posts joined to the rail)', r.components === 1],
          ['rail at the outer edge (reach >= 0.85)', Math.max(r.bbox.max[0], -r.bbox.min[0], r.bbox.max[2], -r.bbox.min[2]) >= 0.85],
        ]);
      },
    },
  },
  {
    id: 'gothic_window',
    prompt: 'Model a stone wall panel 1.4 m wide, 2.8 m tall and 0.4 m thick standing on the ground, with a pointed (gothic lancet) arch opening cut right through it: 0.8 m wide, sill 0.6 m above the ground, apex 2.4 m above the ground. Fill the opening with a 10 mm pane of pale blue tinted glass. Rough sandstone for the wall. Parts `wall` and `glass`.',
    checks(res) {
      const [w, g] = need(res, 'wall', 'glass');
      return guard([w, g], ['wall', 'glass'], () => [
        ['wall 1.4 x 2.8 x 0.4 (+-2 cm)', near(w.size[0], 1.4, 0.02) && near(w.size[1], 2.8, 0.02) && near(w.size[2], 0.4, 0.02)], ['wall has a through opening (genus 1)', w.genus === 1],
        ['pane is thin (8-12 mm)', within(Math.min(...g.size), 0.008, 0.012)], ['pane 0.7-0.84 m wide', within(Math.max(g.size[0], g.size[2]), 0.7, 0.84)], ['pane 1.7-1.9 m tall', within(g.size[1], 1.7, 1.9)],
        ['sill at 0.6 m (+-4 cm)', near(g.bbox.min[1], 0.6, 0.04)], ['apex at 2.4 m (+-6 cm)', near(g.bbox.max[1], 2.4, 0.06)], ['pointed: narrow 10 cm below the apex', widthAt(g, g.bbox.max[1] - 0.1) < 0.45],
        ['pane inside the wall', g.bbox.min[0] >= w.bbox.min[0] && g.bbox.max[0] <= w.bbox.max[0] && g.bbox.max[1] <= w.bbox.max[1]], ['glass is tinted and see-through', isGlass(g) && g.material.color[2] > g.material.color[0]], ['wall is rough stone', matte(w, 0.65) && isDielectric(w)],
      ]);
    },
    snap: (res) => { const g = out(res, 'glass'); return g ? { ...snapBasic(res), apex: g.bbox.max[1], width: Math.max(g.size[0], g.size[2]) } : {}; },
    edit: {
      prompt: 'Tint the glass amber, and make the opening 20 cm taller (apex 20 cm higher) with the same width and sill height.',
      checks(res, before) {
        const [w, g] = need(res, 'wall', 'glass');
        return guard([w, g], ['wall', 'glass'], () => [
          ['glass is amber (red > blue, see-through)', g.material.color[0] > 0.5 && g.material.color[0] > g.material.color[2] * 1.5 && g.material.opacity <= 0.6], ['apex 20 cm higher (+-5 cm)', near(g.bbox.max[1], before.apex + 0.2, 0.05)],
          ['width unchanged (+-2 cm)', near(Math.max(g.size[0], g.size[2]), before.width, 0.02)], ['sill unchanged (+-4 cm)', near(g.bbox.min[1], before.glass.bbox.min[1], 0.04)], ['wall still has its opening, same size', w.genus === 1 && near(w.size[1], 2.8, 0.02)],
        ]);
      },
    },
  },
  {
    id: 'lantern',
    prompt: 'Model a hurricane lantern about 300 mm tall: a round cast-iron base and a round top cap, four slim iron posts joining them, four separate clear glass panes between the posts, a small ring handle on top, and a small glowing flame at the centre (warm orange light). Parts `frame` (all the iron, one connected piece), `glass` and `flame`.',
    checks(res) {
      const [f, g, fl] = need(res, 'frame', 'glass', 'flame');
      return guard([f, g, fl], ['frame', 'glass', 'flame'], () => {
        const fc = center(fl.bbox);
        return [
          ['frame 280-420 mm tall', within(f.size[1], 0.28, 0.42)], ['frame is one connected piece', f.components === 1], ['frame is iron (metallic >= 0.5, rough >= 0.4)', !!f.material && f.material.metallic >= 0.5 && f.material.roughness >= 0.4],
          ['four separate panes', g.components === 4], ['glass see-through', isGlass(g)], ['flame glows warm (emissive r > b)', isEmissive(fl) && fl.material.emissive[0] > fl.material.emissive[2]],
          ['flame small (< 8 cm)', Math.max(...fl.size) < 0.08], ['flame inside the frame', fc[1] > f.bbox.min[1] && fc[1] < f.bbox.max[1] && Math.hypot(fc[0] - center(f.bbox)[0], fc[2] - center(f.bbox)[2]) < 0.03],
          ['panes inside the frame footprint', g.bbox.max[0] <= f.bbox.max[0] + 0.005 && g.bbox.min[0] >= f.bbox.min[0] - 0.005],
        ];
      });
    },
    snap: (res) => snapBasic(res),
    edit: {
      prompt: 'Make the flame glow a deep red-orange and double its size in every direction. Leave everything else exactly as it is.',
      checks(res, before) {
        const [f, g, fl] = need(res, 'frame', 'glass', 'flame');
        return guard([f, g, fl], ['frame', 'glass', 'flame'], () => {
          const e = fl.material?.emissive ?? [0, 0, 0];
          return [
            ['flame deep red-orange (r >= 0.8, g < 0.45 r, b small)', e[0] >= 0.8 && e[1] < 0.45 * e[0] && e[2] < 0.2 * e[0]], ['flame about 2x as big (volume x2.5-10)', within(fl.volume / before.flame.volume, 2.5, 10)],
            ['frame unchanged (+-2 mm)', near(f.size[1], before.frame.size[1], 0.002) && f.components === 1], ['glass unchanged (4 panes)', g.components === 4],
          ];
        });
      },
    },
  },
  {
    id: 'teapot',
    prompt: 'Model a porcelain teapot: a round hollow body about 140 mm wide and 100 mm tall with an open top and a 4 mm wall, a curved spout on one side, a loop handle on the opposite side, a separate lid that sits on the opening with a small knob, and a thin gold band around the rim of the lid. White glazed porcelain. Parts `body` (body, spout and handle together), `lid` and `trim`.',
    checks(res) {
      const [b, l, t] = need(res, 'body', 'lid', 'trim');
      return guard([b, l, t], ['body', 'lid', 'trim'], () => [
        ['body hollow (volume < 800 cm3)', b.volume < 8e-4 && b.volume > 5e-5], ['spout + handle widen it (one horizontal axis 200-330 mm)', within(Math.max(b.size[0], b.size[2]), 0.2, 0.33)], ['body 70-150 mm tall (+handle)', within(b.size[1], 0.07, 0.15)],
        ['body one piece', b.components === 1], ['lid 50-130 mm wide', within(Math.max(l.size[0], l.size[2]), 0.05, 0.13)], ['lid sits at the top of the body (+-25 mm)', Math.abs(l.bbox.min[1] - b.bbox.max[1]) < 0.025 || Math.abs(l.bbox.min[1] - (b.bbox.max[1] - 0.03)) < 0.03],
        ['porcelain is glossy dielectric', glossy(b, 0.35) && isDielectric(b) && lum(b.material.color) > 0.5], ['trim is gold', isMetal(t) && t.material.color[0] > t.material.color[2] * 1.4], ['trim is a thin band (< 20 mm tall)', t.size[1] < 0.02],
      ]);
    },
    snap: (res) => snapBasic(res),
  },
  {
    id: 'vase_wall',
    prompt: 'Model a flared ceramic vase, 300 mm tall, hollow with an open top and a closed base, standing on the ground. Base 120 mm across, widest bulge about 220 mm across around 40% of the height, rim 100 mm across. The wall must be at least 3 mm thick everywhere including the base, and the vase must be watertight. Cobalt blue glaze. One part named `vase`.',
    checks(res) {
      const [v] = need(res, 'vase');
      return guard([v], ['vase'], () => {
        const cs = vslice(v, 0); const loss = openingLoss(cs, 0.003); cs.delete();
        return [
          ['height 280-320 mm', within(v.size[1], 0.28, 0.32)], ['widest 200-240 mm', within(Math.max(v.size[0], v.size[2]), 0.2, 0.24)], ['rim 90-115 mm at the top (outside or opening)', rimWithin(v, 0.09, 0.115)],
          ['wall >= 3 mm everywhere (section opening test)', loss < 0.03], ['one watertight piece, no sealed void', v.components === 1 && v.genus === 0 && v.voids === 0], ['hollow (volume < 1200 cm3)', v.volume < 1.2e-3 && v.volume > 1e-4],
          ['glazed blue', glossy(v, 0.4) && v.material.color[2] > v.material.color[0] * 1.5], ['on the ground', near(v.bbox.min[1], 0, 0.002)],
        ];
      });
    },
    snap: (res) => { const v = out(res, 'vase'); return v ? { ...snapBasic(res), height: v.size[1] } : {}; },
    edit: {
      prompt: 'Widen the opening to 140 mm across. The wall must still be at least 3 mm thick everywhere, and the height and base stay the same.',
      checks(res, before) {
        const [v] = need(res, 'vase');
        return guard([v], ['vase'], () => {
          const cs = vslice(v, 0); const loss = openingLoss(cs, 0.003); cs.delete();
          return [
            ['rim 130-150 mm (outside or opening)', rimWithin(v, 0.13, 0.15)], ['wall >= 3 mm everywhere', loss < 0.03], ['height unchanged (+-3 mm)', near(v.size[1], before.height, 0.003)],
            ['still one watertight piece', v.components === 1 && v.genus === 0 && v.voids === 0],
          ];
        });
      },
    },
  },
  {
    id: 'twisted_tower',
    prompt: 'Model a twisted tower sculpture: a 5-pointed star cross-section (outer radius 100 mm, inner radius 45 mm), 500 mm tall, twisting 120 degrees from the base to the top while tapering linearly to 60% of the base size at the top. Polished copper. One part named `tower`.',
    checks(res) {
      const [t] = need(res, 'tower');
      return guard([t], ['tower'], () => {
        const y0 = t.bbox.min[1];
        const analytic = 5 * 0.1 * 0.045 * Math.sin(Math.PI / 5) * 0.5 * (1 + 0.6 + 0.36) / 3;
        return [
          ['500 mm tall (+-5 mm)', near(t.size[1], 0.5, 0.005)], ['base 180-205 mm across', within(widthAt(t, y0 + 0.005), 0.18, 0.205)], ['top 105-125 mm across', within(widthAt(t, y0 + 0.495), 0.105, 0.125)],
          ['volume within 6% of the analytic value', near(t.volume / analytic, 1, 0.06)], ['five-pointed outline', lobes(t, y0 + 0.01) === 5], ['twisted: outline rotates ~120 deg (top lobes offset from base lobes)', Math.abs(topPhase(t, y0) - 120) < 12 || Math.abs(topPhase(t, y0) - 120 + 72) < 12 || Math.abs(topPhase(t, y0) - 120 - 72) < 12],
          ['one solid piece', t.components === 1 && t.genus === 0], ['polished copper', isMetal(t) && glossy(t, 0.35) && t.material.color[0] > t.material.color[2] * 1.5],
        ];
      });
    },
    snap: (res) => snapBasic(res),
  },
  {
    id: 'mount_plate',
    prompt: 'Model a mounting plate: 160 x 100 mm, 5 mm thick, with 10 mm radius rounded corners, four 6 mm mounting holes centred 12 mm in from the plate edges at each corner, and a raised rim 3 mm wide and 2 mm tall running around the top face, inset 4 mm from the plate edge (following the rounded outline). Brushed aluminium. One part named `plate`.',
    checks(res) {
      const [p] = need(res, 'plate');
      return guard([p], ['plate'], () => [
        ['160 x 100 mm footprint (+-1.5)', near(Math.max(p.size[0], p.size[2]), 0.16, 0.0015) && near(Math.min(p.size[0], p.size[2]), 0.1, 0.0015)], ['7 mm tall including the rim (+-0.6)', near(p.size[1], 0.007, 0.0006)],
        ['four through holes (genus 4)', p.genus === 4], ['one piece', p.components === 1], ['volume within 4% of 82 cm3', near(p.volume / 8.2e-5, 1, 0.04)],
        ['rounded corners (corner section area short of a rectangle by >= 30 mm2)', cornerLoss(p) > 3e-5], ['brushed aluminium', isMetal(p) && within(p.material.roughness, 0.25, 0.7) && lum(p.material.color) > 0.5],
      ]);
    },
    snap: (res) => snapBasic(res),
  },
  {
    id: 'mushroom',
    prompt: 'Model a forest mushroom about 180 mm tall, organic rather than geometric: a cream stem that curves slightly and swells towards the base, a domed red cap about 140 mm wide with a gently wavy lower lip, and about twelve small cream-white spots sitting on top of the cap. Parts `stem`, `cap` and `spots`.',
    checks(res) {
      const [s, c, sp] = need(res, 'stem', 'cap', 'spots');
      return guard([s, c, sp], ['stem', 'cap', 'spots'], () => {
        const all = bboxAll([s, c, sp]);
        return [
          ['total height 160-200 mm', within(all.size[1], 0.16, 0.2)], ['cap 120-165 mm wide', within(Math.max(c.size[0], c.size[2]), 0.12, 0.165)], ['cap sits above the stem', center(c.bbox)[1] > center(s.bbox)[1]],
          ['8-20 separate spots', within(sp.components, 8, 20)], ['spots lie on the cap (inside its footprint, not below its middle)', sp.bbox.min[0] >= c.bbox.min[0] - 0.01 && sp.bbox.max[0] <= c.bbox.max[0] + 0.01 && sp.bbox.min[1] >= center(c.bbox)[1] - 0.01],
          ['stem and cap each one piece', s.components === 1 && c.components === 1], ['cap red, matte-ish', c.material.color[0] > 0.45 && c.material.color[1] < 0.3 && c.material.roughness >= 0.3 && isDielectric(c)], ['stem cream, matte', lum(s.material?.color ?? [0, 0, 0]) > 0.5 && matte(s, 0.4)],
          ['organic: stem is not a plain cylinder (width varies > 20%)', stemVaries(s)], ['meshes under 150k triangles each', [s, c, sp].every((o) => o.tris < 150000)],
        ];
      });
    },
    snap: (res) => snapBasic(res),
  },
  {
    id: 'stone_bridge',
    prompt: 'Model a three-arch stone bridge on the ground: 6 m long (along X), 2.4 m wide (along Z), three equal semicircular arches of 1.6 m span with 0.5 m piers between them, a flat deck above the arches, and 0.6 m tall solid parapet walls along both long edges. The crown of the deck is about 3 m above the ground including the parapets. Rough weathered stone. One part named `bridge`.',
    checks(res) {
      const [b] = need(res, 'bridge');
      return guard([b], ['bridge'], () => {
        // Arches open onto the ground, so a level section just above it cuts the bridge into its
        // abutments and piers; the gaps between them are the arch spans.
        const pieces = levelPieces(b, 0.3); const gaps = pieces.slice(1).map((p, i) => p[0] - pieces[i][1]);
        return [
          ['5.8-6.2 m long', within(b.size[0], 5.8, 6.2)], ['2.3-2.5 m wide', within(b.size[2], 2.3, 2.5)], ['2.8-3.4 m tall', within(b.size[1], 2.8, 3.4)],
          ['two abutments and two piers at ground level', pieces.length === 4], ['three arch spans of 1.5-1.7 m', gaps.length === 3 && gaps.every((g) => within(g, 1.5, 1.7))],
          ['arches equal (spans within 10%)', gaps.length === 3 && Math.max(...gaps) / Math.min(...gaps) < 1.1], ['piers 0.4-0.6 m thick', pieces.length === 4 && [pieces[1], pieces[2]].every((p) => within(p[1] - p[0], 0.4, 0.6))], ['one piece', b.components === 1], ['rough stone', matte(b, 0.7) && isDielectric(b)], ['stands on the ground', near(b.bbox.min[1], 0, 0.02)],
        ];
      });
    },
    snap: (res) => snapBasic(res),
  },
  {
    id: 'desk_lamp',
    prompt: 'Model an articulated desk lamp: a round weighted base 160 mm across, a lower arm 350 mm long leaning 30 degrees from vertical to the left (-X), an upper arm 300 mm long leaning 50 degrees from vertical to the right (+X), a round joint knob at the base joint, the elbow and the head, and a cone shade (opening 120 mm wide) at the end of the upper arm tilted so it points down and to the right, holding a bulb. Matte black powder-coated metal. The bulb emits warm white light. Parts `lamp` (everything but the bulb, one connected piece) and `bulb`.',
    checks(res) {
      const [l, b] = need(res, 'lamp', 'bulb');
      return guard([l, b], ['lamp', 'bulb'], () => [
        ['lamp one connected piece', l.components === 1], ['450-750 mm tall', within(l.size[1], 0.45, 0.75)], ['reaches sideways at least 300 mm', l.size[0] >= 0.3], ['base 150-170 mm across at the ground', within(widthAt(l, l.bbox.min[1] + 0.002), 0.15, 0.17)],
        ['arms lean opposite ways (reaches 12 cm left of the axis and 5 cm right of it)', l.bbox.min[0] <= -0.12 && l.bbox.max[0] >= 0.05], ['bulb glows warm white', isEmissive(b) && b.material.emissive[0] >= b.material.emissive[2]], ['bulb 20-70 mm', within(Math.max(...b.size), 0.02, 0.07)],
        ['bulb is inside the lamp bounds', b.bbox.min[0] >= l.bbox.min[0] - 0.01 && b.bbox.max[0] <= l.bbox.max[0] + 0.01 && b.bbox.max[1] <= l.bbox.max[1] + 0.01], ['matte black', matte(l, 0.5) && lum(l.material.color) < 0.15],
      ]);
    },
    snap: (res) => snapBasic(res),
  },
  {
    id: 'bookshelf',
    prompt: 'Model a bookcase 900 mm wide (X), 1800 mm tall and 300 mm deep (Z) standing on the ground: two 20 mm side panels, a 20 mm bottom and top panel, a 5 mm back panel, and five 20 mm shelves evenly spaced between the bottom and top panels. Stand three books on the second shelf up from the bottom (shelf 2 of the five): one red, one green, one blue, each 25-40 mm thick and 200-300 mm tall, 200 mm deep. Oak for the case. Parts `frame` (sides, bottom, top, back), `shelves` (the five shelves only), `book_red`, `book_green` and `book_blue`.',
    checks(res) {
      const [f, s, r, g, b] = need(res, 'frame', 'shelves', 'book_red', 'book_green', 'book_blue');
      return guard([f, s, r, g, b], ['frame', 'shelves', 'book_red', 'book_green', 'book_blue'], () => {
        const ys = componentBoxes(s).map((c) => c.c[1]).sort((a, c) => a - c); const gaps = ys.slice(1).map((y, i) => y - ys[i]);
        const shelf2Top = componentBoxes(s).sort((a, c) => a.c[1] - c.c[1])[1]?.max[1];
        const books = [r, g, b];
        return [
          ['frame 900 x 1800 x 300 (+-5 mm)', near(f.size[0], 0.9, 0.005) && near(f.size[1], 1.8, 0.005) && near(f.size[2], 0.3, 0.005)], ['five shelves', s.components === 5], ['shelves 20 mm thick', componentBoxes(s).every((c) => near(c.size[1], 0.02, 0.002))],
          ['shelves evenly spaced (sd < 4 mm)', ys.length === 5 && stddev(gaps) < 0.004], ['shelves between bottom and top panels', ys.length === 5 && ys[0] > 0.03 && ys[4] < 1.77],
          ['books stand on shelf 2 (+-4 mm)', books.every((k) => near(k.bbox.min[1], shelf2Top, 0.004))], ['books 200-300 mm tall', books.every((k) => within(k.size[1], 0.2, 0.3))], ['books inside the case width', books.every((k) => k.bbox.min[0] > f.bbox.min[0] && k.bbox.max[0] < f.bbox.max[0])],
          ['book colours are red, green, blue', r.material.color[0] > r.material.color[1] * 1.8 && g.material.color[1] > g.material.color[0] * 1.5 && b.material.color[2] > b.material.color[0] * 1.8], ['oak frame, matte wood', matte(f, 0.4) && isDielectric(f)],
        ];
      });
    },
    snap: (res) => snapBasic(res),
    edit: {
      prompt: 'Add a sixth shelf and re-space all six shelves evenly between the bottom and top panels. The three books stay on the second shelf up from the bottom, resting on it.',
      checks(res, before) {
        const [f, s, r, g, b] = need(res, 'frame', 'shelves', 'book_red', 'book_green', 'book_blue');
        return guard([f, s, r, g, b], ['frame', 'shelves', 'book_red', 'book_green', 'book_blue'], () => {
          const cb = componentBoxes(s).sort((a, c) => a.c[1] - c.c[1]); const ys = cb.map((c) => c.c[1]); const gaps = ys.slice(1).map((y, i) => y - ys[i]);
          return [
            ['six shelves', s.components === 6], ['evenly spaced (sd < 4 mm)', ys.length === 6 && stddev(gaps) < 0.004], ['books rest on the second shelf (+-4 mm)', [r, g, b].every((k) => near(k.bbox.min[1], cb[1]?.max[1], 0.004))],
            ['frame unchanged', near(f.size[1], before.frame.size[1], 0.003)], ['book colours unchanged', dist3(r.material.color, before.book_red.material.color) < 0.05 && dist3(b.material.color, before.book_blue.material.color) < 0.05],
          ];
        });
      },
    },
  },
  {
    id: 'ferris_wheel',
    prompt: 'Model a toy Ferris wheel standing on the ground, wheel axis along Z: a wheel 1.2 m in diameter made of two parallel rim rings 140 mm apart joined by 12 spokes per side and a central hub and axle; 12 small 80 mm cube gondolas hanging at equal angles around the rim between the rings; and an A-frame support (two A-shaped legs, one on each end of the axle) that holds the axle 0.7 m above the ground. Wheel painted white, gondolas red, frame dark steel. Parts `wheel` (rims, spokes, hub; one connected piece), `gondolas` and `frame`.',
    checks(res) {
      const [w, g, f] = need(res, 'wheel', 'gondolas', 'frame');
      return guard([w, g, f], ['wheel', 'gondolas', 'frame'], () => {
        const wc = center(w.bbox); const gb = componentBoxes(g);
        const rad = gb.map((c) => Math.hypot(c.c[0] - wc[0], c.c[1] - wc[1]));
        const ang = gb.map((c) => Math.atan2(c.c[1] - wc[1], c.c[0] - wc[0]) * 180 / Math.PI).sort((a, c) => a - c);
        const gaps = ang.map((a, i) => (i < ang.length - 1 ? ang[i + 1] - a : ang[0] + 360 - a));
        return [
          ['wheel 1.1-1.3 m across', within(w.size[0], 1.1, 1.3) && within(w.size[1], 1.1, 1.3)], ['wheel one connected piece', w.components === 1], ['wheel at least 140 mm wide along Z (axle may stick out)', within(w.size[2], 0.14, 0.6)],
          ['12 separate gondolas', g.components === 12], ['gondolas hang around the rim (radius 0.45-0.62 m from the hub)', rad.length === 12 && rad.every((r) => within(r, 0.45, 0.62))], ['gondolas evenly spaced (gaps 30 +-6 deg)', gaps.length === 12 && gaps.every((a) => near(a, 30, 6))],
          ['axle about 0.7 m high (+-0.08)', near(wc[1], 0.7, 0.08)], ['frame on the ground and supports the axle', near(f.bbox.min[1], 0, 0.01) && f.bbox.max[1] >= 0.65], ['gondolas red, wheel white, frame dark metal', g.material.color[0] > 0.5 && g.material.color[1] < 0.3 && lum(w.material.color) > 0.6 && lum(f.material.color) < 0.3],
        ];
      });
    },
    snap: (res) => snapBasic(res),
  },
  {
    id: 'keepsake_box',
    prompt: 'Model a walnut keepsake box. Body: 200 mm wide (X), 80 mm tall (Y), 120 mm deep (Z), open at the top, with 4 mm walls and floor. Lid: a separate 200 x 120 mm panel 12 mm thick that sits on top of the body, with a 4 mm tall locating lip underneath that fits inside the opening. Two brass hinge barrels (6 mm diameter, 30 mm long, along X) on the back edge (-Z) and a brass clasp plate on the front face. Parts `body`, `lid` and `hardware` (all the brass, hinge and clasp pieces).',
    checks(res) {
      const [b, l, h] = need(res, 'body', 'lid', 'hardware');
      return guard([b, l, h], ['body', 'lid', 'hardware'], () => {
        const cs = vslice(b, 0); const loss = openingLoss(cs, 0.0038); cs.delete();
        return [
          ['body 200 x 80 x 120 mm (+-2)', near(b.size[0], 0.2, 0.002) && near(b.size[1], 0.08, 0.002) && near(b.size[2], 0.12, 0.002)], ['body is an open hollow (volume 150-600 cm3, genus 0)', within(b.volume, 1.5e-4, 6e-4) && b.genus === 0],
          ['walls and floor >= 4 mm (section opening test)', loss < 0.03], ['lid 196-204 x 116-124 mm', within(l.size[0], 0.196, 0.204) && within(l.size[2], 0.116, 0.124)], ['lid is 12-20 mm thick including the lip', within(l.size[1], 0.012, 0.02)],
          ['lid sits on top of the body (+-6 mm)', near(l.bbox.min[1], b.bbox.max[1], 0.006) || near(l.bbox.min[1], b.bbox.max[1] - 0.004, 0.006)], ['at least three hardware pieces (2 barrels + clasp)', h.components >= 3], ['hardware is brass', isMetal(h) && h.material.color[0] > h.material.color[2] * 1.4],
          ['walnut wood body and lid', matte(b, 0.4) && isDielectric(b) && b.material.color[0] > b.material.color[2] && lum(b.material.color) < 0.4],
        ];
      });
    },
    snap: (res) => snapBasic(res),
  },
];

// ---------- task-specific measurement helpers
function topPhase(o, y0) { // angle (deg) between the first outline vertex phase at the base and at the top
  const phase = (y) => {
    const cs = hslice(o, y); const polys = cs.toPolygons(); cs.delete();
    const l = polys.sort((a, c) => Math.abs(c.length) - Math.abs(a.length))[0] ?? [];
    let best = -1, bi = 0; l.forEach((p, i) => { const r = Math.hypot(p[0], p[1]); if (r > best) { best = r; bi = i; } });
    return l.length ? (Math.atan2(l[bi][1], l[bi][0]) * 180) / Math.PI : 0;
  };
  let d = phase(y0 + 0.495) - phase(y0 + 0.005); while (d < 0) d += 360; while (d >= 360) d -= 360; return d;
}
function cornerLoss(p) { // area missing from the bounding rectangle in a section through the plate body
  const cs = hslice(p, p.bbox.min[1] + 0.0025); const b = sectionBounds(cs); const lost = b ? b.w * b.h - cs.area() : 0; cs.delete(); return lost;
}
function stemVaries(s) {
  const ws = [0.2, 0.4, 0.6, 0.8].map((t) => widthAt(s, s.bbox.min[1] + t * s.size[1])).filter((w) => w > 0);
  return ws.length >= 3 && Math.max(...ws) / Math.min(...ws) > 1.2;
}
// the rim is judged by the outside width or the opening diameter, since "rim 100 mm across" can mean either
function rimWithin(v, lo, hi) {
  const y = v.bbox.max[1] - 0.001;
  return within(widthAt(v, y), lo, hi) || within(2 * (boreRadius(v, y) ?? 0), lo, hi);
}
// x extents of the separate pieces of a level section, left to right
function levelPieces(o, y) {
  const cs = hslice(o, y); const polys = cs.toPolygons(); cs.delete();
  return polys.filter((l) => loopAreaSigned(l) > 0).map((l) => [Math.min(...l.map((p) => p[0])), Math.max(...l.map((p) => p[0]))]).sort((a, b) => a[0] - b[0]);
}
function loopAreaSigned(l) { let a = 0; for (let i = 0; i < l.length; i++) { const p = l[i], q = l[(i + 1) % l.length]; a += p[0] * q[1] - q[0] * p[1]; } return a / 2; }
function halfShift(l) { // x of the upper half's centroid minus the lower half's, using vertex averages
  const { verts, numProp } = l; const mid = (l.bbox.min[1] + l.bbox.max[1]) / 2; let lo = [0, 0], hi = [0, 0];
  for (let i = 0; i < verts.length / numProp; i++) { const x = verts[i * numProp], y = verts[i * numProp + 1]; if (y > mid) { hi[0] += x; hi[1]++; } else { lo[0] += x; lo[1]++; } }
  return hi[0] / Math.max(hi[1], 1) - lo[0] / Math.max(lo[1], 1);
}

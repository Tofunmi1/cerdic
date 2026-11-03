// node showcase-gen.mjs > material_showcase.txt : 24 procedural material swatches in a studio.
// Texture programs allow no nested calls: every node an input names is a statement above it.
const M = "u = 1.0 - abs(2.0 * p.x - 1.0); v = 1.0 - abs(2.0 * p.y - 1.0);";
const N = "u = p.x; v = p.y;";
const E = (code) => `expr(code=\\"${code.replace(/\s+/g, " ").trim()}\\")`;
const C = (r, g, b) => E(`out = vec4(${r}, ${g}, ${b}, 1.0)`);
const flat = (r) => E(`out = vec4(vec3(${r}), 1.0)`);
const prog = (o) => Object.entries(o).map(([k, v]) => `${k} = ${v}`).join("; ");
// a step ramp: black below `a`, white above `b` (mortar masks)
const gap = (lo, hi) => `colorize(b, gradient=[0:#000000, ${lo}:#000000, ${hi}:#ffffff, 1:#ffffff])`;

const mats = [
  ["marble", prog({ a: E(`${M} f = fbm(vec2(u*3.0, v*3.0)); s = abs(sin((u*4.0 + v*2.0 + f*5.0)*3.14159)); k = smoothstep(0.0, 0.12, s); out = vec4(mix(vec3(0.26,0.28,0.32), vec3(0.93,0.92,0.89), k) * (0.95 + 0.05*f), 1.0)`), albedo: "a", roughness: flat(0.16) })],
  ["oak_planks", prog({ a: E(`id = floor(p.x*6.0); tt = fract(sin(id*12.9898)*43758.5); g = fbm(vec2(p.x*18.0 + id*7.0, p.y*1.2)); ring = 0.5 + 0.5*sin((g*12.0 + p.y*2.0)*6.2832); e = smoothstep(0.0, 0.03, fract(p.x*6.0)) * smoothstep(0.0, 0.03, 1.0 - fract(p.x*6.0)); out = vec4(mix(vec3(0.33,0.19,0.08), vec3(0.64,0.42,0.22), ring*0.6 + tt*0.4) * (0.25 + 0.75*e), 1.0)`), h: E(`e = smoothstep(0.0, 0.04, fract(p.x*6.0)) * smoothstep(0.0, 0.04, 1.0 - fract(p.x*6.0)); out = vec4(vec3(e), 1.0)`), albedo: "a", roughness: flat(0.55), height: "h" })],
  ["herringbone_parquet", prog({ b: "bricks(pattern=herringbone, rows=10, columns=5, mortar=0.03, bevel=0.02)", w: "fbm(scale_x=3, scale_y=40, iterations=4)", wood: "colorize(w, gradient=[0:#6b4524, 1:#c79b62])", seam: gap(0.05, 0.1), albedo: "blend(seam, wood, seam, blend_type=multiply, amount=1.0)", roughness: flat(0.45), height: "b" })],
  ["cobblestone", prog({ b: "bricks_uneven4()", n: "fbm(scale_x=8, scale_y=8, iterations=5)", stone: "colorize(n, gradient=[0:#6e6a64, 1:#a29c92])", seam: gap(0.12, 0.2), albedo: "blend(seam, stone, seam, blend_type=multiply, amount=1.0)", roughness: flat(0.8), height: "b" })],
  ["slate_tiles", prog({ b: "bricks(pattern=running_bond, rows=5, columns=3, mortar=0.02, bevel=0.03)", n: "fbm(scale_x=6, scale_y=6, iterations=5)", slate: "colorize(n, gradient=[0:#2f3438, 1:#59626a])", seam: gap(0.06, 0.12), albedo: "blend(seam, slate, seam, blend_type=multiply, amount=1.0)", roughness: "colorize(n, gradient=[0:#9a9a9a, 1:#d0d0d0])", height: "b" })],
  ["plaster", prog({ a: E(`${M} out = vec4(vec3(0.82, 0.78, 0.7) * (0.9 + 0.1*fbm(vec2(u*5.0, v*5.0)) + 0.04*fbm(vec2(u*60.0, v*60.0))), 1.0)`), h: E(`${M} out = vec4(vec3(0.5 + 0.5*fbm(vec2(u*9.0, v*9.0))), 1.0)`), albedo: "a", roughness: flat(0.9), height: "h" })],
  ["board_formed_concrete", prog({ a: E(`${M} b = floor(p.y*8.0); tt = fract(sin(b*5.7)*999.0); g = fbm(vec2(u*14.0, v*2.0 + b)); out = vec4(vec3(0.55 + 0.12*g + 0.05*tt - 0.2*smoothstep(0.96, 1.0, fract(p.y*8.0))), 1.0)`), h: E(`${M} b = floor(p.y*8.0); out = vec4(vec3(0.5 + 0.35*fbm(vec2(u*30.0, v*4.0 + b))), 1.0)`), albedo: "a", roughness: flat(0.92), height: "h" })],
  ["verdigris_copper", prog({ m: E(`${N} out = vec4(vec3(smoothstep(0.45, 0.7, fbm(vec2(u*5.0, v*5.0)) + 0.2*fbm(vec2(u*28.0, v*28.0)))), 1.0)`), patina: C(0.22, 0.58, 0.5), copper: C(0.72, 0.38, 0.2), albedo: "blend(patina, copper, m, blend_type=normal, amount=1.0)", metallic: "colorize(m, gradient=[0:#ffffff, 1:#101010])", roughness: "colorize(m, gradient=[0:#5a5a5a, 1:#d8d8d8])", height: "m" })],
  ["brushed_steel", prog({ a: E(`g = fbm(vec2(p.x*2.0, p.y*90.0)); out = vec4(vec3(0.5 + 0.12*g), 1.0)`), albedo: "a", metallic: flat(1.0), roughness: E(`g = fbm(vec2(p.x*2.0, p.y*120.0)); out = vec4(vec3(0.28 + 0.2*g), 1.0)`) })],
  ["corrugated_metal", prog({ h: E(`out = vec4(vec3(0.5 + 0.5*sin(p.x*6.2832*10.0)), 1.0)`), a: E(`${M} out = vec4(vec3(0.5, 0.55, 0.58) * (0.85 + 0.15*fbm(vec2(u*6.0, v*6.0))), 1.0)`), albedo: "a", metallic: flat(0.9), roughness: flat(0.5), height: "h" })],
  ["carbon_fiber", prog({ w: "weave(columns=8, rows=8, width=0.8)", albedo: "colorize(w, gradient=[0:#050505, 1:#2c2f33])", metallic: flat(0.3), roughness: flat(0.28), height: "w" })],
  ["leather", prog({ v: "voronoi(scale_x=22, scale_y=22, randomness=0.9)", n: "fbm(scale_x=40, scale_y=40, iterations=3)", hide: "colorize(v, gradient=[0:#150b06, 0.35:#3a2113, 1:#4b2d1a])", grain: "colorize(n, gradient=[0:#ffffff, 1:#cdbdb0])", albedo: "blend(grain, hide, n, blend_type=multiply, amount=0.4)", roughness: flat(0.5), height: "v" })],
  ["woven_fabric", prog({ w: "weave(columns=30, rows=30, width=0.85)", n: "fbm(scale_x=50, scale_y=50, iterations=3)", cloth: "colorize(w, gradient=[0:#1d2a4a, 1:#4062a8])", fuzz: "colorize(n, gradient=[0:#ffffff, 1:#c9c9d6])", albedo: "blend(fuzz, cloth, n, blend_type=multiply, amount=0.5)", roughness: flat(0.92), height: "w" })],
  ["grass", prog({ a: E(`${M} b = fbm(vec2(u*70.0, v*9.0)); c = fbm(vec2(u*5.0, v*5.0)); out = vec4(mix(vec3(0.07, 0.2, 0.04), vec3(0.42, 0.55, 0.14), clamp(b*1.3 + c*0.5 - 0.2, 0.0, 1.0)), 1.0)`), h: E(`${M} out = vec4(vec3(fbm(vec2(u*70.0, v*9.0))), 1.0)`), albedo: "a", roughness: flat(0.85), height: "h" })],
  ["sand_ripples", prog({ a: E(`${M} f = fbm(vec2(u*4.0, v*4.0)); r = 0.5 + 0.5*sin((u*30.0 + f*6.0)*3.14159); out = vec4(mix(vec3(0.68, 0.56, 0.38), vec3(0.82, 0.7, 0.5), r) * (0.94 + 0.06*fbm(vec2(u*120.0, v*120.0))), 1.0)`), h: E(`${M} f = fbm(vec2(u*4.0, v*4.0)); out = vec4(vec3(0.5 + 0.5*sin((u*30.0 + f*6.0)*3.14159)), 1.0)`), albedo: "a", roughness: flat(0.95), height: "h" })],
  ["snow", prog({ a: E(`${M} f = fbm(vec2(u*6.0, v*6.0)); s = step(0.985, fract(sin(dot(vec2(floor(u*220.0), floor(v*220.0)), vec2(12.9898, 78.233)))*43758.5)); out = vec4(mix(vec3(0.82, 0.88, 0.96), vec3(0.98, 0.99, 1.0), f) + vec3(s*0.3), 1.0)`), h: E(`${M} out = vec4(vec3(fbm(vec2(u*6.0, v*6.0))), 1.0)`), albedo: "a", roughness: flat(0.6), height: "h" })],
  ["ice", prog({ c: E(`${N} f = fbm(vec2(u*4.0, v*4.0)); g = abs(fbm(vec2(u*9.0 + f, v*9.0 - f)) - 0.5); out = vec4(vec3(smoothstep(0.0, 0.05, g)), 1.0)`), clear: C(0.85, 0.95, 1.0), deep: C(0.45, 0.7, 0.85), albedo: "blend(clear, deep, c, blend_type=normal, amount=1.0)", roughness: flat(0.06), height: "c" })],
  ["lava", prog({ c: E(`${N} f = fbm(vec2(u*5.0, v*5.0)); g = abs(fbm(vec2(u*7.0 + f*2.0, v*7.0 - f*2.0)) - 0.5); out = vec4(vec3(1.0 - smoothstep(0.0, 0.09, g)), 1.0)`), hot: C(1.0, 0.45, 0.05), crust: C(0.04, 0.035, 0.03), glow: C(2.0, 0.7, 0.1), none: C(0.0, 0.0, 0.0), albedo: "blend(hot, crust, c, blend_type=normal, amount=1.0)", emissive: "blend(glow, none, c, blend_type=normal, amount=1.0)", roughness: flat(0.9), height: "c" })],
  ["gold", prog({ a: E(`g = fbm(vec2(p.x*60.0, p.y*60.0)); out = vec4(vec3(1.0, 0.77, 0.34) * (0.92 + 0.08*g), 1.0)`), albedo: "a", metallic: flat(1.0), roughness: E(`g = fbm(vec2(p.x*80.0, p.y*80.0)); out = vec4(vec3(0.22 + 0.12*g), 1.0)`) })],
  ["london_brick", prog({ b: "bricks(pattern=running_bond, rows=12, columns=5, mortar=0.06, bevel=0.03)", n: "fbm(scale_x=12, scale_y=12, iterations=5)", fired: "colorize(n, gradient=[0:#9c8450, 1:#cdb27a])", seam: gap(0.08, 0.14), albedo: "blend(seam, fired, seam, blend_type=multiply, amount=1.0)", roughness: flat(0.85), height: "b" })],
  ["ceramic_tiles", prog({ b: "bricks(pattern=running_bond, rows=6, columns=6, row_offset=0, mortar=0.04, bevel=0.03)", albedo: "colorize(b, gradient=[0:#6f6d68, 0.06:#6f6d68, 0.12:#f2f0ea, 1:#f2f0ea])", roughness: "colorize(b, gradient=[0:#e0e0e0, 0.1:#e0e0e0, 0.14:#1f1f1f, 1:#1f1f1f])", height: "b" })],
  ["hex_mosaic", prog({ n: "beehive()", albedo: "n.1", roughness: flat(0.2), height: "n" })],
  ["moss_on_stone", prog({ b: "bricks_uneven4()", n: "fbm(scale_x=10, scale_y=10, iterations=5)", m: "fbm(scale_x=5, scale_y=5, iterations=5)", rock: "colorize(n, gradient=[0:#5f5c57, 1:#8d887f])", seam: gap(0.12, 0.2), stone: "blend(seam, rock, seam, blend_type=multiply, amount=1.0)", moss: "colorize(n, gradient=[0:#16300d, 1:#4f7a22])", cover: "colorize(m, gradient=[0:#000000, 0.45:#000000, 0.65:#ffffff, 1:#ffffff])", albedo: "blend(moss, stone, cover, blend_type=normal, amount=1.0)", roughness: flat(0.9), height: "b" })],
];

const soft = new Set(["plaster","board_formed_concrete","grass","sand_ripples","snow","ice","lava","leather","woven_fabric","verdigris_copper"]);
const cols = 6, gp = 1.3;
const L = [
  "# about: A studio of 24 procedural material swatches, each a short texture program of Material Maker nodes and expressions (marble, planks, parquet, cobbles, slate, plaster, copper, steel, fabric, grass, sand, snow, ice, lava, gold, brick, tiles, mosaic, moss). Relief comes from each program's height channel.",
  "# kind: world",
  "sky = scene.sky@1(sun_elevation=45, sun_azimuth=-35, sun_intensity=6, turbidity=2.2, clouds=false)",
  "output sky = sky",
  "env = scene.environment@1(intensity=1.4, reflection=0.8, gi=false)",
  "output env = env",
  "floor = solid.box@1(size=[14, 0.2, 9])",
  "floor_t = xform.move@1(floor, by=[0, -0.1, 0.5])",
  "floor_m = solid.material@1(floor_t, color=[0.12, 0.12, 0.13], roughness=0.35)",
  "output floor = floor_m",
  "wall = solid.box@1(size=[14, 6.2, 0.2])",
  "wall_t = xform.move@1(wall, by=[0, 3.0, -2.6])",
  "wall_m = solid.material@1(wall_t, color=[0.2, 0.21, 0.23], roughness=0.9)",
  "output wall = wall_m",
];
mats.forEach(([name, program0], i) => {
  const program = soft.has(name) ? program0.replace(/; height = (\w+)/, "; hs = colorize($1, gradient=[0:#707070, 1:#b4b4b4]); height = hs") : program0;
  const c = i % cols, r = Math.floor(i / cols);
  const x = (c - (cols - 1) / 2) * gp, y = 0.75 + (3 - r) * 1.3;
  L.push(`s_${name} = solid.box@1(size=[1.1, 1.1, 0.08])`,
    `s_${name}_m = solid.material@1(s_${name}, color=[0.5, 0.5, 0.5], roughness=0.8)`,
    `s_${name}_t = solid.texture@1(s_${name}_m, program="${program}", tile=1.1, size="512")`,
    `s_${name}_p = xform.move@1(s_${name}_t, by=[${x.toFixed(2)}, ${y.toFixed(2)}, -2.3])`,
    `output ${name} = s_${name}_p`);
});
console.log(L.join("\n"));

// A small library of procedural surface programs for `solid.texture` (guide topic "materials"). Every program is tested: tests/materials.mjs bakes each one in
// the viewer and fails on any bake error. They use only the texture expression language (p is the surface coordinate 0 to 1, fbm, mix, smoothstep ...), mirror
// the coordinates so a tiled surface has no seams, and need `tile` (metres per repeat) on solid.texture because merged or procedural meshes carry no UVs.

const mirror = 'u = 1.0 - abs(2.0 * p.x - 1.0); v = 1.0 - abs(2.0 * p.y - 1.0);';

/** Bark: vertical grooves with tonal variation. `dark` and `light` are r, g, b in 0 to 1; `grain` is how many ridges around (6 broad, 12 fine). */
export function bark(dark = '0.12, 0.085, 0.055', light = '0.52, 0.41, 0.3', grain = 12) {
  const f = `f = fbm(vec2(u * ${grain}.0, v * 1.6));`;
  return `h = expr(code=\\"${mirror} ${f} out = vec4(vec3(clamp((f - 0.5) * 3.2 + 0.5, 0.0, 1.0)), 1.0)\\"); `
    + `n = expr(code=\\"${mirror} ${f} g = fbm(vec2(u * 3.5 + 3.0, v * 0.6)); c = clamp((f - 0.5) * 3.2 + 0.5, 0.0, 1.0); groove = smoothstep(0.3, 0.6, c); base = mix(vec3(${dark}), vec3(${light}), groove); out = vec4(base * (0.75 + 0.5 * g), 1.0)\\"); `
    + 'r = expr(code=\\"out = vec4(vec3(0.88), 1.0)\\"); albedo = n; height = h; roughness = r';
}

/** Concrete with weathering: mottled grey, dark rain streaks running down, rough. */
export const concrete = `h = expr(code=\\"${mirror} out = vec4(vec3(0.5 + 0.5 * fbm(vec2(u * 18.0, v * 18.0))), 1.0)\\"); `
  + `n = expr(code=\\"${mirror} base = 0.5 + 0.18 * fbm(vec2(u * 7.0, v * 7.0)) + 0.06 * fbm(vec2(u * 40.0, v * 40.0)); streak = smoothstep(0.52, 0.85, fbm(vec2(u * 26.0, v * 1.8))) * 0.28 * smoothstep(0.0, 1.0, 1.0 - v); out = vec4(vec3(base - streak) * vec3(1.0, 0.985, 0.95), 1.0)\\"); `
  + 'r = expr(code=\\"out = vec4(vec3(0.92), 1.0)\\"); albedo = n; height = h; roughness = r';

/** Steel gone to rust in patches: bare metal is shiny, rust is dull and orange-brown. */
export const rusty_metal = `${'m = expr(code=\\"' + mirror} out = vec4(vec3(smoothstep(0.52, 0.74, fbm(vec2(u * 6.0, v * 6.0)) + 0.15 * fbm(vec2(u * 30.0, v * 30.0)))), 1.0)\\"); `
  + 'steel = expr(code=\\"out = vec4(0.56, 0.58, 0.62, 1.0)\\"); rust = expr(code=\\"out = vec4(0.42, 0.19, 0.08, 1.0)\\"); '
  + 'albedo = blend(rust, steel, m, blend_type=normal, amount=1.0); metallic = colorize(m, gradient=[0:#ffffff, 1:#2a2a2a]); roughness = colorize(m, gradient=[0:#6a6a6a, 1:#e0e0e0]); height = m';

/** Asphalt: dark, fine grit, with shallow glossy patches where water would sit. */
export const asphalt = `n = expr(code=\\"${mirror} grit = fbm(vec2(u * 90.0, v * 90.0)); patch = fbm(vec2(u * 4.0, v * 4.0)); out = vec4(vec3(0.07 + 0.09 * grit + 0.02 * patch), 1.0)\\"); `
  + `w = expr(code=\\"${mirror} out = vec4(vec3(smoothstep(0.55, 0.72, fbm(vec2(u * 4.0, v * 4.0)))), 1.0)\\"); `
  + 'albedo = n; roughness = colorize(w, gradient=[0:#b8b8b8, 1:#1c1c1c]); height = n';

/** Bare earth: brown, speckled with pebbles, no shine. */
export const dirt = `n = expr(code=\\"${mirror} base = 0.5 + 0.35 * fbm(vec2(u * 9.0, v * 9.0)); peb = smoothstep(0.62, 0.7, fbm(vec2(u * 45.0, v * 45.0))); out = vec4(mix(vec3(0.26, 0.18, 0.11), vec3(0.5, 0.38, 0.26), base) + vec3(0.1) * peb, 1.0)\\"); `
  + 'albedo = n; roughness = colorize(n, gradient=[0:#f0f0f0, 1:#e0e0e0]); height = n';

/** Brick laid in courses with sooty streaks below the sills. */
export const brick = 'b = bricks(rows=22, columns=7, row_offset=0.5, mortar=0.1, bevel=0.0); '
  + 's = fbm(scale_x=14, scale_y=2, iterations=4); '
  + 'brickc = colorize(b, gradient=[0:#6f6a60, 0.5:#6f6a60, 0.51:#8d4a38, 1:#a85e46]); '
  + 'soot = colorize(s, gradient=[0:#ffffff, 0.6:#ffffff, 1:#a09a92]); '
  + 'albedo = blend(soot, brickc, s, blend_type=multiply, amount=0.35); height = b; roughness = colorize(b, gradient=[0:#f0f0f0, 1:#d0d0d0])';

/** The programs a world can use by name, with what each is for and the `tile` (metres per repeat) it was designed for. */
export const MATERIALS = {
  bark_oak: { about: 'brown furrowed bark for trunks and limbs', program: bark('0.12, 0.085, 0.055', '0.52, 0.41, 0.3', 12), tile: 1.2 },
  bark_birch: { about: 'pale papery bark with dark marks', program: bark('0.2, 0.19, 0.17', '0.9, 0.88, 0.82', 6), tile: 1.2 },
  bark_pine: { about: 'dark red-brown plated bark', program: bark('0.1, 0.06, 0.04', '0.42, 0.26, 0.17', 9), tile: 1.2 },
  weathered_concrete: { about: 'mottled grey concrete with rain streaks', program: concrete, tile: 2.0 },
  rusty_metal: { about: 'steel with rust patches (metallic and rough where rusted)', program: rusty_metal, tile: 1.5 },
  wet_asphalt: { about: 'dark road surface with glossy damp patches', program: asphalt, tile: 4.0 },
  dirt: { about: 'bare brown earth with pebbles', program: dirt, tile: 2.0 },
  sooty_brick: { about: 'brick courses with soot streaks', program: brick, tile: 2.0 },
};

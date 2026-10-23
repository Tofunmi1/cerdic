#!/usr/bin/env python3
"""Writes worlds/city_district.txt: a modern district from the city nodes, textured from CC0 photographic materials with procedural windows, grime and wear layered on top.

    python3 -I city_district.py [seed] [out.txt]

The facade textures are texture programs: `image(name="Concrete034_Color", ...)` photographs (ambientCG, CC0, shipped in `outfits/`) tiled at their real size, `expr` nodes for the window
masks, the glass and the grime, and `blend` to layer them. Wall UVs are facade units: u is one 8 m bay, v is three floors (10.8 m).
"""
import sys

seed = int(sys.argv[1]) if len(sys.argv) > 1 else 3
out = sys.argv[2] if len(sys.argv) > 2 else "/Users/mac/Work/victoria-vr-hack/engine/web/dist-bevy-gpu/worlds/city_district.txt"
LAYOUT = f"size=[300, 300], block=40, street_width=9, lot_min=6, lot_max=14, irregularity=0.5, seed={seed}"


def esc(program):
    return program.replace('"', '\\"')


# blend takes its opacity from a node, so "fully opaque" is a node too
ONE = 'one = expr(code="out = vec3(1.0)")'


def picture(material, map_name, rx, ry):
    return f'image(name="{material}_{map_name}", repeat_x={rx}, repeat_y={ry})'


# A facade from one photographic wall material, a window mask (a function of the bay u across and the floors v up), glass, and grime streaks.
def facade(material, mask, glass, tint="vec3(1.0)", rx=4, ry=6, glass_rough=0.12, streak=0.35):
    return "; ".join([
        ONE,
        f"wall = {picture(material, 'Color', rx, ry)}",
        f"wall_r = {picture(material, 'Roughness', rx, ry)}",
        f"wall_n = {picture(material, 'Normal', rx, ry)}",
        f'mask = expr(code="{mask}")',
        f'glass = expr(code="{glass}")',
        f'glass_r = expr(code="out = vec3({glass_rough})")',
        # grime: dark vertical streaks under the sills and a dirtier base, multiplied over the photograph
        f'grime = expr(code="s = fbm(p * vec2(14.0, 1.5)); k = 1.0 - {streak} * smoothstep(0.3, 0.9, s) - 0.25 * (1.0 - smoothstep(0.0, 0.25, p.y)); out = vec3(k) * {tint}")',
        "dirty = blend(grime, wall, one, blend_type=multiply, amount=1.0)",
        "albedo = blend(glass, dirty, mask, blend_type=normal, amount=1.0)",
        "roughness = blend(glass_r, wall_r, mask, blend_type=normal, amount=1.0)",
        "normal = wall_n",
    ])


GLASS = "g = smoothstep(0.0, 1.0, p.y * 0.9 + noise(p * vec2(6.0, 3.0)) * 0.35); out = mix(vec3(0.05, 0.08, 0.12), vec3(0.34, 0.44, 0.54), g)"
GLASS_DARK = "g = smoothstep(0.0, 1.0, p.y * 0.8 + noise(p * vec2(9.0, 4.0)) * 0.4); out = mix(vec3(0.03, 0.04, 0.06), vec3(0.2, 0.27, 0.34), g)"
GLASS_WARM = "l = step(0.82, hash(floor(p * vec2(8.0, 9.0)))); out = mix(vec3(0.07, 0.1, 0.14), vec3(0.8, 0.62, 0.34), l * 0.5)"

FACADES = [
    # glass-and-concrete office: 2 m panels with mullions, spandrels at the floor lines
    ("glass", facade("Concrete048", "fx = fract(p.x * 4.0); fy = fract(p.y * 3.0); out = step(0.07, fx) * step(0.07, 1.0 - fx) * step(0.24, fy) * step(fy, 0.97)", GLASS, "vec3(0.9, 0.92, 0.95)", streak=0.3)),
    # concrete block with ribbon windows
    ("concrete", facade("Concrete034", "fy = fract(p.y * 3.0); fx = fract(p.x); out = step(0.3, fy) * step(fy, 0.78) * step(0.04, fx) * step(fx, 0.96)", GLASS, "vec3(1.0, 0.98, 0.94)", rx=3, ry=5)),
    # red brick with small windows, warm interiors
    ("brick", facade("Bricks113A", "fx = fract(p.x * 4.0); fy = fract(p.y * 3.0); out = step(0.27, fx) * step(fx, 0.73) * step(0.36, fy) * step(fy, 0.8)", GLASS_WARM, rx=4, ry=5, glass_rough=0.2, streak=0.45)),
    # dark glass tower in tile cladding
    ("dark", facade("Tiles141", "fx = fract(p.x * 8.0); fy = fract(p.y * 3.0); out = step(0.1, fx) * step(0.1, 1.0 - fx) * step(0.2, fy)", GLASS_DARK, "vec3(0.55, 0.58, 0.62)", rx=6, ry=8, streak=0.2)),
    # painted plaster apartments with window pairs
    ("plaster", facade("Plaster001", "fx = fract(p.x * 4.0); fy = fract(p.y * 3.0); out = step(0.3, fx) * step(fx, 0.7) * step(0.3, fy) * step(fy, 0.85)", GLASS, "vec3(0.95, 0.85, 0.75)", rx=3, ry=4, streak=0.4)),
    # older brick, a second tone
    ("brick2", facade("Bricks106A", "fx = fract(p.x * 3.0); fy = fract(p.y * 3.0); out = step(0.2, fx) * step(fx, 0.8) * step(0.3, fy) * step(fy, 0.8)", GLASS, "vec3(0.9, 0.85, 0.8)", rx=4, ry=5, streak=0.5)),
]

# shop floor: dark storefront glass, a roller shutter band, a coloured sign above, over concrete
SHOP = "; ".join([
    f"wall = {picture('Concrete034', 'Color', 3, 2)}",
    f"wall_r = {picture('Concrete034', 'Roughness', 3, 2)}",
    f"wall_n = {picture('Concrete034', 'Normal', 3, 2)}",
    f"shutter = {picture('CorrugatedSteel009', 'Color', 2, 1)}",
    'front = expr(code="fx = fract(p.x * 2.0); out = step(0.05, fx) * step(0.05, 1.0 - fx) * step(0.03, p.y) * step(p.y, 0.3)")',
    'sign = expr(code="out = step(0.33, p.y) * step(p.y, 0.44)")',
    'signc = expr(code="out = hsv(fract(floor(p.x * 2.0) * 0.37 + 0.1), 0.6, 0.85)")',
    'glass = expr(code="out = mix(vec3(0.03, 0.05, 0.07), vec3(0.45, 0.4, 0.3), smoothstep(0.05, 0.3, p.y) * 0.5)")',
    "a1 = blend(signc, wall, sign, blend_type=normal, amount=1.0)",
    "albedo = blend(glass, a1, front, blend_type=normal, amount=1.0)",
    "roughness = wall_r",
    "normal = wall_n",
])
ROOF = "; ".join([
    ONE,
    f"tar ={picture('Asphalt033', 'Color', 3, 3)}",
    f"tar_r = {picture('Asphalt033', 'Roughness', 3, 3)}",
    f"tar_n = {picture('Asphalt033', 'Normal', 3, 3)}",
    'wear = expr(code="out = vec3(0.7 + 0.3 * fbm(p * 7.0))")',
    "albedo = blend(wear, tar, one, blend_type=multiply, amount=1.0)",
    "roughness = tar_r",
    "normal = tar_n",
])
ROAD = "; ".join([
    ONE,
    f"a ={picture('Asphalt033', 'Color', 6, 6)}",
    f"a_r = {picture('Asphalt033', 'Roughness', 6, 6)}",
    f"a_n = {picture('Asphalt033', 'Normal', 6, 6)}",
    f"worn = {picture('Road012A', 'Color', 1, 1)}",
    'patch = expr(code="out = vec3(0.65 + 0.35 * fbm(p * 3.0))")',
    "albedo = blend(patch, a, one, blend_type=multiply, amount=1.0)",
    "roughness = a_r",
    "normal = a_n",
])
PAVE = "; ".join([
    ONE,
    f"s ={picture('PavingStones151', 'Color', 4, 4)}",
    f"s_r = {picture('PavingStones151', 'Roughness', 4, 4)}",
    f"s_n = {picture('PavingStones151', 'Normal', 4, 4)}",
    'dirt = expr(code="out = vec3(0.75 + 0.25 * fbm(p * 5.0))")',
    "albedo = blend(dirt, s, one, blend_type=multiply, amount=1.0)",
    "roughness = s_r",
    "normal = s_n",
])

L = []
L.append("sky = scene.sky@1(sun_elevation=26, sun_azimuth=-50, sun_intensity=5, turbidity=4, clouds=true, cloud_cover=0.35, seed=4, haze=0.0)")
L.append("output sky = sky")
L.append("env = scene.environment@1(intensity=0.7, reflection=0.4, bounce=0.5, haze=0.0)")
L.append("output env = env")


def textured(name, source, program, size, rough=1.0):
    L.append(f"{name}_m = solid.material@1({source}, color=[1, 1, 1], roughness={rough})")
    L.append(f'{name}_t = solid.texture@1({name}_m, program="{esc(program)}", size="{size}")')


L.append(f'road = city.ground@1({LAYOUT}, part="road")')
textured("road", "road", ROAD, "1024")
L.append("output road = road_t")
L.append(f'pave = city.ground@1({LAYOUT}, part="pavement")')
textured("pave", "pave", PAVE, "1024")
L.append("output pavement = pave_t")
n = len(FACADES)
for s, (name, program) in enumerate(FACADES):
    L.append(f'w{s} = city.buildings@1({LAYOUT}, part="walls", styles={n}, style={s})')
    textured(f"w{s}", f"w{s}", program, "1024")
    L.append(f"output walls_{name} = w{s}_t")
    L.append(f'sh{s} = city.buildings@1({LAYOUT}, part="shop", styles={n}, style={s})')
    textured(f"sh{s}", f"sh{s}", SHOP, "512")
    L.append(f"output shop_{name} = sh{s}_t")
    L.append(f'rf{s} = city.buildings@1({LAYOUT}, part="roof", styles={n}, style={s})')
    textured(f"rf{s}", f"rf{s}", ROOF, "512")
    L.append(f"output roof_{name} = rf{s}_t")
open(out, "w").write("\n".join(L) + "\n")
print("wrote", out, len(L), "lines")

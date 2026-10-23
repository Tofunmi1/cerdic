#!/usr/bin/env python3
"""Shrinks the CC0 MakeHuman clothes and hair images to the sizes the viewer bakes with and writes them as PNGs a page can fetch.

    python3 -I outfit_textures.py <assets-dir> <out-dir> [size] [asset ...]

Each asset's `<name>_diffuse.png` and `<name>_normal.png` (when it has one) become `<out-dir>/<name>_diffuse.png` and `<name>_normal.png`, square, at most `size`
pixels (1024 by default). Hair keeps its alpha (the strands are cut out with it); the licence header of the asset's .mhmat must say CC0.
"""
import sys
from pathlib import Path

import numpy as np
from PIL import Image

DEFAULT = ["clothes/female_casualsuit01", "clothes/shoes02", "hair/long01", "hair/bob01", "hair/bob02", "hair/short01", "hair/short02", "hair/short03", "hair/short04", "hair/ponytail01"]


def solid_hair(image):
    """The hair pictures keep a black, half-transparent shadow under the strands (it blends in MakeHuman's renderer; ours only cuts out): make that region
    opaque hair colour, and give the cut-out pixels the same colour so no black fringe bleeds into the strands."""
    a = np.array(image).astype(np.float32)
    alpha = a[..., 3]
    opaque = alpha >= 200
    mean = a[..., :3][opaque].mean(axis=0)
    shadow = (alpha >= 60) & (alpha < 200)
    a[..., :3][shadow] = mean * 0.55
    a[..., 3][shadow] = 255
    black = a[..., :3].max(axis=2) < 24  # the picture's black filler beside the strands
    a[..., :3][black & (alpha >= 60)] = mean * 0.55
    a[..., 3][black & (alpha >= 60)] = 255
    clear = alpha < 60
    a[..., :3][clear] = mean * 0.55
    return Image.fromarray(a.astype(np.uint8), "RGBA")


def main():
    assets, out = Path(sys.argv[1]), Path(sys.argv[2])
    size = int(sys.argv[3]) if len(sys.argv) > 3 and sys.argv[3].isdigit() else 1024
    names = [a for a in sys.argv[3:] if not a.isdigit()] or DEFAULT
    out.mkdir(parents=True, exist_ok=True)
    for name in names:
        short = name.split("/")[1]
        head = "".join(line for line, _ in zip(open(assets / name / f"{short}.mhmat", errors="replace"), range(40)))
        if "CC0" not in head:
            raise SystemExit(f"{name}: the material header does not say CC0, refusing it")
        for kind in ("diffuse", "normal"):
            source = assets / name / f"{short}_{kind}.png"
            if not source.exists():
                continue
            image = Image.open(source)
            image = image.convert("RGBA" if kind == "diffuse" else "RGB")
            if kind == "diffuse" and name.startswith("hair/"):
                image = solid_hair(image)
            if max(image.size) > size:
                image = image.resize((size, size), Image.LANCZOS)
            target = out / f"{short}_{kind}.png"
            image.save(target, optimize=True)
            print(f"{target.name}: {image.size[0]}x{image.size[1]} {target.stat().st_size // 1024} KB")


if __name__ == "__main__":
    main()

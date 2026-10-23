#!/usr/bin/env python3
"""Copies the maps of the CC0 photographic materials (ambientCG, https://ambientcg.com, licence CC0 1.0) into the pictures the viewer fetches.

    python3 -I cc0_materials.py <unzipped-materials-dir> <out-dir> [Material ...]

`<unzipped-materials-dir>/<Material>/` holds an ambientCG `<Material>_1K-JPG` download, unzipped. Each material gives `<Material>_Color.jpg`, `<Material>_Roughness.jpg` and
`<Material>_Normal.jpg` (the OpenGL-convention normal map) in `<out-dir>`, ready for `image(name="<Material>_Color")` in a texture program. No pixel is changed; the files are
only renamed. `LICENSES.md` is written beside them naming every material and its source.
"""
import shutil
import sys
from pathlib import Path

DEFAULT = ["Concrete034", "Concrete048", "Bricks113A", "Bricks106A", "Asphalt033", "PavingStones151", "Plaster001", "CorrugatedSteel009", "Tiles141", "Road012A"]
MAPS = {"Color": "Color", "Roughness": "Roughness", "NormalGL": "Normal", "AmbientOcclusion": "AO"}


def main():
    source, out = Path(sys.argv[1]), Path(sys.argv[2])
    names = sys.argv[3:] or DEFAULT
    out.mkdir(parents=True, exist_ok=True)
    lines = ["# CC0 photographic materials", "", "From ambientCG (https://ambientcg.com), released under CC0 1.0 Universal (public domain); no attribution is required, the names are kept for the record.", ""]
    for name in names:
        found = 0
        for suffix, label in MAPS.items():
            picture = source / name / f"{name}_1K-JPG_{suffix}.jpg"
            if picture.exists():
                shutil.copyfile(picture, out / f"{name}_{label}.jpg")
                found += 1
        if not found:
            raise SystemExit(f"{name}: no maps found in {source / name}")
        lines.append(f"- {name}: https://ambientcg.com/view?id={name}")
        print(f"{name}: {found} maps")
    (out / "LICENSES.md").write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()

"""Extrait les pièces de l'écran Jouer d'un OBJ multi-objets (« Chess Pieces 3D Model », low poly) vers des .bin compacts.

Usage : python3 scripts/obj-to-pieces.py "low poly.obj" src/assets/models

Format .bin (petit-boutiste) : u32 nombre de sommets, u32 nombre d'indices, puis f32 positions (x, y, z),
f32 normales (x, y, z), u32 indices. Chaque pièce est recentrée (axe vertical sur l'origine, pied à y = 0)
et ramenée à une hauteur de 1.
"""
import struct
import sys
from pathlib import Path

# Objets de l'OBJ source qui forment chaque pièce (le roi a sa croix à part).
PIECES = {
    "pawn": ["Sphere001"],
    "king": ["Line007", "Line009"],
    "rook": ["Line002"],
    "knight": ["Line003", "Line005"],
}


def main(src: str, out: str) -> None:
    v: list[tuple[float, float, float]] = []
    vn: list[tuple[float, float, float]] = []
    faces: dict[str, list[list[tuple[int, int]]]] = {}
    group = "?"
    for line in open(src, encoding="latin-1"):
        parts = line.split()
        if not parts:
            continue
        if parts[0] == "v":
            v.append(tuple(map(float, parts[1:4])))
        elif parts[0] == "vn":
            vn.append(tuple(map(float, parts[1:4])))
        elif parts[0] == "g":
            group = parts[1]
        elif parts[0] == "f":
            corners = []
            for c in parts[1:]:
                ids = c.split("/")
                corners.append((int(ids[0]) - 1, int(ids[2]) - 1 if len(ids) > 2 and ids[2] else -1))
            faces.setdefault(group, []).append(corners)

    for name, groups in PIECES.items():
        tris = [f for g in groups for f in faces[g]]
        used = [v[i] for f in tris for i, _ in f]
        lo = [min(p[k] for p in used) for k in range(3)]
        hi = [max(p[k] for p in used) for k in range(3)]
        cx, cz = (lo[0] + hi[0]) / 2, (lo[2] + hi[2]) / 2
        scale = 1 / (hi[1] - lo[1])
        index: dict[tuple[int, int], int] = {}
        pos: list[float] = []
        nor: list[float] = []
        idx: list[int] = []
        for f in tris:
            ids = []
            for key in f:
                if key not in index:
                    index[key] = len(index)
                    x, y, z = v[key[0]]
                    pos += [(x - cx) * scale, (y - lo[1]) * scale, (z - cz) * scale]
                    nor += list(vn[key[1]]) if key[1] >= 0 else [0.0, 1.0, 0.0]
                ids.append(index[key])
            for k in range(1, len(ids) - 1):  # éventail : polygones -> triangles
                idx += [ids[0], ids[k], ids[k + 1]]
        data = struct.pack("<II", len(index), len(idx)) + struct.pack(f"<{len(pos)}f", *pos) + struct.pack(f"<{len(nor)}f", *nor) + struct.pack(f"<{len(idx)}I", *idx)
        Path(out, f"{name}.bin").write_bytes(data)
        print(name, len(index), "sommets", len(idx) // 3, "triangles", len(data), "octets")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])

#!/usr/bin/env python3
"""
Build derivative VRoid fixtures used by the VMK#237 canonical-content reproducer.

Takes the symlinked canonical VRoid baseline (`assets/humanoid/vroid_default_F_1_0.vrm`)
and produces derivatives with `VRMC_springBone_extended_collider` shapes added.
Per VRM 1.0 spec, an extended collider replaces the base shape semantically on
loaders that support the extension; non-supporting loaders fall back to the base
shape (if present).

The script produces:
  - `vroid_default_F_extcoll_headbubble.vrm`: spec-compliant form (no base shape,
    only extended). A tight `inside: true` sphere at the head node. Used to
    differentially diagnose extended-collider handling across renderers; if the
    extension is applied, hair joints are constrained to a head bubble of
    radius 0.1m and the render differs sharply from the baseline. If the
    extension is ignored, the render is byte-identical to the baseline.

Both forms exist because three-vrm 3.5.0 rejects spec-compliant extended-only
colliders (separate filing). The non-spec form (base sphere + extended) is a
workaround for strict-loader compatibility when needed.

Usage:
    python3 scripts/build-vroid-extcoll-reproducers.py

Idempotent — run multiple times produces byte-identical output (modulo JSON
key ordering, which Python's dict preserves insertion order since 3.7).
"""
import struct
import json
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "assets" / "humanoid" / "vroid_default_F_1_0.vrm"
DST_SPEC_COMPLIANT = ROOT / "assets" / "humanoid" / "vroid_default_F_extcoll_headbubble.vrm"

JSON_CHUNK_TYPE = 0x4E4F534A
BIN_CHUNK_TYPE = 0x004E4942


def read_glb(path: Path):
    with open(path, "rb") as f:
        magic, version, length = struct.unpack("<4sII", f.read(12))
        if magic != b"glTF":
            raise ValueError(f"not a glb: magic={magic!r}")
        chunks = []
        while f.tell() < length:
            chunk_length, chunk_type = struct.unpack("<II", f.read(8))
            chunks.append((chunk_type, f.read(chunk_length)))
        return version, chunks


def write_glb(path: Path, version: int, chunks):
    padded = []
    for ctype, cdata in chunks:
        pad_byte = b" " if ctype == JSON_CHUNK_TYPE else b"\x00"
        pad_len = (4 - len(cdata) % 4) % 4
        padded.append((ctype, cdata + pad_byte * pad_len))
    total_length = 12 + sum(8 + len(c[1]) for c in padded)
    with open(path, "wb") as f:
        f.write(struct.pack("<4sII", b"glTF", version, total_length))
        for ctype, cdata in padded:
            f.write(struct.pack("<II", len(cdata), ctype))
            f.write(cdata)


def find_head_node(gltf: dict) -> int:
    """Find J_Bip_C_Head node index — the standard VRM 1.0 head humanoid bone in VRoid output."""
    # First try humanoid metadata (spec-correct path)
    human_bones = gltf.get("extensions", {}).get("VRMC_vrm", {}).get("humanoid", {}).get("humanBones", {})
    if "head" in human_bones and "node" in human_bones["head"]:
        return human_bones["head"]["node"]
    # Fallback: by VRoid bone naming convention
    for i, node in enumerate(gltf.get("nodes", [])):
        if node.get("name") == "J_Bip_C_Head":
            return i
    raise ValueError("could not locate head node")


def build_headbubble_spec_compliant(src: Path, dst: Path):
    """Spec-compliant form: extended shape only, no base shape.

    Useful for testing whether a renderer's `VRMC_springBone_extended_collider`
    handler is engaged at all. Three-vrm 3.5.0 rejects this form (known
    separate issue).
    """
    version, chunks = read_glb(src)
    json_idx = next(i for i, (t, _) in enumerate(chunks) if t == JSON_CHUNK_TYPE)
    data = json.loads(chunks[json_idx][1].decode("utf-8"))

    head_node = find_head_node(data)

    extensions_used = data.setdefault("extensionsUsed", [])
    if "VRMC_springBone_extended_collider" not in extensions_used:
        extensions_used.append("VRMC_springBone_extended_collider")

    sb = data["extensions"]["VRMC_springBone"]
    new_collider_idx = len(sb["colliders"])
    sb["colliders"].append({
        "node": head_node,
        "extensions": {
            "VRMC_springBone_extended_collider": {
                "shape": {
                    # `sphere` with `inside: true` — head-bubble containment.
                    # Tight radius (0.1m) so hair joints are actively constrained
                    # if the extension is honored. If the extension is ignored
                    # the derivative file renders identically to the baseline.
                    "sphere": {"offset": [0, 0, 0], "radius": 0.1, "inside": True},
                },
            },
        },
    })

    new_group_idx = len(sb["colliderGroups"])
    sb["colliderGroups"].append({
        "name": "head_inside_bubble_tight",
        "colliders": [new_collider_idx],
    })

    hair_count = 0
    for spring in sb["springs"]:
        if spring.get("name") == "Hair":
            spring.setdefault("colliderGroups", []).append(new_group_idx)
            hair_count += 1

    new_json = json.dumps(data, separators=(",", ":")).encode("utf-8")
    chunks[json_idx] = (JSON_CHUNK_TYPE, new_json)
    write_glb(dst, version, chunks)
    return new_collider_idx, new_group_idx, hair_count


def main():
    if not SRC.exists():
        print(
            f"error: source fixture {SRC} not found.\n"
            f"       run scripts/install-humanoid-fixtures.sh first to materialize the symlink.",
            file=sys.stderr,
        )
        return 1

    print(f"==> Building VMK#237 canonical-content reproducer from {SRC.name}")
    ci, gi, hc = build_headbubble_spec_compliant(SRC, DST_SPEC_COMPLIANT)
    print(f"    {DST_SPEC_COMPLIANT.name}:")
    print(f"      collider[{ci}] at head node — extended sphere r=0.1 inside=true (no base shape)")
    print(f"      collider group[{gi}] referenced by {hc} Hair springs")
    print(f"      size: {os.path.getsize(DST_SPEC_COMPLIANT):,} bytes")
    return 0


if __name__ == "__main__":
    sys.exit(main())

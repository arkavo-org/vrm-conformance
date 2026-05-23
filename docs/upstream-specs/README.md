# Upstream spec mirrors

Local shallow clones of the VRM and glTF specifications so you can `grep`
them offline without round-tripping to GitHub. These trees are
**gitignored** — they're not committed to this repo. Re-fetch with
`scripts/install-upstream-specs.sh` (or the one-liner below) when you
clone vrm-conformance fresh.

## What's here

| path | upstream | size | what it covers |
|---|---|---|---|
| `vrm-specification/` | [vrm-c/vrm-specification](https://github.com/vrm-c/vrm-specification) | ~38 MB | Every VRM extension at every version (VRMC_vrm, VRMC_materials_mtoon, VRMC_springBone, VRMC_springBone_extended_collider, VRMC_vrm_animation, VRMC_node_constraint, VRMC_materials_hdr_emissiveMultiplier, plus the VRM 0.0 tree) |
| `glTF/` | [KhronosGroup/glTF](https://github.com/KhronosGroup/glTF) | ~163 MB | glTF 2.0 base spec (`specification/2.0/Specification.adoc`) + every KHR/EXT extension (KHR_materials_unlit, KHR_lights_punctual, KHR_texture_transform, etc.) |

## Why local

We've round-tripped to GitHub several times when an upstream issue or PR
review came down to spec interpretation:

- **VMK#286** ("does the lookAt `node` reference a translation or
  rotation track?") needed the exact wording from
  `vrm-specification/specification/VRMC_vrm_animation-1.0/README.md`.
- **VMK#213 / #239** (MToon shadingShift / shadingToony curve) referenced
  `vrm-specification/specification/VRMC_materials_mtoon-1.0/README.md`
  multiple times.
- **VMK#183** (Half-Lambert remap) needed cross-references between the
  MToon spec and the glTF base material spec.

Having the text on disk lets you `grep -rn "lookAtTargetSampler"
vrm-specification/` or `rg "alpha-to-coverage"
glTF/extensions/2.0/Khronos/` instead of three browser tabs.

## Re-fetch

```bash
# from repo root
scripts/install-upstream-specs.sh
```

Or directly:

```bash
mkdir -p docs/upstream-specs && cd docs/upstream-specs
git clone --depth 1 https://github.com/vrm-c/vrm-specification.git
git clone --depth 1 https://github.com/KhronosGroup/glTF.git
```

Shallow clones (no history) keep the disk footprint down. Re-clone any
time you want the latest spec text — both upstream repos are actively
maintained and the spec wording does drift (notably the VRMA `lookAt`
section, which has been edited at least twice during 2026).

## Pinning

If you want to lock to a specific commit (e.g., the conformance suite is
being run against a specific dated snapshot for an upstream PR
discussion), record the commit SHAs in your finding or RFC. Don't try
to git-track this directory — the spec repos are too big and change too
often to want as a vendored dependency.

## Quick references

Most-used files:

```
vrm-specification/specification/VRMC_vrm-1.0/README.md
vrm-specification/specification/VRMC_materials_mtoon-1.0/README.md
vrm-specification/specification/VRMC_springBone-1.0/README.md
vrm-specification/specification/VRMC_springBone_extended_collider-1.0/README.md
vrm-specification/specification/VRMC_vrm_animation-1.0/README.md
vrm-specification/specification/VRMC_vrm_animation-1.0/how_to_transform_human_pose.md
glTF/specification/2.0/Specification.adoc
glTF/extensions/2.0/Khronos/KHR_materials_unlit/README.md
```

Schema files (JSON Schema) live under `…/schema/` alongside each README
and are useful when the wording is ambiguous — the schema is what
validators implement against.

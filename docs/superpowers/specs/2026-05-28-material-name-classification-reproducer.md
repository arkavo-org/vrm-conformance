# Conformance reproducer — material-name render classification (VMK `Vita_clothing` z-fighting)

**Status:** Draft spec for a conformance-suite reproducer. No code written yet (authored during a transport-degraded session; the one unconfirmed fact is flagged for execution-time verification). Conformance-side only — no VRMMetalKit changes (report-and-find boundary; the VMK fix is the maintainer's call).

**Belongs to:** the `_v0_quirk_*` quirk-sweep family from the VRM 0.x design (`2026-05-26-vrm-0x-conformance-design.md` line 95), and was flagged as the partially-covered gap in the slice-2 plan self-review (`2026-05-28-vrm-0x-conformance-slice2.md`). This is that family's first member.

---

## What this catches

VMK overrides glTF/VRM `material.doubleSided` based on a **substring of the material name**. A single-material VRM 0.0 outfit named `Vita_clothing` (contains `cloth`) is silently force-double-sided and given an overlay depth bias intended for layered VRChat-style avatars, producing silhouette z-fighting fringe and dark backface bleed. This is the same defect *class* as the 180° flip (VMK#299): a heuristic overriding declared spec data.

The current corpus is **blind** to it: the only related asset, `mtoon_doubleSided_{true,false}` (`sweep.rs:87-94`), sets the glTF flag but its material names (`mtoon_doubleSided_true`) contain none of the trigger tokens, so the name heuristic never fires. No methodology pin states that culling must honor `doubleSided` rather than the name. So there is nothing to conform *against* — this reproducer + pin create the spec.

## Root cause (verified in VMK at pinned rev `392d949`, citations corrected)

Single root cause, not two. The chain:

1. **Name match** — `Sources/VRMMetalKit/Renderer/VRMRenderer.swift:1866-1869`: material name containing `cloth`/`tops`/`bottoms`/`skirt`/`shorts`/`pants` → `faceCategory = "clothing"`. (Original analysis cited `VRMRenderItemBuilder.swift:189` — wrong file/line; the match is in `VRMRenderer.swift`.)
2. **Forced double-sided (THE defect)** — `Sources/VRMMetalKit/Renderer/Systems/VRMRenderItemBuilder.swift:215-216`: any non-nil `faceCategory` sets `effectiveDoubleSided = true` unconditionally, ignoring the glTF material's actual `doubleSided`. (Citation correct; note file is under `Renderer/Systems/`.)
3. **Downstream effects** — clothing draw path `VRMRenderer.swift:2976-2987`:
   - `setCullMode(isDoubleSided ? .none : .back)` (line 2983) — so cull-to-none is **downstream of** step 2, NOT an independent cause. (Original analysis claimed an unconditional `setCullMode(.none)`; the real code is conditional on `isDoubleSided`. This corrects the "two damaging things" framing to one cause + its consequences.)
   - overlay depth bias `depthBias(for: materialName, isOverlay: true)` (line 2986) → `DepthBiasCalculator.swift`: `"Cloth"`/`"Clothing"` base `0.015` (line 58-59) + overlay `0.010` (line 93) = `0.025`, `slopeScale 2.0` (line 122), clamp `0.1`. `DepthBiasCalculator` is in `GLTFCore/Utilities/`, not `Renderer/`.

The `slopeScale: 2.0` term explodes at edge-on (silhouette) surfaces → the fringe; the forced double-sided draws inward-normal backfaces → the dark bleed. Both trace to **`VRMRenderItemBuilder.swift:216`**.

## Reproducer design (generator sweep)

A new sweep `material_name_classification` that emits the **same MToon material** under names that trip the heuristic vs. names that don't, crossed with the glTF `doubleSided` flag. A conformant renderer's output is **invariant to the material name** at a fixed `doubleSided`; a name-heuristic renderer diverges on the `*cloth*` variants.

Variants (6) — all identical MToon params except `id` (→ material name) and `double_sided`:

| variant id | trips heuristic? | double_sided | expected vs control |
|---|---|---|---|
| `matname_plain_singlesided` | no (control) | false | baseline |
| `matname_clothing_singlesided` | **yes** (`clothing`) | false | **must equal control** — divergence = bug |
| `matname_skirt_singlesided` | yes (`skirt`) | false | must equal control |
| `matname_plain_doublesided` | no (control) | true | baseline (double-sided) |
| `matname_clothing_doublesided` | yes | true | must equal `matname_plain_doublesided` |
| `matname_body_singlesided` | no (`body` → different category) | false | guards the body-category path |

**Conformance read:** within a `double_sided` value, all variants should render identically (consensus SSIM ~1.0 among conformant renderers). On VMK, the `*clothing*`/`*skirt*` single-sided variants will diverge from `matname_plain_singlesided` (forced double-sided + bias). This reproduces the `Vita_clothing` artifact deterministically — geometry is the standard sweep sphere, no GPU/Muse model needed; the signal is name-induced, not geometry-induced.

Emit at both `--spec-version 0.x` (the `Vita_clothing` asset is VRM 0.0) and `1.0` (the heuristic is version-agnostic) once slice 2 lands `--spec-version` on sweeps.

## Load-bearing fact: CONFIRMED — `MToonParams.id` *is* the material name

The reproducer depends on `MToonParams.id` becoming the emitted glTF `material.name`. **Verified in source:**
- v0 path: `src/mtoon_v0.rs:26` → `"name": params.id` (with unit test at `mtoon_v0.rs:123` asserting `v["name"] == "custom_id_v0"`).
- v1 path: `src/vrm_ext.rs:324` → `"name": p.id`.

So setting distinct `id`s is sufficient to drive distinct material names — **no `material_name` field needed**. Caveat: the `id` also names the mesh/geom nodes (`emit.rs:50/104` use `"{id}_mesh"` etc.), but those don't contain the trip tokens as a standalone word unless the id does, and VMK's heuristic matches on the *material* name specifically (`item.materialNameLower`). The reproducer ids (`matname_clothing_singlesided`, etc.) are the material names that matter.

Other confirmed facts: `MToonParams` has `pub id: String` (`params.rs:12`) and `pub double_sided: bool` (`params.rs:141`); sweep idiom is `let mut p = MToonParams::defaults(format!(...)); p.double_sided = v; out.push(p);` (`sweep.rs:84-88`); `mtoon_basic_sweep() -> Vec<MToonParams>`.

## Task outline (TDD; fill exact code at execution after the verification above)

1. **Confirm/enable distinct material names** (see verification). Commit if a `material_name` field is added.
2. **`material_name_classification_sweep() -> Vec<MToonParams>`** in `sweep.rs` — the 6 variants above. Unit test: asserts 6 variants, correct names/flags, and that the trip-token variants' material names contain `cloth`/`skirt`.
3. **CLI arm `emit-material-name-classification-sweep`** mirroring `emit-sweep` (with `--spec-version` per slice 2). Integration test: emits 6 triplets; the `*clothing*` variant's `.vrm` JSON `material.name` contains `cloth` and `material.doubleSided == false`.
4. **Methodology pin** (below).
5. **Render + findings** (execution, real adapters): consensus-diff within each `double_sided` group; expect VMK to flag the `*cloth*`/`*skirt*` single-sided variants as outliers. Append `docs/findings.md`.

## Methodology pin (draft text for `docs/methodology.md`)

> **Face culling honors `material.doubleSided`, not material name.** glTF 2.0 and VRM make `material.doubleSided` the sole authority on back-face culling. A conformant renderer MUST NOT change culling, depth bias, or render category based on substrings of the *material name* (e.g. `cloth`, `skirt`, `body`). The `material_name_classification` sweep emits one MToon material under heuristic-tripping and control names at each `doubleSided` value; conformant output is invariant to the name. Name-based reclassification (observed in VRMMetalKit: `cloth`→forced-double-sided + overlay depth bias, `VRMRenderItemBuilder.swift:216`) is a conformance failure — the same defect class as orientation-from-heuristic (VMK#299). Renderers MAY use material name for *non-visible* optimizations only.

## Boundary note

Everything here is `vrm-conformance` (generator + methodology + findings). The VMK fix — gating the clothing-overlay path so it stops misfiring on single-material outfits (your three options: require a body-skin layer / respect glTF `doubleSided` / drop name detection) — is **not** in scope here and is the VMK maintainer's decision. This reproducer makes that fix *verifiable* whichever option is chosen: a correct fix turns the `*cloth*` single-sided variants from outliers into consensus matches.

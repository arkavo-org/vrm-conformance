# Spec-test re-evaluation — `material.doubleSided` back-face culling

**Trigger:** A renderer bug (VMK forces double-sided by material name; `Vita_clothing` z-fighting) prompted the question "does the suite catch this?" Per the conformance principle — **when a bug is found, re-evaluate the spec test first; do not build a reproducer for the implementation's bug** — this re-examines whether the suite faithfully encodes the `doubleSided` spec requirement. It does not.

## The spec requirement

glTF 2.0, Materials (`docs/upstream-specs/glTF/specification/2.0/Specification.adoc:2164`):

> The `doubleSided` property specifies whether the material is double sided.
> When this value is **false, back-face culling is enabled**, i.e., only front-facing triangles are rendered.
> When this value is **true, back-face culling is disabled** and double sided lighting is enabled. The back-face MUST have its normals reversed before the lighting equation is evaluated.

So `material.doubleSided` is the **sole authority** on back-face culling. A conformant renderer culls back-faces iff `doubleSided == false`, regardless of anything else (material name, render category, etc.).

## What the existing spec test actually does — and why it's inadequate

- The corpus's only `doubleSided` coverage is `mtoon_doubleSided_{false,true}` (`sweep.rs:83-88`): two assets differing only in the flag.
- **Every generator asset renders on a closed convex UV-sphere** — `mesh.rs` exposes exactly one fixture, `sphere(radius, lat, lon)`, and `emit.rs:37,258` hardcode `sphere(0.3, 24, 48)`.
- **On a closed convex sphere, back-faces are never visible** — the sphere's own front-faces always occlude its back-faces. So whether back-face culling is on (`doubleSided=false`) or off (`doubleSided=true`), the rendered pixels are identical.

**Conclusion:** the test cannot observe the property it claims to test. It verifies the flag is *plumbed* (the `.test.yaml` carries `double_sided`), not that the renderer *honors* it. Confirmed empirically by the material-name sweep render (2026-05-28 findings entry): all variants byte-identical on the sphere, across both three-vrm and VMK, precisely because the geometry hides culling.

This is a **suite inadequacy**, independent of any renderer. It would let *any* non-conformant culling behavior pass — VMK's name-heuristic is just one example.

## The spec-faithful fix (geometry that can observe culling)

Render an **open / single-layer** surface oriented so a **back-face is in frame**:

- Add an open fixture to `mesh.rs` — a single quad (plane), CCW-front-facing toward +Z, matching the existing convention (sphere front faces +Z).
- A `doubleSided` spec test emits that quad as one material at `doubleSided ∈ {false, true}`, viewed from the **back** of the quad (camera on the −Z side, or the quad rotated 180° so its back faces the +Z camera).
- **Expected, per spec:** `doubleSided=false` → back-face culled → the surface disappears, only background shows. `doubleSided=true` → back-face rendered (normals reversed for lighting) → the surface shows. The two renders MUST differ.
- **Assertion:** a conformant renderer produces visibly different output between the two (low SSIM / "background-colored back region when false"); a renderer that culls by anything other than the flag fails naturally — no implementation knowledge encoded.

The material-name axis (`cloth`/`skirt`/etc.) becomes a *corollary* of this test, not its design: hold `doubleSided=false`, vary only the material name on the back-facing quad; a conformant renderer culls all of them identically (surface gone), a name-heuristic renderer leaves the `cloth`-named one visible. That exposes VMK's specific defect — but only as a consequence of the spec-faithful test, never as its purpose.

## Status / boundary

- **Re-evaluation: done.** The `doubleSided` spec test is inadequate (convex geometry cannot observe culling); the fix is an open-mesh, back-face-in-frame test encoding the glTF culling rule directly.
- **Implementation: NOT done.** Building it requires a new `mesh.rs` fixture + `emit.rs` geometry routing + a back-facing camera + a difference assertion — winding-sensitive code that must be written against reliable source reads. Deferred until the editing transport is reliable (this session was corrupting source reads; writing winding code blind risks a test that silently asserts the inverse of the spec).
- Conformance-side only; no VMK changes; no fix-option chosen for VMK. The earlier "reproducer v2 on thin/layered geometry" framing (in `2026-05-28-material-name-classification-reproducer.md`) is **superseded** by this spec-first framing: the goal is a faithful `doubleSided` culling test, not a VMK reproducer.

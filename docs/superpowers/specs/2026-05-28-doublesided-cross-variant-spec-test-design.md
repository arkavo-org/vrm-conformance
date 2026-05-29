# Design — `doubleSided` cross-variant spec test (open-mesh back-face culling)

**Status:** Approved (2026-05-28). Implementation pending.

**Builds on:** [`2026-05-28-doubleSided-spec-test-reevaluation.md`](2026-05-28-doubleSided-spec-test-reevaluation.md),
which established (a) the existing `mtoon_doubleSided_{false,true}` coverage is inadequate because a
closed convex sphere never shows its back-faces, and (b) the spec-faithful fix is an open mesh viewed
from behind. That re-evaluation is done; the `quad()` fixture with self-verified +Z winding is committed
(`fbdb044`). This document designs the remaining implementation: the emit path, the cross-variant
difference assertion, and the CLI/runner surface.

## The spec requirement (recap)

glTF 2.0 §Materials: `material.doubleSided` is the **sole authority** on back-face culling. When `false`,
back-face culling is enabled (only front-facing triangles render). When `true`, culling is disabled and
the back-face has its normals reversed for lighting. A conformant renderer culls back-faces iff
`doubleSided == false`, regardless of material name, render category, or anything else.

## Observable

Render an open single-quad surface from **behind** (camera on the −Z side of a +Z-front-facing quad):

- `doubleSided=false` → back-face culled → frame is all-background.
- `doubleSided=true` → back-face rendered → colored quad fills the frame.

The two renders of a conformant renderer **MUST differ**. The assertion is therefore a **cross-variant
SSIM** between the two renders of the *same* renderer, passing iff the SSIM is **below** a maximum (they
diverged). A renderer that ignores the flag — e.g. one that force-doubles a `cloth`-named material
(VMK's `Vita_clothing` defect) — renders both variants identically, SSIM ≈ 1.0, and fails. No
implementation knowledge is encoded; non-conformant culling fails naturally.

## Why cross-variant SSIM (not a property assertion)

The existing `PropertyAssertion` measures foreground luminance within the avatar bounding box, and
**errors** (`PropertyError::EmptyBbox`) on an all-background frame — which is exactly the `doubleSided=false`
(culled) case. The current vocabulary literally cannot express "surface correctly absent" as a passing
condition. Cross-variant SSIM is also the closest match to the re-evaluation doc's "the two renders MUST
differ (low SSIM)" phrasing, and it reuses the existing `ssim_pngs` primitive.

## Renderers

Real-adapter-first; **UniVRM is the golden** (already `reference_renderer: "univrm"` in every default
plan). The conformance baseline is established by confirming the UniVRM false/true pair diverges below
`max_ssim`; three-vrm, VMK, and godot are then checked against the same gate. The deterministic mock
renderer is **not involved** — this is a spec-conformance test exercised through real rasterizing
adapters via the bootstrap-goldens flow, not the GPU-less mock smoke path. No mock culling contract is
added.

## Components

### 1. Geometry & camera — `emit_vrm_doublesided_quad` (`vrm-asset-generator/src/emit.rs`)

Modeled on `emit_vrm`, with these deltas:

- Mesh = `quad(0.3)` (a 0.6 m square, ≈ the head-mounted sphere's framing). **No morph targets** — the
  viseme deltas are omitted to remove a confounder; the primitive carries no `targets`.
- Minimal humanoid skeleton retained (a valid VRM 1.0 requires `VRMC_vrm.humanoid.humanBones`). The
  rest pose is pure translation (every bone in `humanoid.rs` carries only `translation`, no rotation),
  so the quad's +Z front-normal survives unrotated into world space. The quad mesh node is parented to
  `head` (world y = 1.36), exactly as `emit_vrm` parents the sphere.
- Material 0 = `base_material(params)`, carrying `double_sided` from `MToonParams`.
- Methodology baseline unchanged: `tone_mapping: none`, `cast_shadows: false`, `receive_shadows: false`.

### 2. Camera — back-facing (in the emitted test plan)

`position: [0.0, 1.36, -1.5]`, `target: [0.0, 1.36, 0.0]`, `up: [0.0, 1.0, 0.0]`, `fov_degrees: 30.0`.

This views the quad's **back** face. Chosen over rotating the quad 180° about Y specifically to avoid
confounding with VMK's documented 180°-flip bug (VMK#299) — the geometry stays canonical and only the
camera moves.

### 3. Test-plan schema (`vrm-test-plan`)

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub cross_variant: Option<CrossVariantAssertion>,

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossVariantAssertion {
    /// Test id (and asset stem) of the sibling variant to compare against.
    pub sibling_id: String,
    /// Pass iff ssim(this_render, sibling_render) <= max_ssim — i.e. the
    /// two variants MUST visibly differ.
    pub max_ssim: f32,
}
```

Only the **`false`** variant's plan carries the block (`sibling_id: "doublesided_quad_true"`,
`max_ssim: 0.85`) — a single, non-redundant declaration. `#[serde(default)]` + `skip_serializing_if`
keep the existing 80-test corpus byte-identical (no `cross_variant` key emitted where absent). The
sidecar is the single source of truth for the threshold, consistent with the generator's
"never hand-authored plans" philosophy.

**Threshold rationale:** `max_ssim = 0.85` aligns with the documented cross-renderer conformance band
(`docs/methodology.md`, `vrm-conformance#2`). The conformant gap here is enormous (all-magenta
background vs a full colored quad → SSIM well under 0.5), so 0.85 has wide margin; a name-heuristic
renderer that renders both identically sits at SSIM ≈ 1.0 and fails clearly.

### 4. Diff engine (`vrm-diff-engine`)

Reuse `ssim_pngs`; add a thin inverted-assertion wrapper:

```rust
pub struct CrossVariantResult { pub ssim: f64, pub max_ssim: f64, pub passed: bool }

pub fn cross_variant_diff(
    false_png: &Utf8Path,
    true_png: &Utf8Path,
    max_ssim: f64,
) -> Result<CrossVariantResult, SsimError>;
// passed = ssim <= max_ssim
```

### 5. Runner subcommand (`vrm-runner`)

```
cross-variant-diff --plan <false.test.yaml> --render-false <png> --render-true <png> [--json]
```

Reads `max_ssim` from `plan.cross_variant` (sidecar = single source of truth; errors if the plan has no
`cross_variant` block), computes `cross_variant_diff`, prints the result, and exits non-zero on fail —
mirroring the standalone `diff` subcommand's exit-gated contract. Added to the `describe` operation
catalog. Slots into the "renderers submit PNGs, the runner compares" trust model: both PNGs are inputs.

### 6. CLI arm (`vrm-asset-generator`)

```
emit-doublesided-spec-test --output-dir <dir> [--json]
```

Emits **two triplets** — `doublesided_quad_false` and `doublesided_quad_true` — identical except for the
`double_sided` flag and the `false` variant's `cross_variant` block. Both use the back-facing camera.
Follows the `emit-material-name-classification-sweep` handler shape (NDJSON progress on stderr, structured
summary on stdout under `--json`).

## Testing (TDD)

- **mesh** — winding already covered (`quad_front_face_normal_points_plus_z`, committed).
- **emit** — parse the emitted GLB JSON and assert: quad geometry (4 verts / 6 indices), the primitive
  carries **no** `targets`, `materials[0].doubleSided` matches the param, and the test plan's camera is
  on the −Z side.
- **schema** — `CrossVariantAssertion` serde round-trip; `cross_variant` absent from plans that don't set
  it (corpus byte-stability).
- **diff engine** — `cross_variant_diff`: two identical PNGs → `passed = false` (SSIM 1.0 > max);
  two divergent PNGs (e.g. all-magenta vs a filled rectangle) → `passed = true`.
- **runner** — `cross-variant-diff` exit code (0 on differ, non-zero on identical) and `--json` shape;
  errors cleanly when the plan lacks `cross_variant`.

## Out of scope (deferred)

- **Material-name corollary** — re-pointing the 6-variant `material_name_classification_sweep` onto the
  quad geometry so VMK's name-heuristic defect is observable end-to-end. This becomes a straightforward
  follow-up once the core pair exists (the spec test it's a corollary of).
- **VMK fixes** (the 180° flip VMK#299, the material-name double-sided misfire) — maintainer's call;
  conformance-side only here.
- **Site badging** of cross-variant pairs.

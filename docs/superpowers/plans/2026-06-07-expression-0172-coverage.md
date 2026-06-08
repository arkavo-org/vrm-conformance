# VMK 0.17.2 expression/morph (#333) Coverage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cover VMK 0.17.2's VRM 1.0 facial-expression fix (#333) with a real-avatar (`vroid_default_F_1_0`) full-preset expression corpus, and bump the VMK pin to 0.17.2.

**Architecture:** A new `emit-expression-clips` subcommand emits VRMA preset-expression clips (reusing the existing expression-channel primitive) that pair with the real VRoid avatar via committed manual plans. #333 froze blink/visemes/emotions on VRM 1.0 avatars where face node index ≠ mesh index; the synthetic corpus is blind (no blink/happy/sad morphs; visemes silently frozen), so the real avatar is the vehicle. Verification (frozen→deforming before/after) runs locally on macOS 26.

**Tech Stack:** Rust (vrm-asset-generator), YAML test plans, Swift (VMK adapter — pin only).

**Spec:** `docs/superpowers/specs/2026-06-07-expression-0172-coverage-design.md`

**Pattern note:** This mirrors the just-merged gaze coverage (`docs/superpowers/plans/2026-06-07-lookat-0171-coverage.md`); reuse its `emit-gaze-sweep`/`emit_gaze_clip` shapes as the reference for the new `emit-expression-clips`/`emit_expression_clip`.

---

### Task 1: Bump VMK pin to 0.17.2

**Files:**
- Modify: `adapters/vrm-metal-kit/Package.swift` (the VRMMetalKit `.package(...)` `revision`, currently `421232b75c77d65d8d2bd827a36159936b68db23`, and the comment block above the `// 0.17.1 (...)` entry)

- [ ] **Step 1: Update the revision and prepend a 0.17.2 changelog comment**

Change the VRMMetalKit `.package(url: ..., revision: "...")` revision from `421232b75c77d65d8d2bd827a36159936b68db23` to `3737e76b1635f9be604e4a8cb4272b5ddbedb58d` (tag `0.17.2`).

Directly above the existing line beginning `// 0.17.1 (commit 421232b, patch release 2026-06-08, closes #332)`, insert:

```swift
        // 0.17.2 (commit 3737e76, patch release 2026-06-08, closes #333) —
        // restores VRM 1.0 facial expressions. Behaviour change (no shader/
        // metallib change vs 0.17.1):
        //   - **VRM 1.0 morph binds were keyed by node, not mesh.** A 1.0
        //          expression `morphTargetBind.node` is a glTF *node* index,
        //          but the renderer and `VRMExpressionController` key morph
        //          weights by *mesh* index (0.x binds already carry the mesh
        //          index). The 1.0 loader stored the raw node index, so on any
        //          model whose face node index ≠ mesh index, every morph bind
        //          matched no primitive and the morph compute pass skipped it —
        //          blink, the five visemes, and every emotion preset silently
        //          produced no mesh deformation. The loader now resolves
        //          `node → nodes[node].mesh` into a resolved `meshIndex` while
        //          preserving the authored `node` for round-trip.
        //   Bone-driven look-at was unaffected (different path), which is why
        //   only *expressions* looked dead; VRM 0.x never hit it. Repro:
        //   `vroid_default_F_1_0` blink bind node=211 → mesh 0. This suite's
        //   new `vroid_default_F_expr_*` corpus (this commit) is the verifier —
        //   the synthetic humanoid corpus has no blink/happy/sad morphs and its
        //   visemes were silently frozen (node 19 ≠ mesh 0). Also adds
        //   `renderer.setExpression(_:weight:)` (additive). Rendering/
        //   before-after verification is local-only (macOS 26 / Xcode 26).
```

- [ ] **Step 2: Verify SPM resolves the new revision**

Run: `cd adapters/vrm-metal-kit && swift package resolve`
Expected: resolves `VRMMetalKit` at `3737e76...` with no error.

- [ ] **Step 3: Commit**

```bash
git add adapters/vrm-metal-kit/Package.swift
git commit -m "deps(vmk): bump VRMMetalKit 0.17.1 -> 0.17.2 (VRM 1.0 expressions #333)"
```

---

### Task 2: Add `expression_clip_sweep()`

**Files:**
- Modify: `crates/vrm-asset-generator/src/sweep.rs` (add `expression_clip_sweep()` after `vrma_expression_sweep()`, ~line 1395, + a test in a new `#[cfg(test)]` module)

Reuses the existing `crate::vrma_params::VrmaExpressionParams` type (no new struct). Emits the 11 preset clips with ids `expr_<name>` (so the manual plans can reference `expr_blink.vrma` etc.).

- [ ] **Step 1: Write the failing test**

Add to `crates/vrm-asset-generator/src/sweep.rs` (new module, following the file's `#[cfg(test)] mod ..._tests` convention):

```rust
#[cfg(test)]
mod expression_clip_sweep_tests {
    use super::*;

    #[test]
    fn expression_clip_sweep_has_11_presets_with_expr_ids() {
        let sweep = expression_clip_sweep();
        assert_eq!(sweep.len(), 11);
        let ids: Vec<&str> = sweep.iter().map(|p| p.id.as_str()).collect();
        for name in [
            "blink", "happy", "angry", "sad", "relaxed", "surprised", "aa", "ih", "ou", "ee",
            "oh",
        ] {
            assert!(
                ids.contains(&format!("expr_{name}").as_str()),
                "missing expr_{name}"
            );
        }
        // All preset, duration 1.0, expression_name is the bare preset name.
        for p in &sweep {
            assert!(p.is_preset, "{} should be preset", p.id);
            assert_eq!(p.duration_s, 1.0);
            assert_eq!(p.id, format!("expr_{}", p.expression_name));
        }
    }
}
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `cargo test -p vrm-asset-generator expression_clip_sweep_has_11`
Expected: FAIL — `cannot find function expression_clip_sweep`.

- [ ] **Step 3: Add `expression_clip_sweep()`**

In `crates/vrm-asset-generator/src/sweep.rs`, after `vrma_expression_sweep()`:

```rust
/// Real-avatar expression clip sweep (11 preset clips) covering VMK 0.17.2 #333.
/// Reuses `VrmaExpressionParams`; ids are `expr_<name>` so manual plans pair the
/// real VRoid avatar with `expr_<name>.vrma`. Presets only — #333 is about preset
/// morph binds (blink, the five visemes, and the emotion presets); custom
/// expressions are out of scope.
pub fn expression_clip_sweep() -> Vec<crate::vrma_params::VrmaExpressionParams> {
    use crate::vrma_params::VrmaExpressionParams;
    [
        "blink", "happy", "angry", "sad", "relaxed", "surprised", "aa", "ih", "ou", "ee", "oh",
    ]
    .iter()
    .map(|n| VrmaExpressionParams {
        id: format!("expr_{n}"),
        expression_name: (*n).into(),
        is_preset: true,
        duration_s: 1.0,
    })
    .collect()
}
```

- [ ] **Step 4: Run the test to confirm it passes**

Run: `cargo test -p vrm-asset-generator expression_clip_sweep_has_11`
Expected: PASS.

- [ ] **Step 5: fmt + clippy + commit**

Run: `cargo fmt -p vrm-asset-generator && cargo clippy -p vrm-asset-generator --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/vrm-asset-generator/src/sweep.rs
git commit -m "feat(asset-gen): expression_clip_sweep (11 preset clips, VMK #333 coverage)"
```

---

### Task 3: Add `emit_expression_clip()`

**Files:**
- Modify: `crates/vrm-asset-generator/src/emit.rs` (add `emit_expression_clip()` after `emit_vrma_expression_triplet`; the existing `emit_gaze_clip` is the structural reference)

Emits only `{id}.vrma` (no `.vrm`/`.test.yaml`). Builds the canonical skeleton + registers all humanoid bones (UniVRM importer invariant, matching `emit_gaze_clip`), appends an expression target node, and adds the preset weight channel (0 → 1 → 0 ramp over `duration_s`).

- [ ] **Step 1: Write the failing tests**

Add a new test module to `crates/vrm-asset-generator/src/emit.rs`:

```rust
#[cfg(test)]
mod expression_clip_emit_tests {
    use super::*;
    use crate::vrma_params::VrmaExpressionParams;
    use camino::Utf8Path;
    use tempfile::tempdir;

    fn doc_of(path: &Utf8Path) -> serde_json::Value {
        let bytes = std::fs::read(path).unwrap();
        let json_chunk = crate::glb::extract_json_chunk(&bytes).unwrap();
        serde_json::from_slice(&json_chunk).unwrap()
    }

    #[test]
    fn preset_expression_clip_wires_preset_node_and_registers_bones() {
        let tmp = tempdir().unwrap();
        let dir = Utf8Path::from_path(tmp.path()).unwrap();
        let p = VrmaExpressionParams {
            id: "expr_blink".into(),
            expression_name: "blink".into(),
            is_preset: true,
            duration_s: 1.0,
        };
        emit_expression_clip(dir, &p).unwrap();
        let doc = doc_of(&dir.join("expr_blink.vrma"));
        let ext = &doc["extensions"]["VRMC_vrm_animation"];
        // preset expression channel wired to a node
        assert!(ext["expressions"]["preset"]["blink"]["node"].is_number());
        // humanoid bones registered (UniVRM importer invariant)
        assert!(ext["humanoid"]["humanBones"]["hips"]["node"].is_number());
        // exactly one animation channel (the expression weight), on translation
        let channels = doc["animations"][0]["channels"].as_array().unwrap();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0]["target"]["path"], "translation");
    }
}
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `cargo test -p vrm-asset-generator expression_clip_emit_tests`
Expected: FAIL — `cannot find function emit_expression_clip`.

- [ ] **Step 3: Implement `emit_expression_clip()`**

In `crates/vrm-asset-generator/src/emit.rs`, after `emit_vrma_expression_triplet`:

```rust
/// Emit a single VRMA preset-expression clip (`{id}.vrma`) for the real-avatar
/// expression corpus (VMK 0.17.2 #333). Emits ONLY the .vrma — the avatar is the
/// real `vroid_default_F_1_0.vrm` fixture and the plan is committed manual YAML.
/// Registers the canonical humanoid skeleton (UniVRM importer invariant, matching
/// `emit_gaze_clip`) and ramps the preset weight 0 → 1 → 0 over `duration_s`.
pub fn emit_expression_clip(
    output_dir: &Utf8Path,
    params: &crate::vrma_params::VrmaExpressionParams,
) -> Result<()> {
    use crate::vrma_emit::{
        add_expression_weight_channel, build_empty_vrma, finalize_vrma_scenes,
        register_all_humanoid_bones, write_vrma_glb, ExpressionKind,
    };

    std::fs::create_dir_all(output_dir)?;

    let skel = crate::humanoid::minimal_skeleton();
    let mut doc = build_empty_vrma();
    doc["nodes"] = skel.nodes_json.clone();
    register_all_humanoid_bones(&mut doc, &skel.bone_to_node);

    let node = {
        let nodes = doc["nodes"].as_array_mut().unwrap();
        nodes.push(serde_json::json!({
            "name": format!("{}_expr_target", params.expression_name)
        }));
        nodes.len() - 1
    };

    let kind = if params.is_preset {
        ExpressionKind::Preset(&params.expression_name)
    } else {
        ExpressionKind::Custom(&params.expression_name)
    };
    let keyframes = [
        (0.0_f32, 0.0_f32),
        (params.duration_s / 2.0, 1.0),
        (params.duration_s, 0.0),
    ];

    let mut buffer = Vec::<u8>::new();
    add_expression_weight_channel(&mut doc, &mut buffer, node, kind, &keyframes);

    finalize_vrma_scenes(&mut doc);

    let vrma_path = output_dir.join(format!("{}.vrma", params.id));
    let vrma_bytes = write_vrma_glb(&doc, &buffer)?;
    std::fs::write(&vrma_path, &vrma_bytes)?;
    Ok(())
}
```

Verify against the actual `add_expression_weight_channel` signature in `vrma_emit.rs` (`(doc, buffer, node_index, kind: ExpressionKind, keyframes: &[(f32, f32)])`) and `ExpressionKind::Preset(&str)`/`Custom(&str)`. `Utf8Path`/`Result` are already in scope in emit.rs.

- [ ] **Step 4: Run the test to confirm it passes**

Run: `cargo test -p vrm-asset-generator expression_clip_emit_tests`
Expected: PASS.

- [ ] **Step 5: fmt + clippy + commit**

Run: `cargo fmt -p vrm-asset-generator && cargo clippy -p vrm-asset-generator --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/vrm-asset-generator/src/emit.rs
git commit -m "feat(asset-gen): emit_expression_clip — VRMA preset-expression clip"
```

---

### Task 4: Wire the `emit-expression-clips` CLI subcommand

**Files:**
- Modify: `crates/vrm-asset-generator/src/cli.rs` (`Cmd` enum, match arm, `describe` entry — mirror the `EmitGazeSweep` subcommand added in the gaze work)

- [ ] **Step 1: Find the reference subcommand**

Run: `grep -n "EmitGazeSweep\|emit-gaze-sweep" crates/vrm-asset-generator/src/cli.rs`
Read the `Cmd::EmitGazeSweep` variant, its match arm, and the `"emit-gaze-sweep"` describe entry. Mirror all three.

- [ ] **Step 2: Add the `Cmd` variant**

After `EmitGazeSweep`:

```rust
    /// Emit the real-avatar expression clips (11 preset .vrma) covering VMK #333.
    EmitExpressionClips {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },
```

(Match the exact `#[arg(...)]` style of `EmitGazeSweep` if it differs.)

- [ ] **Step 3: Add the match arm**

After the `Cmd::EmitGazeSweep { .. } => { .. }` arm, mirroring its structure exactly (the gaze arm is the source of truth for the stdout/stderr result shape):

```rust
        Cmd::EmitExpressionClips { output_dir, json } => {
            use crate::emit::emit_expression_clip;
            use crate::sweep::expression_clip_sweep;

            std::fs::create_dir_all(&output_dir)?;
            let sweep = expression_clip_sweep();
            let total = sweep.len();
            for (i, params) in sweep.iter().enumerate() {
                emit_expression_clip(&output_dir, params)?;
                if json {
                    eprintln!(
                        r#"{{"event":"progress","op":"emit-expression-clips","index":{i},"total":{total},"id":"{id}"}}"#,
                        id = params.id,
                    );
                }
            }
            if json {
                println!(
                    r#"{{"ok":true,"count":{total},"output_dir":"{output_dir}"}}"#
                );
            } else {
                println!("emit-expression-clips: wrote {total} expression clips to {output_dir}");
            }
        }
```

(If the `EmitGazeSweep` arm builds its result differently — e.g. via `serde_json::json!` — copy that exact pattern instead. The gaze arm is the source of truth.)

- [ ] **Step 4: Add the `describe` entry**

Alongside `"emit-gaze-sweep"` in the describe catalog, matching the adjacent entry's schema shape (`input_schema`/`output_schema` if that's what the gaze entry uses):

```rust
                    "emit-expression-clips": {
                        "summary": "Real-avatar expression clips (11 preset .vrma) covering VMK 0.17.2 #333 (VRM 1.0 morph binds keyed by node not mesh -> frozen faces). Presets: blink + happy/angry/sad/relaxed/surprised + 5 visemes (aa/ih/ou/ee/oh). Emits .vrma only — pair with vroid_default_F_1_0.vrm via the committed test-plans/manual/humanoid/vroid_default_F_expr_*.test.yaml plans.",
                    },
```

(Copy the exact field shape — `input_schema`/`output_schema`/`args` — from the `emit-gaze-sweep` describe entry.)

- [ ] **Step 5: Build + smoke**

Run:
```bash
cargo build -p vrm-asset-generator
cargo run -p vrm-asset-generator -- emit-expression-clips --output-dir /tmp/expr-clips
ls /tmp/expr-clips
```
Expected: 11 files — `expr_blink.vrma`, `expr_happy.vrma`, `expr_angry.vrma`, `expr_sad.vrma`, `expr_relaxed.vrma`, `expr_surprised.vrma`, `expr_aa.vrma`, `expr_ih.vrma`, `expr_ou.vrma`, `expr_ee.vrma`, `expr_oh.vrma`.

Run: `cargo run -p vrm-asset-generator -- describe --format json | grep emit-expression-clips`
Expected: one match.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-asset-generator/src/cli.rs
git commit -m "feat(asset-gen): emit-expression-clips subcommand (CLI + describe)"
```

---

### Task 5: Author the 11 manual expression plans

**Files:**
- Create: `test-plans/manual/humanoid/vroid_default_F_expr_blink.test.yaml`
- Create: `test-plans/manual/humanoid/vroid_default_F_expr_happy.test.yaml`
- Create: `test-plans/manual/humanoid/vroid_default_F_expr_angry.test.yaml`
- Create: `test-plans/manual/humanoid/vroid_default_F_expr_sad.test.yaml`
- Create: `test-plans/manual/humanoid/vroid_default_F_expr_relaxed.test.yaml`
- Create: `test-plans/manual/humanoid/vroid_default_F_expr_surprised.test.yaml`
- Create: `test-plans/manual/humanoid/vroid_default_F_expr_aa.test.yaml`
- Create: `test-plans/manual/humanoid/vroid_default_F_expr_ih.test.yaml`
- Create: `test-plans/manual/humanoid/vroid_default_F_expr_ou.test.yaml`
- Create: `test-plans/manual/humanoid/vroid_default_F_expr_ee.test.yaml`
- Create: `test-plans/manual/humanoid/vroid_default_F_expr_oh.test.yaml`

- [ ] **Step 1: Write `vroid_default_F_expr_blink.test.yaml` (the template)**

Whole-face camera (eyes + nose + mouth) so any preset's deformation is in-frame.
**Quote `spec_section`** (it contains ` #333`, which YAML would otherwise truncate as a
comment — this bit the gaze plans). `apply_at_time: 0.5` is the weight peak of the
0→1→0 ramp.

```yaml
id: vroid_default_F_expr_blink
spec_section: 'VRMC_vrm expressions (VRM 1.0 morph bind — VMK 0.17.2 #333; blink must close the eyelids, not freeze)'
asset: vroid_default_F_1_0.vrm
animation:
  vrma:
    path: expr_blink.vrma
    apply_at_time: 0.5
camera:
  position:
  - 0.0
  - 1.27
  - 0.55
  target:
  - 0.0
  - 1.27
  - 0.02
  up:
  - 0.0
  - 1.0
  - 0.0
  fov_degrees: 24.0
lighting:
  directional:
    dir:
    - -0.3
    - -0.6
    - -0.7
    color:
    - 1.0
    - 1.0
    - 1.0
    intensity: 1.0
  ambient:
    color:
    - 0.5
    - 0.5
    - 0.5
    intensity: 0.3
  cast_shadows: false
  receive_shadows: false
post_processing:
  tone_mapping: none
  exposure: 1.0
output:
  width: 1024
  height: 1024
  color_space: srgb
  msaa: 4
diff:
  mode: ssim
  threshold: 0.90
  reference_renderer: three-vrm
  pose_tolerance:
    per_bone_quaternion_radians: 0.010
    hips_translation_m: 0.005
    per_preset_expression: 0.005
    per_custom_expression: 0.005
    look_at_yaw_pitch_degrees: 1.0
    offset_from_head_bone_m: 0.001
  conformance_status:
    kind: included
ignore_renderers: []
properties: []
```

Validate this template against the schema before mass-producing: compare every block to an existing gaze plan (`test-plans/manual/humanoid/vroid_default_F_gaze_center.test.yaml`) — they share the camera/lighting/output/diff/animation shapes. Do not invent fields.

- [ ] **Step 2: Write the other 10 plans**

Identical to the template except `id`, `spec_section`, and `animation.vrma.path` (= `expr_<name>.vrma`). The avatar, whole-face camera, lighting, output, and diff blocks are identical across all 11. Use this table (keep `spec_section` single-quoted):

| id | vrma.path | spec_section |
|---|---|---|
| `vroid_default_F_expr_happy` | `expr_happy.vrma` | `'VRMC_vrm expressions (VRM 1.0 morph bind — VMK 0.17.2 #333; happy emotion preset)'` |
| `vroid_default_F_expr_angry` | `expr_angry.vrma` | `'VRMC_vrm expressions (VRM 1.0 morph bind — VMK 0.17.2 #333; angry emotion preset)'` |
| `vroid_default_F_expr_sad` | `expr_sad.vrma` | `'VRMC_vrm expressions (VRM 1.0 morph bind — VMK 0.17.2 #333; sad emotion preset)'` |
| `vroid_default_F_expr_relaxed` | `expr_relaxed.vrma` | `'VRMC_vrm expressions (VRM 1.0 morph bind — VMK 0.17.2 #333; relaxed emotion preset)'` |
| `vroid_default_F_expr_surprised` | `expr_surprised.vrma` | `'VRMC_vrm expressions (VRM 1.0 morph bind — VMK 0.17.2 #333; surprised emotion preset)'` |
| `vroid_default_F_expr_aa` | `expr_aa.vrma` | `'VRMC_vrm expressions (VRM 1.0 morph bind — VMK 0.17.2 #333; aa viseme / lip-sync)'` |
| `vroid_default_F_expr_ih` | `expr_ih.vrma` | `'VRMC_vrm expressions (VRM 1.0 morph bind — VMK 0.17.2 #333; ih viseme / lip-sync)'` |
| `vroid_default_F_expr_ou` | `expr_ou.vrma` | `'VRMC_vrm expressions (VRM 1.0 morph bind — VMK 0.17.2 #333; ou viseme / lip-sync)'` |
| `vroid_default_F_expr_ee` | `expr_ee.vrma` | `'VRMC_vrm expressions (VRM 1.0 morph bind — VMK 0.17.2 #333; ee viseme / lip-sync)'` |
| `vroid_default_F_expr_oh` | `expr_oh.vrma` | `'VRMC_vrm expressions (VRM 1.0 morph bind — VMK 0.17.2 #333; oh viseme / lip-sync)'` |

- [ ] **Step 3: Validate every plan parses + spec_section survives quoting**

Run:
```bash
python3 -c "
import yaml,glob
for f in sorted(glob.glob('test-plans/manual/humanoid/vroid_default_F_expr_*.test.yaml')):
    d=yaml.safe_load(open(f))
    s=d['spec_section']
    assert '#333' in s, f'{f}: spec_section truncated -> {s!r}'
    print(f.split('/')[-1], 'OK')
"
for f in test-plans/manual/humanoid/vroid_default_F_expr_*.test.yaml; do
  cargo run -q -p vrm-runner -- diff --plan "$f" --render /dev/null --reference /dev/null --renderer-name x --json 2>&1 | head -1
done
```
Expected: every plan prints `OK` (spec_section intact), and each runner invocation fails on the PNG (not on a YAML schema field). Fix any schema error a plan reports.

- [ ] **Step 4: Commit**

```bash
git add test-plans/manual/humanoid/vroid_default_F_expr_*.test.yaml
git commit -m "test(expr): 11 manual expression plans on vroid_default_F (VMK #333 coverage)"
```

---

### Task 6: Wire `emit-expression-clips` into the fixture install path

**Files:**
- Modify: `scripts/install-humanoid-fixtures.sh` (after the existing `emit-gaze-sweep` line)
- Modify: `.gitignore` (ignore `assets/humanoid/expr_*.vrma`)

- [ ] **Step 1: Find the gaze emission block**

Run: `grep -n "emit-gaze-sweep\|DEST" scripts/install-humanoid-fixtures.sh`
The gaze work added `cargo run -q -p vrm-asset-generator -- emit-gaze-sweep --output-dir "$DEST"`. Add the expression-clips emission right after it, using the same `$DEST` variable.

- [ ] **Step 2: Append the expression-clip emission**

After the gaze-sweep line in `scripts/install-humanoid-fixtures.sh`:

```bash
# Expression VRMA clips for the VMK #333 facial-expression corpus (vroid_default_F_expr_*).
echo "Emitting expression VRMA clips (VMK #333 coverage)..."
cargo run -q -p vrm-asset-generator -- emit-expression-clips --output-dir "$DEST"
```

- [ ] **Step 3: Gitignore the generated clips**

Run: `grep -n "gaze_\*.vrma\|expr_\|assets/humanoid" .gitignore`
The gaze work added `assets/humanoid/gaze_*.vrma`. Add a sibling rule:

```
# Generated expression VRMA clips (emitted by scripts/install-humanoid-fixtures.sh)
assets/humanoid/expr_*.vrma
```

- [ ] **Step 4: Verify**

Run:
```bash
bash -n scripts/install-humanoid-fixtures.sh
cargo run -q -p vrm-asset-generator -- emit-expression-clips --output-dir assets/humanoid && ls assets/humanoid/expr_*.vrma
git status --porcelain | grep 'expr_.*\.vrma' || echo "expr clips correctly ignored"
```
Expected: syntax OK; 11 `expr_*.vrma` present; the generated clips do NOT show as untracked.

- [ ] **Step 5: Commit**

```bash
git add scripts/install-humanoid-fixtures.sh .gitignore
git commit -m "chore(fixtures): emit expression VRMA clips into the humanoid asset dir"
```

---

### Task 7: Local render verification + findings

Rendering requires macOS 26 / Xcode 26 (CI build-validates only). Run on an M-series Mac; record as a deliverable.

**Files:**
- Modify: `docs/findings.md` (append a "VMK 0.17.2 VRM 1.0 expressions (#333)" entry)

- [ ] **Step 1: Build the VMK adapter at 0.17.2**

Run: `cd adapters/vrm-metal-kit && swift build` (debug is sufficient for render verification)
Expected: builds against `VRMMetalKit @ 3737e76`. Keep a 0.17.1 binary available for the A/B (a binary built before the Task 1 bump, or rebuild from the prior revision).

- [ ] **Step 2: Install fixtures + emit clips**

Run: `scripts/install-humanoid-fixtures.sh` (symlinks `vroid_default_F_1_0.vrm` and emits the `expr_*.vrma` + `gaze_*.vrma` into `assets/humanoid`).

- [ ] **Step 3: Render the expression plans through 0.17.2**

Run (per plan):
```bash
for f in test-plans/manual/humanoid/vroid_default_F_expr_*.test.yaml; do
  pid=$(basename "$f" .test.yaml)
  cargo run -q -p vrm-runner -- execute-test-plan \
    --plan "$f" \
    --adapter-bin adapters/vrm-metal-kit/.build/debug/vrm-metal-kit-adapter \
    --asset-dir assets/humanoid \
    --output-dir "/tmp/expr-verify/$pid" \
    --renderer-name vrm-metal-kit --json | python3 -c 'import sys,json; print(json.load(sys.stdin)["overall_passed"])'
done
```
Expected: each pipeline runs (PNG + pose produced).

- [ ] **Step 4: Capture the frozen→deforming before/after**

Render a **neutral baseline** (the same avatar with no `animation` block, or with a zero-weight clip) plus `expr_blink`, `expr_happy`, `expr_sad`, and one viseme (`expr_aa`) through **both** a 0.17.1 binary and the 0.17.2 binary. Confirm:
- On **0.17.1** each expression render is **near-identical to neutral** (frozen face — SSIM ≈ 1.0 vs neutral).
- On **0.17.2** each expression render **differs from neutral** (deformed — blink closes the eyelids, `aa` opens the mouth, happy/sad change the brow/mouth).
Visually inspect `/tmp/expr-verify/*/` to confirm the deformation is the expected one per preset.

- [ ] **Step 5: Synthetic viseme check**

Render a synthetic viseme (`vrma_expression_preset_aa`, emit its triplet via `emit-vrma-expression-sweep` into a temp asset-dir) through 0.17.1 and 0.17.2 and confirm the synthetic `aa` (bound node 19 ≠ mesh 0) also goes **frozen → deforming**. This proves the synthetic visemes were silently passing on frozen output and now actually deform.

- [ ] **Step 6: Write the findings entry**

Append to `docs/findings.md` a dated "VMK 0.17.2 VRM 1.0 expressions (#333) — suite coverage landed" entry (newest-first, after the intro) recording: the bug (node-vs-mesh morph keying), the new `vroid_default_F_expr_*` corpus, the 0.17.1→0.17.2 frozen→deforming before/after on blink/happy/sad/aa, the synthetic-viseme confirmation, and that `dump_expression_weights` can't see the frozen mesh (image is the signal). Note the deferred follow-ups (synthetic blink/happy/sad authoring; a first-class differs-from-neutral deformation assertion).

- [ ] **Step 7: Commit**

```bash
git add docs/findings.md
git commit -m "docs(findings): VMK 0.17.2 VRM 1.0 expressions (#333) — coverage + before/after"
```

---

### Final: workspace gate + branch finish

- [ ] **Step 1: Full workspace check**

Run:
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: all green (clippy zero-warning is a hard merge gate).

- [ ] **Step 2: Finish the branch**

Use the `superpowers:finishing-a-development-branch` skill to choose merge/PR/cleanup for branch `expression-0172-coverage`.

---

## Notes for the implementer

- **Reuse the gaze shapes.** `emit_expression_clip` ↔ `emit_gaze_clip`, `expression_clip_sweep` ↔ `gaze_sweep`, `EmitExpressionClips` ↔ `EmitGazeSweep`, the plans ↔ `vroid_default_F_gaze_*`. When a snippet here diverges from the real gaze code, the gaze code is the source of truth — match it.
- **`spec_section` must be single-quoted** in every plan — the bare ` #333` is otherwise eaten as a YAML comment (this regressed the gaze plans and was caught in review; don't repeat it).
- **Verification is local-only** (macOS 26 / Xcode 26). The expression deformation is invisible to `dump_expression_weights` (which reports the controller weight, upstream of the #333 keying) — the rendered-vs-neutral face is the signal.
- **Camera is tunable.** The whole-face nominal (target y≈1.27, z=0.55, fov 24) is a starting point; tune against the avatar at Step 4 (eyes ≈ y 1.304, mouth lower) so blink and mouth visemes are both clearly in-frame, and update the 11 plans if needed.
- **Out of scope (tracked follow-ups):** synthetic blink/happy/sad morph authoring; a differs-from-neutral deformation assertion in the diff engine.

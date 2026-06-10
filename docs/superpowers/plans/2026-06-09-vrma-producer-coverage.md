# VRMA Producer Coverage (Spatial Export Gaps) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the VRMC_vrm_animation conformance gaps exposed by Spatial iOS's VRMA export: hips root-motion translation, large custom-expression sets (52 ARKit blendshapes), multi-channel merged clips, finger bones, and real producer-exported `.vrma` files as corpus fixtures.

**Architecture:** Extend the existing paired-triplet pipeline in `vrm-asset-generator` (params → sweep → emit triplet → sidecar plan → CLI subcommand → describe entry), reusing the channel helpers in `vrma_emit.rs`. One new channel helper (hips translation), one skeleton extension (fingers), four new sweep subcommands, plus a cross-repo fixture export from Spatial's `VRMAGLBWriter` committed as manual-plan fixtures. Every new triplet is smoke-verified end-to-end through `vrm-mock-renderer` (which already implements `load_vrma` / `apply_vrma_at_time` / all three `dump_*` ops).

**Tech Stack:** Rust 1.88 (workspace), serde_json glTF document building, clap CLI, Swift 6 + Swift Testing (Spatial repo, Task 13 only), bash (bootstrap script).

**Background (read before starting):**
- The VRMA spec (`VRMC_vrm_animation` 1.0) has three independently-optional channels: `humanoid` (bone rotations + hips-only translation), `expressions` (preset/custom weights encoded as `translation.x`), `lookAt` (gaze rotation). The existing corpus covers each channel **in isolation** and never emits a hips translation channel.
- Spatial's exporter (`/Users/arkavo/Projects/Spatial/Packages/ArkavoScan/Sources/ArkavoScan/Export/VRM/VRMAGLBWriter.swift`) emits: many bone rotation tracks + an optional hips VEC3 translation track + up to 52 ARKit-named custom expression tracks + preset tracks, all merged on one timeline. That is exactly the shape this plan adds to the corpus.
- Methodology pin: sweeps are one-axis-at-a-time. The multi-channel sweep (Phase 3) is a **deliberate exception** — its purpose is channel-coexistence, and each variant is documented as such in its `spec_section`.
- Existing committed VRMA fixtures live in `assets/humanoid/` (e.g. `expr_happy.vrma`) paired with manual plans in `test-plans/manual/humanoid/`. Follow that precedent for Phase 5.

---

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `crates/vrm-asset-generator/src/vrma_emit.rs` | modify | + `add_hips_translation_channel` (the one spec-legal humanoid translation channel) |
| `crates/vrm-asset-generator/src/vrma_params.rs` | modify | + `VrmaHipsTranslationParams`, `VrmaMultiChannelParams` (+ spec sub-structs) |
| `crates/vrm-asset-generator/src/sweep.rs` | modify | + `vrma_hips_translation_sweep`, `arkit_blendshape_names`, `vrma_arkit_expression_sweep`, `vrma_multichannel_sweep`, `vrma_finger_sweep` |
| `crates/vrm-asset-generator/src/humanoid.rs` | modify | + `skeleton_with_fingers` (30 VRM 1.0 finger bones), refactor `minimal_skeleton` over a shared builder |
| `crates/vrm-asset-generator/src/emit.rs` | modify | + `emit_vrma_hips_translation_triplet`, `emit_vrma_multichannel_triplet`, `emit_vrma_finger_triplet`; refactor `emit_vrm_with_custom_expressions` → `emit_vrm_inner(.., skeleton)` + new `emit_vrm_with_skeleton` |
| `crates/vrm-asset-generator/src/sidecar.rs` | modify | + `build_vrma_hips_translation_test_plan`, `build_vrma_multichannel_test_plan` |
| `crates/vrm-asset-generator/src/cli.rs` | modify | + 4 subcommands (`emit-vrma-hips-translation-sweep`, `emit-arkit-expression-sweep`, `emit-vrma-multichannel-sweep`, `emit-vrma-finger-sweep`) + describe entries |
| `scripts/bootstrap-goldens.sh` | modify | + 4 sweep blocks (VRM 1.0 only) |
| `assets/humanoid/spatial_*.vrma` | create | 3 producer-exported fixtures (committed binaries) |
| `test-plans/manual/humanoid/vroid_default_F_spatial_*.test.yaml` | create | 3 manual plans pairing producer fixtures with the real VRoid avatar |
| `docs/methodology.md` | modify | + "Producer interop fixtures" section |
| `/Users/arkavo/Projects/Spatial/Packages/ArkavoScan/Tests/ArkavoScanTests/VRM/VRMAConformanceFixtureTests.swift` | create (Spatial repo) | deterministic fixture export gated on `VRMA_FIXTURE_OUT_DIR` |

No changes to `crates/vrm-test-plan`, `crates/vrm-ops`, `crates/vrm-runner`, or any adapter: the op surface (`load_vrma`, `apply_vrma_at_time`, `dump_humanoid_pose` incl. hips translation, `dump_expression_weights`, `dump_look_at_state`) and the plan schema (`animation.vrma`, `diff.pose_tolerance.hips_translation_m`) already support everything here.

---

## Phase 1 — Hips translation channel (root motion)

### Task 1: `add_hips_translation_channel` helper

**Files:**
- Modify: `crates/vrm-asset-generator/src/vrma_emit.rs`

- [ ] **Step 1: Write the failing test**

Add to the existing `mod tests` at the bottom of `vrma_emit.rs` (after `look_at_emits_node_rotation_channel`):

```rust
    #[test]
    fn hips_translation_emits_translation_channel_and_registers_bone() {
        let mut doc = build_empty_vrma();
        let mut buffer = Vec::<u8>::new();
        doc["nodes"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "name": "hips" }));

        add_hips_translation_channel(
            &mut doc,
            &mut buffer,
            0,
            &[(0.0_f32, [0.0_f32, 0.86, 0.0]), (1.0, [0.0, 0.86, 0.3])],
        );

        // hips must be registered in humanBones (the spec ties the
        // translation channel to the hips humanoid bone).
        assert_eq!(
            doc["extensions"]["VRMC_vrm_animation"]["humanoid"]["humanBones"]["hips"]["node"],
            0
        );

        let anim = &doc["animations"][0];
        let channels = anim["channels"].as_array().unwrap();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0]["target"]["path"], "translation");
        assert_eq!(channels[0]["target"]["node"], 0);

        // 2 f32 timestamps (8 B) + 2 VEC3 values (24 B), 4-aligned.
        assert!(buffer.len() >= 32, "buffer too small: {}", buffer.len());
        assert_eq!(buffer.len() % 4, 0, "buffer not 4-aligned");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vrm-asset-generator hips_translation_emits_translation_channel_and_registers_bone`
Expected: FAIL to compile — `add_hips_translation_channel` not found.

- [ ] **Step 3: Write minimal implementation**

Add after `add_humanoid_bone_rotation_channel` (after line 125) in `vrma_emit.rs`:

```rust
/// Add a hips translation animation channel.
///
/// Per the VRMA spec, hips is the ONLY humanoid bone allowed a
/// translation channel (root motion); all other bones are rotation-only.
///
/// `keyframes` is `[(time_seconds, [x, y, z])]`. Values are absolute
/// node-local translations — a glTF translation channel REPLACES
/// `node.translation`, so the first keyframe should equal the hips rest
/// translation, not zero.
///
/// Side effects mirror [`add_humanoid_bone_rotation_channel`]: appends a
/// sampler + channel, accessors, bufferViews, buffer bytes, and registers
/// `humanoid.humanBones.hips.node = node_index`.
pub fn add_hips_translation_channel(
    doc: &mut Value,
    buffer: &mut Vec<u8>,
    node_index: usize,
    keyframes: &[(f32, [f32; 3])],
) {
    add_node_translation_channel(doc, buffer, node_index, keyframes);

    let ext = doc["extensions"]["VRMC_vrm_animation"]
        .as_object_mut()
        .unwrap();
    let humanoid = ext
        .entry("humanoid")
        .or_insert_with(|| json!({ "humanBones": {} }));
    let human_bones = humanoid["humanBones"].as_object_mut().unwrap();
    human_bones.insert("hips".to_string(), json!({ "node": node_index }));
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vrm-asset-generator vrma_emit`
Expected: all `vrma_emit::tests` PASS, including the new test.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-asset-generator/src/vrma_emit.rs
git commit -m "feat(asset-gen): add_hips_translation_channel — the one spec-legal humanoid translation channel"
```

### Task 2: Hips sweep params + variants

**Files:**
- Modify: `crates/vrm-asset-generator/src/vrma_params.rs`
- Modify: `crates/vrm-asset-generator/src/sweep.rs`

- [ ] **Step 1: Write the failing tests**

In `vrma_params.rs`, add to the existing `mod tests`:

```rust
    #[test]
    fn hips_translation_params_roundtrips() {
        let p = VrmaHipsTranslationParams {
            id: "vrma_hips_trans_forward".into(),
            offset_m: [0.0, 0.0, 0.3],
            duration_s: 1.0,
        };
        let s = serde_json::to_string(&p).unwrap();
        let back: VrmaHipsTranslationParams = serde_json::from_str(&s).unwrap();
        assert_eq!(back.id, "vrma_hips_trans_forward");
        assert!((back.offset_m[2] - 0.3).abs() < 1e-6);
    }
```

In `sweep.rs`, add a new test module after `mod vrma_expression_sweep_tests` (search for that module to find the location):

```rust
#[cfg(test)]
mod vrma_hips_translation_sweep_tests {
    use super::*;

    #[test]
    fn five_single_direction_variants_with_unique_ids() {
        let variants = vrma_hips_translation_sweep();
        assert_eq!(variants.len(), 5);
        let mut ids: Vec<&str> = variants.iter().map(|v| v.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 5, "duplicate sweep ids");
        for v in &variants {
            let nonzero = v.offset_m.iter().filter(|c| c.abs() > f32::EPSILON).count();
            assert_eq!(nonzero, 1, "{}: one-axis-at-a-time violated", v.id);
            assert!(v.duration_s > 0.0);
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vrm-asset-generator hips_translation`
Expected: FAIL to compile — `VrmaHipsTranslationParams` and `vrma_hips_translation_sweep` not found.

- [ ] **Step 3: Implement**

In `vrma_params.rs`, add after the `GazeParams` struct:

```rust
/// Hips root-motion sweep. Translates hips from rest to rest + `offset_m`
/// over `duration_s`, linear. Covers the one humanoid translation channel
/// the VRMA spec allows — producers (e.g. Spatial iOS's VRMA export) emit
/// it for locomotion; all other humanoid bones are rotation-only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VrmaHipsTranslationParams {
    pub id: String,
    /// Offset from the hips rest translation, metres, applied at t = duration_s.
    pub offset_m: [f32; 3],
    pub duration_s: f32,
}
```

In `sweep.rs`, add after `vrma_humanoid_sweep` (search for `pub fn vrma_humanoid_sweep`):

```rust
/// Hips root-motion sweep (5 variants). One-axis-at-a-time: each variant
/// translates hips along a single world axis. Covers the VRMA spec's only
/// humanoid translation channel (root motion), which the rotation-only
/// `vrma_humanoid_sweep` never exercises.
pub fn vrma_hips_translation_sweep() -> Vec<crate::vrma_params::VrmaHipsTranslationParams> {
    use crate::vrma_params::VrmaHipsTranslationParams;
    let variants: [(&str, [f32; 3]); 5] = [
        ("vrma_hips_trans_forward", [0.0, 0.0, 0.3]),
        ("vrma_hips_trans_backward", [0.0, 0.0, -0.3]),
        ("vrma_hips_trans_lateral", [0.2, 0.0, 0.0]),
        ("vrma_hips_trans_crouch", [0.0, -0.15, 0.0]),
        ("vrma_hips_trans_rise", [0.0, 0.10, 0.0]),
    ];
    variants
        .iter()
        .map(|(id, offset)| VrmaHipsTranslationParams {
            id: (*id).into(),
            offset_m: *offset,
            duration_s: 1.0,
        })
        .collect()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vrm-asset-generator hips_translation`
Expected: 2 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-asset-generator/src/vrma_params.rs crates/vrm-asset-generator/src/sweep.rs
git commit -m "feat(asset-gen): hips translation sweep params + 5 one-axis variants"
```

### Task 3: Hips triplet emission + sidecar + CLI subcommand

**Files:**
- Modify: `crates/vrm-asset-generator/src/emit.rs`
- Modify: `crates/vrm-asset-generator/src/sidecar.rs`
- Modify: `crates/vrm-asset-generator/src/cli.rs`

- [ ] **Step 1: Write the failing test**

In `emit.rs`, add a new test module at the bottom of the file (after the last existing `#[cfg(test)]` module):

```rust
#[cfg(test)]
mod vrma_hips_translation_emit_tests {
    use super::*;
    use camino::Utf8Path;
    use tempfile::tempdir;

    #[test]
    fn hips_triplet_emits_vrm_vrma_and_plan_with_translation_channel() {
        let params = crate::vrma_params::VrmaHipsTranslationParams {
            id: "hips_trans_test".into(),
            offset_m: [0.0, 0.0, 0.3],
            duration_s: 1.0,
        };
        let tmp = tempdir().unwrap();
        let dir = Utf8Path::from_path(tmp.path()).unwrap();
        emit_vrma_hips_translation_triplet(dir, &params).unwrap();

        assert!(dir.join("hips_trans_test.vrm").exists());
        assert!(dir.join("hips_trans_test.test.yaml").exists());

        let vrma_bytes = std::fs::read(dir.join("hips_trans_test.vrma")).unwrap();
        let json_chunk = crate::glb::extract_json_chunk(&vrma_bytes).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&json_chunk).unwrap();

        // Exactly one channel: hips translation.
        let channels = doc["animations"][0]["channels"].as_array().unwrap();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0]["target"]["path"], "translation");
        let hips_node = doc["extensions"]["VRMC_vrm_animation"]["humanoid"]["humanBones"]["hips"]
            ["node"]
            .as_u64()
            .unwrap();
        assert_eq!(channels[0]["target"]["node"].as_u64().unwrap(), hips_node);

        // The plan samples at full offset.
        let plan_yaml = std::fs::read_to_string(dir.join("hips_trans_test.test.yaml")).unwrap();
        assert!(plan_yaml.contains("apply_at_time: 1.0"), "{plan_yaml}");
        assert!(plan_yaml.contains("hips_trans_test.vrma"), "{plan_yaml}");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vrm-asset-generator hips_triplet_emits`
Expected: FAIL to compile — `emit_vrma_hips_translation_triplet` not found.

- [ ] **Step 3: Implement the sidecar builder**

In `sidecar.rs`, add after `build_vrma_lookat_test_plan` (ends near line 476):

```rust
/// Build a test plan for a VRMA hips-translation sweep triplet.
///
/// Starts from the default camera/lighting/output settings and overlays:
/// - `spec_section`: names VRMC_vrm_animation and the hips offset
/// - `animation.vrma`: path + `apply_at_time` at full offset (t = duration_s)
/// - `diff.pose_tolerance`: tight hips-translation tolerance
pub fn build_vrma_hips_translation_test_plan(
    params: &crate::vrma_params::VrmaHipsTranslationParams,
    vrm_relpath: &str,
    vrma_relpath: &str,
) -> TestPlan {
    let mtoon_defaults = crate::params::MToonParams::defaults(&params.id);
    let mut plan = build_default_test_plan(&mtoon_defaults, vrm_relpath);
    plan.spec_section = format!(
        "VRMC_vrm_animation (hips translation sweep: [{:+.2}, {:+.2}, {:+.2}] m)",
        params.offset_m[0], params.offset_m[1], params.offset_m[2]
    );
    plan.animation = Some(AnimationConfig {
        root_transform: None,
        vrma: Some(VrmaAnimation {
            path: vrma_relpath.into(),
            apply_at_time: params.duration_s,
        }),
    });
    plan.diff.pose_tolerance = Some(vrm_test_plan::PoseTolerance {
        per_bone_quaternion_radians: 0.010,
        hips_translation_m: 0.005,
        per_preset_expression: 0.005,
        per_custom_expression: 0.005,
        look_at_yaw_pitch_degrees: 1.0,
        offset_from_head_bone_m: 0.001,
    });
    plan
}
```

- [ ] **Step 4: Implement the emit triplet**

In `emit.rs`, add after `emit_vrma_humanoid_triplet` (ends near line 2863). Also add the small shared helper `hips_rest_translation` — Phase 3 reuses it:

```rust
/// Hips rest translation read back from the canonical skeleton's node
/// JSON (single source of truth — `humanoid.rs::bones()`).
fn hips_rest_translation(skel: &crate::humanoid::Skeleton) -> [f32; 3] {
    let hips_node = skel.bone_to_node["hips"];
    let t = skel.nodes_json[hips_node]["translation"].as_array().unwrap();
    [
        t[0].as_f64().unwrap() as f32,
        t[1].as_f64().unwrap() as f32,
        t[2].as_f64().unwrap() as f32,
    ]
}

/// Emit a VRMA hips-translation sweep triplet: .vrm + .vrma + .test.yaml.
///
/// The .vrm is the canonical default avatar. The .vrma carries a single
/// hips translation channel from rest to rest + offset over `duration_s`
/// (the only humanoid translation channel the VRMA spec allows). Keyframe
/// values are absolute node translations because glTF translation channels
/// replace `node.translation`. The plan samples at t = duration_s.
pub fn emit_vrma_hips_translation_triplet(
    output_dir: &Utf8Path,
    params: &crate::vrma_params::VrmaHipsTranslationParams,
) -> Result<()> {
    use crate::vrma_emit::{
        add_hips_translation_channel, build_empty_vrma, finalize_vrma_scenes,
        register_all_humanoid_bones, write_vrma_glb,
    };

    std::fs::create_dir_all(output_dir)?;

    // 1. .vrm — canonical default avatar; the .vrma carries the test signal.
    let vrm_relpath = format!("{}.vrm", params.id);
    let vrm_path = output_dir.join(&vrm_relpath);
    let mtoon_defaults = crate::params::MToonParams::defaults(&params.id);
    emit_vrm(&mtoon_defaults, &vrm_path)?;

    // 2. .vrma — one hips translation channel, rest → rest + offset.
    let skel = crate::humanoid::minimal_skeleton();
    let hips_node = skel.bone_to_node["hips"];
    let rest = hips_rest_translation(&skel);

    let mut doc = build_empty_vrma();
    doc["nodes"] = skel.nodes_json.clone();
    register_all_humanoid_bones(&mut doc, &skel.bone_to_node);

    let target = [
        rest[0] + params.offset_m[0],
        rest[1] + params.offset_m[1],
        rest[2] + params.offset_m[2],
    ];
    let keyframes = [(0.0_f32, rest), (params.duration_s, target)];
    let mut buffer = Vec::<u8>::new();
    add_hips_translation_channel(&mut doc, &mut buffer, hips_node, &keyframes);

    finalize_vrma_scenes(&mut doc);

    let vrma_relpath = format!("{}.vrma", params.id);
    let vrma_bytes = write_vrma_glb(&doc, &buffer)?;
    std::fs::write(output_dir.join(&vrma_relpath), &vrma_bytes)?;

    // 3. .test.yaml.
    let plan =
        crate::sidecar::build_vrma_hips_translation_test_plan(params, &vrm_relpath, &vrma_relpath);
    crate::sidecar::write_test_yaml(&plan, &output_dir.join(format!("{}.test.yaml", params.id)))?;
    Ok(())
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vrm-asset-generator hips_triplet_emits`
Expected: PASS.

- [ ] **Step 6: Wire the CLI subcommand**

In `cli.rs`:

(a) Add the enum variant to `pub enum Cmd`, after `EmitExpressionClips` (line ~438):

```rust
    /// Emit the VRMA hips-translation sweep (5 plans). The only humanoid
    /// translation channel the VRMA spec allows (root motion). One axis
    /// per variant: forward/backward (Z), lateral (X), crouch/rise (Y).
    EmitVrmaHipsTranslationSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },
```

(b) Add the match arm after the `Cmd::EmitExpressionClips { .. } => { ... }` arm (ends near line 2157):

```rust
        Cmd::EmitVrmaHipsTranslationSweep {
            output_dir,
            json: emit_json,
        } => {
            use crate::emit::emit_vrma_hips_translation_triplet;
            use crate::sweep::vrma_hips_translation_sweep;

            std::fs::create_dir_all(&output_dir)?;
            let sweep = vrma_hips_translation_sweep();
            let total = sweep.len();
            for (i, params) in sweep.iter().enumerate() {
                emit_vrma_hips_translation_triplet(&output_dir, params)?;
                if emit_json {
                    eprintln!(
                        r#"{{"event":"progress","op":"emit-vrma-hips-translation-sweep","index":{i},"total":{total},"id":"{id}"}}"#,
                        id = params.id,
                    );
                } else {
                    eprintln!("[{:3}/{}] {}", i + 1, total, params.id);
                }
            }
            if emit_json {
                let summary = serde_json::json!({
                    "ok": true,
                    "count": total,
                    "output_dir": output_dir,
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!("emitted {total} VRMA hips-translation sweep plans to {output_dir}");
            }
            Ok(())
        }
```

(c) Add the describe entry in the operations JSON, after the `"emit-expression-clips"` entry (ends near line 2895):

```rust
                    "emit-vrma-hips-translation-sweep": {
                        "summary": "VRMA hips-translation sweep (5 plans). The only humanoid translation channel the VRMA spec allows (root motion). One axis per variant: forward/backward (+-0.3 m Z), lateral (0.2 m X), crouch/rise (-0.15/+0.10 m Y). Each plan emits a .vrm + .vrma + .test.yaml triplet; plans sample at full offset (t = 1.0 s).",
                        "input_schema": {
                            "type": "object",
                            "required": ["output_dir"],
                            "properties": {
                                "output_dir": { "type": "string" },
                                "json": {
                                    "type": "boolean",
                                    "description": "Emit NDJSON progress on stderr and a JSON summary on stdout"
                                }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "count": { "type": "integer" },
                                "output_dir": { "type": "string" }
                            }
                        }
                    },
```

- [ ] **Step 7: End-to-end smoke through the mock renderer**

```bash
cargo build --release -p vrm-mock-renderer
cargo run -p vrm-asset-generator -- emit-vrma-hips-translation-sweep --output-dir /tmp/vrma-hips --json
cargo run -p vrm-runner -- execute-test-plan \
    --plan /tmp/vrma-hips/vrma_hips_trans_forward.test.yaml \
    --adapter-bin target/release/vrm-mock-renderer \
    --asset-dir /tmp/vrma-hips --output-dir /tmp/vrma-hips-out \
    --renderer-name mock --json
```

Expected: emit prints a summary with `"count":5`; the runner exits 0 and stdout JSON shows the pipeline ran (`load_vrma → apply_vrma_at_time → dump_*` ops executed; no `Unimplemented` errors).

- [ ] **Step 8: Quality gates + commit**

```bash
cargo fmt --all
cargo clippy -p vrm-asset-generator --all-targets -- -D warnings
cargo test -p vrm-asset-generator
git add crates/vrm-asset-generator/src/emit.rs crates/vrm-asset-generator/src/sidecar.rs crates/vrm-asset-generator/src/cli.rs
git commit -m "feat(asset-gen): emit-vrma-hips-translation-sweep subcommand (5 root-motion triplets)"
```

---

## Phase 2 — ARKit custom-expression sweep (52 blendshapes)

Spatial exports all 52 ARKit blendshapes losslessly as `expressions.custom` tracks. The corpus has exactly 2 custom-expression variants (`smug`, `drowsy`). Per the address-the-whole-gap pattern, sweep all 52 names. `emit_vrma_expression_triplet` already handles custom expressions (it pre-registers the name on the avatar via `emit_vrm_with_custom_expressions`), so this phase is sweep + CLI only.

### Task 4: ARKit name list + sweep

**Files:**
- Modify: `crates/vrm-asset-generator/src/sweep.rs`

- [ ] **Step 1: Write the failing tests**

Add a new test module in `sweep.rs`:

```rust
#[cfg(test)]
mod vrma_arkit_expression_sweep_tests {
    use super::*;

    #[test]
    fn exactly_52_names_lexicographic_and_unique() {
        let names = arkit_blendshape_names();
        assert_eq!(names.len(), 52);
        let mut sorted = names.to_vec();
        sorted.sort_unstable();
        assert_eq!(names.to_vec(), sorted, "must stay in canonical lexicographic order");
        sorted.dedup();
        assert_eq!(sorted.len(), 52, "duplicate blendshape names");
    }

    #[test]
    fn sweep_is_all_custom_with_arkit_ids() {
        let variants = vrma_arkit_expression_sweep();
        assert_eq!(variants.len(), 52);
        for v in &variants {
            assert!(!v.is_preset, "{}: ARKit tracks are custom-classified", v.id);
            assert_eq!(v.id, format!("vrma_arkit_{}", v.expression_name));
            assert!((v.duration_s - 1.0).abs() < 1e-6);
        }
        assert!(variants.iter().any(|v| v.expression_name == "jawOpen"));
        assert!(variants.iter().any(|v| v.expression_name == "tongueOut"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vrm-asset-generator arkit`
Expected: FAIL to compile — `arkit_blendshape_names` / `vrma_arkit_expression_sweep` not found.

- [ ] **Step 3: Implement**

Add to `sweep.rs` after `expression_clip_sweep`. The list below is verified verbatim against Spatial's `MotionStreamSchema.canonicalBlendShapeOrder` (`/Users/arkavo/Projects/Spatial/Packages/ArkavoSensorStream/Sources/ArkavoSensorStream/Motion/MotionStreamSchema.swift:14-29`) — do not reorder or rename:

```rust
/// The 52 ARKit `ARFaceAnchor.BlendShapeLocation` raw names in canonical
/// lexicographic order. Mirrors Spatial's
/// `MotionStreamSchema.canonicalBlendShapeOrder` — producers exporting
/// VRMA from ARKit face capture (Spatial iOS) emit each of these as an
/// `expressions.custom` track keyed by this exact camelCase name.
pub fn arkit_blendshape_names() -> [&'static str; 52] {
    [
        "browDownLeft",
        "browDownRight",
        "browInnerUp",
        "browOuterUpLeft",
        "browOuterUpRight",
        "cheekPuff",
        "cheekSquintLeft",
        "cheekSquintRight",
        "eyeBlinkLeft",
        "eyeBlinkRight",
        "eyeLookDownLeft",
        "eyeLookDownRight",
        "eyeLookInLeft",
        "eyeLookInRight",
        "eyeLookOutLeft",
        "eyeLookOutRight",
        "eyeLookUpLeft",
        "eyeLookUpRight",
        "eyeSquintLeft",
        "eyeSquintRight",
        "eyeWideLeft",
        "eyeWideRight",
        "jawForward",
        "jawLeft",
        "jawOpen",
        "jawRight",
        "mouthClose",
        "mouthDimpleLeft",
        "mouthDimpleRight",
        "mouthFrownLeft",
        "mouthFrownRight",
        "mouthFunnel",
        "mouthLeft",
        "mouthLowerDownLeft",
        "mouthLowerDownRight",
        "mouthPressLeft",
        "mouthPressRight",
        "mouthPucker",
        "mouthRight",
        "mouthRollLower",
        "mouthRollUpper",
        "mouthShrugLower",
        "mouthShrugUpper",
        "mouthSmileLeft",
        "mouthSmileRight",
        "mouthStretchLeft",
        "mouthStretchRight",
        "mouthUpperUpLeft",
        "mouthUpperUpRight",
        "noseSneerLeft",
        "noseSneerRight",
        "tongueOut",
    ]
}

/// ARKit custom-expression sweep (52 variants — one per blendshape).
/// Each variant is a 0 → 1 → 0 weight ramp on one `expressions.custom`
/// track named with the raw ARKit camelCase name, paired with an avatar
/// that pre-registers that custom expression. Covers the producer surface
/// Spatial iOS exports (all 52 blendshapes, lossless, custom-classified).
pub fn vrma_arkit_expression_sweep() -> Vec<crate::vrma_params::VrmaExpressionParams> {
    use crate::vrma_params::VrmaExpressionParams;
    arkit_blendshape_names()
        .iter()
        .map(|name| VrmaExpressionParams {
            id: format!("vrma_arkit_{name}"),
            expression_name: (*name).to_string(),
            is_preset: false,
            duration_s: 1.0,
        })
        .collect()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vrm-asset-generator arkit`
Expected: 2 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-asset-generator/src/sweep.rs
git commit -m "feat(asset-gen): 52-blendshape ARKit custom-expression sweep (Spatial producer surface)"
```

### Task 5: `emit-arkit-expression-sweep` CLI subcommand

**Files:**
- Modify: `crates/vrm-asset-generator/src/cli.rs`

- [ ] **Step 1: Add the enum variant** (after `EmitVrmaHipsTranslationSweep` from Task 3):

```rust
    /// Emit the ARKit custom-expression sweep (52 plans — one per ARKit
    /// blendshape, custom-classified, matching Spatial iOS's VRMA export).
    EmitArkitExpressionSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },
```

- [ ] **Step 2: Add the match arm** (after the `EmitVrmaHipsTranslationSweep` arm; reuses the existing `emit_vrma_expression_triplet`):

```rust
        Cmd::EmitArkitExpressionSweep {
            output_dir,
            json: emit_json,
        } => {
            use crate::emit::emit_vrma_expression_triplet;
            use crate::sweep::vrma_arkit_expression_sweep;

            std::fs::create_dir_all(&output_dir)?;
            let sweep = vrma_arkit_expression_sweep();
            let total = sweep.len();
            for (i, params) in sweep.iter().enumerate() {
                emit_vrma_expression_triplet(&output_dir, params)?;
                if emit_json {
                    eprintln!(
                        r#"{{"event":"progress","op":"emit-arkit-expression-sweep","index":{i},"total":{total},"id":"{id}"}}"#,
                        id = params.id,
                    );
                } else {
                    eprintln!("[{:3}/{}] {}", i + 1, total, params.id);
                }
            }
            if emit_json {
                let summary = serde_json::json!({
                    "ok": true,
                    "count": total,
                    "output_dir": output_dir,
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!("emitted {total} ARKit expression sweep plans to {output_dir}");
            }
            Ok(())
        }
```

- [ ] **Step 3: Add the describe entry** (after `"emit-vrma-hips-translation-sweep"`):

```rust
                    "emit-arkit-expression-sweep": {
                        "summary": "ARKit custom-expression sweep (52 plans — one per ARKit ARFaceAnchor blendshape, e.g. jawOpen, browInnerUp). Each variant ramps a single expressions.custom track 0 -> 1 -> 0 over 1 s against an avatar pre-registering that custom expression; plans sample at peak (t=0.5). Covers the producer surface Spatial iOS's VRMA export emits (all 52 blendshapes, lossless camelCase names, custom-classified). Each plan emits a .vrm + .vrma + .test.yaml triplet.",
                        "input_schema": {
                            "type": "object",
                            "required": ["output_dir"],
                            "properties": {
                                "output_dir": { "type": "string" },
                                "json": {
                                    "type": "boolean",
                                    "description": "Emit NDJSON progress on stderr and a JSON summary on stdout"
                                }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "count": { "type": "integer" },
                                "output_dir": { "type": "string" }
                            }
                        }
                    },
```

- [ ] **Step 4: Verify end-to-end**

```bash
cargo run -p vrm-asset-generator -- emit-arkit-expression-sweep --output-dir /tmp/vrma-arkit --json
ls /tmp/vrma-arkit/*.vrma | wc -l
cargo run -p vrm-runner -- execute-test-plan \
    --plan /tmp/vrma-arkit/vrma_arkit_jawOpen.test.yaml \
    --adapter-bin target/release/vrm-mock-renderer \
    --asset-dir /tmp/vrma-arkit --output-dir /tmp/vrma-arkit-out \
    --renderer-name mock --json
```

Expected: summary `"count":52`; `ls | wc -l` prints 52; runner exits 0.

- [ ] **Step 5: Quality gates + commit**

```bash
cargo fmt --all
cargo clippy -p vrm-asset-generator --all-targets -- -D warnings
cargo test -p vrm-asset-generator
git add crates/vrm-asset-generator/src/cli.rs
git commit -m "feat(asset-gen): emit-arkit-expression-sweep subcommand (52 custom-expression triplets)"
```

---

## Phase 3 — Multi-channel merged clips

Spatial merges body + face onto one timeline; every existing corpus clip is single-channel. These 6 variants are a **deliberate methodology exception** to one-axis-at-a-time: the test signal is channel coexistence itself.

### Task 6: Multi-channel params + sweep

**Files:**
- Modify: `crates/vrm-asset-generator/src/vrma_params.rs`
- Modify: `crates/vrm-asset-generator/src/sweep.rs`

- [ ] **Step 1: Write the failing tests**

In `vrma_params.rs` tests:

```rust
    #[test]
    fn multichannel_params_roundtrips() {
        let p = VrmaMultiChannelParams {
            id: "vrma_multi_bone_hips".into(),
            bones: vec![BoneRotationSpec {
                bone_name: "head".into(),
                axis: RotationAxis::Y,
                angle_deg: 30.0,
            }],
            hips_offset_m: Some([0.0, 0.0, 0.2]),
            expressions: vec![ExpressionWeightSpec {
                name: "happy".into(),
                is_preset: true,
                peak_weight: 1.0,
            }],
            look_at: Some(GazeDirectionSpec {
                axis: RotationAxis::Y,
                angle_deg: 30.0,
            }),
            duration_s: 1.0,
        };
        let s = serde_json::to_string(&p).unwrap();
        let back: VrmaMultiChannelParams = serde_json::from_str(&s).unwrap();
        assert_eq!(back.bones[0].bone_name, "head");
        assert_eq!(back.expressions[0].name, "happy");
        assert!(back.hips_offset_m.is_some());
        assert!(back.look_at.is_some());
    }
```

In `sweep.rs`, new test module:

```rust
#[cfg(test)]
mod vrma_multichannel_sweep_tests {
    use super::*;

    #[test]
    fn six_variants_each_combining_at_least_two_channels() {
        let variants = vrma_multichannel_sweep();
        assert_eq!(variants.len(), 6);
        let mut ids: Vec<&str> = variants.iter().map(|v| v.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 6, "duplicate ids");
        for v in &variants {
            let channel_kinds = [
                !v.bones.is_empty(),
                v.hips_offset_m.is_some(),
                !v.expressions.is_empty(),
                v.look_at.is_some(),
            ]
            .iter()
            .filter(|b| **b)
            .count();
            assert!(channel_kinds >= 2, "{}: not multi-channel", v.id);
        }
    }

    #[test]
    fn double_drive_variant_pairs_preset_with_custom() {
        let variants = vrma_multichannel_sweep();
        let dd = variants
            .iter()
            .find(|v| v.id == "vrma_multi_double_drive")
            .expect("double-drive variant");
        assert!(dd.expressions.iter().any(|e| e.is_preset));
        assert!(dd.expressions.iter().any(|e| !e.is_preset));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vrm-asset-generator multichannel`
Expected: FAIL to compile.

- [ ] **Step 3: Implement params**

In `vrma_params.rs`, add after `VrmaHipsTranslationParams`:

```rust
/// One bone-rotation channel inside a multi-channel clip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoneRotationSpec {
    pub bone_name: String,
    pub axis: RotationAxis,
    pub angle_deg: f32,
}

/// One expression-weight channel inside a multi-channel clip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressionWeightSpec {
    pub name: String,
    pub is_preset: bool,
    pub peak_weight: f32,
}

/// LookAt gaze direction inside a multi-channel clip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GazeDirectionSpec {
    pub axis: RotationAxis,
    pub angle_deg: f32,
}

/// Multi-channel clip: humanoid rotations + optional hips root motion +
/// expression weights + optional lookAt, all sharing one timeline — the
/// shape real producers (Spatial iOS merged body+face export) emit.
/// Deliberate one-axis-at-a-time exception: the test signal is channel
/// coexistence (does channel B still apply when channel A is present?).
/// Every channel peaks at t = duration_s / 2 so a single `apply_at_time`
/// samples all of them at peak.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VrmaMultiChannelParams {
    pub id: String,
    pub bones: Vec<BoneRotationSpec>,
    pub hips_offset_m: Option<[f32; 3]>,
    pub expressions: Vec<ExpressionWeightSpec>,
    pub look_at: Option<GazeDirectionSpec>,
    pub duration_s: f32,
}
```

- [ ] **Step 4: Implement sweep**

In `sweep.rs`, add after `vrma_hips_translation_sweep`:

```rust
/// Multi-channel sweep (6 variants). Deliberate one-axis exception —
/// documented in each plan's spec_section. Variant 3 is the "double-drive"
/// surface (preset + ARKit-named custom coexisting, per Spatial's exporter
/// and PerfectSync-style avatars); variant 6 has all four channel kinds.
pub fn vrma_multichannel_sweep() -> Vec<crate::vrma_params::VrmaMultiChannelParams> {
    use crate::vrma_params::{
        BoneRotationSpec, ExpressionWeightSpec, GazeDirectionSpec, RotationAxis,
        VrmaMultiChannelParams,
    };

    fn bone(name: &str, axis: RotationAxis, angle: f32) -> BoneRotationSpec {
        BoneRotationSpec {
            bone_name: name.into(),
            axis,
            angle_deg: angle,
        }
    }
    fn expr(name: &str, is_preset: bool, peak: f32) -> ExpressionWeightSpec {
        ExpressionWeightSpec {
            name: name.into(),
            is_preset,
            peak_weight: peak,
        }
    }

    vec![
        VrmaMultiChannelParams {
            id: "vrma_multi_bone_hips".into(),
            bones: vec![bone("head", RotationAxis::Y, 30.0)],
            hips_offset_m: Some([0.0, 0.0, 0.2]),
            expressions: vec![],
            look_at: None,
            duration_s: 1.0,
        },
        VrmaMultiChannelParams {
            id: "vrma_multi_bone_expr".into(),
            bones: vec![bone("head", RotationAxis::Y, 30.0)],
            hips_offset_m: None,
            expressions: vec![expr("happy", true, 1.0)],
            look_at: None,
            duration_s: 1.0,
        },
        VrmaMultiChannelParams {
            id: "vrma_multi_double_drive".into(),
            bones: vec![],
            hips_offset_m: Some([0.0, 0.0, 0.1]),
            expressions: vec![expr("happy", true, 1.0), expr("mouthSmileLeft", false, 1.0)],
            look_at: None,
            duration_s: 1.0,
        },
        VrmaMultiChannelParams {
            id: "vrma_multi_body_face".into(),
            bones: vec![
                bone("spine", RotationAxis::Y, 20.0),
                bone("leftUpperArm", RotationAxis::X, 40.0),
            ],
            hips_offset_m: Some([0.0, 0.0, 0.2]),
            expressions: vec![expr("aa", true, 1.0)],
            look_at: None,
            duration_s: 1.0,
        },
        VrmaMultiChannelParams {
            id: "vrma_multi_two_bones_two_exprs".into(),
            bones: vec![
                bone("head", RotationAxis::X, 20.0),
                bone("rightUpperArm", RotationAxis::Y, 30.0),
            ],
            hips_offset_m: None,
            expressions: vec![expr("blink", true, 1.0), expr("ou", true, 0.6)],
            look_at: None,
            duration_s: 1.0,
        },
        VrmaMultiChannelParams {
            id: "vrma_multi_all_channels".into(),
            bones: vec![bone("head", RotationAxis::Y, 20.0)],
            hips_offset_m: Some([0.1, 0.0, 0.0]),
            expressions: vec![expr("happy", true, 0.8)],
            look_at: Some(GazeDirectionSpec {
                axis: RotationAxis::Y,
                angle_deg: 30.0,
            }),
            duration_s: 1.0,
        },
    ]
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p vrm-asset-generator multichannel`
Expected: 3 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-asset-generator/src/vrma_params.rs crates/vrm-asset-generator/src/sweep.rs
git commit -m "feat(asset-gen): multi-channel VRMA sweep params + 6 coexistence variants"
```

### Task 7: Multi-channel triplet emission + sidecar

**Files:**
- Modify: `crates/vrm-asset-generator/src/emit.rs`
- Modify: `crates/vrm-asset-generator/src/sidecar.rs`

- [ ] **Step 1: Write the failing test**

In `emit.rs`, add to the `vrma_hips_translation_emit_tests` module's file area a new module:

```rust
#[cfg(test)]
mod vrma_multichannel_emit_tests {
    use super::*;
    use camino::Utf8Path;
    use tempfile::tempdir;

    #[test]
    fn all_channels_variant_emits_every_extension_block() {
        let sweep = crate::sweep::vrma_multichannel_sweep();
        let params = sweep
            .iter()
            .find(|v| v.id == "vrma_multi_all_channels")
            .unwrap();
        let tmp = tempdir().unwrap();
        let dir = Utf8Path::from_path(tmp.path()).unwrap();
        emit_vrma_multichannel_triplet(dir, params).unwrap();

        let vrma_bytes = std::fs::read(dir.join("vrma_multi_all_channels.vrma")).unwrap();
        let json_chunk = crate::glb::extract_json_chunk(&vrma_bytes).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&json_chunk).unwrap();
        let ext = &doc["extensions"]["VRMC_vrm_animation"];

        assert!(ext["humanoid"]["humanBones"]["head"].is_object());
        assert!(ext["expressions"]["preset"]["happy"].is_object());
        assert!(ext["lookAt"]["node"].is_number());

        // head rotation + hips translation + expression translation + gaze rotation
        let channels = doc["animations"][0]["channels"].as_array().unwrap();
        assert_eq!(channels.len(), 4, "{channels:?}");
    }

    #[test]
    fn double_drive_variant_registers_custom_on_avatar() {
        let sweep = crate::sweep::vrma_multichannel_sweep();
        let params = sweep
            .iter()
            .find(|v| v.id == "vrma_multi_double_drive")
            .unwrap();
        let tmp = tempdir().unwrap();
        let dir = Utf8Path::from_path(tmp.path()).unwrap();
        emit_vrma_multichannel_triplet(dir, params).unwrap();

        let vrm_bytes = std::fs::read(dir.join("vrma_multi_double_drive.vrm")).unwrap();
        let json_chunk = crate::glb::extract_json_chunk(&vrm_bytes).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&json_chunk).unwrap();
        assert!(
            doc["extensions"]["VRMC_vrm"]["expressions"]["custom"]["mouthSmileLeft"].is_object(),
            "avatar must pre-register the custom expression"
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vrm-asset-generator multichannel_emit`
Expected: FAIL to compile — `emit_vrma_multichannel_triplet` not found.

- [ ] **Step 3: Implement sidecar builder**

In `sidecar.rs`, after `build_vrma_hips_translation_test_plan`:

```rust
/// Build a test plan for a multi-channel VRMA triplet. `apply_at_time` is
/// duration/2 — every channel in the clip peaks there (bones/hips hold,
/// expressions ramp back down), so one sample hits all peaks.
pub fn build_vrma_multichannel_test_plan(
    params: &crate::vrma_params::VrmaMultiChannelParams,
    vrm_relpath: &str,
    vrma_relpath: &str,
) -> TestPlan {
    let mtoon_defaults = crate::params::MToonParams::defaults(&params.id);
    let mut plan = build_default_test_plan(&mtoon_defaults, vrm_relpath);
    plan.spec_section = format!(
        "VRMC_vrm_animation (multi-channel coexistence — deliberate one-axis exception: \
         {} bone(s), hips {}, {} expression(s), lookAt {})",
        params.bones.len(),
        if params.hips_offset_m.is_some() { "yes" } else { "no" },
        params.expressions.len(),
        if params.look_at.is_some() { "yes" } else { "no" },
    );
    plan.animation = Some(AnimationConfig {
        root_transform: None,
        vrma: Some(VrmaAnimation {
            path: vrma_relpath.into(),
            apply_at_time: params.duration_s / 2.0,
        }),
    });
    plan.diff.pose_tolerance = Some(vrm_test_plan::PoseTolerance {
        per_bone_quaternion_radians: 0.010,
        hips_translation_m: 0.005,
        per_preset_expression: 0.005,
        per_custom_expression: 0.005,
        look_at_yaw_pitch_degrees: 1.0,
        offset_from_head_bone_m: 0.001,
    });
    plan
}
```

- [ ] **Step 4: Implement emit triplet**

In `emit.rs`, after `emit_vrma_hips_translation_triplet`:

```rust
/// Emit a multi-channel VRMA triplet: .vrm + .vrma + .test.yaml.
///
/// All channels share one timeline and peak at t = duration/2: bone and
/// hips channels ramp identity→peak then HOLD to the end; expression
/// channels ramp 0→peak→0; the lookAt channel ramps identity→gaze then
/// holds. The plan samples once at duration/2, catching every channel at
/// peak. Custom expression names are pre-registered on the avatar (the
/// same invariant as `emit_vrma_expression_triplet`).
pub fn emit_vrma_multichannel_triplet(
    output_dir: &Utf8Path,
    params: &crate::vrma_params::VrmaMultiChannelParams,
) -> Result<()> {
    use crate::vrma_emit::{
        add_expression_weight_channel, add_hips_translation_channel,
        add_humanoid_bone_rotation_channel, add_look_at_channel, build_empty_vrma,
        finalize_vrma_scenes, register_all_humanoid_bones, write_vrma_glb, ExpressionKind,
    };
    use crate::vrma_params::RotationAxis;

    fn axis_quat(axis: RotationAxis, angle_deg: f32) -> [f32; 4] {
        let half = angle_deg.to_radians() / 2.0;
        let (s, c) = (half.sin(), half.cos());
        match axis {
            RotationAxis::X => [s, 0.0, 0.0, c],
            RotationAxis::Y => [0.0, s, 0.0, c],
            RotationAxis::Z => [0.0, 0.0, s, c],
        }
    }

    std::fs::create_dir_all(output_dir)?;

    // 1. .vrm — pre-register any custom expression names.
    let vrm_relpath = format!("{}.vrm", params.id);
    let vrm_path = output_dir.join(&vrm_relpath);
    let mtoon_defaults = crate::params::MToonParams::defaults(&params.id);
    let custom_names: Vec<&str> = params
        .expressions
        .iter()
        .filter(|e| !e.is_preset)
        .map(|e| e.name.as_str())
        .collect();
    emit_vrm_with_custom_expressions(&mtoon_defaults, &vrm_path, &custom_names)?;

    // 2. .vrma — every requested channel on one shared timeline.
    let skel = crate::humanoid::minimal_skeleton();
    let mut doc = build_empty_vrma();
    doc["nodes"] = skel.nodes_json.clone();
    register_all_humanoid_bones(&mut doc, &skel.bone_to_node);

    let d = params.duration_s;
    let identity = [0.0_f32, 0.0, 0.0, 1.0];
    let mut buffer = Vec::<u8>::new();

    for spec in &params.bones {
        let node_idx = *skel
            .bone_to_node
            .get(&spec.bone_name)
            .unwrap_or_else(|| panic!("bone {} not in canonical skeleton", spec.bone_name));
        let q = axis_quat(spec.axis, spec.angle_deg);
        let kf = [(0.0_f32, identity), (d / 2.0, q), (d, q)];
        add_humanoid_bone_rotation_channel(&mut doc, &mut buffer, node_idx, &spec.bone_name, &kf);
    }

    if let Some(offset) = params.hips_offset_m {
        let hips_node = skel.bone_to_node["hips"];
        let rest = hips_rest_translation(&skel);
        let target = [rest[0] + offset[0], rest[1] + offset[1], rest[2] + offset[2]];
        let kf = [(0.0_f32, rest), (d / 2.0, target), (d, target)];
        add_hips_translation_channel(&mut doc, &mut buffer, hips_node, &kf);
    }

    for espec in &params.expressions {
        let node_idx = {
            let nodes = doc["nodes"].as_array_mut().unwrap();
            nodes.push(serde_json::json!({
                "name": format!("{}_expr_target", espec.name)
            }));
            nodes.len() - 1
        };
        let kind = if espec.is_preset {
            ExpressionKind::Preset(&espec.name)
        } else {
            ExpressionKind::Custom(&espec.name)
        };
        let kf = [(0.0_f32, 0.0_f32), (d / 2.0, espec.peak_weight), (d, 0.0)];
        add_expression_weight_channel(&mut doc, &mut buffer, node_idx, kind, &kf);
    }

    if let Some(gaze) = &params.look_at {
        let node_idx = {
            let nodes = doc["nodes"].as_array_mut().unwrap();
            nodes.push(serde_json::json!({ "name": "gaze_target" }));
            nodes.len() - 1
        };
        let q = axis_quat(gaze.axis, gaze.angle_deg);
        let kf = [(0.0_f32, identity), (d / 2.0, q), (d, q)];
        add_look_at_channel(&mut doc, &mut buffer, node_idx, [0.0, 0.06, 0.0], &kf);
    }

    finalize_vrma_scenes(&mut doc);

    let vrma_relpath = format!("{}.vrma", params.id);
    let vrma_bytes = write_vrma_glb(&doc, &buffer)?;
    std::fs::write(output_dir.join(&vrma_relpath), &vrma_bytes)?;

    // 3. .test.yaml.
    let plan =
        crate::sidecar::build_vrma_multichannel_test_plan(params, &vrm_relpath, &vrma_relpath);
    crate::sidecar::write_test_yaml(&plan, &output_dir.join(format!("{}.test.yaml", params.id)))?;
    Ok(())
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p vrm-asset-generator multichannel`
Expected: all PASS (params, sweep, and both emit tests).

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-asset-generator/src/emit.rs crates/vrm-asset-generator/src/sidecar.rs
git commit -m "feat(asset-gen): multi-channel VRMA triplet emission (bones+hips+expressions+lookAt on one timeline)"
```

### Task 8: `emit-vrma-multichannel-sweep` CLI subcommand

**Files:**
- Modify: `crates/vrm-asset-generator/src/cli.rs`

- [ ] **Step 1: Add the enum variant** (after `EmitArkitExpressionSweep`):

```rust
    /// Emit the multi-channel VRMA sweep (6 plans). Channel-coexistence
    /// coverage: bones + hips + expressions + lookAt on one timeline
    /// (the merged body+face shape Spatial iOS exports). Deliberate
    /// one-axis-at-a-time exception, documented per-plan in spec_section.
    EmitVrmaMultichannelSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },
```

- [ ] **Step 2: Add the match arm** (after the `EmitArkitExpressionSweep` arm):

```rust
        Cmd::EmitVrmaMultichannelSweep {
            output_dir,
            json: emit_json,
        } => {
            use crate::emit::emit_vrma_multichannel_triplet;
            use crate::sweep::vrma_multichannel_sweep;

            std::fs::create_dir_all(&output_dir)?;
            let sweep = vrma_multichannel_sweep();
            let total = sweep.len();
            for (i, params) in sweep.iter().enumerate() {
                emit_vrma_multichannel_triplet(&output_dir, params)?;
                if emit_json {
                    eprintln!(
                        r#"{{"event":"progress","op":"emit-vrma-multichannel-sweep","index":{i},"total":{total},"id":"{id}"}}"#,
                        id = params.id,
                    );
                } else {
                    eprintln!("[{:3}/{}] {}", i + 1, total, params.id);
                }
            }
            if emit_json {
                let summary = serde_json::json!({
                    "ok": true,
                    "count": total,
                    "output_dir": output_dir,
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!("emitted {total} multi-channel VRMA sweep plans to {output_dir}");
            }
            Ok(())
        }
```

- [ ] **Step 3: Add the describe entry** (after `"emit-arkit-expression-sweep"`):

```rust
                    "emit-vrma-multichannel-sweep": {
                        "summary": "Multi-channel VRMA sweep (6 plans). Channel-coexistence coverage: humanoid rotations + hips root motion + expression weights + lookAt sharing one timeline, the merged body+face shape real producers (Spatial iOS) export. Includes a preset+custom double-drive variant and an all-four-channels variant. Every channel peaks at t = duration/2; plans sample once there. Deliberate one-axis-at-a-time exception. Each plan emits a .vrm + .vrma + .test.yaml triplet.",
                        "input_schema": {
                            "type": "object",
                            "required": ["output_dir"],
                            "properties": {
                                "output_dir": { "type": "string" },
                                "json": {
                                    "type": "boolean",
                                    "description": "Emit NDJSON progress on stderr and a JSON summary on stdout"
                                }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "count": { "type": "integer" },
                                "output_dir": { "type": "string" }
                            }
                        }
                    },
```

- [ ] **Step 4: Verify end-to-end**

```bash
cargo run -p vrm-asset-generator -- emit-vrma-multichannel-sweep --output-dir /tmp/vrma-multi --json
cargo run -p vrm-runner -- execute-test-plan \
    --plan /tmp/vrma-multi/vrma_multi_all_channels.test.yaml \
    --adapter-bin target/release/vrm-mock-renderer \
    --asset-dir /tmp/vrma-multi --output-dir /tmp/vrma-multi-out \
    --renderer-name mock --json
```

Expected: summary `"count":6`; runner exits 0 with all dump ops executed.

- [ ] **Step 5: Quality gates + commit**

```bash
cargo fmt --all
cargo clippy -p vrm-asset-generator --all-targets -- -D warnings
cargo test -p vrm-asset-generator
git add crates/vrm-asset-generator/src/cli.rs
git commit -m "feat(asset-gen): emit-vrma-multichannel-sweep subcommand (6 coexistence triplets)"
```

---

## Phase 4 — Finger bones (30 VRM 1.0 optional bones)

### Task 9: `skeleton_with_fingers()`

**Files:**
- Modify: `crates/vrm-asset-generator/src/humanoid.rs`

- [ ] **Step 1: Write the failing tests**

Add a new test module at the bottom of `humanoid.rs`:

```rust
#[cfg(test)]
mod finger_skeleton_tests {
    use super::*;

    #[test]
    fn minimal_skeleton_is_unchanged_at_19_bones() {
        let sk = minimal_skeleton();
        assert_eq!(sk.bone_to_node.len(), 19);
        assert!(!sk.bone_to_node.contains_key("leftIndexProximal"));
    }

    #[test]
    fn finger_skeleton_has_49_bones_with_all_finger_names() {
        let sk = skeleton_with_fingers();
        assert_eq!(sk.bone_to_node.len(), 49, "19 core + 30 finger bones");
        for side in ["left", "right"] {
            for seg in [
                "ThumbMetacarpal",
                "ThumbProximal",
                "ThumbDistal",
                "IndexProximal",
                "IndexIntermediate",
                "IndexDistal",
                "MiddleProximal",
                "MiddleIntermediate",
                "MiddleDistal",
                "RingProximal",
                "RingIntermediate",
                "RingDistal",
                "LittleProximal",
                "LittleIntermediate",
                "LittleDistal",
            ] {
                assert!(
                    sk.bone_to_node.contains_key(&format!("{side}{seg}")),
                    "missing {side}{seg}"
                );
            }
        }
    }

    #[test]
    fn finger_chains_are_parented_to_hands() {
        let sk = skeleton_with_fingers();
        let nodes = sk.nodes_json.as_array().unwrap();
        let left_hand = sk.bone_to_node["leftHand"];
        let left_index_prox = sk.bone_to_node["leftIndexProximal"];
        let hand_children: Vec<u64> = nodes[left_hand]["children"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_u64().unwrap())
            .collect();
        assert!(hand_children.contains(&(left_index_prox as u64)));
    }

    #[test]
    fn right_fingers_mirror_left_in_x() {
        let sk = skeleton_with_fingers();
        let nodes = sk.nodes_json.as_array().unwrap();
        let l = sk.bone_to_node["leftIndexProximal"];
        let r = sk.bone_to_node["rightIndexProximal"];
        let lx = nodes[l]["translation"][0].as_f64().unwrap();
        let rx = nodes[r]["translation"][0].as_f64().unwrap();
        assert!((lx + rx).abs() < 1e-6, "X must mirror: {lx} vs {rx}");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vrm-asset-generator finger_skeleton`
Expected: FAIL to compile — `skeleton_with_fingers` not found.

- [ ] **Step 3: Implement**

(a) Make `B` copyable. Change:

```rust
struct B {
```

to:

```rust
#[derive(Clone, Copy)]
struct B {
```

(b) Extract the body of `minimal_skeleton()` into a shared builder. Replace the existing `pub fn minimal_skeleton()` (lines 147–183) with:

```rust
pub fn minimal_skeleton() -> Skeleton {
    build_skeleton(bones())
}

/// Canonical skeleton plus the 30 VRM 1.0 optional finger bones.
/// Opt-in: used only by finger-sweep emission so node indices in the
/// existing corpus stay stable.
pub fn skeleton_with_fingers() -> Skeleton {
    let mut all: Vec<B> = bones().to_vec();
    all.extend_from_slice(finger_bones());
    build_skeleton(&all)
}

fn build_skeleton(bones: &[B]) -> Skeleton {
    let mut bone_to_node = BTreeMap::new();
    for (i, b) in bones.iter().enumerate() {
        bone_to_node.insert(b.name.to_string(), i);
    }

    // Build children arrays.
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); bones.len()];
    for (i, b) in bones.iter().enumerate() {
        if let Some(parent_name) = b.parent {
            let pidx = bone_to_node[parent_name];
            children[pidx].push(i);
        }
    }

    let nodes: Vec<Value> = bones
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let mut node = json!({
                "name": b.name,
                "translation": b.t,
            });
            if !children[i].is_empty() {
                node["children"] = json!(children[i]);
            }
            node
        })
        .collect();

    Skeleton {
        nodes_json: Value::Array(nodes),
        root_node: bone_to_node["hips"],
        bone_to_node,
    }
}
```

(c) Add the finger bone table after `fn bones()`:

```rust
/// VRM 1.0 optional finger bones (15 per hand). Rest translations are
/// rough plausible defaults relative to the hand bone — the conformance
/// signal is the dumped rotation quaternion, not anatomy. Left hand local
/// +X is distal (toward fingertips); right hand mirrors X.
fn finger_bones() -> &'static [B] {
    &[
        // Left thumb
        B { name: "leftThumbMetacarpal", parent: Some("leftHand"), t: [0.02, -0.01, 0.03] },
        B { name: "leftThumbProximal", parent: Some("leftThumbMetacarpal"), t: [0.03, 0.0, 0.01] },
        B { name: "leftThumbDistal", parent: Some("leftThumbProximal"), t: [0.03, 0.0, 0.01] },
        // Left index
        B { name: "leftIndexProximal", parent: Some("leftHand"), t: [0.08, 0.0, 0.02] },
        B { name: "leftIndexIntermediate", parent: Some("leftIndexProximal"), t: [0.035, 0.0, 0.0] },
        B { name: "leftIndexDistal", parent: Some("leftIndexIntermediate"), t: [0.025, 0.0, 0.0] },
        // Left middle
        B { name: "leftMiddleProximal", parent: Some("leftHand"), t: [0.085, 0.0, 0.0] },
        B { name: "leftMiddleIntermediate", parent: Some("leftMiddleProximal"), t: [0.04, 0.0, 0.0] },
        B { name: "leftMiddleDistal", parent: Some("leftMiddleIntermediate"), t: [0.027, 0.0, 0.0] },
        // Left ring
        B { name: "leftRingProximal", parent: Some("leftHand"), t: [0.08, 0.0, -0.018] },
        B { name: "leftRingIntermediate", parent: Some("leftRingProximal"), t: [0.037, 0.0, 0.0] },
        B { name: "leftRingDistal", parent: Some("leftRingIntermediate"), t: [0.025, 0.0, 0.0] },
        // Left little
        B { name: "leftLittleProximal", parent: Some("leftHand"), t: [0.07, 0.0, -0.034] },
        B { name: "leftLittleIntermediate", parent: Some("leftLittleProximal"), t: [0.03, 0.0, 0.0] },
        B { name: "leftLittleDistal", parent: Some("leftLittleIntermediate"), t: [0.02, 0.0, 0.0] },
        // Right thumb
        B { name: "rightThumbMetacarpal", parent: Some("rightHand"), t: [-0.02, -0.01, 0.03] },
        B { name: "rightThumbProximal", parent: Some("rightThumbMetacarpal"), t: [-0.03, 0.0, 0.01] },
        B { name: "rightThumbDistal", parent: Some("rightThumbProximal"), t: [-0.03, 0.0, 0.01] },
        // Right index
        B { name: "rightIndexProximal", parent: Some("rightHand"), t: [-0.08, 0.0, 0.02] },
        B { name: "rightIndexIntermediate", parent: Some("rightIndexProximal"), t: [-0.035, 0.0, 0.0] },
        B { name: "rightIndexDistal", parent: Some("rightIndexIntermediate"), t: [-0.025, 0.0, 0.0] },
        // Right middle
        B { name: "rightMiddleProximal", parent: Some("rightHand"), t: [-0.085, 0.0, 0.0] },
        B { name: "rightMiddleIntermediate", parent: Some("rightMiddleProximal"), t: [-0.04, 0.0, 0.0] },
        B { name: "rightMiddleDistal", parent: Some("rightMiddleIntermediate"), t: [-0.027, 0.0, 0.0] },
        // Right ring
        B { name: "rightRingProximal", parent: Some("rightHand"), t: [-0.08, 0.0, -0.018] },
        B { name: "rightRingIntermediate", parent: Some("rightRingProximal"), t: [-0.037, 0.0, 0.0] },
        B { name: "rightRingDistal", parent: Some("rightRingIntermediate"), t: [-0.025, 0.0, 0.0] },
        // Right little
        B { name: "rightLittleProximal", parent: Some("rightHand"), t: [-0.07, 0.0, -0.034] },
        B { name: "rightLittleIntermediate", parent: Some("rightLittleProximal"), t: [-0.03, 0.0, 0.0] },
        B { name: "rightLittleDistal", parent: Some("rightLittleIntermediate"), t: [-0.02, 0.0, 0.0] },
    ]
}
```

Note: `cargo fmt` will reflow the struct literals; let it.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vrm-asset-generator`
Expected: new finger tests PASS and ALL existing tests still pass (the refactor must not change `minimal_skeleton` output — `chain_axis_tests` and every emit test guard this).

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-asset-generator/src/humanoid.rs
git commit -m "feat(asset-gen): skeleton_with_fingers — 30 VRM 1.0 optional finger bones (opt-in)"
```

### Task 10: `emit_vrm_with_skeleton`

**Files:**
- Modify: `crates/vrm-asset-generator/src/emit.rs`

- [ ] **Step 1: Write the failing test**

Add a new test module in `emit.rs`:

```rust
#[cfg(test)]
mod skeleton_param_emit_tests {
    use super::*;
    use crate::params::MToonParams;
    use camino::Utf8Path;
    use tempfile::tempdir;

    #[test]
    fn emit_vrm_with_finger_skeleton_registers_finger_bones() {
        let params = MToonParams::defaults("finger_avatar_test");
        let tmp = tempdir().unwrap();
        let vrm_path = Utf8Path::from_path(tmp.path()).unwrap().join("out.vrm");
        let skeleton = crate::humanoid::skeleton_with_fingers();
        emit_vrm_with_skeleton(&params, &vrm_path, &skeleton).unwrap();

        let bytes = std::fs::read(&vrm_path).unwrap();
        let json_chunk = crate::glb::extract_json_chunk(&bytes).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&json_chunk).unwrap();
        let human_bones = &doc["extensions"]["VRMC_vrm"]["humanoid"]["humanBones"];
        assert!(human_bones["leftIndexProximal"]["node"].is_number());
        assert!(human_bones["rightLittleDistal"]["node"].is_number());
        // Core bones still present.
        assert!(human_bones["hips"]["node"].is_number());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vrm-asset-generator emit_vrm_with_finger_skeleton`
Expected: FAIL to compile — `emit_vrm_with_skeleton` not found.

- [ ] **Step 3: Refactor**

In `emit.rs` (current lines 27–31), the existing function:

```rust
pub fn emit_vrm_with_custom_expressions(
    params: &MToonParams,
    output: &Utf8Path,
    custom_expression_names: &[&str],
) -> Result<()> {
```

becomes three functions — the two public wrappers plus the renamed inner (keep the existing doc comment on `emit_vrm_with_custom_expressions`):

```rust
pub fn emit_vrm_with_custom_expressions(
    params: &MToonParams,
    output: &Utf8Path,
    custom_expression_names: &[&str],
) -> Result<()> {
    emit_vrm_inner(params, output, custom_expression_names, &minimal_skeleton())
}

/// Like [`emit_vrm`] but with a caller-supplied humanoid skeleton (e.g.
/// `skeleton_with_fingers()` for the VRMA finger sweep). The skeleton's
/// full bone map is registered in `VRMC_vrm.humanoid.humanBones`.
pub fn emit_vrm_with_skeleton(
    params: &MToonParams,
    output: &Utf8Path,
    skeleton: &crate::humanoid::Skeleton,
) -> Result<()> {
    emit_vrm_inner(params, output, &[], skeleton)
}

fn emit_vrm_inner(
    params: &MToonParams,
    output: &Utf8Path,
    custom_expression_names: &[&str],
    skeleton: &crate::humanoid::Skeleton,
) -> Result<()> {
```

Inside the (renamed) body, delete the line:

```rust
    let skeleton = minimal_skeleton();
```

All subsequent `skeleton.bone_to_node[...]` / `skeleton.nodes_json` references compile unchanged against `&Skeleton`. If any line moves `skeleton` by value, borrow instead (e.g. `skeleton.nodes_json.as_array().unwrap().clone()` already clones).

- [ ] **Step 4: Run the full crate test suite**

Run: `cargo test -p vrm-asset-generator`
Expected: ALL tests pass — the refactor is behavior-preserving for every existing caller.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-asset-generator/src/emit.rs
git commit -m "refactor(asset-gen): emit_vrm_inner takes a skeleton param; add emit_vrm_with_skeleton"
```

### Task 11: Finger sweep + triplet emission

**Files:**
- Modify: `crates/vrm-asset-generator/src/sweep.rs`
- Modify: `crates/vrm-asset-generator/src/emit.rs`

- [ ] **Step 1: Write the failing tests**

In `sweep.rs`:

```rust
#[cfg(test)]
mod vrma_finger_sweep_tests {
    use super::*;

    #[test]
    fn thirty_variants_one_per_finger_bone() {
        let variants = vrma_finger_sweep();
        assert_eq!(variants.len(), 30);
        let mut bones: Vec<&str> = variants.iter().map(|v| v.bone_name.as_str()).collect();
        bones.sort_unstable();
        bones.dedup();
        assert_eq!(bones.len(), 30, "each finger bone exactly once");
        assert!(variants.iter().any(|v| v.bone_name == "leftIndexProximal"));
        assert!(variants.iter().any(|v| v.bone_name == "rightThumbDistal"));
    }

    #[test]
    fn finger_bones_resolve_in_finger_skeleton() {
        let sk = crate::humanoid::skeleton_with_fingers();
        for v in vrma_finger_sweep() {
            assert!(
                sk.bone_to_node.contains_key(&v.bone_name),
                "{} not in finger skeleton",
                v.bone_name
            );
        }
    }
}
```

In `emit.rs`:

```rust
#[cfg(test)]
mod vrma_finger_emit_tests {
    use super::*;
    use camino::Utf8Path;
    use tempfile::tempdir;

    #[test]
    fn finger_triplet_animates_the_finger_bone() {
        let params = crate::vrma_params::VrmaHumanoidParams {
            id: "finger_test".into(),
            bone_name: "leftIndexProximal".into(),
            axis: crate::vrma_params::RotationAxis::Z,
            angle_deg: -45.0,
            duration_s: 1.0,
        };
        let tmp = tempdir().unwrap();
        let dir = Utf8Path::from_path(tmp.path()).unwrap();
        emit_vrma_finger_triplet(dir, &params).unwrap();

        let vrma_bytes = std::fs::read(dir.join("finger_test.vrma")).unwrap();
        let json_chunk = crate::glb::extract_json_chunk(&vrma_bytes).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&json_chunk).unwrap();
        let bone_node = doc["extensions"]["VRMC_vrm_animation"]["humanoid"]["humanBones"]
            ["leftIndexProximal"]["node"]
            .as_u64()
            .unwrap();
        let channels = doc["animations"][0]["channels"].as_array().unwrap();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0]["target"]["node"].as_u64().unwrap(), bone_node);
        assert_eq!(channels[0]["target"]["path"], "rotation");

        // The paired .vrm must also know the finger bone, else no renderer
        // can retarget the channel.
        let vrm_bytes = std::fs::read(dir.join("finger_test.vrm")).unwrap();
        let vrm_json = crate::glb::extract_json_chunk(&vrm_bytes).unwrap();
        let vrm_doc: serde_json::Value = serde_json::from_slice(&vrm_json).unwrap();
        assert!(
            vrm_doc["extensions"]["VRMC_vrm"]["humanoid"]["humanBones"]["leftIndexProximal"]
                .is_object()
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vrm-asset-generator finger`
Expected: skeleton tests from Task 9 PASS; new sweep/emit tests FAIL to compile.

- [ ] **Step 3: Implement the sweep**

In `sweep.rs`, after `vrma_multichannel_sweep`:

```rust
/// Finger-bone sweep: one variant per VRM 1.0 optional finger bone
/// (30 = 15 per hand). Fingers curl about Z (±45°, sign mirrored per
/// side); thumbs rotate about Y (±30°, mirrored). Sign choices are for
/// visual plausibility only — the conformance signal is the dumped
/// quaternion matching across renderers, not anatomy. Reuses
/// `VrmaHumanoidParams`; emission pairs with `skeleton_with_fingers()`.
pub fn vrma_finger_sweep() -> Vec<crate::vrma_params::VrmaHumanoidParams> {
    use crate::vrma_params::{RotationAxis, VrmaHumanoidParams};
    const SEGMENTS: [(&str, &str); 15] = [
        ("ThumbMetacarpal", "thumb_metacarpal"),
        ("ThumbProximal", "thumb_proximal"),
        ("ThumbDistal", "thumb_distal"),
        ("IndexProximal", "index_proximal"),
        ("IndexIntermediate", "index_intermediate"),
        ("IndexDistal", "index_distal"),
        ("MiddleProximal", "middle_proximal"),
        ("MiddleIntermediate", "middle_intermediate"),
        ("MiddleDistal", "middle_distal"),
        ("RingProximal", "ring_proximal"),
        ("RingIntermediate", "ring_intermediate"),
        ("RingDistal", "ring_distal"),
        ("LittleProximal", "little_proximal"),
        ("LittleIntermediate", "little_intermediate"),
        ("LittleDistal", "little_distal"),
    ];
    let mut out = Vec::with_capacity(30);
    for (side, side_label, sign) in [("left", "l", 1.0_f32), ("right", "r", -1.0_f32)] {
        for (seg, seg_label) in SEGMENTS {
            let is_thumb = seg.starts_with("Thumb");
            let (axis, angle_deg) = if is_thumb {
                (RotationAxis::Y, 30.0 * sign)
            } else {
                (RotationAxis::Z, -45.0 * sign)
            };
            out.push(VrmaHumanoidParams {
                id: format!("vrma_finger_{side_label}_{seg_label}"),
                bone_name: format!("{side}{seg}"),
                axis,
                angle_deg,
                duration_s: 1.0,
            });
        }
    }
    out
}
```

- [ ] **Step 4: Implement the emit triplet**

In `emit.rs`, after `emit_vrma_multichannel_triplet`:

```rust
/// Emit a VRMA finger sweep triplet: .vrm + .vrma + .test.yaml.
///
/// Identical flow to `emit_vrma_humanoid_triplet` except both the avatar
/// and the clip are built from `skeleton_with_fingers()` so the animated
/// finger bone exists in `VRMC_vrm.humanoid.humanBones` (avatar) and
/// `VRMC_vrm_animation.humanoid.humanBones` (clip). Reuses the humanoid
/// sidecar builder — a finger bone IS a humanoid bone.
pub fn emit_vrma_finger_triplet(
    output_dir: &Utf8Path,
    params: &crate::vrma_params::VrmaHumanoidParams,
) -> Result<()> {
    use crate::vrma_emit::{
        add_humanoid_bone_rotation_channel, build_empty_vrma, finalize_vrma_scenes,
        register_all_humanoid_bones, write_vrma_glb,
    };
    use crate::vrma_params::RotationAxis;

    std::fs::create_dir_all(output_dir)?;

    let skel = crate::humanoid::skeleton_with_fingers();

    // 1. .vrm with the finger skeleton.
    let vrm_relpath = format!("{}.vrm", params.id);
    let vrm_path = output_dir.join(&vrm_relpath);
    let mtoon_defaults = crate::params::MToonParams::defaults(&params.id);
    emit_vrm_with_skeleton(&mtoon_defaults, &vrm_path, &skel)?;

    // 2. .vrma rotating the single finger bone.
    let node_idx = *skel
        .bone_to_node
        .get(&params.bone_name)
        .unwrap_or_else(|| panic!("bone {} not in finger skeleton", params.bone_name));

    let mut doc = build_empty_vrma();
    doc["nodes"] = skel.nodes_json.clone();
    register_all_humanoid_bones(&mut doc, &skel.bone_to_node);

    let mut buffer = Vec::<u8>::new();
    let half_rad = params.angle_deg.to_radians() / 2.0;
    let sin_h = half_rad.sin();
    let target_quat = match params.axis {
        RotationAxis::X => [sin_h, 0.0, 0.0, half_rad.cos()],
        RotationAxis::Y => [0.0, sin_h, 0.0, half_rad.cos()],
        RotationAxis::Z => [0.0, 0.0, sin_h, half_rad.cos()],
    };
    let keyframes = [
        (0.0_f32, [0.0_f32, 0.0, 0.0, 1.0]),
        (params.duration_s, target_quat),
    ];
    add_humanoid_bone_rotation_channel(
        &mut doc,
        &mut buffer,
        node_idx,
        &params.bone_name,
        &keyframes,
    );

    finalize_vrma_scenes(&mut doc);

    let vrma_relpath = format!("{}.vrma", params.id);
    let vrma_bytes = write_vrma_glb(&doc, &buffer)?;
    std::fs::write(output_dir.join(&vrma_relpath), &vrma_bytes)?;

    // 3. .test.yaml — reuse the humanoid plan builder.
    let plan = crate::sidecar::build_vrma_humanoid_test_plan(params, &vrm_relpath, &vrma_relpath);
    crate::sidecar::write_test_yaml(&plan, &output_dir.join(format!("{}.test.yaml", params.id)))?;
    Ok(())
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p vrm-asset-generator finger`
Expected: all finger tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-asset-generator/src/sweep.rs crates/vrm-asset-generator/src/emit.rs
git commit -m "feat(asset-gen): finger-bone sweep (30 variants) + triplet emission on the finger skeleton"
```

### Task 12: `emit-vrma-finger-sweep` CLI subcommand

**Files:**
- Modify: `crates/vrm-asset-generator/src/cli.rs`

- [ ] **Step 1: Add the enum variant** (after `EmitVrmaMultichannelSweep`):

```rust
    /// Emit the VRMA finger-bone sweep (30 plans — one per VRM 1.0
    /// optional finger bone, 15 per hand). Avatar and clip share the
    /// finger-extended skeleton.
    EmitVrmaFingerSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },
```

- [ ] **Step 2: Add the match arm** (after the `EmitVrmaMultichannelSweep` arm):

```rust
        Cmd::EmitVrmaFingerSweep {
            output_dir,
            json: emit_json,
        } => {
            use crate::emit::emit_vrma_finger_triplet;
            use crate::sweep::vrma_finger_sweep;

            std::fs::create_dir_all(&output_dir)?;
            let sweep = vrma_finger_sweep();
            let total = sweep.len();
            for (i, params) in sweep.iter().enumerate() {
                emit_vrma_finger_triplet(&output_dir, params)?;
                if emit_json {
                    eprintln!(
                        r#"{{"event":"progress","op":"emit-vrma-finger-sweep","index":{i},"total":{total},"id":"{id}"}}"#,
                        id = params.id,
                    );
                } else {
                    eprintln!("[{:3}/{}] {}", i + 1, total, params.id);
                }
            }
            if emit_json {
                let summary = serde_json::json!({
                    "ok": true,
                    "count": total,
                    "output_dir": output_dir,
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!("emitted {total} VRMA finger sweep plans to {output_dir}");
            }
            Ok(())
        }
```

- [ ] **Step 3: Add the describe entry** (after `"emit-vrma-multichannel-sweep"`):

```rust
                    "emit-vrma-finger-sweep": {
                        "summary": "VRMA finger-bone sweep (30 plans — one per VRM 1.0 optional finger bone, 15 per hand: thumb metacarpal/proximal/distal + index/middle/ring/little proximal/intermediate/distal). Fingers curl about Z (+-45 deg, mirrored per side); thumbs rotate about Y (+-30 deg). Avatar and clip share a finger-extended skeleton (49 bones). Covers the optional-bone surface producers (Spatial iOS hand retargeting) emit. Each plan emits a .vrm + .vrma + .test.yaml triplet.",
                        "input_schema": {
                            "type": "object",
                            "required": ["output_dir"],
                            "properties": {
                                "output_dir": { "type": "string" },
                                "json": {
                                    "type": "boolean",
                                    "description": "Emit NDJSON progress on stderr and a JSON summary on stdout"
                                }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "count": { "type": "integer" },
                                "output_dir": { "type": "string" }
                            }
                        }
                    },
```

- [ ] **Step 4: Verify end-to-end**

```bash
cargo run -p vrm-asset-generator -- emit-vrma-finger-sweep --output-dir /tmp/vrma-finger --json
ls /tmp/vrma-finger/*.test.yaml | wc -l
cargo run -p vrm-runner -- execute-test-plan \
    --plan /tmp/vrma-finger/vrma_finger_l_index_proximal.test.yaml \
    --adapter-bin target/release/vrm-mock-renderer \
    --asset-dir /tmp/vrma-finger --output-dir /tmp/vrma-finger-out \
    --renderer-name mock --json
```

Expected: `"count":30`; `wc -l` prints 30; runner exits 0.

- [ ] **Step 5: Quality gates + commit**

```bash
cargo fmt --all
cargo clippy -p vrm-asset-generator --all-targets -- -D warnings
cargo test -p vrm-asset-generator
git add crates/vrm-asset-generator/src/cli.rs
git commit -m "feat(asset-gen): emit-vrma-finger-sweep subcommand (30 finger-bone triplets)"
```

---

## Phase 5 — Spatial producer fixtures

Everything so far tests files **we** emit. This phase commits files **Spatial's exporter** emits, so the conformance surface is exercised against a real third-party producer (writer-side quirks included).

### Task 13: Deterministic fixture export from Spatial (cross-repo)

**Files:**
- Create: `/Users/arkavo/Projects/Spatial/Packages/ArkavoScan/Tests/ArkavoScanTests/VRM/VRMAConformanceFixtureTests.swift`

This task touches the Spatial repo. Commit it there on a branch per that repo's conventions (check `git -C /Users/arkavo/Projects/Spatial status` first; do not commit to its main if it has a PR flow).

- [ ] **Step 1: Write the export suite**

```swift
import Testing
import Foundation
import simd
@testable import ArkavoScan

/// Emits deterministic producer-interop `.vrma` fixtures consumed by
/// arkavo-org/vrm-conformance (`assets/humanoid/spatial_*.vrma`).
///
/// No-op unless `VRMA_FIXTURE_OUT_DIR` is set:
///
///     VRMA_FIXTURE_OUT_DIR=/tmp/spatial-vrma swift test --filter VRMAConformanceFixtures
///
/// Inputs are hand-built `VRMAClip`s (not recorded captures) so the output
/// is byte-stable across runs; regeneration is deliberate, never automatic.
@Suite("VRMAConformanceFixtures")
struct VRMAConformanceFixtureTests {
    /// 31 samples at 30 Hz over 1 s.
    private let times: [Float] = (0...30).map { Float($0) / 30.0 }

    private func slerpTrack(to target: simd_quatf) -> [simd_quatf] {
        let identity = simd_quatf(ix: 0, iy: 0, iz: 0, r: 1)
        let n = times.count
        return (0..<n).map { i in simd_slerp(identity, target, Float(i) / Float(n - 1)) }
    }

    /// 0 → 1 → 0 triangle ramp peaking at the middle sample (t = 0.5 s).
    private func triangleRamp() -> [Float] {
        let n = times.count
        return (0..<n).map { i in
            let x = Float(i) / Float(n - 1)
            return 1.0 - abs(2.0 * x - 1.0)
        }
    }

    @Test("emit producer-interop fixtures for vrm-conformance")
    func emitFixtures() throws {
        guard let outDir = ProcessInfo.processInfo.environment["VRMA_FIXTURE_OUT_DIR"] else {
            return
        }
        let dir = URL(fileURLWithPath: outDir, isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)

        let armRaise = simd_quatf(angle: .pi / 4, axis: SIMD3<Float>(0, 0, 1))
        let headTurn = simd_quatf(angle: .pi / 6, axis: SIMD3<Float>(0, 1, 0))
        let n = times.count
        let hipsTrack: [SIMD3<Float>] = (0..<n).map { i in
            SIMD3<Float>(0, 0.9, 0.2 * Float(i) / Float(n - 1))
        }

        // Body-only: two bone tracks + hips root motion.
        let body = VRMAClip(
            times: times,
            boneTracks: [
                .rightUpperArm: slerpTrack(to: armRaise),
                .head: slerpTrack(to: headTurn),
            ],
            hipsTranslation: hipsTrack
        )
        // Face-only: preset-named track ("happy" → expressions.preset) +
        // ARKit-named tracks (→ expressions.custom), the SPL-114 split.
        let face = VRMAClip(
            times: times,
            expressionTracks: [
                "happy": triangleRamp(),
                "jawOpen": triangleRamp(),
                "mouthSmileLeft": triangleRamp(),
            ]
        )
        // Merged body + face on one timeline (the standard Spatial export shape).
        let merged = VRMAClip(
            times: times,
            boneTracks: [.rightUpperArm: slerpTrack(to: armRaise)],
            hipsTranslation: hipsTrack,
            expressionTracks: ["happy": triangleRamp(), "jawOpen": triangleRamp()]
        )

        try VRMAGLBWriter.write(clip: body)
            .write(to: dir.appendingPathComponent("spatial_body_motion.vrma"))
        try VRMAGLBWriter.write(clip: face)
            .write(to: dir.appendingPathComponent("spatial_face_blend.vrma"))
        try VRMAGLBWriter.write(clip: merged)
            .write(to: dir.appendingPathComponent("spatial_merged_motion.vrma"))
    }
}
```

- [ ] **Step 2: Run it and verify output**

```bash
cd /Users/arkavo/Projects/Spatial/Packages/ArkavoScan
VRMA_FIXTURE_OUT_DIR=/tmp/spatial-vrma swift test --filter VRMAConformanceFixtures
ls -la /tmp/spatial-vrma/
```

Expected: test passes; three `.vrma` files exist, each a few KB, each starting with `glTF` magic (`head -c 4 /tmp/spatial-vrma/spatial_body_motion.vrma` prints `glTF`).

- [ ] **Step 3: Verify determinism**

```bash
VRMA_FIXTURE_OUT_DIR=/tmp/spatial-vrma2 swift test --filter VRMAConformanceFixtures
diff /tmp/spatial-vrma/spatial_body_motion.vrma /tmp/spatial-vrma2/spatial_body_motion.vrma && echo BYTE-IDENTICAL
```

Expected: `BYTE-IDENTICAL`. If not (e.g. dictionary-order nondeterminism in the writer's JSON serialization), STOP and report — committing nondeterministic fixtures breaks regeneration diffs; the fix belongs in `VRMAGLBWriter` (sorted keys), not in this plan.

- [ ] **Step 4: Commit (Spatial repo)**

```bash
cd /Users/arkavo/Projects/Spatial
git add Packages/ArkavoScan/Tests/ArkavoScanTests/VRM/VRMAConformanceFixtureTests.swift
git commit -m "test(vrma): deterministic conformance fixture export (VRMA_FIXTURE_OUT_DIR gate)"
```

### Task 14: Commit fixtures + manual plans in vrm-conformance

**Files:**
- Create: `assets/humanoid/spatial_body_motion.vrma` (copied binary)
- Create: `assets/humanoid/spatial_face_blend.vrma` (copied binary)
- Create: `assets/humanoid/spatial_merged_motion.vrma` (copied binary)
- Create: `test-plans/manual/humanoid/vroid_default_F_spatial_body.test.yaml`
- Create: `test-plans/manual/humanoid/vroid_default_F_spatial_face.test.yaml`
- Create: `test-plans/manual/humanoid/vroid_default_F_spatial_merged.test.yaml`

- [ ] **Step 1: Copy the fixtures**

```bash
cp /tmp/spatial-vrma/spatial_body_motion.vrma /tmp/spatial-vrma/spatial_face_blend.vrma /tmp/spatial-vrma/spatial_merged_motion.vrma \
   /Users/arkavo/Projects/vrm-conformance/assets/humanoid/
```

- [ ] **Step 2: Write the three manual plans**

`test-plans/manual/humanoid/vroid_default_F_spatial_body.test.yaml` (full-body camera; samples at clip end where the hips offset and bone arcs are at maximum):

```yaml
id: vroid_default_F_spatial_body
spec_section: 'VRMC_vrm_animation (producer interop: Spatial iOS VRMA export — body bones + hips root motion)'
asset: vroid_default_F_1_0.vrm
animation:
  vrma:
    path: spatial_body_motion.vrma
    apply_at_time: 1.0
camera:
  position:
  - 0.0
  - 1.0
  - 2.6
  target:
  - 0.0
  - 0.9
  - 0.0
  up:
  - 0.0
  - 1.0
  - 0.0
  fov_degrees: 30.0
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

`test-plans/manual/humanoid/vroid_default_F_spatial_face.test.yaml` (face camera copied from the committed `vroid_default_F_expr_happy.test.yaml`; samples at the t=0.5 ramp peak). Note in spec_section: the ARKit-named custom tracks (`jawOpen`, `mouthSmileLeft`) are expected-absent on the VRoid avatar — it registers no matching `expressions.custom` entries — so the conformance signal is the `happy` preset; renderers must also agree on *ignoring* the unregistered customs:

```yaml
id: vroid_default_F_spatial_face
spec_section: 'VRMC_vrm_animation (producer interop: Spatial iOS VRMA export — preset + ARKit custom expression tracks; customs expected-absent on this avatar, signal is preset happy + agreement on ignoring unregistered customs)'
asset: vroid_default_F_1_0.vrm
animation:
  vrma:
    path: spatial_face_blend.vrma
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

`test-plans/manual/humanoid/vroid_default_F_spatial_merged.test.yaml` (full-body camera; t=0.5 catches the expression peak and the bone/hips mid-arc — both deterministic under LINEAR interpolation):

```yaml
id: vroid_default_F_spatial_merged
spec_section: 'VRMC_vrm_animation (producer interop: Spatial iOS VRMA export — merged body + face on one timeline)'
asset: vroid_default_F_1_0.vrm
animation:
  vrma:
    path: spatial_merged_motion.vrma
    apply_at_time: 0.5
camera:
  position:
  - 0.0
  - 1.0
  - 2.6
  target:
  - 0.0
  - 0.9
  - 0.0
  up:
  - 0.0
  - 1.0
  - 0.0
  fov_degrees: 30.0
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

- [ ] **Step 3: Verify the plans drive the pipeline through the mock renderer**

```bash
cd /Users/arkavo/Projects/vrm-conformance
for p in body face merged; do
  cargo run -p vrm-runner -- execute-test-plan \
      --plan test-plans/manual/humanoid/vroid_default_F_spatial_${p}.test.yaml \
      --adapter-bin target/release/vrm-mock-renderer \
      --asset-dir assets/humanoid --output-dir /tmp/spatial-fixture-out \
      --renderer-name mock --json || echo "FAILED: $p"
done
```

Expected: all three exit 0, no `Unimplemented` / parse errors — this proves Spatial's writer output round-trips through our op pipeline. (The mock's pose dumps are what the runner diffs against once a second renderer's results exist; here we verify load + apply + dump succeed.) If `load_vrma` rejects a fixture, that is a real interop finding — record it in `docs/findings.md` and stop to triage rather than papering over it.

- [ ] **Step 4: Commit**

```bash
git add assets/humanoid/spatial_*.vrma test-plans/manual/humanoid/vroid_default_F_spatial_*.test.yaml
git commit -m "test(vrma): Spatial producer-interop fixtures + 3 manual plans on vroid_default_F"
```

### Task 15: Methodology documentation

**Files:**
- Modify: `docs/methodology.md`

- [ ] **Step 1: Add the section**

Append to `docs/methodology.md` (read the file first and place the new `##` section alongside the other test-design sections, matching the file's heading style):

```markdown
## Producer interop fixtures

`assets/humanoid/spatial_*.vrma` are NOT generated by `vrm-asset-generator`.
They are committed outputs of Arkavo Spatial's VRMA exporter
(`ArkavoScan/Export/VRM/VRMAGLBWriter.swift`), regenerated deliberately via:

    cd Spatial/Packages/ArkavoScan
    VRMA_FIXTURE_OUT_DIR=… swift test --filter VRMAConformanceFixtures

They exist to exercise the conformance surface against a real third-party
producer — writer-side conventions included (expression weights as
translation.x, hips VEC3 root-motion track, preset/custom name
classification) — not just against our own emitter.

Rules:

- Regeneration is a deliberate act reviewed in PR (byte-diff expected to be
  empty unless Spatial's writer changed); never regenerate as a side effect.
- The paired plans use `vroid_default_F_1_0.vrm`. ARKit-named custom tracks
  (`jawOpen`, `mouthSmileLeft`) are expected-absent on that avatar (no
  matching `expressions.custom` registration); the conformance signal is
  the preset channel plus cross-renderer agreement on ignoring unregistered
  customs. Synthetic per-blendshape coverage lives in
  `emit-arkit-expression-sweep` instead.
- A fixture that fails `load_vrma` on any adapter is a finding
  (`docs/findings.md`), not a fixture bug to silently work around.
```

- [ ] **Step 2: Commit**

```bash
git add docs/methodology.md
git commit -m "docs(methodology): producer interop fixtures — provenance, regeneration, expected-absent customs"
```

---

## Phase 6 — Corpus wiring + final verification

### Task 16: Wire the four new sweeps into bootstrap-goldens.sh

**Files:**
- Modify: `scripts/bootstrap-goldens.sh`

- [ ] **Step 1: Add four sweep blocks**

Insert after the existing VRMA lookAt block (the `if [ "$SPEC_VERSION" != "0.x" ]` block ending `VRMA_LOOKAT_DIR=""`, near line 271), following the exact same pattern:

```bash
    if [ "$SPEC_VERSION" != "0.x" ]; then
        echo "==> Emitting VRMA hips-translation sweep (producer coverage: 5 plans)"
        VRMA_HIPS_DIR="$GOLDENS_DIR/_assets_vrma_hips"
        rm -rf "$VRMA_HIPS_DIR"; mkdir -p "$VRMA_HIPS_DIR"
        cargo run --release -q -p vrm-asset-generator -- emit-vrma-hips-translation-sweep \
            --output-dir "$VRMA_HIPS_DIR" --json >/dev/null
    else
        echo "    SKIP emit-vrma-hips-translation-sweep: VRM 1.0-era VRMA; no 0.x form"
        VRMA_HIPS_DIR=""
    fi

    if [ "$SPEC_VERSION" != "0.x" ]; then
        echo "==> Emitting ARKit custom-expression sweep (producer coverage: 52 plans)"
        VRMA_ARKIT_DIR="$GOLDENS_DIR/_assets_vrma_arkit"
        rm -rf "$VRMA_ARKIT_DIR"; mkdir -p "$VRMA_ARKIT_DIR"
        cargo run --release -q -p vrm-asset-generator -- emit-arkit-expression-sweep \
            --output-dir "$VRMA_ARKIT_DIR" --json >/dev/null
    else
        echo "    SKIP emit-arkit-expression-sweep: VRM 1.0-era VRMA; no 0.x form"
        VRMA_ARKIT_DIR=""
    fi

    if [ "$SPEC_VERSION" != "0.x" ]; then
        echo "==> Emitting multi-channel VRMA sweep (producer coverage: 6 plans)"
        VRMA_MULTI_DIR="$GOLDENS_DIR/_assets_vrma_multichannel"
        rm -rf "$VRMA_MULTI_DIR"; mkdir -p "$VRMA_MULTI_DIR"
        cargo run --release -q -p vrm-asset-generator -- emit-vrma-multichannel-sweep \
            --output-dir "$VRMA_MULTI_DIR" --json >/dev/null
    else
        echo "    SKIP emit-vrma-multichannel-sweep: VRM 1.0-era VRMA; no 0.x form"
        VRMA_MULTI_DIR=""
    fi

    if [ "$SPEC_VERSION" != "0.x" ]; then
        echo "==> Emitting VRMA finger sweep (producer coverage: 30 plans)"
        VRMA_FINGER_DIR="$GOLDENS_DIR/_assets_vrma_finger"
        rm -rf "$VRMA_FINGER_DIR"; mkdir -p "$VRMA_FINGER_DIR"
        cargo run --release -q -p vrm-asset-generator -- emit-vrma-finger-sweep \
            --output-dir "$VRMA_FINGER_DIR" --json >/dev/null
    else
        echo "    SKIP emit-vrma-finger-sweep: VRM 1.0-era VRMA; no 0.x form"
        VRMA_FINGER_DIR=""
    fi
```

The downstream `find "$GOLDENS_DIR" -maxdepth 3 -name '*.test.yaml'` loop picks the new plans up automatically — no further changes.

- [ ] **Step 2: Syntax-check the script**

Run: `bash -n scripts/bootstrap-goldens.sh`
Expected: no output (exit 0).

- [ ] **Step 3: Commit**

```bash
git add scripts/bootstrap-goldens.sh
git commit -m "chore(goldens): wire hips/arkit/multichannel/finger VRMA sweeps into bootstrap"
```

### Task 17: Full-workspace verification

- [ ] **Step 1: Run every CI gate locally**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
scripts/smoke.sh
```

Expected: all four pass. `scripts/smoke.sh` exercises asset gen → mock render → diff → site build end-to-end.

- [ ] **Step 2: Corpus count sanity**

```bash
for cmd in emit-vrma-hips-translation-sweep emit-arkit-expression-sweep emit-vrma-multichannel-sweep emit-vrma-finger-sweep; do
  out=/tmp/corpus-check/$cmd
  rm -rf "$out"; mkdir -p "$out"
  cargo run --release -q -p vrm-asset-generator -- $cmd --output-dir "$out" --json
done
find /tmp/corpus-check -name '*.test.yaml' | wc -l
```

Expected: final count 93 (5 + 52 + 6 + 30).

- [ ] **Step 3: Verify `describe` exposes the new ops**

```bash
cargo run -p vrm-asset-generator -- describe --format json | \
  grep -o "emit-vrma-hips-translation-sweep\|emit-arkit-expression-sweep\|emit-vrma-multichannel-sweep\|emit-vrma-finger-sweep" | \
  sort -u
```

Expected: all four op names printed (4 lines), regardless of whether the describe JSON is single-line or pretty-printed.

- [ ] **Step 4: Final commit if anything was touched by fmt**

```bash
git status --short
# if dirty:
git add -A && git commit -m "chore: fmt"
```

---

## Out of scope (deliberate)

- **Adapter implementations**: VMK / three-vrm / godot-vrm VRMA ops remain `Unimplemented` (phases 5–7 of the VRMA rollout track that). The new corpus runs today on the mock renderer and UniVRM (single-frame); it becomes cross-renderer consensus material as adapters land — no rework needed.
- **`render_sequence.apply_vrma`**: deferred everywhere; temporal VRMA playback is RFC-0004 follow-up territory.
- **Synthetic eye bones / blink morphs on the parametric rig**: tracked as an existing findings follow-up, independent of producer coverage.
- **Goldens/S3 manifest entries**: renderer maintainers submit PNGs via the normal PR flow once real adapters render the new corpus.

## Execution order & independence

Phases 1 → 2 → 3 must run in order within themselves; across phases: Phase 2 is independent; Phase 3 depends on Task 1 (hips helper) and Task 3 (the `hips_rest_translation` helper); Phase 4 is independent of 1–3; Phase 5 is independent of 1–4; Phase 6 last. If parallelizing with subagents, safe groupings: {Phase 1+3}, {Phase 2}, {Phase 4}, {Phase 5}.

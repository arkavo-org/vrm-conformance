# VMK 0.17.1 lookAt (#332) Coverage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cover both sub-bugs of VMK 0.17.1's bone-driven eye look-at fix (#332) with a real-avatar (`vroid_default_F_1_0.vrm`) gaze corpus, and bump the VMK pin to 0.17.1.

**Architecture:** A new `emit-gaze-sweep` asset-generator subcommand emits VRMA gaze clips (gaze direction + optional spine yaw for the turned-head case) that pair with the real VRoid avatar via committed manual test plans. The pose-dump reference bone list gains `leftEye`/`rightEye` so the cross-renderer pose diff carries a numeric eye-bone signal. Verification (image before/after) runs locally on macOS 26.

**Tech Stack:** Rust (vrm-asset-generator, vrm-ops), Swift (VMK adapter), HTML/JS (three-vrm host), YAML test plans.

**Spec:** `docs/superpowers/specs/2026-06-07-lookat-0171-coverage-design.md`

---

### Task 1: Bump VMK pin to 0.17.1

**Files:**
- Modify: `adapters/vrm-metal-kit/Package.swift:473-492` (the `0.17.0` comment block + `revision`)

- [ ] **Step 1: Update the revision and prepend a 0.17.1 changelog comment**

In `adapters/vrm-metal-kit/Package.swift`, change the `revision` string on the `.package(...)` for VRMMetalKit from `5cd0a95c6f05fe8c7960d958781d201b36184369` to `421232b75c77d65d8d2bd827a36159936b68db23` (tag `0.17.1`).

Directly above the existing `// 0.17.0 (commit 5cd0a95, **final** release ...` line, insert this comment block:

```swift
        // 0.17.1 (commit 421232b, patch release 2026-06-08, closes #332) —
        // corrects bone-driven eye look-at. Two behaviour changes to rendered
        // eye direction (no shader/metallib change vs 0.17.0):
        //   - **Head-local gaze resolution**: `updateTargetAngles` computed
        //          yaw/pitch in world space but wrote them as a *local*
        //          eye-bone rotation, so any turned head (body yawed at the
        //          root) drove the eyes off by the head's yaw. Targets now
        //          resolve through the head's inverse world matrix
        //          (`.headLocalPoint` was equally affected).
        //   - **Eye-bone rest composition**: `applyToBones` /
        //          `applyToAnimationState` overwrote the eye bones with a bare
        //          gaze quaternion, discarding the authored rest. VRoid rigs
        //          (`J_Adj_*_FaceEye`) carry a mirrored outward ~±22° eye rest;
        //          discarding it splayed the eyes wall-eyed at center and
        //          inverted gaze. Now composes `gaze * initialRotation`.
        //   This closes the long-deferred suite-side asset-coverage follow-up
        //   on `docs/upstream/VMK-vrma-lookat-renderer-propagation.md`: the new
        //   `vroid_default_F_gaze_*` corpus (this commit) drives gaze on the
        //   real VRoid avatar VMK names as the validation target — the
        //   synthetic humanoid corpus has no eye bones, which is why the
        //   `vrma_lookat_*` history could only ever verify the gaze *parse*.
        //   Rendering/before-after verification is local-only (macOS 26 /
        //   Xcode 26); CI build-validates the adapter but does not render.
```

- [ ] **Step 2: Verify SPM resolves the new revision**

Run: `cd adapters/vrm-metal-kit && swift package resolve`
Expected: resolves `VRMMetalKit` at `421232b...` with no error. (If on a non-macOS-26 host this still resolves the manifest; a full `swift build` is local-only.)

- [ ] **Step 3: Commit**

```bash
git add adapters/vrm-metal-kit/Package.swift adapters/vrm-metal-kit/Package.resolved
git commit -m "deps(vmk): bump VRMMetalKit 0.17.0 -> 0.17.1 (eye look-at #332)"
```

---

### Task 2: Add `GazeParams` + `gaze_sweep()`

**Files:**
- Modify: `crates/vrm-asset-generator/src/vrma_params.rs` (add `GazeParams` struct after `VrmaLookAtParams`)
- Modify: `crates/vrm-asset-generator/src/sweep.rs` (add `gaze_sweep()` after `vrma_lookat_sweep`, ~line 1327)

- [ ] **Step 1: Write the failing test for `gaze_sweep()`**

Add to the `#[cfg(test)] mod tests` block in `crates/vrm-asset-generator/src/sweep.rs`:

```rust
#[test]
fn gaze_sweep_covers_both_bugs() {
    let sweep = gaze_sweep();
    let ids: Vec<&str> = sweep.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(sweep.len(), 8);
    // Bug B: gaze directions at neutral body.
    assert!(ids.contains(&"gaze_center"));
    assert!(ids.contains(&"gaze_left"));
    assert!(ids.contains(&"gaze_right"));
    assert!(ids.contains(&"gaze_up"));
    assert!(ids.contains(&"gaze_down"));
    // Bug A: turned-head variants.
    assert!(ids.contains(&"gaze_center_bodyL"));
    assert!(ids.contains(&"gaze_center_bodyR"));
    assert!(ids.contains(&"gaze_right_bodyL"));
    // center has zero gaze; bodyL turns +35.
    let center = sweep.iter().find(|p| p.id == "gaze_center").unwrap();
    assert_eq!(center.gaze_angle_deg, 0.0);
    assert_eq!(center.body_yaw_deg, 0.0);
    let body_l = sweep.iter().find(|p| p.id == "gaze_center_bodyL").unwrap();
    assert_eq!(body_l.body_yaw_deg, 35.0);
}
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `cargo test -p vrm-asset-generator gaze_sweep_covers_both_bugs`
Expected: FAIL — `cannot find function gaze_sweep` and `GazeParams` unresolved.

- [ ] **Step 3: Add the `GazeParams` struct**

In `crates/vrm-asset-generator/src/vrma_params.rs`, after the `VrmaLookAtParams` struct, add:

```rust
/// Real-avatar gaze sweep variant. Drives a single-axis gaze direction
/// (yaw OR pitch) plus an optional spine yaw (the turned-head case). The
/// .vrma is avatar-agnostic; manual plans pair it with `vroid_default_F_1_0.vrm`.
/// Covers VMK 0.17.1 #332: `body_yaw_deg != 0` exercises head-local gaze
/// resolution; `gaze_angle_deg == 0` at body 0 exercises eye-rest composition
/// (wall-eye at center).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GazeParams {
    pub id: String,
    /// Gaze rotation axis: `Y` = yaw (left/right), `X` = pitch (up/down).
    pub gaze_axis: RotationAxis,
    /// Gaze angle in degrees about `gaze_axis`. 0 = straight ahead.
    pub gaze_angle_deg: f32,
    /// Spine yaw (about Y) in degrees. 0 = upright. Non-zero turns the head.
    pub body_yaw_deg: f32,
    pub duration_s: f32,
}
```

- [ ] **Step 4: Add `gaze_sweep()`**

In `crates/vrm-asset-generator/src/sweep.rs`, after `vrma_lookat_sweep()`:

```rust
/// Real-avatar gaze sweep (8 clips) covering VMK 0.17.1 #332. Five neutral-body
/// gaze directions exercise the eye-rest-composition bug (wall-eye at center,
/// inverted side gaze); three turned-head variants exercise head-local gaze
/// resolution. Sign convention follows `vrma_lookat_sweep` (gaze rotation vs
/// world frame). Each entry emits one `.vrma` (the avatar is the real VRoid
/// fixture, paired via a committed manual plan).
pub fn gaze_sweep() -> Vec<crate::vrma_params::GazeParams> {
    use crate::vrma_params::{GazeParams, RotationAxis};
    let g = |id: &str, axis: RotationAxis, angle: f32, body: f32| GazeParams {
        id: id.to_string(),
        gaze_axis: axis,
        gaze_angle_deg: angle,
        body_yaw_deg: body,
        duration_s: 1.0,
    };
    vec![
        // Bug B: eye-rest composition (neutral body).
        g("gaze_center", RotationAxis::Y, 0.0, 0.0),
        g("gaze_left", RotationAxis::Y, 30.0, 0.0),
        g("gaze_right", RotationAxis::Y, -30.0, 0.0),
        g("gaze_up", RotationAxis::X, 20.0, 0.0),
        g("gaze_down", RotationAxis::X, -20.0, 0.0),
        // Bug A: head-local resolution (turned head).
        g("gaze_center_bodyL", RotationAxis::Y, 0.0, 35.0),
        g("gaze_center_bodyR", RotationAxis::Y, 0.0, -35.0),
        g("gaze_right_bodyL", RotationAxis::Y, -30.0, 35.0),
    ]
}
```

- [ ] **Step 5: Run the test to confirm it passes**

Run: `cargo test -p vrm-asset-generator gaze_sweep_covers_both_bugs`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-asset-generator/src/vrma_params.rs crates/vrm-asset-generator/src/sweep.rs
git commit -m "feat(asset-gen): GazeParams + gaze_sweep (8 clips, VMK #332 coverage)"
```

---

### Task 3: Add `emit_gaze_clip()`

**Files:**
- Modify: `crates/vrm-asset-generator/src/emit.rs` (add `emit_gaze_clip` after `emit_vrma_lookat_triplet`, ~line 3010)

Emits only the `.vrma` (no `.vrm`/`.test.yaml`): the avatar is the real fixture and the plans are committed manual YAML. Always builds the canonical skeleton + registers all humanoid bones (so UniVRM's importer invariant is satisfied — closing the issue-#8 lookAt-only-VRMA load failure), adds a spine rotation channel only when `body_yaw_deg != 0`, and always adds the gaze lookAt channel on an appended target node.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)]` tests in `crates/vrm-asset-generator/src/emit.rs` (e.g. a new `mod gaze_emit_tests`):

```rust
#[cfg(test)]
mod gaze_emit_tests {
    use super::*;
    use crate::vrma_params::{GazeParams, RotationAxis};
    use camino::Utf8Path;
    use tempfile::tempdir;

    fn doc_of(path: &Utf8Path) -> serde_json::Value {
        let bytes = std::fs::read(path).unwrap();
        let json_chunk = crate::glb::extract_json_chunk(&bytes).unwrap();
        serde_json::from_slice(&json_chunk).unwrap()
    }

    #[test]
    fn neutral_gaze_clip_has_lookat_and_registered_bones_no_spine_channel() {
        let tmp = tempdir().unwrap();
        let dir = Utf8Path::from_path(tmp.path()).unwrap();
        let p = GazeParams {
            id: "gaze_center".into(),
            gaze_axis: RotationAxis::Y,
            gaze_angle_deg: 0.0,
            body_yaw_deg: 0.0,
            duration_s: 1.0,
        };
        emit_gaze_clip(dir, &p).unwrap();
        let doc = doc_of(&dir.join("gaze_center.vrma"));
        let ext = &doc["extensions"]["VRMC_vrm_animation"];
        // lookAt channel present.
        assert!(ext["lookAt"]["node"].is_number());
        // Humanoid bones registered (UniVRM importer invariant).
        assert!(ext["humanoid"]["humanBones"]["hips"]["node"].is_number());
        assert!(ext["humanoid"]["humanBones"]["spine"]["node"].is_number());
        // Exactly one animation channel (the gaze) — no spine rotation channel.
        let channels = doc["animations"][0]["channels"].as_array().unwrap();
        assert_eq!(channels.len(), 1);
    }

    #[test]
    fn turned_head_clip_adds_spine_rotation_channel() {
        let tmp = tempdir().unwrap();
        let dir = Utf8Path::from_path(tmp.path()).unwrap();
        let p = GazeParams {
            id: "gaze_center_bodyL".into(),
            gaze_axis: RotationAxis::Y,
            gaze_angle_deg: 0.0,
            body_yaw_deg: 35.0,
            duration_s: 1.0,
        };
        emit_gaze_clip(dir, &p).unwrap();
        let doc = doc_of(&dir.join("gaze_center_bodyL.vrma"));
        // Two channels: spine rotation + gaze lookAt.
        let channels = doc["animations"][0]["channels"].as_array().unwrap();
        assert_eq!(channels.len(), 2);
        let spine_node = doc["extensions"]["VRMC_vrm_animation"]["humanoid"]
            ["humanBones"]["spine"]["node"]
            .as_u64()
            .unwrap();
        // One channel targets the spine node.
        assert!(channels
            .iter()
            .any(|c| c["target"]["node"].as_u64() == Some(spine_node)
                && c["target"]["path"] == "rotation"));
    }
}
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `cargo test -p vrm-asset-generator gaze_emit_tests`
Expected: FAIL — `cannot find function emit_gaze_clip`.

- [ ] **Step 3: Implement `emit_gaze_clip()`**

In `crates/vrm-asset-generator/src/emit.rs`, after `emit_vrma_lookat_triplet`:

```rust
/// Emit a single VRMA gaze clip (`{id}.vrma`) for the real-avatar gaze corpus.
///
/// Unlike the sweep triplets, this emits ONLY the .vrma — the avatar is the real
/// `vroid_default_F_1_0.vrm` fixture and the test plan is committed manual YAML.
/// The clip always registers the canonical humanoid skeleton (UniVRM importer
/// invariant), adds a spine yaw channel when `body_yaw_deg != 0` (turned head),
/// and always adds the gaze lookAt channel on an appended target node.
pub fn emit_gaze_clip(output_dir: &Utf8Path, params: &crate::vrma_params::GazeParams) -> Result<()> {
    use crate::vrma_emit::{
        add_humanoid_bone_rotation_channel, add_look_at_channel, build_empty_vrma,
        finalize_vrma_scenes, register_all_humanoid_bones, write_vrma_glb,
    };
    use crate::vrma_params::RotationAxis;

    std::fs::create_dir_all(output_dir)?;

    let skel = crate::humanoid::minimal_skeleton();
    let mut doc = build_empty_vrma();
    doc["nodes"] = skel.nodes_json.clone();
    register_all_humanoid_bones(&mut doc, &skel.bone_to_node);

    let mut buffer = Vec::<u8>::new();

    // Turned-head: spine yaw 0 -> body_yaw_deg over duration.
    if params.body_yaw_deg != 0.0 {
        let half = params.body_yaw_deg.to_radians() / 2.0;
        let spine_quat = [0.0_f32, half.sin(), 0.0, half.cos()];
        let spine_kf = [
            (0.0_f32, [0.0_f32, 0.0, 0.0, 1.0]),
            (params.duration_s, spine_quat),
        ];
        let spine_node = skel.bone_to_node["spine"];
        add_humanoid_bone_rotation_channel(&mut doc, &mut buffer, spine_node, "spine", &spine_kf);
    }

    // Gaze: append a lookAt target node, ramp identity -> gaze over duration.
    let gaze_node = {
        let nodes = doc["nodes"].as_array_mut().unwrap();
        nodes.push(serde_json::json!({ "name": "gaze_target" }));
        nodes.len() - 1
    };
    let half = params.gaze_angle_deg.to_radians() / 2.0;
    let (sin_h, cos_h) = (half.sin(), half.cos());
    let gaze_quat = match params.gaze_axis {
        RotationAxis::X => [sin_h, 0.0, 0.0, cos_h],
        RotationAxis::Y => [0.0, sin_h, 0.0, cos_h],
        RotationAxis::Z => [0.0, 0.0, sin_h, cos_h],
    };
    let gaze_kf = [
        (0.0_f32, [0.0_f32, 0.0, 0.0, 1.0]),
        (params.duration_s, gaze_quat),
    ];
    add_look_at_channel(&mut doc, &mut buffer, gaze_node, [0.0, 0.06, 0.0], &gaze_kf);

    finalize_vrma_scenes(&mut doc);

    let vrma_path = output_dir.join(format!("{}.vrma", params.id));
    let vrma_bytes = write_vrma_glb(&doc, &buffer)?;
    std::fs::write(&vrma_path, &vrma_bytes)?;
    Ok(())
}
```

- [ ] **Step 4: Run the tests to confirm they pass**

Run: `cargo test -p vrm-asset-generator gaze_emit_tests`
Expected: PASS (both tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-asset-generator/src/emit.rs
git commit -m "feat(asset-gen): emit_gaze_clip — VRMA gaze clip (gaze + optional spine yaw)"
```

---

### Task 4: Wire the `emit-gaze-sweep` CLI subcommand

**Files:**
- Modify: `crates/vrm-asset-generator/src/cli.rs` (`Cmd` enum near line 417; match arm near line 2043; `describe` JSON near line 2748)

- [ ] **Step 1: Add the `Cmd` variant**

In `crates/vrm-asset-generator/src/cli.rs`, after the `EmitVrmaLookatSweep { ... }` variant (~line 417), add:

```rust
    /// Emit the real-avatar gaze sweep (8 .vrma clips) covering VMK #332.
    EmitGazeSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },
```

(Match the exact field attributes used by the neighbouring `EmitVrmaLookatSweep` variant — copy its `#[arg(...)]` annotations verbatim if they differ from the above.)

- [ ] **Step 2: Add the match arm**

After the `Cmd::EmitVrmaLookatSweep { .. } => { .. }` arm (~line 2043), add:

```rust
        Cmd::EmitGazeSweep { output_dir, json } => {
            use crate::emit::emit_gaze_clip;
            use crate::sweep::gaze_sweep;

            let sweep = gaze_sweep();
            let total = sweep.len();
            for (i, params) in sweep.iter().enumerate() {
                emit_gaze_clip(&output_dir, params)?;
                if json {
                    eprintln!(
                        r#"{{"event":"progress","op":"emit-gaze-sweep","index":{i},"total":{total},"id":"{id}"}}"#,
                        id = params.id,
                    );
                }
            }
            if json {
                println!(
                    r#"{{"op":"emit-gaze-sweep","count":{total},"output_dir":"{output_dir}"}}"#,
                );
            } else {
                println!("emit-gaze-sweep: wrote {total} gaze clips to {output_dir}");
            }
        }
```

(If the neighbouring arms format their stdout result differently, follow that pattern. The key invariant: progress NDJSON on stderr, structured result on stdout.)

- [ ] **Step 3: Add the `describe` entry**

In the `describe` JSON object (~line 2748, alongside `"emit-vrma-lookat-sweep"`), add:

```rust
                    "emit-gaze-sweep": {
                        "summary": "Real-avatar gaze sweep (8 .vrma clips) covering VMK 0.17.1 #332 bone-driven eye look-at. 5 neutral-body gaze directions (center/left/right/up/down) exercise eye-rest composition (wall-eye); 3 turned-head variants (spine yaw +-35deg) exercise head-local gaze resolution. Emits .vrma only — pair with vroid_default_F_1_0.vrm via the committed test-plans/manual/humanoid/vroid_default_F_gaze_*.test.yaml plans.",
                        "args": { "output_dir": "string", "json": "bool" }
                    },
```

(Match the schema shape of the adjacent `describe` entries — copy the `"args"` style from `emit-vrma-lookat-sweep` if it differs.)

- [ ] **Step 4: Build + smoke the command**

Run:
```bash
cargo build -p vrm-asset-generator
cargo run -p vrm-asset-generator -- emit-gaze-sweep --output-dir /tmp/gaze-sweep
ls /tmp/gaze-sweep
```
Expected: 8 files — `gaze_center.vrma`, `gaze_left.vrma`, `gaze_right.vrma`, `gaze_up.vrma`, `gaze_down.vrma`, `gaze_center_bodyL.vrma`, `gaze_center_bodyR.vrma`, `gaze_right_bodyL.vrma`.

- [ ] **Step 5: Confirm `describe` lists the new op**

Run: `cargo run -p vrm-asset-generator -- describe --format json | grep emit-gaze-sweep`
Expected: one match.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-asset-generator/src/cli.rs
git commit -m "feat(asset-gen): emit-gaze-sweep subcommand (CLI + describe)"
```

---

### Task 5: Add `leftEye`/`rightEye` to the pose-dump reference bone list

Gives the cross-renderer pose diff a numeric eye-bone signal so bug B (eye-rest composition) is caught beyond image SSIM. Both adapters already skip bones absent on the rig, so synthetic corpora are unaffected.

**Files:**
- Modify: `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift:1203-1209` (`referenceHumanoidBones`)
- Modify: `adapters/three-vrm/src/renderer-host.html:270-276` (`HUMANOID_BONES`)

- [ ] **Step 1: Extend the VMK adapter list**

In `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift`, change `referenceHumanoidBones` to append the eye bones (keep existing order; add a new line before the closing `]`):

```swift
    private static let referenceHumanoidBones: [String] = [
        "hips", "spine", "chest", "neck", "head",
        "leftShoulder", "leftUpperArm", "leftLowerArm", "leftHand",
        "rightShoulder", "rightUpperArm", "rightLowerArm", "rightHand",
        "leftUpperLeg", "leftLowerLeg", "leftFoot",
        "rightUpperLeg", "rightLowerLeg", "rightFoot",
        "leftEye", "rightEye",
    ]
```

Also update the doc comment above it (`// 19 bones cover ...`) to read `// 21 bones; leftEye/rightEye carry the gaze signal for bone-driven lookAt`.

- [ ] **Step 2: Extend the three-vrm host list (same order)**

In `adapters/three-vrm/src/renderer-host.html`, change `HUMANOID_BONES`:

```javascript
      const HUMANOID_BONES = [
        "hips", "spine", "chest", "neck", "head",
        "leftShoulder", "leftUpperArm", "leftLowerArm", "leftHand",
        "rightShoulder", "rightUpperArm", "rightLowerArm", "rightHand",
        "leftUpperLeg", "leftLowerLeg", "leftFoot",
        "rightUpperLeg", "rightLowerLeg", "rightFoot",
        "leftEye", "rightEye",
      ];
```

- [ ] **Step 3: Add/extend a Swift unit test asserting the eye bones are present**

In the VMK adapter test target (`adapters/vrm-metal-kit/Tests/VRMMetalKitAdapterTests/`), add a test (new file `PoseDumpBoneListTests.swift` if no suitable file exists):

```swift
import XCTest
@testable import VRMMetalKitAdapter

final class PoseDumpBoneListTests: XCTestCase {
    func testReferenceBonesIncludeEyes() {
        let bones = Operations.referenceHumanoidBonesForTest
        XCTAssertTrue(bones.contains("leftEye"))
        XCTAssertTrue(bones.contains("rightEye"))
        XCTAssertEqual(bones.count, 21)
    }
}
```

If `referenceHumanoidBones` is `private`, add an internal test accessor next to it in `Operations.swift`:

```swift
    #if DEBUG
    static var referenceHumanoidBonesForTest: [String] { referenceHumanoidBones }
    #endif
```

- [ ] **Step 4: Run the Swift test (local macOS 26 only)**

Run: `cd adapters/vrm-metal-kit && swift test --filter PoseDumpBoneListTests`
Expected: PASS. (Skip on non-macOS-26 hosts; CI build-validates only.)

- [ ] **Step 5: Rebuild the three-vrm host bundle**

Run: `cd adapters/three-vrm && npm run build`
Expected: build succeeds (the host HTML is bundled into the adapter).

- [ ] **Step 6: Commit**

```bash
git add adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift \
        adapters/vrm-metal-kit/Tests adapters/three-vrm/src/renderer-host.html
git commit -m "feat(adapters): add leftEye/rightEye to pose-dump bone list (gaze signal)"
```

---

### Task 6: Author the 8 manual gaze test plans

**Files:**
- Create: `test-plans/manual/humanoid/vroid_default_F_gaze_center.test.yaml`
- Create: `test-plans/manual/humanoid/vroid_default_F_gaze_left.test.yaml`
- Create: `test-plans/manual/humanoid/vroid_default_F_gaze_right.test.yaml`
- Create: `test-plans/manual/humanoid/vroid_default_F_gaze_up.test.yaml`
- Create: `test-plans/manual/humanoid/vroid_default_F_gaze_down.test.yaml`
- Create: `test-plans/manual/humanoid/vroid_default_F_gaze_center_bodyL.test.yaml`
- Create: `test-plans/manual/humanoid/vroid_default_F_gaze_center_bodyR.test.yaml`
- Create: `test-plans/manual/humanoid/vroid_default_F_gaze_right_bodyL.test.yaml`

- [ ] **Step 1: Write `vroid_default_F_gaze_center.test.yaml` (the template)**

Face/eye-tight camera (target at eye height ~1.40 m, close, small FOV) so the iris splay/shift is a real SSIM signal. Methodology pins applied. `apply_at_time` equals the clip duration (1.0) for full gaze.

```yaml
id: vroid_default_F_gaze_center
spec_section: VRMC_vrm lookAt (bone-driven eye gaze — VMK 0.17.1 #332 eye-rest composition; center should render parallel, not wall-eyed)
asset: vroid_default_F_1_0.vrm
animation:
  vrma:
    path: gaze_center.vrma
    apply_at_time: 1.0
camera:
  position:
  - 0.0
  - 1.40
  - 0.30
  target:
  - 0.0
  - 1.40
  - 0.0
  up:
  - 0.0
  - 1.0
  - 0.0
  fov_degrees: 18.0
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

- [ ] **Step 2: Write the other 7 plans**

Copy the template and change only `id`, the `spec_section` note, and `animation.vrma.path` (= `<id-without-prefix>.vrma`). The avatar, camera, lighting, output, and diff blocks are identical across all 8.

| plan id | `animation.vrma.path` | spec_section note |
|---|---|---|
| `vroid_default_F_gaze_left` | `gaze_left.vrma` | bone-driven eye gaze, yaw left — side gaze direction |
| `vroid_default_F_gaze_right` | `gaze_right.vrma` | bone-driven eye gaze, yaw right — side gaze direction |
| `vroid_default_F_gaze_up` | `gaze_up.vrma` | bone-driven eye gaze, pitch up |
| `vroid_default_F_gaze_down` | `gaze_down.vrma` | bone-driven eye gaze, pitch down |
| `vroid_default_F_gaze_center_bodyL` | `gaze_center_bodyL.vrma` | #332 head-local resolution — body yawed +35, eyes must stay parallel & head-relative |
| `vroid_default_F_gaze_center_bodyR` | `gaze_center_bodyR.vrma` | #332 head-local resolution — body yawed -35 |
| `vroid_default_F_gaze_right_bodyL` | `gaze_right_bodyL.vrma` | #332 head-local — gaze right while body yawed +35 (gaze and turn opposed) |

For the `*_body*` plans, widen the camera slightly so the turned head stays framed: change `camera.position` to `[0.0, 1.40, 0.45]` and `fov_degrees` to `24.0` (tunable at bootstrap).

- [ ] **Step 3: Validate every plan parses against the test-plan schema**

Run:
```bash
for f in test-plans/manual/humanoid/vroid_default_F_gaze_*.test.yaml; do
  cargo run -p vrm-runner -- diff --plan "$f" --render /dev/null --reference /dev/null --renderer-name x --json 2>&1 | head -1 || true
done
```
Expected: each plan loads and fails on missing PNG (a *parse* failure would name the YAML field; a missing-file error means the schema accepted the plan). If any plan reports an unknown/!invalid field, fix the YAML. (Rendering itself is local-only — see Task 8.)

- [ ] **Step 4: Commit**

```bash
git add test-plans/manual/humanoid/vroid_default_F_gaze_*.test.yaml
git commit -m "test(gaze): 8 manual gaze plans on vroid_default_F (VMK #332 coverage)"
```

---

### Task 7: Wire `emit-gaze-sweep` into the fixture install path

So the gaze `.vrma` clips land next to `vroid_default_F_1_0.vrm` in the asset dir the runner reads.

**Files:**
- Modify: `scripts/install-humanoid-fixtures.sh` (after the `vroid_default_F_1_0.vrm` install line, ~line 57)

- [ ] **Step 1: Find the install target dir**

Run: `grep -n "install_one\|ASSET_DIR\|assets/humanoid\|output" scripts/install-humanoid-fixtures.sh | head`
Expected: identifies the dir fixtures install into (the `assets/humanoid` path used by `install_one`).

- [ ] **Step 2: Append the gaze-clip emission**

After the `install_one "vroid_default_F_1_0.vrm" ...` line in `scripts/install-humanoid-fixtures.sh`, add (use the same dir variable the script already uses for the avatar; shown as `assets/humanoid` here):

```bash
# Gaze VRMA clips for the VMK #332 eye look-at corpus (vroid_default_F_gaze_*).
# Generated next to the avatar so the runner's --asset-dir finds both.
echo "Emitting gaze VRMA clips (VMK #332 coverage)..."
cargo run -q -p vrm-asset-generator -- emit-gaze-sweep --output-dir assets/humanoid
```

- [ ] **Step 3: Ensure generated `.vrma` are gitignored**

Run: `grep -n "vrma\|assets/humanoid\|gaze" .gitignore`
Expected: confirm generated assets under `assets/humanoid` are ignored (matching how the symlinked `.vrm` is untracked). If `gaze_*.vrma` is not covered, add `assets/humanoid/gaze_*.vrma` to `.gitignore`.

- [ ] **Step 4: Dry-run the emission into the asset dir**

Run: `cargo run -p vrm-asset-generator -- emit-gaze-sweep --output-dir assets/humanoid && ls assets/humanoid/gaze_*.vrma`
Expected: 8 `gaze_*.vrma` files present.

- [ ] **Step 5: Commit**

```bash
git add scripts/install-humanoid-fixtures.sh .gitignore
git commit -m "chore(fixtures): emit gaze VRMA clips into the humanoid asset dir"
```

---

### Task 8: Local render verification + findings entry

Rendering requires macOS 26 / Xcode 26 (CI build-validates only). This task is run on an M-series Mac and recorded as a deliverable.

**Files:**
- Modify: `docs/findings.md` (append a "VMK 0.17.1 eye look-at (#332)" entry)
- Modify: `docs/upstream/VMK-vrma-lookat-renderer-propagation.md` (mark the suite-side asset coverage follow-up closed)

- [ ] **Step 1: Build the VMK adapter at 0.17.1**

Run: `cd adapters/vrm-metal-kit && swift build -c release`
Expected: builds against `VRMMetalKit @ 421232b`.

- [ ] **Step 2: Emit the gaze clips into a render asset dir**

Run: `cargo run -p vrm-asset-generator -- emit-gaze-sweep --output-dir assets/humanoid`
Expected: 8 clips. (Ensure `vroid_default_F_1_0.vrm` is installed via `scripts/install-humanoid-fixtures.sh`.)

- [ ] **Step 3: Render each gaze plan through VMK**

Run (per plan):
```bash
for f in test-plans/manual/humanoid/vroid_default_F_gaze_*.test.yaml; do
  pid=$(basename "$f" .test.yaml)
  cargo run -p vrm-runner -- execute-test-plan \
    --plan "$f" \
    --adapter-bin adapters/vrm-metal-kit/.build/release/vrm-metal-kit-adapter \
    --asset-dir assets/humanoid \
    --output-dir "goldens-cache/gaze/$pid" \
    --renderer-name vrm-metal-kit --json | python3 -c 'import sys,json; d=json.load(sys.stdin); print(d["overall_passed"])'
done
```
Expected: each pipeline runs (PNG + pose.json produced).

- [ ] **Step 4: Capture the before/after signal**

Visually inspect `goldens-cache/gaze/*/`: on `gaze_center` the eyes must be **parallel / straight-ahead** (0.17.1) versus wall-eyed-splayed (0.17.0); on `gaze_center_bodyL/R` the eyes must track **head-relative** rather than offset by the body yaw. Confirm the pose dump now reports non-trivial `leftEye`/`rightEye` rotations and that `look_at.yaw_deg`/`pitch_deg` match the clip. To get the explicit 0.17.0 comparison, temporarily check out `Package.swift` at the prior revision, rebuild, re-render `gaze_center` + `gaze_center_bodyL`, and diff.

- [ ] **Step 5: Run cross-renderer consensus where available**

Run: `cargo run -p vrm-runner -- consensus-diff --plan test-plans/manual/humanoid/vroid_default_F_gaze_center.test.yaml --render vrm-metal-kit=<png> --render three-vrm=<png> [--render godot-vrm=<png>]`
Expected: VMK now agrees with three-vrm (both compose eye rest correctly). Record the SSIM. (UniVRM real-1.0 oracle remains blocked by the `execute-test-batch` manual-plan limitation — note, do not block.)

- [ ] **Step 6: Write the findings entry**

Append to `docs/findings.md` a dated "VMK 0.17.1 eye look-at (#332) — suite coverage landed" entry recording: the two sub-bugs, the new corpus, the 0.17.0→0.17.1 before/after (wall-eyed→parallel; offset→head-relative), the eye-bone pose-dump values, and the cross-renderer consensus number. In `docs/upstream/VMK-vrma-lookat-renderer-propagation.md`, mark the "suite-side asset coverage needs extending" follow-up **closed** by this corpus.

- [ ] **Step 7: Commit**

```bash
git add docs/findings.md docs/upstream/VMK-vrma-lookat-renderer-propagation.md
git commit -m "docs(findings): VMK 0.17.1 eye look-at (#332) — suite coverage + before/after"
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

Use the `superpowers:finishing-a-development-branch` skill to choose merge/PR/cleanup for branch `lookat-0171-coverage`.

---

## Notes for the implementer

- **Order matters in the VMK adapter wiring** (verify during Task 8): `applyImmediately()` must run *after* the VRMA spine rotation is baked into the head's world matrix, or the head-local resolution reads a stale head transform and the `*_body*` clips won't differ on 0.17.1 either. If the `*_body*` clips render byte-identical to their non-turned counterparts, inspect `handleApplyVrmaAtTime` in `Operations.swift` for apply ordering — this is the likely culprit, not the asset.
- **Angles are tunable.** Gaze ±30° yaw / ±20° pitch and body ±35° are nominal; if they exceed the avatar's lookAt `rangeMap.inputMaxValue` the eye rotation clamps. Tune at bootstrap (Task 8) and update the sweep constants if needed.
- **Out of scope (tracked follow-ups):** parametric synthetic eye bones with mirrored rest; UniVRM real-1.0 oracle via `execute-test-batch` manual-plan support.

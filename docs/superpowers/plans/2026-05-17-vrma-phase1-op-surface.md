# VRMA Phase 1 — Op Surface + Unimplemented Stubs

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the five VRMA op types in `crates/vrm-ops/` and have every adapter return `-32000 Unimplemented` (with `phase: "vrma-v1"` in the error envelope) for all five. After this phase, the operation contract published by `describe` declares VRMA support, and the runner-side wiring in subsequent phases can be developed against a stable surface.

**Architecture:** Five new `*Params` / `*Result` types in `crates/vrm-ops/src/tools.rs` mirroring the existing op-type pattern, with full serde round-trip tests. Each adapter (VMK / three-vrm / godot-vrm / UniVRM) dispatches the five new method names through its existing Unimplemented escape route — no real implementation lands in this phase. The describe catalog reads the same types via the same JSON-Schema-via-`schemars` path the existing ops use; tests verify catalog exposure.

**Tech Stack:** Rust workspace (vrm-ops), Swift (VMK adapter), TypeScript (three-vrm adapter), Rust + GDScript (godot-vrm adapter via vrm-godot-shim), C# (UniVRM adapter).

**Spec:** [`docs/superpowers/specs/2026-05-17-vrma-conformance-design.md`](../specs/2026-05-17-vrma-conformance-design.md) — Op surface additions section.

---

## File structure

**Modify:**
- `crates/vrm-ops/src/tools.rs` — add 5 op param/result types (~150 LOC additions, all in the same file because that's where every other op type lives in this crate)
- `crates/vrm-ops/tests/serde.rs` — add serde round-trip tests for the new types
- `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift` — add 5 method names to the Unimplemented deferral set
- `adapters/three-vrm/src/operations.ts` — add 5 method handlers returning Unimplemented
- `crates/vrm-godot-shim/src/bridge.rs` — add 5 method names to the shim's Unimplemented set
- `adapters/univrm/UniVRMConformance/Assets/Conformance/Conformance.cs` — add 5 method names to the batch dispatcher's Unimplemented path
- `docs/operation-contract.md` — add the five new ops to the contract documentation

**Create:** none (this phase is pure additions to existing files).

---

## Task 1: `LoadVrma` op types in vrm-ops

**Files:**
- Modify: `crates/vrm-ops/src/tools.rs`
- Test: `crates/vrm-ops/tests/serde.rs`

- [ ] **Step 1.1: Write the failing serde round-trip test**

Append to `crates/vrm-ops/tests/serde.rs`:

```rust
#[test]
fn load_vrma_params_roundtrip() {
    let p = LoadVrmaParams {
        vrma_path: "/tmp/test.vrma".into(),
    };
    let s = serde_json::to_string(&p).unwrap();
    let back: LoadVrmaParams = serde_json::from_str(&s).unwrap();
    assert_eq!(back.vrma_path, "/tmp/test.vrma");
    assert!(s.contains(r#""vrma_path":"/tmp/test.vrma""#));
}

#[test]
fn load_vrma_result_roundtrip() {
    let r = LoadVrmaResult {
        vrma_handle: 42,
        channel_summary: VrmaChannelSummary {
            humanoid_bones: 15,
            expressions: 3,
            has_look_at: true,
            duration_seconds: 1.0,
        },
    };
    let s = serde_json::to_string(&r).unwrap();
    let back: LoadVrmaResult = serde_json::from_str(&s).unwrap();
    assert_eq!(back.vrma_handle, 42);
    assert_eq!(back.channel_summary.humanoid_bones, 15);
    assert!(back.channel_summary.has_look_at);
}
```

- [ ] **Step 1.2: Run test to verify it fails**

Run: `cargo test -p vrm-ops --test serde load_vrma`
Expected: FAIL with `error[E0422]: cannot find struct, variant or union type 'LoadVrmaParams'`.

- [ ] **Step 1.3: Add the types in tools.rs**

Append to `crates/vrm-ops/src/tools.rs` after `DumpBonePositionsResult`:

```rust
/// Load a `.vrma` file (VRMC_vrm_animation glTF) and return an opaque handle
/// plus a summary of the channels it contains. Only the first animation
/// (`animations[0]`) is treated as the portable clip per VRMA spec; multi-
/// animation `.vrma` files are accepted but only `animations[0]` is sampled.
///
/// Adapters that do not implement VRMA MUST return `-32000 Unimplemented`
/// with `data: { phase: "vrma-v1" }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadVrmaParams {
    /// Filesystem path to a `.vrma` file. BLAKE3 refs (`blake3:<64-hex>`)
    /// are also accepted by adapters that resolve content-addressed inputs.
    pub vrma_path: String,
}

/// Summary of the channels a loaded `.vrma` references.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VrmaChannelSummary {
    /// Count of humanoid bones referenced (by `humanoid.humanBones`).
    pub humanoid_bones: u32,
    /// Count of expressions referenced (preset + custom combined).
    pub expressions: u32,
    /// True if the `.vrma` contains a `lookAt` block.
    pub has_look_at: bool,
    /// Duration of `animations[0]` in seconds.
    pub duration_seconds: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadVrmaResult {
    /// Opaque handle the adapter assigns; subsequent ops reference this.
    pub vrma_handle: u32,
    pub channel_summary: VrmaChannelSummary,
}
```

- [ ] **Step 1.4: Run test to verify it passes**

Run: `cargo test -p vrm-ops --test serde load_vrma`
Expected: PASS — both tests pass.

- [ ] **Step 1.5: Commit**

```bash
git add crates/vrm-ops/src/tools.rs crates/vrm-ops/tests/serde.rs
git commit -m "$(cat <<'EOF'
feat(vrm-ops): add LoadVrmaParams + LoadVrmaResult + VrmaChannelSummary

First of five VRMA op types. Mirrors LoadVrmParams's parse-then-handle
shape; the channel summary lets callers preview a .vrma before applying
it. Per spec, animations[0] is the portable clip.

Per docs/superpowers/specs/2026-05-17-vrma-conformance-design.md.
EOF
)"
```

---

## Task 2: `ApplyVrmaAtTime` op types

**Files:**
- Modify: `crates/vrm-ops/src/tools.rs`
- Test: `crates/vrm-ops/tests/serde.rs`

- [ ] **Step 2.1: Write the failing serde round-trip test**

Append to `crates/vrm-ops/tests/serde.rs`:

```rust
#[test]
fn apply_vrma_at_time_params_roundtrip() {
    let p = ApplyVrmaAtTimeParams {
        session_id: "sess-vrma".into(),
        vrma_handle: 7,
        vrm_handle: 3,
        time_seconds: 0.5,
    };
    let s = serde_json::to_string(&p).unwrap();
    let back: ApplyVrmaAtTimeParams = serde_json::from_str(&s).unwrap();
    assert_eq!(back.vrma_handle, 7);
    assert_eq!(back.time_seconds, 0.5);
}

#[test]
fn apply_vrma_at_time_result_roundtrip() {
    let r = ApplyVrmaAtTimeResult {
        channels_applied: VrmaChannelsApplied {
            humanoid_bones: 12,
            expressions: 2,
            look_at: false,
        },
    };
    let s = serde_json::to_string(&r).unwrap();
    let back: ApplyVrmaAtTimeResult = serde_json::from_str(&s).unwrap();
    assert_eq!(back.channels_applied.humanoid_bones, 12);
    assert!(!back.channels_applied.look_at);
}
```

- [ ] **Step 2.2: Run test to verify it fails**

Run: `cargo test -p vrm-ops --test serde apply_vrma_at_time`
Expected: FAIL with `cannot find struct ApplyVrmaAtTimeParams`.

- [ ] **Step 2.3: Add the types in tools.rs**

Append to `crates/vrm-ops/src/tools.rs`:

```rust
/// Sample the loaded `.vrma` at `time_seconds` and write the resulting pose
/// onto the avatar identified by `vrm_handle`. Linear interpolation is the
/// spec-mandated default. State-advancing — the subsequent `dump_*` ops
/// capture this op's effect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyVrmaAtTimeParams {
    pub session_id: String,
    pub vrma_handle: u32,
    pub vrm_handle: u32,
    pub time_seconds: f32,
}

/// Per-channel application counts. Lets callers verify that each channel
/// in the loaded `.vrma` was actually applied (zero counts surface
/// silent-skip bugs).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VrmaChannelsApplied {
    pub humanoid_bones: u32,
    pub expressions: u32,
    pub look_at: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyVrmaAtTimeResult {
    pub channels_applied: VrmaChannelsApplied,
}
```

- [ ] **Step 2.4: Run test to verify it passes**

Run: `cargo test -p vrm-ops --test serde apply_vrma_at_time`
Expected: PASS.

- [ ] **Step 2.5: Commit**

```bash
git add crates/vrm-ops/src/tools.rs crates/vrm-ops/tests/serde.rs
git commit -m "$(cat <<'EOF'
feat(vrm-ops): add ApplyVrmaAtTimeParams + VrmaChannelsApplied + result

State-advancing op; samples the loaded .vrma at t. Linear interpolation
per VRMA spec. The channels_applied counts surface silent-skip bugs in
adapter implementations.
EOF
)"
```

---

## Task 3: `DumpHumanoidPose` op types

**Files:**
- Modify: `crates/vrm-ops/src/tools.rs`
- Test: `crates/vrm-ops/tests/serde.rs`

- [ ] **Step 3.1: Write the failing serde round-trip test**

Append to `crates/vrm-ops/tests/serde.rs`:

```rust
#[test]
fn dump_humanoid_pose_result_roundtrip() {
    let r = DumpHumanoidPoseResult {
        bones: vec![
            HumanoidBoneRotation {
                name: "leftUpperArm".into(),
                local_rotation_quat: [0.0, 0.0, 0.7071, 0.7071],
            },
            HumanoidBoneRotation {
                name: "head".into(),
                local_rotation_quat: [0.0, 0.0, 0.0, 1.0],
            },
        ],
        hips_translation: [0.0, 0.05, 0.0],
        bones_missing: vec!["leftThumbDistal".into()],
    };
    let s = serde_json::to_string(&r).unwrap();
    let back: DumpHumanoidPoseResult = serde_json::from_str(&s).unwrap();
    assert_eq!(back.bones.len(), 2);
    assert_eq!(back.bones[0].name, "leftUpperArm");
    assert_eq!(back.hips_translation[1], 0.05);
    assert_eq!(back.bones_missing[0], "leftThumbDistal");
}
```

- [ ] **Step 3.2: Run test to verify it fails**

Run: `cargo test -p vrm-ops --test serde dump_humanoid_pose`
Expected: FAIL with `cannot find struct DumpHumanoidPoseResult`.

- [ ] **Step 3.3: Add the types in tools.rs**

Append to `crates/vrm-ops/src/tools.rs`:

```rust
/// Dump per-bone local rotations + hips translation for the loaded VRM as
/// of the most recent state-advancing op (`apply_vrma_at_time`, `render`,
/// `reset_physics`, etc.). Per the VRMA spec, only the `hips` bone carries
/// translation; all other humanoid bones contribute only rotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpHumanoidPoseParams {
    pub session_id: String,
}

/// Single humanoid bone rotation. The name follows the spec's bone-name
/// enum (`hips`, `leftUpperArm`, ...). Quaternion in `[x, y, z, w]` order
/// matching glTF convention.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HumanoidBoneRotation {
    pub name: String,
    pub local_rotation_quat: [f32; 4],
}

/// Bones present in the .vrm with their local rotations, plus the hips
/// translation, plus any bones that the .vrma referenced but the .vrm
/// does not have (excluded from per-bone diff per methodology hazard #3).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DumpHumanoidPoseResult {
    pub bones: Vec<HumanoidBoneRotation>,
    pub hips_translation: [f32; 3],
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bones_missing: Vec<String>,
}
```

- [ ] **Step 3.4: Run test to verify it passes**

Run: `cargo test -p vrm-ops --test serde dump_humanoid_pose`
Expected: PASS.

- [ ] **Step 3.5: Commit**

```bash
git add crates/vrm-ops/src/tools.rs crates/vrm-ops/tests/serde.rs
git commit -m "$(cat <<'EOF'
feat(vrm-ops): add DumpHumanoidPose op types

Returns per-bone local quaternion rotations + hips translation, per VRMA
spec rules. Bones referenced by the .vrma but absent from the .vrm
appear in bones_missing and are excluded from diff per methodology
hazard #3.
EOF
)"
```

---

## Task 4: `DumpExpressionWeights` op types

**Files:**
- Modify: `crates/vrm-ops/src/tools.rs`
- Test: `crates/vrm-ops/tests/serde.rs`

- [ ] **Step 4.1: Write the failing serde round-trip test**

Append to `crates/vrm-ops/tests/serde.rs`:

```rust
#[test]
fn dump_expression_weights_result_roundtrip() {
    let mut presets = std::collections::BTreeMap::new();
    presets.insert("happy".to_string(), 0.83_f32);
    presets.insert("blink".to_string(), 0.02_f32);
    let mut custom = std::collections::BTreeMap::new();
    custom.insert("smug".to_string(), 0.5_f32);
    let r = DumpExpressionWeightsResult { presets, custom };
    let s = serde_json::to_string(&r).unwrap();
    let back: DumpExpressionWeightsResult = serde_json::from_str(&s).unwrap();
    assert_eq!(back.presets.get("happy"), Some(&0.83));
    assert_eq!(back.custom.get("smug"), Some(&0.5));
}
```

- [ ] **Step 4.2: Run test to verify it fails**

Run: `cargo test -p vrm-ops --test serde dump_expression_weights`
Expected: FAIL with `cannot find struct DumpExpressionWeightsResult`.

- [ ] **Step 4.3: Add the types in tools.rs**

Append to `crates/vrm-ops/src/tools.rs`:

```rust
/// Dump current expression weights (preset + custom). Per the VRMA spec,
/// weights are encoded as the X-component of bound-node translation
/// animation, clamped to [0, 1]; this op returns the clamped values the
/// renderer actually applies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpExpressionWeightsParams {
    pub session_id: String,
}

/// Preset and custom expressions kept structurally separate per spec.
/// Preset name set per VRMA spec: happy, angry, sad, relaxed, surprised,
/// aa, ih, ou, ee, oh, blink, blinkLeft, blinkRight, neutral.
/// `lookUp/lookDown/lookLeft/lookRight` are NOT VRMA presets — driven by
/// LookAt and reported via `dump_look_at_state` instead.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DumpExpressionWeightsResult {
    pub presets: std::collections::BTreeMap<String, f32>,
    pub custom: std::collections::BTreeMap<String, f32>,
}
```

- [ ] **Step 4.4: Run test to verify it passes**

Run: `cargo test -p vrm-ops --test serde dump_expression_weights`
Expected: PASS.

- [ ] **Step 4.5: Commit**

```bash
git add crates/vrm-ops/src/tools.rs crates/vrm-ops/tests/serde.rs
git commit -m "$(cat <<'EOF'
feat(vrm-ops): add DumpExpressionWeights op types

BTreeMap<String, f32> for preset and custom expressions kept separate
per spec. The 14 preset names are spec-defined; custom expressions allow
any non-preset name. BTreeMap chosen for deterministic JSON ordering.
EOF
)"
```

---

## Task 5: `DumpLookAtState` op types

**Files:**
- Modify: `crates/vrm-ops/src/tools.rs`
- Test: `crates/vrm-ops/tests/serde.rs`

- [ ] **Step 5.1: Write the failing serde round-trip test**

Append to `crates/vrm-ops/tests/serde.rs`:

```rust
#[test]
fn dump_look_at_state_result_roundtrip() {
    let r = DumpLookAtStateResult {
        gaze_direction_quat: [0.0, 0.2588, 0.0, 0.9659],  // 30° yaw
        yaw_deg: 30.0,
        pitch_deg: 0.0,
        applied_via: LookAtAppliedVia::Bone,
        offset_from_head_bone: [0.0, 0.06, 0.0],
    };
    let s = serde_json::to_string(&r).unwrap();
    let back: DumpLookAtStateResult = serde_json::from_str(&s).unwrap();
    assert!(matches!(back.applied_via, LookAtAppliedVia::Bone));
    assert_eq!(back.yaw_deg, 30.0);

    // Variant serialization sanity
    assert!(s.contains(r#""applied_via":"bone""#));
}

#[test]
fn dump_look_at_state_applied_via_off_serializes() {
    let r = DumpLookAtStateResult {
        gaze_direction_quat: [0.0, 0.0, 0.0, 1.0],
        yaw_deg: 0.0,
        pitch_deg: 0.0,
        applied_via: LookAtAppliedVia::Off,
        offset_from_head_bone: [0.0, 0.0, 0.0],
    };
    let s = serde_json::to_string(&r).unwrap();
    assert!(s.contains(r#""applied_via":"off""#));
}
```

- [ ] **Step 5.2: Run test to verify it fails**

Run: `cargo test -p vrm-ops --test serde dump_look_at_state`
Expected: FAIL.

- [ ] **Step 5.3: Add the types in tools.rs**

Append to `crates/vrm-ops/src/tools.rs`:

```rust
/// Dump current eye gaze state. Per the VRMA spec, the .vrma file declares
/// gaze direction via a node rotation quaternion plus `offsetFromHeadBone`.
/// The avatar's `VRMC_vrm.lookAt.type` (bone vs aim) determines how that
/// direction is applied — that distinction lives in the avatar config,
/// not in VRMA. This op exposes both:
///   - the raw VRMA-declared gaze (quat + spec-defined Extrinsic ZXY
///     yaw/pitch)
///   - the avatar's application mode (`applied_via`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpLookAtStateParams {
    pub session_id: String,
}

/// How the avatar (per its `VRMC_vrm.lookAt.type`) applies the gaze
/// direction declared by the .vrma. Reported by the adapter from the
/// avatar's config, not derived from the .vrma.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LookAtAppliedVia {
    /// Avatar `VRMC_vrm.lookAt.type: bone` — gaze rotates head/eye bones.
    Bone,
    /// Avatar `VRMC_vrm.lookAt.type: expression` — gaze drives lookUp/
    /// lookDown/lookLeft/lookRight preset expressions.
    Expression,
    /// Avatar has no LookAt configured, or the renderer doesn't apply it.
    Off,
}

/// Raw quaternion gaze direction + spec-defined Extrinsic ZXY yaw/pitch
/// (yaw = rotation around Y, pitch = rotation around X) + avatar's
/// application mode + head-local offset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DumpLookAtStateResult {
    pub gaze_direction_quat: [f32; 4],
    pub yaw_deg: f32,
    pub pitch_deg: f32,
    pub applied_via: LookAtAppliedVia,
    pub offset_from_head_bone: [f32; 3],
}
```

- [ ] **Step 5.4: Run test to verify it passes**

Run: `cargo test -p vrm-ops --test serde dump_look_at_state`
Expected: PASS (both tests).

- [ ] **Step 5.5: Commit**

```bash
git add crates/vrm-ops/src/tools.rs crates/vrm-ops/tests/serde.rs
git commit -m "$(cat <<'EOF'
feat(vrm-ops): add DumpLookAtState op types

Exposes both raw quat (per .vrma) and spec-defined Extrinsic ZXY
yaw/pitch. The applied_via enum captures the avatar's VRMC_vrm.lookAt.type
choice — that distinction is an avatar property, not a VRMA property.
EOF
)"
```

---

## Task 6: Document the 5 ops in operation-contract.md

**Files:**
- Modify: `docs/operation-contract.md`

- [ ] **Step 6.1: Append the VRMA op section**

Open `docs/operation-contract.md`. After the existing `Physics operations` section but before `Reserved operations (Phase 2+)`, insert a new section:

```markdown
## VRMA (VRMC_vrm_animation) operations

Defined in the [VRMA conformance design spec](superpowers/specs/2026-05-17-vrma-conformance-design.md). Adapters that have not implemented VRMA return the standard Unimplemented envelope:

```
-32000 Unimplemented   data: { "phase": "vrma-v1" }
```

### `load_vrma`

Parse a `.vrma` file (VRMC_vrm_animation glTF) and return an opaque handle plus a summary of the channels it contains. Only `animations[0]` is treated as the portable clip per VRMA spec.

- Params: `LoadVrmaParams { vrma_path: string }`
- Result: `LoadVrmaResult { vrma_handle: u32, channel_summary: { humanoid_bones, expressions, has_look_at, duration_seconds } }`

### `apply_vrma_at_time`

Sample the loaded clip at `time_seconds` and write the resulting pose onto the avatar identified by `vrm_handle`. Linear interpolation is the spec-mandated default. State-advancing.

- Params: `ApplyVrmaAtTimeParams { session_id, vrma_handle, vrm_handle, time_seconds }`
- Result: `ApplyVrmaAtTimeResult { channels_applied: { humanoid_bones, expressions, look_at } }`

### `dump_humanoid_pose`

Return per-bone local rotations + the hips translation as of the most recent state-advancing op. Bones referenced by the .vrma but absent from the .vrm appear in `bones_missing` and are excluded from per-bone diff (methodology hazard #3).

- Params: `DumpHumanoidPoseParams { session_id }`
- Result: `DumpHumanoidPoseResult { bones, hips_translation, bones_missing }`

### `dump_expression_weights`

Return current expression weights, preset + custom kept structurally separate per spec. Weights are clamped to `[0, 1]`.

- Params: `DumpExpressionWeightsParams { session_id }`
- Result: `DumpExpressionWeightsResult { presets: map<string, f32>, custom: map<string, f32> }`

### `dump_look_at_state`

Return current gaze direction (raw quat + spec-defined Extrinsic ZXY yaw/pitch) and the avatar's application mode (`bone | expression | off`).

- Params: `DumpLookAtStateParams { session_id }`
- Result: `DumpLookAtStateResult { gaze_direction_quat, yaw_deg, pitch_deg, applied_via, offset_from_head_bone }`
```

- [ ] **Step 6.2: Verify the doc renders coherently**

Run: `head -300 docs/operation-contract.md`
Expected: New `## VRMA (VRMC_vrm_animation) operations` section appears with all 5 op subsections; existing sections intact.

- [ ] **Step 6.3: Commit**

```bash
git add docs/operation-contract.md
git commit -m "$(cat <<'EOF'
docs(operation-contract): document the 5 VRMA ops

load_vrma, apply_vrma_at_time, dump_humanoid_pose,
dump_expression_weights, dump_look_at_state. Standard Unimplemented
envelope for adapters that haven't shipped VRMA support (phase: vrma-v1).
EOF
)"
```

---

## Task 7: VMK adapter Unimplemented stubs

**Files:**
- Modify: `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift:48-53`

- [ ] **Step 7.1: Read the current reservedPhases dictionary**

Run: `sed -n '44,57p' adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift`
Expected: shows the existing `reservedPhases` static dictionary mapping method names → phase labels.

- [ ] **Step 7.2: Add the 5 VRMA op names to the reservedPhases dict**

Edit `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift`. Replace the `reservedPhases` block with:

```swift
    /// Reserved ops — declared by every adapter, return Unimplemented in v0.1.
    /// Phase labels match `docs/operation-contract.md`. L3-e promoted the
    /// three Phase 2 physics ops out of the deferral block; they now have
    /// real handlers backed by VRMMetalKit's spring-bone GPU system.
    /// VRMA ops are deferred to a future phase pending VMK#165 closure.
    static let reservedPhases: [String: String] = [
        "set_environment":         "v1.x",
        "set_expression":          "Phase 3",
        "set_humanoid_pose":       "Phase 2",
        "set_root_transform":      "Phase 2",
        "load_vrma":               "vrma-v1",
        "apply_vrma_at_time":      "vrma-v1",
        "dump_humanoid_pose":      "vrma-v1",
        "dump_expression_weights": "vrma-v1",
        "dump_look_at_state":      "vrma-v1",
    ]
```

- [ ] **Step 7.3: Build the adapter and verify it compiles**

Run: `cd adapters/vrm-metal-kit && swift build`
Expected: `Build complete!` with no errors.

- [ ] **Step 7.4: Test Unimplemented dispatch via JSON-RPC**

Run:

```bash
cd adapters/vrm-metal-kit
swift run vrm-metal-kit-adapter <<< '{"jsonrpc":"2.0","id":1,"method":"load_vrma","params":{"vrma_path":"/tmp/x.vrma"}}'
```

Expected: stdout contains `"error":{"code":-32000` and `"phase":"vrma-v1"`. (Exact framing may differ; inspect stderr if no stdout response.)

- [ ] **Step 7.5: Commit**

```bash
git add adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift
git commit -m "$(cat <<'EOF'
feat(adapters/vrm-metal-kit): add VRMA op stubs returning Unimplemented

All 5 VRMA ops return -32000 with phase: "vrma-v1". Real implementation
deferred pending VMK#165 closure upstream.
EOF
)"
```

---

## Task 8: three-vrm adapter Unimplemented stubs

**Files:**
- Modify: `adapters/three-vrm/src/operations.ts`

- [ ] **Step 8.1: Identify the existing dispatcher pattern**

Run: `grep -n "method\|case \"\|switch\|Unimplemented" adapters/three-vrm/src/operations.ts | head -30`
Expected: shows the existing method-name dispatcher (likely a switch statement or map).

- [ ] **Step 8.2: Add the 5 VRMA method cases**

Edit `adapters/three-vrm/src/operations.ts`. Add the 5 op names to whatever Unimplemented branch the existing reserved ops use. The pattern is "if method is one of these, return error -32000 with phase". Add to the existing set:

```typescript
const VRMA_V1_OPS = new Set([
  "load_vrma",
  "apply_vrma_at_time",
  "dump_humanoid_pose",
  "dump_expression_weights",
  "dump_look_at_state",
]);

// In the method dispatcher, before the unknown-method fallthrough:
if (VRMA_V1_OPS.has(method)) {
  return {
    error: {
      code: -32000,
      message: "Unimplemented",
      data: { phase: "vrma-v1" },
    },
  };
}
```

Adapt placement to match the file's existing dispatch shape — the goal is `-32000 + phase: "vrma-v1"` for these 5 method names.

- [ ] **Step 8.3: Build and test**

Run: `cd adapters/three-vrm && npm run build && npm test`
Expected: build succeeds, existing tests pass.

- [ ] **Step 8.4: Add an Unimplemented test**

Append to whatever test file exercises Unimplemented elsewhere (`adapters/three-vrm/test/operations.test.ts` or similar — find with `grep -rn "Unimplemented" adapters/three-vrm/test/`):

```typescript
it("returns Unimplemented for VRMA ops with phase vrma-v1", async () => {
  for (const method of ["load_vrma", "apply_vrma_at_time", "dump_humanoid_pose", "dump_expression_weights", "dump_look_at_state"]) {
    const resp = await dispatch({ jsonrpc: "2.0", id: 1, method, params: {} });
    assert.equal(resp.error.code, -32000);
    assert.equal(resp.error.data.phase, "vrma-v1");
  }
});
```

- [ ] **Step 8.5: Run the new test**

Run: `cd adapters/three-vrm && npm test`
Expected: new VRMA Unimplemented test passes; existing tests still pass.

- [ ] **Step 8.6: Commit**

```bash
git add adapters/three-vrm/src/operations.ts adapters/three-vrm/test/
git commit -m "$(cat <<'EOF'
feat(adapters/three-vrm): add VRMA op stubs returning Unimplemented

All 5 VRMA ops return -32000 with phase: "vrma-v1". Real implementation
will follow in vrma-phase5 by wiring @pixiv/three-vrm-animation.
EOF
)"
```

---

## Task 9: godot-vrm adapter Unimplemented stubs (via vrm-godot-shim)

**Files:**
- Modify: `crates/vrm-godot-shim/src/bridge.rs`

- [ ] **Step 9.1: Identify the shim's method dispatch**

Run: `grep -n "method\|Unimplemented\|-32000" crates/vrm-godot-shim/src/bridge.rs | head -20`
Expected: shows the shim's JSON-RPC dispatch table or match arms.

- [ ] **Step 9.2: Add VRMA ops to the Unimplemented branch**

Edit `crates/vrm-godot-shim/src/bridge.rs`. Locate the dispatch (likely a `match method.as_str() { ... }`). Add a branch for the 5 VRMA op names that returns:

```rust
"load_vrma" | "apply_vrma_at_time" | "dump_humanoid_pose"
    | "dump_expression_weights" | "dump_look_at_state" => {
    Ok(JsonRpcError::unimplemented_with_phase("vrma-v1"))
}
```

If `unimplemented_with_phase` doesn't exist on the existing error helper, follow whatever helper the shim already uses for `-32000` + `phase` envelope.

- [ ] **Step 9.3: Build the shim**

Run: `cargo build -p vrm-godot-shim`
Expected: build succeeds.

- [ ] **Step 9.4: Add a shim-level test**

Append to `crates/vrm-godot-shim/src/bridge.rs` (or a sibling tests file the crate uses):

```rust
#[test]
fn vrma_ops_return_unimplemented_with_phase_vrma_v1() {
    let methods = [
        "load_vrma",
        "apply_vrma_at_time",
        "dump_humanoid_pose",
        "dump_expression_weights",
        "dump_look_at_state",
    ];
    for method in methods {
        let req = format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"{method}","params":{{}}}}"#
        );
        let resp = dispatch(&req).unwrap();
        let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(v["error"]["code"], -32000, "method={method}");
        assert_eq!(v["error"]["data"]["phase"], "vrma-v1", "method={method}");
    }
}
```

Adjust the `dispatch` call to match the actual entry point in `bridge.rs`.

- [ ] **Step 9.5: Run the test**

Run: `cargo test -p vrm-godot-shim vrma_ops_return_unimplemented`
Expected: PASS.

- [ ] **Step 9.6: Commit**

```bash
git add crates/vrm-godot-shim/src/bridge.rs
git commit -m "$(cat <<'EOF'
feat(vrm-godot-shim): add VRMA op stubs returning Unimplemented

All 5 VRMA ops return -32000 with phase: "vrma-v1" at the shim level
without round-tripping through Godot. Upstream V-Sekai/godot-vrm support
remains an open follow-up — godot-vrm's VRMC_vrm_animation.gd is a stub.
EOF
)"
```

---

## Task 10: UniVRM adapter Unimplemented stubs

**Files:**
- Modify: `adapters/univrm/UniVRMConformance/Assets/Conformance/Conformance.cs`

These are temporary stubs. Phase 4 of the VRMA work replaces them with real implementations against `VrmAnimationImporter` + `Vrm10AnimationInstance`. For now we ensure UniVRM's batch dispatcher reports Unimplemented for the 5 op names, mirroring the other adapters.

- [ ] **Step 10.1: Identify the existing op dispatcher**

Run: `grep -n "method\|case \|Unimplemented\|-32000\|RunBatch" adapters/univrm/UniVRMConformance/Assets/Conformance/Conformance.cs | head -30`
Expected: shows the dispatch — UniVRM batches via filesystem (RFC-0003); each test case method-dispatches inside the batch.

- [ ] **Step 10.2: Add the 5 VRMA op names to the Unimplemented branch**

In `Conformance.cs`, find the method-name dispatch (likely a switch statement or method-name lookup in the batch loop). Add a branch:

```csharp
case "load_vrma":
case "apply_vrma_at_time":
case "dump_humanoid_pose":
case "dump_expression_weights":
case "dump_look_at_state":
    return UnimplementedError("vrma-v1");
```

If `UnimplementedError` doesn't exist, follow whatever helper Conformance.cs already uses for `-32000` + `phase` envelopes. Some adapters' Conformance.cs has a `MakeError(code, message, phase)` helper.

- [ ] **Step 10.3: Build via Unity batch (or skip if Unity isn't available locally)**

Run: `adapters/univrm/launcher.sh --validate-only` (if this flag exists; otherwise just `swift build` equivalent for Unity)
Expected: compilation OK. If Unity isn't installed locally, CI build-validate step covers this — note the omission in the commit.

- [ ] **Step 10.4: Commit**

```bash
git add adapters/univrm/UniVRMConformance/Assets/Conformance/Conformance.cs
git commit -m "$(cat <<'EOF'
feat(adapters/univrm): add VRMA op stubs returning Unimplemented

Temporary stubs for the 5 VRMA ops. Phase 4 of the VRMA plan replaces
these with real implementations bound to UniVRM's VrmAnimationImporter
and Vrm10AnimationInstance. Until then, all 5 return -32000 with
phase: "vrma-v1".
EOF
)"
```

---

## Task 11: Integration sanity test — describe catalog includes the 5 new ops

**Files:**
- Create or extend: `adapters/three-vrm/test/describe.test.ts` (or wherever describe is tested)

- [ ] **Step 11.1: Add a describe-catalog assertion**

Find the existing `describe` test (run `grep -rn "describe" adapters/three-vrm/test/`). Extend it (or add a new test) so it asserts the 5 new ops appear in the catalog:

```typescript
it("describe catalog exposes 5 VRMA ops", async () => {
  const catalog = await runCli(["describe", "--format", "json"]);
  const parsed = JSON.parse(catalog);
  const methods = new Set(parsed.methods?.map((m: any) => m.name) ?? []);
  for (const op of ["load_vrma", "apply_vrma_at_time", "dump_humanoid_pose", "dump_expression_weights", "dump_look_at_state"]) {
    assert(methods.has(op), `describe catalog missing ${op}`);
  }
});
```

If `describe` is wired differently in this adapter, mirror its existing shape — the assertion is "method-name list from `describe --format json` includes all 5 VRMA op names."

- [ ] **Step 11.2: Run the test**

Run: `cd adapters/three-vrm && npm test`
Expected: new describe test passes alongside existing tests.

- [ ] **Step 11.3: Commit**

```bash
git add adapters/three-vrm/test/
git commit -m "$(cat <<'EOF'
test(adapters/three-vrm): verify describe catalog exposes 5 VRMA ops

Asserts load_vrma, apply_vrma_at_time, and the 3 dump ops appear in
'describe --format json' output. Catches future regressions where an op
gets dropped from the catalog.
EOF
)"
```

---

## Task 12: Workspace cleanup — fmt + clippy

**Files:**
- (none touched directly; cleanup pass)

- [ ] **Step 12.1: Run cargo fmt across workspace**

Run: `cargo fmt --all`
Expected: no changes (if everything was already formatted) OR formatter applies fixes.

- [ ] **Step 12.2: Run clippy with -D warnings**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: zero warnings, zero errors. If clippy flags anything in the new types (e.g. `needless_borrow`, `derivable_impls`), fix inline.

- [ ] **Step 12.3: Run the full test suite**

Run: `cargo test --workspace`
Expected: all tests pass, including the new serde round-trip tests for the 5 VRMA ops.

- [ ] **Step 12.4: Commit any fmt/clippy fixes (if needed)**

If fmt or clippy made changes:

```bash
git add -u
git commit -m "$(cat <<'EOF'
chore: cargo fmt + clippy clean-up after VRMA op surface

Phase 1 of VRMA conformance. Zero clippy warnings, zero fmt diffs
across the workspace.
EOF
)"
```

If no changes were needed, skip this commit.

---

## Phase 1 completion checklist

- [ ] All 5 VRMA op types in `crates/vrm-ops/src/tools.rs` with serde round-trip tests passing
- [ ] `docs/operation-contract.md` documents all 5 ops in a new VRMA section
- [ ] VMK adapter declares the 5 ops in its `reservedPhases` map (Unimplemented with `phase: "vrma-v1"`)
- [ ] three-vrm adapter dispatches the 5 ops to its Unimplemented branch
- [ ] godot-vrm adapter (via vrm-godot-shim) dispatches the 5 ops to Unimplemented
- [ ] UniVRM adapter dispatches the 5 ops to Unimplemented (temporary; phase 4 replaces with real)
- [ ] `describe --format json` output includes all 5 new method names
- [ ] `cargo fmt --check` clean
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [ ] `cargo test --workspace` green

After this phase, the op contract is published. Phase 2 (diff engine + test plan schema + manifest + runner integration) builds on this surface.

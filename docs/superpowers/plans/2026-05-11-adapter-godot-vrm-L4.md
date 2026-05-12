# godot-vrm Adapter L4 — Spring-Bone Physics Ops

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **Builds on:** [`2026-05-11-adapter-godot-vrm-L3.md`](./2026-05-11-adapter-godot-vrm-L3.md). L3 landed the six Phase 1 ops; spring-bone physics was deferred. L4 closes that gap so godot-vrm renders the **full 80-test corpus**, removing the last conformance-suite blocker for VMK 1.0 launch.

**Goal:** Implement `step_physics`, `reset_physics`, and `animate_root_transform` on godot-vrm so all 36 spring-bone tests (18 settle + 18 swing) render through this adapter, bringing it to 80/80 corpus coverage and enabling three-renderer consensus across the entire test suite.

**Architecture:** Append three methods to `adapters/godot-vrm/src/session.gd`. The control surface is V-Sekai/godot-vrm's `VRMSecondary` Node3D, which auto-steps via `_physics_process` at the Godot engine's fixed step. Take manual control:

1. After `load_vrm`, find the `VRMSecondary` child of the scene and set `process_mode = PROCESS_MODE_DISABLED`.
2. `step_physics(count, dt_seconds)` calls `vrm_secondary.do_process(dt_seconds)` `count` times explicitly.
3. `reset_physics(settle_steps)` calls `vrm_secondary._ready()` to re-initialize spring chains to rest pose, then runs `settle_steps` manual steps at `1/60` s.
4. `animate_root_transform(start, end, duration, fps)` linearly interpolates `scene.position` between samples, calling `do_process(1/fps)` per sample.

```
runner → vrm-godot-shim → godot child
                              ├─ session.load_vrm → finds VRMSecondary, disables auto-stepping
                              ├─ session.reset_physics(settle_steps) → vrm_secondary._ready() + manual stepping
                              ├─ session.animate_root_transform(...) → interpolate translation + step
                              ├─ session.step_physics(count, dt) → vrm_secondary.do_process(dt) × count
                              └─ session.render → SubViewport → PNG
```

**Tech Stack:** Same as L3 (GDScript on Godot 4.6.2, no new Rust changes).

---

## Pre-flight assumption to verify

V-Sekai/godot-vrm's `VRMSecondary` exposes a clean manual-step path:
- `VRMSecondary.do_process(delta)` does one fixed-step integration. (Confirmed via reading `addons/vrm/vrm_secondary.gd:344`.)
- `process_mode = PROCESS_MODE_DISABLED` stops both `_process` and `_physics_process` from auto-firing. (Standard Godot Node3D behavior.)
- `VRMSecondary._ready()` re-initializes the spring chains to rest pose. (Tick logic at line 304 of `vrm_secondary.gd` calls `_ready()` when `needs_reintialize` fires; we invoke the same path manually.)

Spike (Task 1) validates these assumptions on a real spring-bone VRM: load, find VRMSecondary, disable process_mode, observe initial joint position, run N manual steps, observe joint position changed.

---

## Spike 1 result

- Date: 2026-05-11
- Asset: `vrm-asset-generator emit-springbone --id l4_spike` (gravity_dir [0,-1,0], gravity_scale 0.5, drag 0.5, 4-joint vertical chain at y=1.26..1.11).
- Found VRMSecondary: node name `secondary`, class `Node3D`, script `addons/vrm/vrm_secondary.gd` — `_find_vrm_secondary` walks fine.
- Default process_mode: `0` (PROCESS_MODE_INHERIT).
- After setting `PROCESS_MODE_DISABLED`: process_mode reads `4`. Auto-stepping confirmed off.
- `spring_bones_internal[0].verlets` exists (3 verlets for a 4-joint chain), `current_tail` field present on each `VRMSpringBoneLogic`.
- **Claim 1 (manual step advances physics): CONFIRMED.** With the test asset's perfectly-axis-aligned chain, gravity-only stepping produces zero motion because the verlet length-constraint at `vrm_spring_bone_logic.gd:81` re-normalizes the force vector back onto the chain axis. Adding a non-axis perturbation `springbone_add_force = (1, 0, 0)` and running 30 steps at 1/60 s moves verlet[0] from `(0, 1.26, 0)` to `(0.0447, 1.2876, 0)` — delta magnitude `0.0526`. Without perturbation, delta is `0.0` (correct physics, not a control-surface bug). Real test plans excite chains via `animate_root_transform` translation, which produces the perturbation.
- **Claim 2 (process_mode disable stops auto-stepping): CONFIRMED.** Setting `process_mode = PROCESS_MODE_DISABLED` (4) before any process tick prevents `_process`/`_physics_process` from running. In Godot 4.6 the addon also installs a `SkeletonModifier3D` child that auto-pumps physics — `PROCESS_MODE_DISABLED` propagates to that child too (verified: bone overrides do not change between manual steps).
- **Claim 3 (`_ready()` resets to rest): CONFIRMED WITH CAVEAT.** Calling `_ready()` alone after the chain has been perturbed does NOT restore tail positions, because the addon writes pose overrides via `skel.set_bone_global_pose(...)` (in `vrm_spring_bone_logic.gd:99`) and `_ready()` re-initializes verlets from the current (overridden) skeleton pose. Full reset requires clearing pose state first:
  ```gdscript
  skel.clear_bones_global_pose_override()
  for i in range(skel.get_bone_count()):
      skel.set_bone_pose_rotation(i, skel.get_bone_rest(i).basis.get_rotation_quaternion())
      skel.set_bone_pose_position(i, skel.get_bone_rest(i).origin)
  secondary._ready()
  ```
  With this sequence, `current_tail` returns exactly to `(0, 1.26, 0)` — delta from initial: `0.0`.
- Outcome: control surface confirmed. `reset_physics` (Task 3) must include the pose-clear-and-rest preamble before `_ready()`, otherwise reset is a no-op after the first stepped frame. Plan body of Task 3 needs that update.

---

## File Structure

```
adapters/godot-vrm/src/session.gd        # APPEND 3 methods + 1 field (vrm_secondary)
adapters/godot-vrm/src/operations.gd     # MOVE 3 ops from PHASE_BY_RESERVED_METHOD to PHASE1_METHODS
crates/vrm-godot-shim/tests/contract.rs  # ADD a spring-bone physics test (#[ignore]'d)
adapters/godot-vrm/README.md             # Update: L4 shipped, remove deferral notes
README.md                                # Update godot-vrm row: full 80-test corpus
CLAUDE.md                                # Update adapter status: Phase 2 implemented
docs/findings.md                         # ADD "Seventh run" entry with 3-way spring-bone consensus
```

No new files; the existing Session + dispatch architecture absorbs all changes.

---

## Operation contract reminder

From `docs/operation-contract.md`:

| op | input | semantics |
|---|---|---|
| `step_physics` | `{ session_id, dt_seconds, count }` | Advance physics by `count` steps of `dt_seconds`. For the test corpus: always `dt_seconds = 1/60`. |
| `reset_physics` | `{ session_id, settle_steps }` | Reset all spring chains to rest pose, then advance `settle_steps` frames at 1/60 s. Default `settle_steps = 30`. |
| `animate_root_transform` | `{ session_id, translation_start[3], translation_end[3], duration_seconds, fps }` | Linearly interpolate scene root translation from start to end over duration, advancing physics by `1/fps` between samples. Translation-only in v0.1. |

Per `docs/methodology.md`: 60 Hz fixed step, 30 settle steps from rest pose before measurement, translation-only excitation.

---

## Task list

7 tasks. One gating spike (Task 1); the rest are implementation + verification.

---

### Task 1: Spike — manual spring-bone stepping

**Files:** `/tmp/godot-l4-springbone-spike.gd` (throwaway)

Validate the manual-step surface on a real spring-bone VRM.

- [ ] **Step 1: Generate a sample spring-bone VRM**

```bash
cargo build --release -p vrm-asset-generator 2>&1 | tail -3
./target/release/vrm-asset-generator emit-springbone --id l4_spike --output-dir /tmp/godot-l4-assets 2>&1 | tail -3
ls /tmp/godot-l4-assets/
```

Expected: `l4_spike.vrm`, `l4_spike.meta.json`, `l4_spike.test.yaml`.

- [ ] **Step 2: Write the spike**

```bash
cat > /tmp/godot-l4-springbone-spike.gd <<'GD'
extends SceneTree

const VrmRuntimeExtensions := preload("res://src/vrm_runtime_extensions.gd")

func _find_vrm_secondary(node: Node) -> Node:
    for child in node.get_children():
        if child.get_class() == "Node3D" and child.get_script() != null:
            var s = child.get_script()
            # VRMSecondary has `class_name VRMSecondary` — check the script's resource name.
            if s.resource_path.ends_with("vrm_secondary.gd"):
                return child
        var found := _find_vrm_secondary(child)
        if found != null:
            return found
    return null

func _first_bone_tail_pos(secondary: Node) -> Vector3:
    var spring_bones_internal: Array = secondary.get("spring_bones_internal")
    if spring_bones_internal == null or spring_bones_internal.is_empty():
        return Vector3.INF
    var first_chain = spring_bones_internal[0]
    var verlets: Array = first_chain.get("verlets")
    if verlets == null or verlets.is_empty():
        return Vector3.INF
    return verlets[0].current_tail

func _init() -> void:
    var args := OS.get_cmdline_user_args()
    if args.is_empty():
        push_error("need vrm path"); quit(2); return

    var gltf := GLTFDocument.new()
    var registered := VrmRuntimeExtensions.register_all()
    var state := GLTFState.new()
    state.set_additional_data(&"vrm/head_hiding_method", 0)
    state.handle_binary_image = GLTFState.HANDLE_BINARY_EMBED_AS_UNCOMPRESSED
    var err := gltf.append_from_file(args[0], state, 0)
    VrmRuntimeExtensions.unregister_all(registered)
    if err != OK:
        push_error("load err %d" % err); quit(2); return
    var scene: Node = gltf.generate_scene(state)
    root.add_child(scene)

    var secondary := _find_vrm_secondary(scene)
    if secondary == null:
        push_error("VRMSecondary node not found"); quit(2); return
    print("found VRMSecondary: %s" % secondary.name)
    print("default process_mode: %d" % secondary.process_mode)

    # Disable auto-stepping. PROCESS_MODE_DISABLED = 4.
    secondary.process_mode = Node.PROCESS_MODE_DISABLED
    print("disabled process_mode: %d" % secondary.process_mode)

    # Let the scene settle into _ready() and one tick.
    await process_frame
    await process_frame

    var initial_tail := _first_bone_tail_pos(secondary)
    print("initial first-chain tail pos: %s" % initial_tail)
    if initial_tail == Vector3.INF:
        push_error("could not read first verlet tail"); quit(2); return

    # Manual step 30 times at 1/60 s. The chain should settle under gravity.
    for i in 30:
        secondary.do_process(1.0/60.0)

    var settled_tail := _first_bone_tail_pos(secondary)
    print("settled first-chain tail pos (30 steps): %s" % settled_tail)
    print("delta magnitude: %s" % (settled_tail - initial_tail).length())

    if settled_tail.is_equal_approx(initial_tail):
        push_error("manual stepping had no effect — control surface broken"); quit(2); return

    # Verify reset returns to rest.
    secondary._ready()
    await process_frame  # let any reinitialize run
    var reset_tail := _first_bone_tail_pos(secondary)
    print("post-reset first-chain tail pos: %s" % reset_tail)

    quit(0)
GD
```

- [ ] **Step 3: Run the spike**

```bash
godot --display-driver macos --rendering-driver metal --audio-driver Dummy \
  --path adapters/godot-vrm \
  --script /tmp/godot-l4-springbone-spike.gd \
  -- /tmp/godot-l4-assets/l4_spike.vrm 2>&1 | grep -E "found|process_mode|tail pos|delta|ERROR" | head -15
```

Expected:
- `found VRMSecondary: <name>` — node located.
- `default process_mode: 0` (PROCESS_MODE_INHERIT).
- `disabled process_mode: 4` (PROCESS_MODE_DISABLED).
- `initial first-chain tail pos: (X, Y, Z)` — some baseline.
- `settled first-chain tail pos (30 steps): (X', Y', Z')` — different from initial (chain has fallen under gravity).
- `delta magnitude: 0.0NN` — small but non-zero.
- `post-reset first-chain tail pos: (X'', Y'', Z'')` — close to initial.

If `delta magnitude` is 0 or NaN, the manual stepping isn't actually advancing physics — investigate before continuing.

If `_find_vrm_secondary` returns null, the addon hierarchy may have shifted between versions; fallback: walk scene with `print_tree_pretty()` and find the secondary node manually.

- [ ] **Step 4: Record spike result + commit**

Append to `docs/superpowers/plans/2026-05-11-adapter-godot-vrm-L4.md`:

```markdown
## Spike 1 result

- Date: 2026-05-11
- Initial first-chain tail: (X, Y, Z)
- After 30 manual steps at 1/60 s: (X', Y', Z')
- Delta magnitude: 0.0NN
- Post-reset tail: matches initial within ±epsilon
- Outcome: manual VRMSecondary.do_process stepping confirmed; reset via _ready() works.
```

```bash
git add docs/superpowers/plans/2026-05-11-adapter-godot-vrm-L4.md
git commit -m "docs(plan/godot-vrm-L4): record spring-bone manual-step spike result"
```

---

### Task 2: Session — find VRMSecondary + add field

**Files:** `adapters/godot-vrm/src/session.gd`

Extend Session to locate and own the VRMSecondary node after load.

- [ ] **Step 1: Add field + helper**

After the existing state fields in `session.gd` (around `var environment: Environment = null`), add:

```gdscript
var vrm_secondary: Node = null
```

After the load_vrm `_ok({ "session_id": session_id })` line BUT before the function ends — actually rework `load_vrm` to find the secondary node after `viewport.add_child(scene)`. Insert this code block just before `return _ok({ "session_id": session_id })`:

```gdscript
    vrm_secondary = _find_vrm_secondary(scene)
    if vrm_secondary != null:
        # Disable auto-stepping so step_physics/reset_physics/animate_root_transform
        # have full control over the physics pump.
        vrm_secondary.process_mode = Node.PROCESS_MODE_DISABLED
```

Then add this helper at the end of the file (before `_ok`/`_err`):

```gdscript
static func _find_vrm_secondary(node: Node) -> Node:
    for child in node.get_children():
        var script = child.get_script()
        if script != null and script.resource_path.ends_with("vrm_secondary.gd"):
            return child
        var found := _find_vrm_secondary(child)
        if found != null:
            return found
    return null
```

In `dispose`, add `vrm_secondary = null` to the nil-everything block.

- [ ] **Step 2: Parse + existing tests**

```bash
godot --headless --path adapters/godot-vrm --quit-after 1 2>&1 | tail -3
godot --headless --path adapters/godot-vrm --script tests/run_gdscript_tests.gd 2>&1 | tail -3
```

Expected: parse clean; 7/0.

- [ ] **Step 3: Smoke — confirm secondary node is found**

```bash
cat > /tmp/godot-l4-find-smoke.gd <<'GD'
extends SceneTree
const Session := preload("res://src/session.gd")
func _init() -> void:
    var args := OS.get_cmdline_user_args()
    var s := Session.new()
    var r: Dictionary = s.load_vrm(root, { "path": args[0] })
    if not r.get("ok"): push_error("load failed"); quit(2); return
    print("vrm_secondary: %s" % (s.vrm_secondary.name if s.vrm_secondary else "null"))
    print("process_mode: %d" % (s.vrm_secondary.process_mode if s.vrm_secondary else -1))
    s.dispose({})
    quit(0)
GD

godot --display-driver macos --rendering-driver metal --audio-driver Dummy \
  --path adapters/godot-vrm --script /tmp/godot-l4-find-smoke.gd \
  -- /tmp/godot-l4-assets/l4_spike.vrm 2>&1 | grep -E "vrm_secondary|process_mode|ERROR" | head -3
```

Expected: `vrm_secondary: <some-name>`, `process_mode: 4`.

- [ ] **Step 4: Commit**

```bash
git add adapters/godot-vrm/src/session.gd
git commit -m "feat(adapters/godot-vrm): locate VRMSecondary + disable auto-stepping"
```

---

### Task 3: Session — `step_physics` + `reset_physics`

**Files:** `adapters/godot-vrm/src/session.gd`

- [ ] **Step 1: Append the two methods**

Insert before `_ok`/`_err` (and after the helpers from prior tasks). The methods are sync (no async needed — they don't render):

```gdscript
func step_physics(params: Dictionary) -> Dictionary:
    if vrm_secondary == null:
        # Adapter loaded a VRM without spring bones — treat as no-op for protocol
        # compliance (cf. vrm-mock-renderer's no-op behavior).
        return _ok({})
    var dt_seconds: float = params.get("dt_seconds", 1.0/60.0)
    var count: int = params.get("count", 1)
    for i in count:
        vrm_secondary.do_process(dt_seconds)
    return _ok({})

func reset_physics(params: Dictionary) -> Dictionary:
    if vrm_secondary == null:
        return _ok({})
    var settle_steps: int = params.get("settle_steps", 30)
    # Re-initialize chains to rest pose. _ready() rebuilds spring_bones_internal
    # + verlets from the export'd spring_bones array.
    vrm_secondary._ready()
    for i in settle_steps:
        vrm_secondary.do_process(1.0/60.0)
    return _ok({})
```

- [ ] **Step 2: Parse + existing tests**

```bash
godot --headless --path adapters/godot-vrm --quit-after 1 2>&1 | tail -3
godot --headless --path adapters/godot-vrm --script tests/run_gdscript_tests.gd 2>&1 | tail -3
```

Expected: parse clean, 7/0.

- [ ] **Step 3: Smoke — load, step, reset**

```bash
cat > /tmp/godot-l4-step-smoke.gd <<'GD'
extends SceneTree
const Session := preload("res://src/session.gd")
func _init() -> void:
    var args := OS.get_cmdline_user_args()
    var s := Session.new()
    var r: Dictionary = s.load_vrm(root, { "path": args[0] })
    if not r.get("ok"): push_error("load failed"); quit(2); return

    # Wait for initial _ready ticks
    await process_frame
    await process_frame

    var initial = s.vrm_secondary.get("spring_bones_internal")[0].get("verlets")[0].current_tail
    print("initial tail: %s" % initial)

    s.step_physics({ "dt_seconds": 1.0/60.0, "count": 30 })
    var stepped = s.vrm_secondary.get("spring_bones_internal")[0].get("verlets")[0].current_tail
    print("after 30 manual steps: %s" % stepped)
    print("delta: %s" % (stepped - initial).length())

    s.reset_physics({ "settle_steps": 0 })
    await process_frame
    var reset_pos = s.vrm_secondary.get("spring_bones_internal")[0].get("verlets")[0].current_tail
    print("after reset(0): %s" % reset_pos)

    s.dispose({})
    quit(0)
GD

godot --display-driver macos --rendering-driver metal --audio-driver Dummy \
  --path adapters/godot-vrm --script /tmp/godot-l4-step-smoke.gd \
  -- /tmp/godot-l4-assets/l4_spike.vrm 2>&1 | grep -E "tail|delta|ERROR" | head -5
```

Expected: initial and `after 30 steps` differ measurably; `after reset(0)` matches initial closely.

- [ ] **Step 4: Commit**

```bash
git add adapters/godot-vrm/src/session.gd
git commit -m "feat(adapters/godot-vrm): step_physics + reset_physics ops"
```

---

### Task 4: Session — `animate_root_transform`

**Files:** `adapters/godot-vrm/src/session.gd`

- [ ] **Step 1: Append the method**

Insert before `_ok`/`_err`:

```gdscript
func animate_root_transform(params: Dictionary) -> Dictionary:
    if scene == null:
        return _err(-32002, "RenderFailed", { "reason": "no session active; call load_vrm first" })
    var start_arr = params.get("translation_start", [0.0, 0.0, 0.0])
    var end_arr = params.get("translation_end", [0.0, 0.0, 0.0])
    var duration: float = params.get("duration_seconds", 0.25)
    var fps: int = params.get("fps", 60)
    var start := Vector3(start_arr[0], start_arr[1], start_arr[2])
    var end := Vector3(end_arr[0], end_arr[1], end_arr[2])

    var dt := 1.0 / float(fps)
    var sample_count := int(round(duration * float(fps)))
    if sample_count < 1:
        sample_count = 1

    if scene is Node3D:
        (scene as Node3D).position = start
    for i in range(sample_count):
        var t: float = float(i + 1) / float(sample_count)
        if scene is Node3D:
            (scene as Node3D).position = start.lerp(end, t)
        if vrm_secondary != null:
            vrm_secondary.do_process(dt)
    return _ok({})
```

- [ ] **Step 2: Parse + existing tests**

```bash
godot --headless --path adapters/godot-vrm --quit-after 1 2>&1 | tail -3
godot --headless --path adapters/godot-vrm --script tests/run_gdscript_tests.gd 2>&1 | tail -3
```

Expected: parse clean, 7/0.

- [ ] **Step 3: Commit**

```bash
git add adapters/godot-vrm/src/session.gd
git commit -m "feat(adapters/godot-vrm): animate_root_transform op"
```

---

### Task 5: Promote ops to Phase 1 in dispatch

**Files:** `adapters/godot-vrm/src/operations.gd`, `adapters/godot-vrm/tests/test_operations.gd`

- [ ] **Step 1: Move 3 ops from reserved to Phase 1 in operations.gd**

Edit `adapters/godot-vrm/src/operations.gd`:

In `PHASE_BY_RESERVED_METHOD`, remove the entries for `animate_root_transform`, `step_physics`, `reset_physics`. The reserved table should keep only:
- `set_environment` → `v1.x`
- `set_expression` → `Phase 3`
- `set_humanoid_pose` → `Phase 2`
- `set_root_transform` → `Phase 2`

In `PHASE1_METHODS`, add `"step_physics"`, `"reset_physics"`, `"animate_root_transform"`. The new array:

```gdscript
const PHASE1_METHODS := [
    "load_vrm", "set_camera", "set_lighting",
    "set_post_processing", "render", "dispose",
    "step_physics", "reset_physics", "animate_root_transform",
]
```

In the `match method:` block, add three new arms after the existing `"dispose":` arm:

```gdscript
            "step_physics":
                outcome = session.step_physics(params if typeof(params) == TYPE_DICTIONARY else {})
            "reset_physics":
                outcome = session.reset_physics(params if typeof(params) == TYPE_DICTIONARY else {})
            "animate_root_transform":
                outcome = session.animate_root_transform(params if typeof(params) == TYPE_DICTIONARY else {})
```

- [ ] **Step 2: Update dispatch unit tests**

Edit `adapters/godot-vrm/tests/test_operations.gd`. The current 7 tests include `test_set_humanoid_pose_returns_phase_2`, etc. After this change:

- `set_humanoid_pose` should still return Phase 2 (still reserved).
- `set_root_transform` should still return Phase 2 (still reserved).
- `animate_root_transform`, `step_physics`, `reset_physics` are NO LONGER reserved — they route through Session.

Two tests need updating: `test_set_root_transform_returns_phase_2` should stay (it's still reserved), but if there's a `test_animate_root_transform_returns_phase_2` it must go (now goes to Session, which would NPE without a real load).

Read the current test file:

```bash
grep -n "^func test_" adapters/godot-vrm/tests/test_operations.gd
```

If any test asserts these 3 ops return Phase 2:
- Remove or rewrite those tests so they don't invoke Phase 1 routes without a session.
- Keep `set_humanoid_pose` / `set_root_transform` Phase 2 tests.

To keep the test count at 7 (or adjust the runner's expected count), replace any removed tests with new ones that exercise the reserved-table integrity:
- `test_set_root_transform_returns_phase_2` (if not already present).
- `test_step_physics_routed_to_phase1` — verify by checking `Operations.PHASE1_METHODS.has("step_physics")` returns true. Doesn't dispatch.

Actually simpler: write a single new test that asserts the 9-method PHASE1 list contents:

```gdscript
func test_phase1_methods_include_physics_ops() -> void:
    const Operations := preload("res://src/operations.gd")
    _assert_eq(Operations.PHASE1_METHODS.has("step_physics"), true, "step_physics in PHASE1")
    _assert_eq(Operations.PHASE1_METHODS.has("reset_physics"), true, "reset_physics in PHASE1")
    _assert_eq(Operations.PHASE1_METHODS.has("animate_root_transform"), true, "animate_root_transform in PHASE1")
```

Adjust the existing tests so the total still passes. Aim for 7-9 passing tests.

- [ ] **Step 3: Verify tests pass**

```bash
godot --headless --path adapters/godot-vrm --script tests/run_gdscript_tests.gd 2>&1 | tail -5
```

Expected: `N passed, 0 failed` where N is the new test count.

- [ ] **Step 4: Commit**

```bash
git add adapters/godot-vrm/src/operations.gd adapters/godot-vrm/tests/test_operations.gd
git commit -m "feat(adapters/godot-vrm): promote physics ops to Phase 1 dispatch"
```

---

### Task 6: Rust contract test — physics ops round-trip

**Files:** `crates/vrm-godot-shim/tests/contract.rs`

- [ ] **Step 1: Add a new `#[ignore]`'d test**

Append after the existing `reserved_ops_still_return_unimplemented` (and before `malformed_json_returns_parse_error_with_null_id`):

```rust
#[test]
#[ignore]
fn springbone_physics_ops_render_a_real_vrm() {
    let project_dir = workspace_root().join("adapters").join("godot-vrm");
    let tmp = tempfile::tempdir().expect("tempdir");
    let asset_dir = tmp.path();
    let status = std::process::Command::new("cargo")
        .arg("run").arg("--release").arg("-q")
        .arg("-p").arg("vrm-asset-generator").arg("--")
        .arg("emit-springbone")
        .arg("--id").arg("contract_l4")
        .arg("--output-dir").arg(asset_dir)
        .current_dir(workspace_root())
        .status().expect("emit-springbone");
    assert!(status.success(), "emit-springbone failed");
    let vrm_path = asset_dir.join("contract_l4.vrm");
    let out_png = asset_dir.join("contract_l4.png");

    let mut child = Command::new(shim_binary())
        .env("GODOT_VRM_ADAPTER_DIR", &project_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn shim");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();

    let calls: Vec<(i64, &str, serde_json::Value)> = vec![
        (1, "load_vrm", serde_json::json!({"path": vrm_path.to_string_lossy()})),
        (2, "set_camera", serde_json::json!({"position":[0,1.4,1.5],"target":[0,1.4,0],"up":[0,1,0],"fov_degrees":30})),
        (3, "set_lighting", serde_json::json!({"directional":{"dir":[-0.3,-0.6,-0.7],"color":[1,1,1],"intensity":1},"ambient":{"color":[0.5,0.5,0.5],"intensity":0.3},"cast_shadows":false,"receive_shadows":false})),
        (4, "set_post_processing", serde_json::json!({"tone_mapping":"None","exposure":1.0})),
        (5, "reset_physics", serde_json::json!({"settle_steps":30})),
        (6, "animate_root_transform", serde_json::json!({"translation_start":[0,0,0],"translation_end":[0.15,0,0],"duration_seconds":0.25,"fps":60})),
        (7, "step_physics", serde_json::json!({"dt_seconds":1.0/60.0,"count":5})),
        (8, "render", serde_json::json!({"width":1024,"height":1024,"output_path":out_png.to_string_lossy(),"color_space":"Srgb","msaa":4,"output_type":"Color"})),
        (9, "dispose", serde_json::json!({})),
    ];
    for (id, method, params) in &calls {
        let req = serde_json::json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}).to_string();
        stdin.write_all(&frame(req.as_bytes())).unwrap();
        stdin.flush().unwrap();
        let body = read_framed(&mut stdout);
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(parsed["error"].is_null(), "{method} failed: {parsed}");
        let resp_id = parsed["id"].as_i64().expect("integer id");
        assert_eq!(resp_id, *id, "id mismatch for {method}");
    }
    drop(stdin);
    assert!(child.wait().unwrap().success());

    let png = std::fs::read(&out_png).expect("read PNG");
    assert!(png.len() > 5000, "PNG too small: {} bytes", png.len());
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "bad PNG magic");
    let width = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
    let height = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
    assert_eq!((width, height), (1024, 1024));
}
```

The `reserved_ops_still_return_unimplemented` test's case list also needs updating — remove any cases asserting these 3 ops return Phase 2. The remaining reserved cases:

- Unknown method → -32601
- `set_humanoid_pose` → Phase 2
- `set_environment` → v1.x
- `set_expression` → Phase 3

Adjust the `cases` vec in `reserved_ops_still_return_unimplemented` accordingly (drop any of the three physics ops if they're listed).

- [ ] **Step 2: Run contract tests**

```bash
cargo test -p vrm-godot-shim --test contract -- --ignored --nocapture 2>&1 | tail -10
```

Expected: **4 passed** (`phase1_ops_render_a_real_vrm`, `reserved_ops_still_return_unimplemented`, `springbone_physics_ops_render_a_real_vrm`, `malformed_json_returns_parse_error_with_null_id`).

The springbone test takes a bit longer (~3s — physics steps + render).

- [ ] **Step 3: cargo test --workspace stays clean**

```bash
cargo test --workspace 2>&1 | tail -5
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3
```

Expected: all non-ignored tests pass; clippy clean.

- [ ] **Step 4: Commit**

```bash
git add crates/vrm-godot-shim/tests/contract.rs
git commit -m "test(vrm-godot-shim): assert spring-bone physics ops round-trip (L4)"
```

---

### Task 7: Full corpus rerun + docs + findings "Seventh run"

**Files:** `scripts/bootstrap-goldens.sh` (no change expected), `adapters/godot-vrm/README.md`, `README.md`, `CLAUDE.md`, `docs/findings.md`.

- [ ] **Step 1: Clean rerun**

```bash
rm -rf goldens-cache/_assets goldens-cache/_assets_swing
rm -rf goldens-cache/godot-vrm goldens-cache/three-vrm goldens-cache/vrm-metal-kit
rm -f goldens-cache/local-manifest.json goldens-cache/consensus-report.json
./scripts/bootstrap-goldens.sh 2>&1 | tee /tmp/bootstrap-l4-full.log | tail -10
./scripts/consensus-report.sh 2>&1 | tee /tmp/consensus-l4.log | tail -60
```

Expected: **godot-vrm: 80 succeeded, 0 failed** (or thereabouts — all 80 tests render now). The two prior renderers stay at 80/80. Consensus report shows 3 pairs all at `n=80`.

If godot-vrm has failures, capture the failure plans — likely a `reset_physics`/`animate_root_transform` edge case in specific test plans.

- [ ] **Step 2: Update `adapters/godot-vrm/README.md`**

In the status table:
- L3 row stays as "shipped".
- Change the L4 row from "deferred — spring-bone settle/swing tests skip godot-vrm" to **"shipped"**.

In the phase-label table:
- Remove the rows for `step_physics`, `reset_physics`, `animate_root_transform` (no longer reserved).
- Keep only: `set_humanoid_pose` (Phase 2), `set_root_transform` (Phase 2), `set_environment` (v1.x), `set_expression` (Phase 3), unknown (-32601).

- [ ] **Step 3: Update `README.md` (root)**

Change the godot-vrm adapter row to:

```markdown
| `adapters/godot-vrm/` | Godot 4 / GDScript adapter for [V-Sekai/godot-vrm](https://github.com/V-Sekai/godot-vrm). Pairs with `crates/vrm-godot-shim/` (Rust) for stdio framing. **L4 — Phase 1 + spring-bone physics live**. Third real renderer for the full 80-test corpus. |
```

- [ ] **Step 4: Update `CLAUDE.md`**

Change the godot-vrm adapter-status bullet to:

```markdown
- `adapters/godot-vrm/` — Godot 4 / GDScript paired with the `crates/vrm-godot-shim/` Rust shim. L4 (Phase 1 + spring-bone physics real). Full 80-test corpus renders end-to-end. Runner consumes `target/release/vrm-godot-shim` as `--adapter-bin`. Requires Godot 4.x on `PATH`.
```

- [ ] **Step 5: Add `## Seventh run` to `docs/findings.md`**

Append after the existing `## Sixth run` section. Include:
- Date + commit SHA.
- Trigger: "godot-vrm L4 shipped — spring-bone physics ops live; full 80-test corpus through three real adapters."
- Method: bootstrap + consensus across all three adapters on 80 test_ids.
- Headline: corpus_passed counts, mean SSIM across all three pairs at n=80.
- Top divergent test_ids (top 15 from consensus report).
- Observations: how did the spring-bone tests pair up? Is godot-vrm closer to three-vrm or vrm-metal-kit on the swing variants? Any single-renderer outliers surfaced now that all 80 tests have 3-way coverage?
- Open follow-ups: any spring-bone-specific divergence findings.

Use real numbers from `/tmp/consensus-l4.log`.

- [ ] **Step 6: Commit**

```bash
git add adapters/godot-vrm/README.md README.md CLAUDE.md docs/findings.md
git commit -m "docs: godot-vrm L4 shipped + record seventh corpus run"
```

---

## Out of scope (deferred to v1.x or beyond)

- **Rotation excitation** for `animate_root_transform` — v0.1 is translation-only per `docs/operation-contract.md:177`.
- **`set_humanoid_pose` / `set_root_transform`** — Phase 2 reserved ops that pose joints directly. godot-vrm has the API (it's a Skeleton3D), but no test plan in the current corpus exercises them. Defer until a plan uses them.
- **Linux CI driver spike** — still pending from L3.

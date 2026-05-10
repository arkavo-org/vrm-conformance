# Phase 2D-b — Spring-Bone Renderer Support

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire `step_physics` and `reset_physics` from the operation contract into the two real renderer adapters (vrm-mock-renderer + three-vrm), and extend the test plan + runner to optionally drive a gravity-settling pass before rendering. After this plan, spring-bone-bearing assets emitted by 2D-a's `emit-springbone` CLI render through both adapters with the settle pass actually exercising three-vrm's internal spring-bone manager.

**Architecture:** Three coordinated changes: (1) `vrm-test-plan` gains an optional `physics: PhysicsConfig` block (`settle_steps: u32`, defaulting to 30 per `docs/methodology.md`). (2) Adapters honor the reserved ops: mock treats them as no-ops (deterministic mock has no physics to step), three-vrm calls `vrm.update(1/60)` `count` times for `step_physics` and reloads the .vrm before stepping for `reset_physics`. (3) `vrm-runner` inserts a `reset_physics(settle_steps)` call between `set_post_processing` and `render` when the plan has a physics block. `animate_root_transform` excitation is deferred to 2D-c.

**Tech Stack:** Existing workspace (Rust + TypeScript). No new external dependencies.

**Why scope-bound:**
- 2D-a put spring-bone-bearing assets in the corpus; 2D-b makes them actually render through the spring-bone pipeline.
- Gravity-settling-only (no excitation) is intentionally limited: it verifies the renderer accepts the physics ops and produces a stable PNG. Whether the springs *look right* under stiffness/drag sweeps is a 2D-c concern that needs excitation to be meaningful.
- The mock renderer's no-op physics is correct: the deterministic synthesis doesn't model anything that physics could change. The shape-encoding fingerprint stays unchanged whether `step_physics` was called or not.
- Three-vrm's spring-bone manager runs inside `vrm.update(dt)` and is already in the renderer; we just need to call update repeatedly during settle, then once at render time (which already happens in 2C-b).

**YAGNI scope guards:**
- ✅ No `animate_root_transform` / `set_root_transform` / `set_humanoid_pose` — they stay Unimplemented (deferred to 2D-c).
- ✅ No collider scenarios.
- ✅ Mock physics is no-op; we are NOT teaching the mock to fake spring-bone visual fingerprinting.
- ✅ No new test plans committed under `test-plans/manual/` — emit-springbone's default sidecar gets a `physics:` block but otherwise stays the same shape.
- ✅ Per-joint params (stiffness/drag/gravity) sweep stays one-asset-per-variant via 2D-a's existing CLI; no new sweep matrix CLI in this round.

---

## File Layout

| File | Status | Responsibility |
|---|---|---|
| `crates/vrm-test-plan/src/lib.rs` | Modify | Add `pub struct PhysicsConfig { settle_steps: u32 }` + optional field `pub physics: Option<PhysicsConfig>` on `TestPlan`. Default-omitted preserves existing YAML wire shape. |
| `crates/vrm-test-plan/tests/roundtrip.rs` | Modify | New round-trip test for a YAML doc that includes `physics: { settle_steps: 30 }`. |
| `crates/vrm-asset-generator/src/sidecar.rs` | Modify | When emitting a spring-bone sidecar, populate `physics.settle_steps = 30` so the runner has the signal it needs. |
| `crates/vrm-mock-renderer/src/main.rs` | Modify | Move `step_physics` and `reset_physics` out of the reserved-Unimplemented list; route them to no-op handlers. |
| `crates/vrm-mock-renderer/src/handlers.rs` | Modify | Add `step_physics` and `reset_physics` handlers (no-op, return `UnitResult`). |
| `crates/vrm-mock-renderer/tests/contract.rs` | Modify | New test: step_physics and reset_physics return ok results, not -32000. |
| `adapters/three-vrm/src/renderer-host.html` | Modify | Add `window.__stepPhysics(params)` and `window.__resetPhysics(params)`. step calls `vrm.update(1/60)` count times; reset reloads from the last-loaded URL and runs settle_steps updates. |
| `adapters/three-vrm/src/browser-session.ts` | Modify | Add `stepPhysics` and `resetPhysics` methods bridging into `page.evaluate`. |
| `adapters/three-vrm/src/operations.ts` | Modify | Replace step_physics/reset_physics reserved entries with real handlers; reserved table loses those two keys. |
| `adapters/three-vrm/test/contract.test.ts` | Modify | Update assertions: step_physics and reset_physics no longer return -32000; they return result envelopes. |
| `crates/vrm-runner/src/execute.rs` | Modify | When `plan.physics` is `Some`, call `reset_physics({ settle_steps })` after `set_post_processing` and before `render`. |
| `crates/vrm-ops/src/tools.rs` | Modify | Add `StepPhysicsParams { session_id, dt_seconds: f32, count: u32 }` and `ResetPhysicsParams { session_id, settle_steps: u32 }`. |
| `docs/operation-contract.md` | Modify | Move `step_physics` and `reset_physics` out of "Reserved" into the Phase 1 required-but-renderer-specific section, with updated JSON shapes. |

---

## Section A — Test plan schema

### Task A1: Add `PhysicsConfig` to TestPlan (TDD)

**Files:**
- Modify: `crates/vrm-test-plan/src/lib.rs`
- Modify: `crates/vrm-test-plan/tests/roundtrip.rs`

- [ ] **Step 1: Failing test**

Append to `crates/vrm-test-plan/tests/roundtrip.rs`:

```rust
#[test]
fn parses_plan_with_physics_block() {
    let yaml = r#"
id: physics_test
spec_section: VRMC_springBone
asset: physics.vrm
camera:
  position: [0.0, 1.4, 1.5]
  target: [0.0, 1.4, 0.0]
  up: [0.0, 1.0, 0.0]
  fov_degrees: 30.0
lighting:
  directional:
    dir: [0.0, -1.0, 0.0]
    color: [1.0, 1.0, 1.0]
    intensity: 1.0
  ambient:
    color: [0.5, 0.5, 0.5]
    intensity: 0.3
  cast_shadows: false
  receive_shadows: false
output:
  width: 256
  height: 256
  color_space: linear
  msaa: 4
diff:
  mode: ssim
  threshold: 0.985
  reference_renderer: vrm-metal-kit
physics:
  settle_steps: 30
"#;

    let plan: vrm_test_plan::TestPlan = serde_yml::from_str(yaml).unwrap();
    let physics = plan.physics.as_ref().expect("physics block should be parsed");
    assert_eq!(physics.settle_steps, 30);
}

#[test]
fn plan_without_physics_serializes_without_field() {
    use vrm_test_plan::*;
    let plan = TestPlan {
        id: "no_phys".into(),
        spec_section: "x".into(),
        asset: "a.vrm".into(),
        camera: Camera {
            position: [0.0, 0.0, 1.0],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_degrees: 30.0,
        },
        lighting: Lighting {
            directional: DirectionalLight {
                dir: [0.0, -1.0, 0.0],
                color: [1.0, 1.0, 1.0],
                intensity: 1.0,
            },
            ambient: AmbientLight {
                color: [0.5, 0.5, 0.5],
                intensity: 0.3,
            },
            cast_shadows: false,
            receive_shadows: false,
        },
        post_processing: PostProcessing {
            tone_mapping: ToneMapping::None,
            exposure: 1.0,
        },
        output: Output {
            width: 256,
            height: 256,
            color_space: ColorSpace::Linear,
            msaa: 4,
        },
        diff: Diff {
            mode: DiffMode::Ssim,
            threshold: 0.985,
            reference_renderer: "x".into(),
        },
        ignore_renderers: Vec::new(),
        properties: Vec::new(),
        physics: None,
    };
    let yaml = serde_yml::to_string(&plan).unwrap();
    assert!(
        !yaml.contains("physics"),
        "physics field should be omitted when None, got: {yaml}"
    );
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo test -p vrm-test-plan`

Expected: compile error (PhysicsConfig + physics field don't exist).

- [ ] **Step 3: Implement**

In `crates/vrm-test-plan/src/lib.rs`, find the `TestPlan` struct and add an optional `physics` field:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestPlan {
    pub id: String,
    pub spec_section: String,
    pub asset: String,
    pub camera: Camera,
    pub lighting: Lighting,
    #[serde(default)]
    pub post_processing: PostProcessing,
    pub output: Output,
    pub diff: Diff,
    #[serde(default)]
    pub ignore_renderers: Vec<String>,
    #[serde(default)]
    pub properties: Vec<PropertyAssertion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physics: Option<PhysicsConfig>,
}
```

Add the `PhysicsConfig` type next to `Diff`:

```rust
/// Optional physics-stepping config for spring-bone / collider tests.
/// When present, the runner calls `reset_physics(settle_steps)` between
/// `set_post_processing` and `render`. Defaults to 30 steps at 60 Hz
/// (0.5 s) per `docs/methodology.md` "Spring bone initial state".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhysicsConfig {
    pub settle_steps: u32,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self { settle_steps: 30 }
    }
}
```

- [ ] **Step 4: Tests pass**

```bash
cargo test -p vrm-test-plan
```

Expected: 5 tests pass (existing 3 + 2 new).

- [ ] **Step 5: Workspace still compiles**

Existing code that constructs `TestPlan { ... }` literally will fail because the struct gained a field. Fix the only known offending site in `crates/vrm-runner/tests/diff_integration.rs::synthetic_plan` (constructed inline):

Find this block:

```rust
        diff: Diff {
            mode: DiffMode::Ssim,
            threshold,
            reference_renderer: "test-renderer".into(),
        },
        ignore_renderers: Vec::new(),
        properties: Vec::new(),
    }
}
```

Replace with:

```rust
        diff: Diff {
            mode: DiffMode::Ssim,
            threshold,
            reference_renderer: "test-renderer".into(),
        },
        ignore_renderers: Vec::new(),
        properties: Vec::new(),
        physics: None,
    }
}
```

Same for `crates/vrm-asset-generator/src/sidecar.rs::build_default_test_plan`. Find the trailing block:

```rust
        ignore_renderers: Vec::new(),
        properties: default_properties(params),
    }
}
```

Replace with:

```rust
        ignore_renderers: Vec::new(),
        properties: default_properties(params),
        physics: None,
    }
}
```

```bash
cargo build --workspace
cargo test --workspace
```

Expected: clean compile, all tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-test-plan/src/lib.rs crates/vrm-test-plan/tests/roundtrip.rs crates/vrm-runner/tests/diff_integration.rs crates/vrm-asset-generator/src/sidecar.rs
git commit -m "feat(test-plan): add optional PhysicsConfig block (settle_steps)"
```

---

### Task A2: Asset generator emits `physics: { settle_steps: 30 }` for spring-bone assets

**Files:**
- Modify: `crates/vrm-asset-generator/src/sidecar.rs`

The `build_default_test_plan` function takes `&MToonParams` today; we extend the spring-bone emit path to set `physics = Some(default)` on its plan. Non-spring-bone plans stay physics-less.

- [ ] **Step 1: Add a spring-bone-aware variant of `build_default_test_plan`**

In `crates/vrm-asset-generator/src/sidecar.rs`, after the existing `build_default_test_plan`, append:

```rust
/// Same as `build_default_test_plan` but with `physics: { settle_steps: 30 }`
/// — used by the spring-bone emit path so the runner knows to settle the
/// chain before rendering.
pub fn build_spring_bone_test_plan(params: &MToonParams, asset_relpath: &str) -> TestPlan {
    let mut plan = build_default_test_plan(params, asset_relpath);
    plan.physics = Some(PhysicsConfig {
        settle_steps: 30,
    });
    plan.spec_section = "VRMC_materials_mtoon + VRMC_springBone".into();
    plan
}
```

Make sure `PhysicsConfig` is imported at the top of the file; if not, add it to the `use vrm_test_plan::{...}` line.

- [ ] **Step 2: Use the new function in `emit_with_sidecars_spring_bone`**

In `crates/vrm-asset-generator/src/emit.rs`, find `emit_with_sidecars_spring_bone`. Replace the line:

```rust
    let plan = build_default_test_plan(mtoon, &asset_relpath);
```

with:

```rust
    let plan = crate::sidecar::build_spring_bone_test_plan(mtoon, &asset_relpath);
```

- [ ] **Step 3: Verify**

```bash
cd /Users/arkavo/Projects/vrm-conformance
mkdir -p /tmp/sb-meta-check
cargo run -q -p vrm-asset-generator -- emit-springbone --id sb_meta --output-dir /tmp/sb-meta-check --json > /dev/null
cat /tmp/sb-meta-check/sb_meta.test.yaml | grep -A 1 "^physics:"
```

Expected:

```
physics:
  settle_steps: 30
```

- [ ] **Step 4: Workspace clean**

```bash
cargo test -p vrm-asset-generator
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

All green.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-asset-generator/src/sidecar.rs crates/vrm-asset-generator/src/emit.rs
git commit -m "feat(asset-generator): spring-bone test plans carry physics.settle_steps=30"
```

---

## Section B — Mock renderer honors physics ops (as no-ops)

### Task B1: Move step_physics / reset_physics out of mock's reserved-Unimplemented list

**Files:**
- Modify: `crates/vrm-mock-renderer/src/main.rs`
- Modify: `crates/vrm-mock-renderer/src/handlers.rs`
- Modify: `crates/vrm-mock-renderer/tests/contract.rs`

The deterministic mock has no physics; the ops succeed as no-ops. This keeps spring-bone test plans usable against the mock for CI E2E coverage without giving the mock a faux physics model.

- [ ] **Step 1: Add handler functions**

In `crates/vrm-mock-renderer/src/handlers.rs`, append:

```rust
pub fn step_physics(
    _registry: &mut SessionRegistry,
    _params: serde_json::Value,
) -> Result<ops::UnitResult, RpcError> {
    // Mock has no physics state. The deterministic synthesis is the same
    // before and after stepping; we just acknowledge the op.
    Ok(ops::UnitResult {})
}

pub fn reset_physics(
    _registry: &mut SessionRegistry,
    _params: serde_json::Value,
) -> Result<ops::UnitResult, RpcError> {
    Ok(ops::UnitResult {})
}
```

- [ ] **Step 2: Route them in dispatch**

In `crates/vrm-mock-renderer/src/main.rs`, find the dispatch match. The current shape routes Phase 1 ops to real handlers and lists reserved ops by name. Add explicit cases for `step_physics` and `reset_physics`:

```rust
        "step_physics" => json_result(handlers::step_physics(registry, params)),
        "reset_physics" => json_result(handlers::reset_physics(registry, params)),
```

(Insert after the existing `"dispose"` arm and before the reserved-ops cluster.)

Remove `step_physics` and `reset_physics` from the reserved-Phase-2 cluster. The remaining reserved ops are `set_humanoid_pose`, `set_root_transform`, `animate_root_transform`. Update that arm:

```rust
        "set_humanoid_pose"
        | "set_root_transform"
        | "animate_root_transform" => Err(handlers::unimplemented(method, "Phase 2")),
```

- [ ] **Step 3: Update contract test**

In `crates/vrm-mock-renderer/tests/contract.rs`, the `reserved_phase_2_op_returns_unimplemented_phase_2` test uses `step_physics` as its sample. Change to a still-reserved op:

```rust
#[test]
fn reserved_phase_2_op_returns_unimplemented_phase_2() {
    let Some(cfg) = config_or_skip() else {
        return;
    };
    let mut child = spawn_mock();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let resp = rpc(
        &mut stdin,
        &mut stdout,
        1,
        "set_humanoid_pose",
        serde_json::json!({ "session_id": "x", "bone_rotations": {} }),
    );
    assert_eq!(resp["error"]["code"], -32000);
    assert_eq!(resp["error"]["data"]["phase"], "Phase 2");

    drop(stdin);
    let _ = child.wait();
}
```

> If your existing test isn't gated by `config_or_skip`, drop that line. The mock crate's contract.rs doesn't need a validator config — only the asset-generator crate does. Use the same spawn pattern as the surrounding tests.

Add a new test verifying physics ops are no-ops, not -32000:

```rust
#[test]
fn step_physics_returns_ok_result_not_unimplemented() {
    let mut child = spawn_mock();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let resp = rpc(
        &mut stdin,
        &mut stdout,
        1,
        "step_physics",
        serde_json::json!({ "session_id": "ignored", "dt_seconds": 0.016, "count": 30 }),
    );
    assert!(
        resp.get("result").is_some(),
        "step_physics should succeed, got: {resp:#?}"
    );

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn reset_physics_returns_ok_result() {
    let mut child = spawn_mock();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let resp = rpc(
        &mut stdin,
        &mut stdout,
        1,
        "reset_physics",
        serde_json::json!({ "session_id": "ignored", "settle_steps": 30 }),
    );
    assert!(
        resp.get("result").is_some(),
        "reset_physics should succeed, got: {resp:#?}"
    );

    drop(stdin);
    let _ = child.wait();
}
```

> **Caveat for the implementing engineer:** the existing `reserved_phase_2_op_returns_unimplemented_phase_2` test in `contract.rs` may not use `config_or_skip()`. Look at the existing test and match the spawn-pattern exactly; the snippet above is illustrative.

- [ ] **Step 4: Run tests**

```bash
cargo test -p vrm-mock-renderer
```

Expected: contract tests pass (the new ones + the updated reserved-Phase-2 one).

- [ ] **Step 5: Workspace clean**

```bash
cargo clippy -p vrm-mock-renderer --all-targets -- -D warnings
cargo fmt --all -- --check
```

Both clean.

- [ ] **Step 6: Commit**

```bash
git add crates/vrm-mock-renderer/
git commit -m "feat(mock-renderer): step_physics + reset_physics as deterministic no-ops"
```

---

## Section C — three-vrm honors physics ops

### Task C1: renderer-host implements `__stepPhysics` and `__resetPhysics`

**Files:**
- Modify: `adapters/three-vrm/src/renderer-host.html`

three-vrm's `vrm.update(deltaSeconds)` advances both LookAt and the spring-bone manager. For settle/step, we just call it in a loop. Reset is trickier: there's no public "reset spring positions" API on `@pixiv/three-vrm@3.5.0`, so we reload the .vrm from the same URL (page.route is still intercepting it) — that gives a fresh chain at rest pose. Then optionally run settle_steps updates.

- [ ] **Step 1: Add the window API functions**

In `adapters/three-vrm/src/renderer-host.html`, find the section where `window.__dispose` is defined. Just before it, add:

```js
      window.__stepPhysics = async function (params) {
        if (!state.vrm) return;
        ensureRenderer(state.canvas?.width ?? 1024, state.canvas?.height ?? 1024);
        const dt = (typeof params.dt_seconds === "number" ? params.dt_seconds : 1 / 60);
        const count = (typeof params.count === "number" ? params.count : 1);
        for (let i = 0; i < count; i++) {
          state.vrm.update(dt);
        }
      };

      // Re-loads the most recently-loaded VRM (the BrowserSession serves it
      // via the page.route interceptor at https://app.local/asset), giving
      // a fresh chain at rest pose. Then runs `settle_steps` updates so the
      // chain reaches a stable hanging position before rendering.
      window.__resetPhysics = async function (params) {
        ensureRenderer(state.canvas?.width ?? 1024, state.canvas?.height ?? 1024);
        const settle = (typeof params.settle_steps === "number" ? params.settle_steps : 0);

        // If we have a VRM, replace it with a fresh load from the same URL.
        // This is the bluntest "reset to rest pose" hammer; future
        // three-vrm versions may expose a finer-grained reset.
        if (state.vrm) {
          state.scene.remove(state.vrm.scene);
          state.vrm = null;
          const loader = new GLTFLoader();
          loader.register((parser) => new VRMLoaderPlugin(parser));
          const gltf = await loader.loadAsync("https://app.local/asset");
          const vrm = gltf.userData.vrm;
          state.vrm = vrm;
          state.scene.add(vrm.scene);
        }

        // Run settle steps at 60 Hz.
        if (state.vrm && settle > 0) {
          for (let i = 0; i < settle; i++) {
            state.vrm.update(1 / 60);
          }
        }
      };
```

- [ ] **Step 2: Rebuild the dist HTML**

```bash
cd /Users/arkavo/Projects/vrm-conformance/adapters/three-vrm
npm run build
```

(The build copies HTML to dist; no TypeScript change yet.)

- [ ] **Step 3: Commit**

```bash
git add adapters/three-vrm/src/renderer-host.html
git commit -m "feat(three-vrm): renderer-host exposes __stepPhysics + __resetPhysics"
```

---

### Task C2: BrowserSession methods + operations.ts dispatch

**Files:**
- Modify: `adapters/three-vrm/src/browser-session.ts`
- Modify: `adapters/three-vrm/src/operations.ts`
- Modify: `adapters/three-vrm/test/contract.test.ts`

- [ ] **Step 1: Add `stepPhysics` and `resetPhysics` to BrowserSession**

In `adapters/three-vrm/src/browser-session.ts`, after `setPostProcessing`:

```ts
  async stepPhysics(params: unknown): Promise<void> {
    if (!this.page) throw new Error("BrowserSession not started");
    await this.page.evaluate(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (p) => (window as any).__stepPhysics(p),
      params,
    );
  }

  async resetPhysics(params: unknown): Promise<void> {
    if (!this.page) throw new Error("BrowserSession not started");
    await this.page.evaluate(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (p) => (window as any).__resetPhysics(p),
      params,
    );
  }
```

- [ ] **Step 2: Wire dispatch in `operations.ts`**

In `adapters/three-vrm/src/operations.ts`, remove `step_physics` and `reset_physics` from `PHASE_BY_RESERVED_METHOD`:

```ts
const PHASE_BY_RESERVED_METHOD: Record<string, string> = {
  set_environment: "v1.x",
  set_expression: "Phase 3",
  set_humanoid_pose: "Phase 2",
  set_root_transform: "Phase 2",
  animate_root_transform: "Phase 2",
};
```

Add real handlers in the dispatch switch (after `dispose`):

```ts
      case "step_physics": {
        await ctx.session.stepPhysics(params);
        return { ok: true, result: {} };
      }
      case "reset_physics": {
        await ctx.session.resetPhysics(params);
        return { ok: true, result: {} };
      }
```

- [ ] **Step 3: Update contract test**

In `adapters/three-vrm/test/contract.test.ts`, the existing test `reserved phase-2 op (step_physics) returns phase Phase 2` is now wrong — step_physics is a real op. Update it to use a still-reserved op:

```ts
test("reserved phase-2 op (set_humanoid_pose) returns phase Phase 2", async () => {
  const h = spawnAdapter();
  try {
    const resp = await rpc(h, 3, "set_humanoid_pose", {
      session_id: "x",
      bone_rotations: {},
    });
    assert.equal(resp.error?.code, -32000);
    assert.equal(
      (resp.error?.data as { phase?: string } | undefined)?.phase,
      "Phase 2",
    );
  } finally {
    h.stdin.end();
    await new Promise((r) => h.child.on("exit", r));
  }
});
```

(Find the existing `step_physics` test and replace its method name with `set_humanoid_pose`. Note that the test title may also need updating.)

The render.test.ts integration test doesn't currently call step_physics or reset_physics — leave it alone in this round; the runner change in Section D will exercise the ops end-to-end.

- [ ] **Step 4: Build + run tests**

```bash
cd /Users/arkavo/Projects/vrm-conformance/adapters/three-vrm
npm run build
npm test
```

Expected: 12 tests pass (the existing 11 contract/framing + 1 render test, with the step_physics reservation assertion now using set_humanoid_pose).

- [ ] **Step 5: Commit**

```bash
cd /Users/arkavo/Projects/vrm-conformance
git add adapters/three-vrm/src/browser-session.ts adapters/three-vrm/src/operations.ts adapters/three-vrm/test/contract.test.ts
git commit -m "feat(three-vrm): real step_physics + reset_physics handlers"
```

---

## Section D — Runner wires the physics call

### Task D1: `execute_plan` calls `reset_physics` when plan has physics

**Files:**
- Modify: `crates/vrm-ops/src/tools.rs`
- Modify: `crates/vrm-runner/src/execute.rs`

- [ ] **Step 1: Add op param types**

In `crates/vrm-ops/src/tools.rs`, append after `DisposeParams`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepPhysicsParams {
    pub session_id: String,
    pub dt_seconds: f32,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetPhysicsParams {
    pub session_id: String,
    pub settle_steps: u32,
}
```

- [ ] **Step 2: Wire into execute_plan**

In `crates/vrm-runner/src/execute.rs`, find the section after `set_post_processing` and before the `png` path is built. The current shape is:

```rust
    progress(opts, "set_post_processing", &plan.id, json!({}));
    let _: ops::UnitResult = adapter
        .call(
            "set_post_processing",
            post_processing_params(&session_id, &plan.post_processing),
        )
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

    let png = opts
        .output_dir
        .join(format!("{}_{}.png", plan.id, opts.renderer_name));
```

Insert the optional physics step between these:

```rust
    progress(opts, "set_post_processing", &plan.id, json!({}));
    let _: ops::UnitResult = adapter
        .call(
            "set_post_processing",
            post_processing_params(&session_id, &plan.post_processing),
        )
        .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;

    if let Some(physics) = &plan.physics {
        progress(
            opts,
            "reset_physics",
            &plan.id,
            json!({ "settle_steps": physics.settle_steps }),
        );
        let _: ops::UnitResult = adapter
            .call(
                "reset_physics",
                ops::ResetPhysicsParams {
                    session_id: session_id.clone(),
                    settle_steps: physics.settle_steps,
                },
            )
            .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;
    }

    let png = opts
        .output_dir
        .join(format!("{}_{}.png", plan.id, opts.renderer_name));
```

- [ ] **Step 3: Verify build + tests**

```bash
cargo build -p vrm-runner
cargo test -p vrm-runner
```

Expected: clean, all 3 diff_integration tests still pass (they have `physics: None`, so the new code path is skipped).

- [ ] **Step 4: Workspace clean**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Both clean.

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-ops/src/tools.rs crates/vrm-runner/src/execute.rs
git commit -m "feat(runner): execute_plan calls reset_physics when plan has physics block"
```

---

## Section E — End-to-end smoke

### Task E1: smoke.sh exercises spring-bone settle via three-vrm

**Files:**
- Modify: `scripts/smoke.sh`

Add an opt-in step (gated on `RUN_THREE_VRM=1`, same env var that already gates three-vrm in the smoke) that emits a spring-bone asset and runs it through the three-vrm adapter. The adapter calls reset_physics(30) before rendering thanks to the runner change.

- [ ] **Step 1: Add the step**

In `scripts/smoke.sh`, find the existing `# ---- step 4c: optional three-vrm exercise` block. After the existing block, append:

```bash
# ---- step 4d: optional three-vrm spring-bone exercise --------------------
if [ "${RUN_THREE_VRM:-0}" = "1" ] && [ "$SKIP_RENDER" != "1" ] && [ -f "$THREE_VRM_DIR/dist/main.js" ]; then
    echo "==> Running three-vrm spring-bone settle + render"
    SB_ID="smoke_spring"
    cargo run --release -p vrm-asset-generator -- emit-springbone \
        --id "$SB_ID" \
        --output-dir "$ASSETS" \
        --json
    SB_OUT="$OUTPUTS/${SB_ID}_three-vrm-sb.png"
    if cargo run --release -p vrm-runner -- execute-test-plan \
            --plan "$ASSETS/${SB_ID}.test.yaml" \
            --adapter-bin node \
            --adapter-args "$THREE_VRM_DIR/dist/main.js" \
            --asset-dir "$ASSETS" \
            --output-dir "$OUTPUTS" \
            --renderer-name three-vrm-sb \
            --json; then
        if [ -f "$SB_OUT" ]; then
            echo "    three-vrm spring-bone produced: $SB_OUT ($(wc -c < "$SB_OUT" | tr -d ' ') bytes)"
        fi
    else
        echo "    three-vrm spring-bone runner step exited non-zero (continuing)" >&2
    fi
fi
```

> **Caveat:** the output PNG filename is `<test_id>_<renderer_name>.png`. The asset-generator's emit-springbone emits a test plan whose `id` is `smoke_spring`, and we pass `--renderer-name three-vrm-sb`, so the runner writes `smoke_spring_three-vrm-sb.png`. The `$SB_OUT` variable above reflects that.

- [ ] **Step 2: Smoke-test**

```bash
cd /Users/arkavo/Projects/vrm-conformance
RUN_THREE_VRM=1 ./scripts/smoke.sh 2>&1 | grep -E "spring-bone|spring|==> Running three-vrm"
```

Expected: the new step runs, three-vrm settles the chain for 30 steps, produces a PNG, smoke completes green.

If the test plan's `physics:` block is properly honored, the runner's `--json` output should include a `phase: reset_physics` progress event. Verify:

```bash
RUN_THREE_VRM=1 ./scripts/smoke.sh 2>&1 | grep '"phase":"reset_physics"' | head -3
```

Expected: at least one NDJSON line containing `"phase":"reset_physics"`.

- [ ] **Step 3: Commit**

```bash
git add scripts/smoke.sh
git commit -m "chore(smoke): exercise three-vrm spring-bone settle path under RUN_THREE_VRM=1"
```

---

### Task E2: Update operation-contract docs

**Files:**
- Modify: `docs/operation-contract.md`

Move `step_physics` and `reset_physics` out of the "Reserved" section into the body of required-but-renderer-specific ops; update spec.

- [ ] **Step 1: Add an "Physics ops" section before "Reserved"**

In `docs/operation-contract.md`, find the "Reserved operations (Phase 2+)" section. Just before it, add a new section:

```markdown
## Physics operations

These ops are required for renderer adapters that support `VRMC_springBone`. Adapters without spring-bone support may implement them as no-ops (the mock does this).

### `step_physics`

```json
{
  "input": {
    "session_id": "string",
    "dt_seconds": 0.016666666,
    "count": 1
  },
  "output": {}
}
```

Advances the renderer's internal physics state by `count` steps of `dt_seconds`. For spring-bone determinism, callers always pass `dt_seconds = 1/60` (16.66ms).

### `reset_physics`

```json
{
  "input": {
    "session_id": "string",
    "settle_steps": 30
  },
  "output": {}
}
```

Resets all spring-bone chains to their rest pose, then advances physics by `settle_steps` frames so the chain reaches a stable hanging position before measurement. Default `settle_steps = 30` (0.5 s at 60 Hz) is documented in `docs/methodology.md` as the convention.

The runner calls `reset_physics({ settle_steps })` after `set_post_processing` and before `render` whenever the test plan has a `physics:` block.
```

- [ ] **Step 2: Remove them from the Reserved list**

In the "Reserved operations (Phase 2+)" section, the current list is:

```
- `set_environment` (HDRI) — v1.x
- `set_expression` — Phase 3
- `set_humanoid_pose` — Phase 2
- `set_root_transform`, `animate_root_transform` — Phase 2
- `step_physics`, `reset_physics` — Phase 2
```

Replace with:

```
- `set_environment` (HDRI) — v1.x
- `set_expression` — Phase 3
- `set_humanoid_pose` — Phase 2
- `set_root_transform`, `animate_root_transform` — Phase 2
```

- [ ] **Step 3: Commit**

```bash
git add docs/operation-contract.md
git commit -m "docs: step_physics + reset_physics promoted out of Reserved (now Phase 1 / Physics)"
```

---

## Self-Review

**Spec coverage:**

| 2D-b goal | Task |
|---|---|
| TestPlan schema with PhysicsConfig | A1 |
| Asset generator emits physics block | A2 |
| Mock honors physics as no-ops | B1 |
| Three-vrm renderer-host physics functions | C1 |
| Three-vrm BrowserSession + dispatch wiring | C2 |
| Runner calls reset_physics when plan asks | D1 |
| End-to-end smoke exercises three-vrm spring-bone settle | E1 |
| Docs reflect new op promotion | E2 |

**Placeholder scan:** none. All code blocks complete; tests assert behavior, not just structure.

**Type consistency:**

- `PhysicsConfig { settle_steps: u32 }` consistent across A1, A2, D1.
- `StepPhysicsParams` / `ResetPhysicsParams` defined in D1 Step 1; consumed in D1 Step 2 — the only Rust call site.
- Mock + three-vrm both accept `step_physics({ session_id, dt_seconds, count })` and `reset_physics({ session_id, settle_steps })` — schema-aligned with what the runner sends.

**YAGNI guards:**

- ✅ No animate_root_transform / set_humanoid_pose / set_root_transform (still reserved Phase 2).
- ✅ Mock's no-op is correct: deterministic mock has no physics to step.
- ✅ Three-vrm's reset uses page.route's existing asset interceptor; no new asset-loading paths.
- ✅ No new sweep matrix CLI subcommand — emit-springbone gets the physics block via its existing default sidecar.

**Risk register:**

- **Three-vrm `reset_physics` via reload.** Reloading the .vrm is a blunt hammer. Side effects: scene state (camera/lights) is preserved because they live on `state` outside the VRM. Material params, mesh, and spring-bone state are all reset to file values. For Phase 2D-b's settling-only scope this is correct. If 2D-c's excitation requires preserving anything across a reset (it shouldn't — that's literally what reset means), revisit.
- **`vrm.update` semantics on three-vrm@3.5.0.** Updates LookAt + spring-bone manager + expressions. We don't currently set any LookAt target, so the LookAt component is a no-op. Spring bones step. Expressions don't move because we don't set expression weights. So `vrm.update(1/60)` in our context is effectively "step springs forward 1/60 s." Good.
- **Existing render.test.ts.** It doesn't use the physics path. The render.test.ts plan has `physics: None`, so the runner skips the new code path. Test still passes. Good.

---

## Execution Handoff

Plan saved to `docs/superpowers/plans/2026-05-10-phase2d-b-springbone-renderer-support.md`. Two execution options:

1. **Subagent-Driven** — fresh subagent per task. 8 tasks; A → B/C in parallel possible (mock and three-vrm changes are independent of each other once A1 lands).
2. **Inline Execution (recommended)** — A1 must come first; B/C/D can be done in either order; E depends on D and C. Estimated 30-40 minutes.

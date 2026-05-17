# VRMA Phase 5 — three-vrm Adapter (Real)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make three-vrm a real (non-Unimplemented) VRMA adapter. The Playwright-driven browser session loads `.vrma` files via `@pixiv/three-vrm-animation`'s `VRMAnimationLoaderPlugin`, applies them to the loaded `VRM` instance at the requested time, and produces the pose.json shape the runner reads via `diff_pose_one`.

After this phase, **two real VRMA adapters** (UniVRM from phase 4, three-vrm here) produce pose-vector output for the runner to cross-renderer diff. Phase 6 lands manual humanoid clips + bootstrap + findings + upstream issues.

**Architecture:**
- **Library:** `@pixiv/three-vrm-animation` (~3.5.0, separate npm package alongside `@pixiv/three-vrm` core). Exports `VRMAnimationLoaderPlugin` (registers a GLTFLoader hook that parses `VRMC_vrm_animation` extension) and `createVRMAnimationClip(animation, vrm)` (builds a THREE.AnimationClip retargeted to a specific VRM avatar). Clean library API — no Unity-style Mecanim conflicts.
- **Browser-host bridge:** Same `window.__*` pattern the existing ops use. Five new functions live in `renderer-host.html`: `__loadVrma`, `__applyVrmaAtTime`, `__dumpHumanoidPose`, `__dumpExpressionWeights`, `__dumpLookAtState`.
- **Application path:** load .vrma → keep the `VRMAnimation` instance → on apply, `const clip = createVRMAnimationClip(animation, state.vrm)` → `const mixer = new THREE.AnimationMixer(state.vrm.scene)` → `mixer.clipAction(clip).play()` → `mixer.setTime(time)` (seeks the mixer to t deterministically) → `state.vrm.update(0)` (drives spring-bone + lookAt update from the new pose).
- **Pose-dump:** read humanoid bones via `state.vrm.humanoid.getRawBoneNode('Hips').rotation` (etc.) or `state.vrm.humanoid.getNormalizedBonePosition()`. Expressions via `state.vrm.expressionManager.getValue('happy')`. LookAt via `state.vrm.lookAt.yaw` / `.pitch`.

**Tech Stack:** TypeScript + Playwright + three.js + @pixiv/three-vrm + @pixiv/three-vrm-animation.

**Spec:** [`docs/superpowers/specs/2026-05-17-vrma-conformance-design.md`](../specs/2026-05-17-vrma-conformance-design.md).

**Builds on:** Phases 1-4 (commits `36b663d..35db5c6`). Phase 4's pose.json shape is the contract this phase must produce.

---

## File structure

**Modify:**
- `adapters/three-vrm/package.json` — add `@pixiv/three-vrm-animation` dependency
- `adapters/three-vrm/src/renderer-host.html` — register `VRMAnimationLoaderPlugin`; add 5 new `window.__*` functions
- `adapters/three-vrm/src/browser-session.ts` — add 5 new BrowserSession methods (load_vrma, apply_vrma_at_time, dump_humanoid_pose, dump_expression_weights, dump_look_at_state)
- `adapters/three-vrm/src/operations.ts` — remove the 5 VRMA op names from `PHASE_BY_RESERVED_METHOD`; add 5 real dispatch arms
- `adapters/three-vrm/test/contract.test.ts` — extend tests to cover the real ops + remove the "5 VRMA ops return Unimplemented" test (it's expected to fail now)

**No new files.**

---

## Task 1: Add `@pixiv/three-vrm-animation` dependency

**Files:**
- Modify: `adapters/three-vrm/package.json`

- [ ] **Step 1.1: Inspect npm-side availability**

```bash
cd adapters/three-vrm
npm view @pixiv/three-vrm-animation versions --json 2>/dev/null | head -20
```

Verify a `@pixiv/three-vrm-animation` version compatible with `@pixiv/three-vrm@^3.5.0` exists. If multiple versions, pick the latest in the `3.x` family that matches the three-vrm core version range.

- [ ] **Step 1.2: Add dependency**

Edit `adapters/three-vrm/package.json`. Add to the `dependencies` block:

```json
"@pixiv/three-vrm-animation": "^3.5.0",
```

Run:

```bash
cd adapters/three-vrm
npm install --silent
```

Verify `package-lock.json` updates to include the new package; `node_modules/@pixiv/three-vrm-animation/` exists.

- [ ] **Step 1.3: Verify package exports**

```bash
ls node_modules/@pixiv/three-vrm-animation/
cat node_modules/@pixiv/three-vrm-animation/package.json | head -20
```

Expected: package exists, has `main`/`module`/`types` fields, version matches what npm offered.

- [ ] **Step 1.4: Commit**

```bash
git add adapters/three-vrm/package.json adapters/three-vrm/package-lock.json
git commit -m "$(cat <<'EOF'
feat(adapters/three-vrm): add @pixiv/three-vrm-animation dependency

Library for parsing + applying VRMC_vrm_animation files in three-vrm.
Registers a GLTFLoader plugin (VRMAnimationLoaderPlugin) that returns
a VRMAnimation instance from a .vrma; createVRMAnimationClip builds a
THREE.AnimationClip retargeted to a specific VRM avatar. Drives the
phase 5 real VRMA implementation that follows in subsequent commits.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

Return DONE with commit SHA.

---

## Task 2: Register VRMAnimationLoaderPlugin + add `__loadVrma` in renderer-host.html

**Files:**
- Modify: `adapters/three-vrm/src/renderer-host.html`

- [ ] **Step 2.1: Add VRMAnimation imports**

Find the existing import block at the top of the module script (around line 22-26). Add an import:

```html
<script type="module">
  import * as THREE from "three";
  import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
  import { VRMLoaderPlugin } from "@pixiv/three-vrm";
  import {
    VRMAnimationLoaderPlugin,
    createVRMAnimationClip,
  } from "@pixiv/three-vrm-animation";
```

- [ ] **Step 2.2: Add VRMA state**

Find the existing `const state = { ... }` block. Add fields:

```javascript
const state = {
  // ... existing fields ...
  vrmAnimation: null,       // VRMAnimation instance (parsed .vrma)
  vrmaMixer: null,          // THREE.AnimationMixer driving the clip
  vrmaClip: null,           // THREE.AnimationClip created via createVRMAnimationClip
};
```

- [ ] **Step 2.3: Implement `window.__loadVrma`**

Add a new `window.__loadVrma` function below `__loadVrm`:

```javascript
window.__loadVrma = async function (url) {
  if (!state.vrm) {
    throw new Error("loadVrma: no VRM loaded — call loadVrm first");
  }
  // Clean up any previously loaded VRMA.
  if (state.vrmaMixer) {
    state.vrmaMixer.stopAllAction();
    state.vrmaMixer = null;
  }
  state.vrmAnimation = null;
  state.vrmaClip = null;

  const loader = new GLTFLoader();
  loader.register((parser) => new VRMAnimationLoaderPlugin(parser));
  const gltf = await loader.loadAsync(url);
  const animations = gltf.userData.vrmAnimations;
  if (!animations || animations.length === 0) {
    throw new Error("loadVrma: no VRMAnimation found in glTF userData");
  }
  // VRMA spec says animations[0] is the portable clip.
  state.vrmAnimation = animations[0];

  // Build a retargeted clip + mixer for the loaded VRM.
  state.vrmaClip = createVRMAnimationClip(state.vrmAnimation, state.vrm);
  state.vrmaMixer = new THREE.AnimationMixer(state.vrm.scene);
  const action = state.vrmaMixer.clipAction(state.vrmaClip);
  action.play();

  return {
    vrma_handle: 1,
    channel_summary: {
      humanoid_bones: Object.keys(state.vrmAnimation.humanoidTracks?.rotation ?? {}).length,
      expressions: Object.keys(state.vrmAnimation.expressionTracks ?? {}).length,
      has_look_at: state.vrmAnimation.lookAtTrack != null,
      duration_seconds: state.vrmaClip.duration,
    },
  };
};
```

The exact shape of `state.vrmAnimation` (e.g. `humanoidTracks.rotation` vs `humanoidTracks` containing rotation map directly) depends on `@pixiv/three-vrm-animation`'s API. Verify in step 2.4 by inspecting the library's TypeScript declarations:

```bash
cat adapters/three-vrm/node_modules/@pixiv/three-vrm-animation/types/VRMAnimation.d.ts 2>/dev/null
```

Adapt the field accessors to match what the library actually exposes.

- [ ] **Step 2.4: Build + manual smoke**

```bash
cd adapters/three-vrm
npm run build
```

Expected: clean. If the import names are wrong (`VRMAnimationLoaderPlugin` vs `VRMAnimationLoaderPlugin` is a guess), TypeScript will flag.

- [ ] **Step 2.5: Commit**

```bash
git add adapters/three-vrm/src/renderer-host.html
git commit -m "$(cat <<'EOF'
feat(adapters/three-vrm): VRMAnimationLoaderPlugin + window.__loadVrma

Registers @pixiv/three-vrm-animation's GLTFLoader plugin to parse
VRMC_vrm_animation extensions. __loadVrma reads animations[0] per spec,
builds a retargeted AnimationClip for the loaded VRM, and creates an
AnimationMixer ready for time-deterministic seek via setTime in the
subsequent __applyVrmaAtTime call.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: `window.__applyVrmaAtTime` — seek the mixer to t deterministically

**Files:**
- Modify: `adapters/three-vrm/src/renderer-host.html`

- [ ] **Step 3.1: Implement**

After `__loadVrma`, add:

```javascript
window.__applyVrmaAtTime = function (params) {
  if (!state.vrm) throw new Error("applyVrmaAtTime: no VRM loaded");
  if (!state.vrmaMixer || !state.vrmaClip) {
    throw new Error("applyVrmaAtTime: no VRMA loaded — call loadVrma first");
  }
  const time = params.time_seconds ?? 0.0;

  // Seek deterministically. setTime sets the mixer's playhead but
  // does not advance simulation state on its own — call update(0) so
  // the bound bones, expression weights, and lookAt evaluator pick
  // up the new sampled state.
  state.vrmaMixer.setTime(time);
  state.vrmaMixer.update(0);

  // Drive the VRM's per-frame update so spring-bone + lookAt
  // evaluators re-derive their state from the newly-set bones.
  state.vrm.update(0);

  return {
    channels_applied: {
      humanoid_bones: Object.keys(state.vrmAnimation.humanoidTracks?.rotation ?? {}).length,
      expressions: Object.keys(state.vrmAnimation.expressionTracks ?? {}).length,
      look_at: state.vrmAnimation.lookAtTrack != null,
    },
  };
};
```

- [ ] **Step 3.2: Build + commit**

```bash
cd adapters/three-vrm && npm run build
```

Expected: clean.

```bash
git add adapters/three-vrm/src/renderer-host.html
git commit -m "$(cat <<'EOF'
feat(adapters/three-vrm): window.__applyVrmaAtTime — seek + update

mixer.setTime(t) seeks the mixer's playhead deterministically; update(0)
advances simulation by zero delta so the bone hierarchy / expressions /
lookAt evaluator pick up the new sampled state. Then vrm.update(0)
drives spring-bone + lookAt re-derive from the newly-applied pose.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: Three dump functions in renderer-host.html

**Files:**
- Modify: `adapters/three-vrm/src/renderer-host.html`

- [ ] **Step 4.1: Inspect three-vrm humanoid + expression + lookAt API**

```bash
cat adapters/three-vrm/node_modules/@pixiv/three-vrm/types/humanoid/VRMHumanoid.d.ts 2>/dev/null | head -40
cat adapters/three-vrm/node_modules/@pixiv/three-vrm/types/expressions/VRMExpressionManager.d.ts 2>/dev/null | head -40
cat adapters/three-vrm/node_modules/@pixiv/three-vrm/types/lookAt/VRMLookAt.d.ts 2>/dev/null | head -40
```

Identify the canonical accessors:
- `vrm.humanoid.getNormalizedBoneNode('hips')` or `getRawBoneNode('hips')` returns `THREE.Object3D | null`; `.quaternion` reads localRotation
- `vrm.expressionManager.getValue('happy')` or `.getExpression('happy')` reads weights
- `vrm.lookAt.yaw`, `.pitch` directly

If the accessors are different, adapt. The 19 humanoid bone names match the VRMA spec (`'hips'`, `'spine'`, etc. — lowercase camelCase).

- [ ] **Step 4.2: Implement `window.__dumpHumanoidPose`**

```javascript
const HUMANOID_BONES = [
  "hips", "spine", "chest", "neck", "head",
  "leftShoulder", "leftUpperArm", "leftLowerArm", "leftHand",
  "rightShoulder", "rightUpperArm", "rightLowerArm", "rightHand",
  "leftUpperLeg", "leftLowerLeg", "leftFoot",
  "rightUpperLeg", "rightLowerLeg", "rightFoot",
];

window.__dumpHumanoidPose = function () {
  if (!state.vrm) throw new Error("dumpHumanoidPose: no VRM loaded");
  const bones = [];
  const missing = [];
  let hipsTranslation = [0, 0, 0];
  for (const name of HUMANOID_BONES) {
    const node = state.vrm.humanoid.getNormalizedBoneNode(name);
    if (node == null) {
      missing.push(name);
      continue;
    }
    const q = node.quaternion;
    bones.push({ name, local_rotation_quat: [q.x, q.y, q.z, q.w] });
    if (name === "hips") {
      const p = node.position;
      hipsTranslation = [p.x, p.y, p.z];
    }
  }
  return {
    bones,
    hips_translation: hipsTranslation,
    bones_missing: missing,
  };
};
```

- [ ] **Step 4.3: Implement `window.__dumpExpressionWeights`**

```javascript
const PRESET_EXPRESSIONS = [
  "happy", "angry", "sad", "relaxed", "surprised",
  "aa", "ih", "ou", "ee", "oh",
  "blink", "blinkLeft", "blinkRight", "neutral",
];

window.__dumpExpressionWeights = function () {
  if (!state.vrm) throw new Error("dumpExpressionWeights: no VRM loaded");
  const presets = {};
  const custom = {};
  if (!state.vrm.expressionManager) {
    return { presets, custom };
  }
  for (const name of PRESET_EXPRESSIONS) {
    const v = state.vrm.expressionManager.getValue(name);
    presets[name] = v ?? 0.0;
  }
  // Custom expressions: iterate any expression name not in PRESET_EXPRESSIONS.
  // The expressionManager exposes a list of registered expression names; if
  // accessing it directly isn't available, iterate vrm.expressionManager.expressions.
  const presetSet = new Set(PRESET_EXPRESSIONS);
  const expressions = state.vrm.expressionManager.expressions ?? [];
  for (const expr of expressions) {
    const exprName = expr.expressionName ?? expr.name;
    if (!exprName || presetSet.has(exprName)) continue;
    custom[exprName] = state.vrm.expressionManager.getValue(exprName) ?? 0.0;
  }
  return { presets, custom };
};
```

The exact property name for expression registry (`expressions` vs `expressionMap` vs similar) varies between three-vrm versions. Verify in the `.d.ts` from step 4.1; adapt if needed.

- [ ] **Step 4.4: Implement `window.__dumpLookAtState`**

```javascript
window.__dumpLookAtState = function () {
  if (!state.vrm) throw new Error("dumpLookAtState: no VRM loaded");

  // three-vrm's VRMLookAt exposes yaw + pitch directly. If yaw/pitch
  // are null/undefined when no lookAtTarget is configured (the test
  // path), they fall back to zero.
  const lookAt = state.vrm.lookAt;
  const yaw = lookAt?.yaw ?? 0.0;
  const pitch = lookAt?.pitch ?? 0.0;

  // Derive gaze quaternion via Extrinsic ZXY per spec. THREE.Euler with
  // 'YXZ' order matches: yaw around Y, pitch around X, no roll.
  const euler = new THREE.Euler(
    THREE.MathUtils.degToRad(pitch),
    THREE.MathUtils.degToRad(yaw),
    0,
    "YXZ",
  );
  const q = new THREE.Quaternion().setFromEuler(euler);

  // applied_via: from VRMC_vrm.lookAt.type on the loaded VRM.
  // three-vrm's VRMLookAt has applier types — VRMLookAtBoneApplier for
  // bone, VRMLookAtExpressionApplier for expression. Check the
  // constructor name of vrm.lookAt.applier.
  let appliedVia = "off";
  if (lookAt?.applier) {
    const applierCtor = lookAt.applier.constructor?.name ?? "";
    if (applierCtor.includes("Bone")) appliedVia = "bone";
    else if (applierCtor.includes("Expression")) appliedVia = "expression";
  }

  // offsetFromHeadBone: read from the VRM's lookAt config.
  const offset = lookAt?.offsetFromHeadBone;
  const offsetArr = offset ? [offset.x, offset.y, offset.z] : [0, 0, 0];

  return {
    gaze_direction_quat: [q.x, q.y, q.z, q.w],
    yaw_deg: yaw,
    pitch_deg: pitch,
    applied_via: appliedVia,
    offset_from_head_bone: offsetArr,
  };
};
```

- [ ] **Step 4.5: Build + commit**

```bash
cd adapters/three-vrm && npm run build
```

Expected: clean.

```bash
git add adapters/three-vrm/src/renderer-host.html
git commit -m "$(cat <<'EOF'
feat(adapters/three-vrm): three pose-dump host bridge functions

window.__dumpHumanoidPose, __dumpExpressionWeights, __dumpLookAtState
read the post-apply state from VRMHumanoid (getNormalizedBoneNode),
VRMExpressionManager (getValue), and VRMLookAt (yaw/pitch + applier
constructor name discrimination). Output shape matches the runner's
ReferencePoseFixture for direct pose.json composition by the Node-side
session.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: BrowserSession TypeScript bindings for the 5 ops

**Files:**
- Modify: `adapters/three-vrm/src/browser-session.ts`

- [ ] **Step 5.1: Add 5 async methods**

Find the existing class body (after `dumpBonePositions` around line 154). Add:

```typescript
  async loadVrma(diskPath: string): Promise<{
    vrma_handle: number;
    channel_summary: {
      humanoid_bones: number;
      expressions: number;
      has_look_at: boolean;
      duration_seconds: number;
    };
  }> {
    if (!this.page) throw new Error("BrowserSession not started");
    if (!fsSync.existsSync(diskPath)) {
      throw new Error(`vrma not found: ${diskPath}`);
    }
    this.currentVrma = { diskPath };
    return await this.page.evaluate(
      ({ url }: { url: string }) =>
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (window as any).__loadVrma(url),
      { url: "https://app.local/vrma" },
    );
  }

  async applyVrmaAtTime(params: unknown): Promise<{
    channels_applied: { humanoid_bones: number; expressions: number; look_at: boolean };
  }> {
    if (!this.page) throw new Error("BrowserSession not started");
    return await this.page.evaluate(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (p) => (window as any).__applyVrmaAtTime(p),
      params,
    );
  }

  async dumpHumanoidPose(): Promise<{
    bones: Array<{ name: string; local_rotation_quat: number[] }>;
    hips_translation: number[];
    bones_missing: string[];
  }> {
    if (!this.page) throw new Error("BrowserSession not started");
    return await this.page.evaluate(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      () => (window as any).__dumpHumanoidPose(),
    );
  }

  async dumpExpressionWeights(): Promise<{
    presets: Record<string, number>;
    custom: Record<string, number>;
  }> {
    if (!this.page) throw new Error("BrowserSession not started");
    return await this.page.evaluate(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      () => (window as any).__dumpExpressionWeights(),
    );
  }

  async dumpLookAtState(): Promise<{
    gaze_direction_quat: number[];
    yaw_deg: number;
    pitch_deg: number;
    applied_via: string;
    offset_from_head_bone: number[];
  }> {
    if (!this.page) throw new Error("BrowserSession not started");
    return await this.page.evaluate(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      () => (window as any).__dumpLookAtState(),
    );
  }
```

Also add a private `currentVrma: { diskPath: string } | null = null;` field next to the existing `currentAsset` field.

The Playwright `page.evaluate` URL routing for `vrma` (vs `asset` for VRMs) needs a parallel route handler. Find where the existing `app.local/asset` route is set up (likely in `start()` around line 42-86) and add a parallel route for `app.local/vrma` that reads `this.currentVrma?.diskPath`:

```typescript
// (in start(), near the existing 'asset' route handler)
await this.page.route("https://app.local/vrma", async (route) => {
  const path = this.currentVrma?.diskPath;
  if (!path) {
    await route.abort("failed");
    return;
  }
  const body = await fs.readFile(path);
  await route.fulfill({
    status: 200,
    contentType: "application/octet-stream",
    body,
  });
});
```

Adapt the pattern from the existing 'asset' route — the route hostname, fs import, and error handling shape are likely already set up.

- [ ] **Step 5.2: Build + commit**

```bash
cd adapters/three-vrm && npm run build
```

Expected: clean.

```bash
git add adapters/three-vrm/src/browser-session.ts
git commit -m "$(cat <<'EOF'
feat(adapters/three-vrm): BrowserSession 5 VRMA op bindings

loadVrma + applyVrmaAtTime + dumpHumanoidPose + dumpExpressionWeights +
dumpLookAtState — each thin Playwright-evaluate wrappers around the
window.__* host functions. Adds Playwright route handler for
https://app.local/vrma to stream .vrma bytes from disk.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: operations.ts — promote 5 VRMA ops from Unimplemented to real dispatch

**Files:**
- Modify: `adapters/three-vrm/src/operations.ts`

- [ ] **Step 6.1: Remove 5 entries from PHASE_BY_RESERVED_METHOD**

Open `adapters/three-vrm/src/operations.ts`. Find the `PHASE_BY_RESERVED_METHOD` const (around line 21). Remove these 5 entries:

```typescript
  load_vrma: "vrma-v1",
  apply_vrma_at_time: "vrma-v1",
  dump_humanoid_pose: "vrma-v1",
  dump_expression_weights: "vrma-v1",
  dump_look_at_state: "vrma-v1",
```

Leave the existing reserved entries (`set_environment`, `set_expression`, `set_humanoid_pose`, `set_root_transform`) alone.

- [ ] **Step 6.2: Add 5 real dispatch arms in the switch**

Find the switch block in `dispatch()` (around line 45-115). After the existing `case "dump_bone_positions"`, add the 5 new arms:

```typescript
      case "load_vrma": {
        const p = params as { vrma_path?: string };
        if (!p?.vrma_path) return badParams("missing vrma_path");
        const result = await ctx.session.loadVrma(p.vrma_path);
        return { ok: true, result };
      }
      case "apply_vrma_at_time": {
        const p = params as {
          session_id?: string;
          vrma_handle?: number;
          vrm_handle?: number;
          time_seconds?: number;
        };
        if (!p?.session_id) return badParams("missing session_id");
        const result = await ctx.session.applyVrmaAtTime(p);
        return { ok: true, result };
      }
      case "dump_humanoid_pose": {
        const p = params as { session_id?: string };
        if (!p?.session_id) return badParams("missing session_id");
        const result = await ctx.session.dumpHumanoidPose();
        return { ok: true, result };
      }
      case "dump_expression_weights": {
        const p = params as { session_id?: string };
        if (!p?.session_id) return badParams("missing session_id");
        const result = await ctx.session.dumpExpressionWeights();
        return { ok: true, result };
      }
      case "dump_look_at_state": {
        const p = params as { session_id?: string };
        if (!p?.session_id) return badParams("missing session_id");
        const result = await ctx.session.dumpLookAtState();
        return { ok: true, result };
      }
```

- [ ] **Step 6.3: Update knownMethods()**

Find `knownMethods()` (around line 173). It already includes `...Object.keys(PHASE_BY_RESERVED_METHOD)` for reserved-ops surface; but VRMA ops are no longer reserved. Add them as first-class entries:

```typescript
export function knownMethods(): string[] {
  return [
    "load_vrm",
    "set_camera",
    "set_lighting",
    "set_post_processing",
    "render",
    "dispose",
    "step_physics",
    "reset_physics",
    "animate_root_transform",
    "dump_bone_positions",
    "load_vrma",
    "apply_vrma_at_time",
    "dump_humanoid_pose",
    "dump_expression_weights",
    "dump_look_at_state",
    ...Object.keys(PHASE_BY_RESERVED_METHOD),
  ];
}
```

- [ ] **Step 6.4: Update contract.test.ts**

Open `adapters/three-vrm/test/contract.test.ts`. Find the test from phase 1 (`c3e9500`) that asserts VRMA ops return -32000:

```typescript
test("VRMA ops return Unimplemented with phase vrma-v1", async () => {
```

DELETE this test (the assumption no longer holds; VRMA ops are real now).

Also find the `knownMethods()` regression test from phase 1 (`0dc216b`). Update or leave — the test asserts the 5 VRMA names appear in `knownMethods()`, which still holds.

- [ ] **Step 6.5: Build + test**

```bash
cd adapters/three-vrm
npm run build
npm test
```

Expected: build clean; all tests pass (the deleted test is gone; remaining tests still hold).

- [ ] **Step 6.6: Commit**

```bash
git add adapters/three-vrm/src/operations.ts adapters/three-vrm/test/contract.test.ts
git commit -m "$(cat <<'EOF'
feat(adapters/three-vrm): VRMA ops dispatched to real BrowserSession

PHASE_BY_RESERVED_METHOD no longer lists the 5 VRMA ops; they route to
session.loadVrma / applyVrmaAtTime / dumpHumanoidPose /
dumpExpressionWeights / dumpLookAtState. knownMethods() promotes them
to first-class. Contract test "VRMA ops return Unimplemented" deleted
— assumption no longer holds.

three-vrm is now the 2nd real VRMA adapter (UniVRM was the 1st in
phase 4).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: End-to-end smoke against a phase-3 VRMA plan

**Files:** none directly.

- [ ] **Step 7.1: Generate one VRMA test plan**

```bash
rm -rf /tmp/vrma-three-vrm-smoke
cargo run -p vrm-asset-generator --release -- emit-vrma-humanoid-sweep \
    --output-dir /tmp/vrma-three-vrm-smoke 2>&1 | tail -5
```

- [ ] **Step 7.2: Filter to one plan**

```bash
mkdir -p /tmp/vrma-three-vrm-smoke-single
cp /tmp/vrma-three-vrm-smoke/vrma_humanoid_head_yaw_45.* /tmp/vrma-three-vrm-smoke-single/
mkdir -p /tmp/vrma-three-vrm-smoke-output
```

- [ ] **Step 7.3: Run via vrm-runner against three-vrm adapter**

```bash
cd adapters/three-vrm && npx --yes playwright install chromium >/dev/null 2>&1
cargo build --release -p vrm-runner
TVM_BIN="adapters/three-vrm/dist/main.js"
target/release/vrm-runner execute-test-plan \
    --plan /tmp/vrma-three-vrm-smoke-single/vrma_humanoid_head_yaw_45.test.yaml \
    --adapter-bin "$(command -v node)" \
    --adapter-args "$TVM_BIN" \
    --asset-dir /tmp/vrma-three-vrm-smoke-single \
    --output-dir /tmp/vrma-three-vrm-smoke-output \
    --renderer-name three-vrm \
    --vrma /tmp/vrma-three-vrm-smoke-single/vrma_humanoid_head_yaw_45.vrma \
    --apply-at-time 1.0 \
    --json 2>&1 | tee /tmp/vrma-three-vrm-smoke.log | tail -20
```

(The `--vrma` flag is the phase 2 CLI surface. If the runner's `execute-test-plan` reads `animation.vrma` from the plan YAML itself, the explicit `--vrma` flag may be redundant — check via the runner's --help.)

- [ ] **Step 7.4: Verify pose.json shape**

```bash
ls -la /tmp/vrma-three-vrm-smoke-output/
cat /tmp/vrma-three-vrm-smoke-output/vrma_humanoid_head_yaw_45_three-vrm.pose.json | python3 -m json.tool | head -50
```

```bash
python3 -c "
import json, math
with open('/tmp/vrma-three-vrm-smoke-output/vrma_humanoid_head_yaw_45_three-vrm.pose.json') as f:
    d = json.load(f)
head = next((b for b in d['humanoid']['bones'] if b['name'] == 'head'), None)
print('head quat:', head['local_rotation_quat'] if head else 'MISSING')
if head:
    y, w = head['local_rotation_quat'][1], head['local_rotation_quat'][3]
    angle = 2 * math.degrees(math.atan2(abs(y), abs(w)))
    print(f'inferred Y rotation: {angle:.1f}°')
"
```

Expected: head quat shows ~`[0, 0.383, 0, 0.924]`, inferred Y rotation ≈ 45°.

- [ ] **Step 7.5: If anything doesn't match**

Capture the actual values and report. Most likely failure modes:
- pose.json shape doesn't match runner's `ReferencePoseFixture` → revisit dump functions
- head bone shows ~0° instead of ~45° → `setTime` didn't drive the bones; check the mixer setup
- pose.json doesn't appear at all → the runner's VRMA op sequence didn't fire; check `animation.vrma` in the test.yaml

---

## Task 8: Workspace fmt + clippy + test pass

**Files:** none directly.

- [ ] **Step 8.1: Run cleanup**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd adapters/three-vrm && npm test
```

All four clean.

- [ ] **Step 8.2: Commit if fixes**

```bash
git status -s
```

If modifications:

```bash
git add -u
git commit -m "$(cat <<'EOF'
chore: fmt + clippy + tests clean after VRMA phase 5

Final workspace pass after VRMA phase 5 (three-vrm real adapter).
Zero clippy warnings, zero fmt diffs, all Rust + npm tests green.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Phase 5 completion checklist

- [ ] `@pixiv/three-vrm-animation` listed in `adapters/three-vrm/package.json` dependencies
- [ ] `renderer-host.html` registers `VRMAnimationLoaderPlugin` + exposes `window.__loadVrma`, `__applyVrmaAtTime`, `__dumpHumanoidPose`, `__dumpExpressionWeights`, `__dumpLookAtState`
- [ ] `BrowserSession` has 5 corresponding async methods + a Playwright route handler for `https://app.local/vrma`
- [ ] `operations.ts` dispatcher routes 5 VRMA ops to real BrowserSession methods (no longer in `PHASE_BY_RESERVED_METHOD`)
- [ ] `knownMethods()` lists the 5 VRMA ops as first-class entries
- [ ] Contract test asserting "VRMA ops return Unimplemented" is removed
- [ ] One phase-3-emitted VRMA plan renders end-to-end through three-vrm with pose.json shape matching `ReferencePoseFixture`
- [ ] Head bone shows inferred Y rotation ≈ 45° in pose.json
- [ ] `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `npm test` all clean

After this phase, two real VRMA adapters (UniVRM + three-vrm) produce pose-vector output for cross-renderer diff. Phase 6 lands manual humanoid clips + bootstrap + findings + upstream issues against the remaining `Unimplemented` adapters (godot-vrm, VMK).

## Caveats for the implementer

1. **`@pixiv/three-vrm-animation` API names are sketched.** `VRMAnimationLoaderPlugin`, `createVRMAnimationClip`, `gltf.userData.vrmAnimations`, `state.vrmAnimation.humanoidTracks.rotation`, `state.vrmAnimation.expressionTracks`, `state.vrmAnimation.lookAtTrack`, `vrm.humanoid.getNormalizedBoneNode`, `vrm.expressionManager.getValue`, `vrm.lookAt.yaw`, `vrm.lookAt.applier.constructor.name` — verify each against the library's `.d.ts` in `node_modules/@pixiv/three-vrm-animation/types/` and `node_modules/@pixiv/three-vrm/types/`.

2. **three-vrm's mixer setTime is the right time-deterministic seek.** `mixer.setTime(t)` followed by `update(0)` advances by zero delta — the mixer state matches "as if t seconds elapsed from initial mixer state" without depending on the wall clock. This is the canonical "sample at t" pattern in three.js.

3. **Smoke test takes ~10-20s** (Playwright browser boot + 1 plan). Much faster than Unity phase 4. If it appears to hang for minutes, the Playwright `app.local/vrma` route handler isn't firing — check that `route()` registration in `start()` is awaited.

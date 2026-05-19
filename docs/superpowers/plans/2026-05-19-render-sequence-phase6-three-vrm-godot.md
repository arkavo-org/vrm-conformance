# `render_sequence` Phase 6 — three-vrm + godot-vrm Implementers

> **For agentic workers:** Use superpowers:subagent-driven-development. Phase 5 (VMK) is landed at `8b80f24`. The pattern is established: per-engine frame loop wrapping the existing single-frame render call, animate_root_transform interpolation, zero-sentinel BLAKE3 (runner re-hashes), validation (mutual exclusion + 60Hz floor + apply_vrma + non-PNG output_format rejection).

**Goal:** Implement `render_sequence` in both `adapters/three-vrm` (TypeScript + Playwright) and `adapters/godot-vrm` (GDScript). Both adapters currently return `-32000 Unimplemented` via their `PHASE_BY_RESERVED_METHOD` maps. After Phase 6, three adapters (VMK + three-vrm + godot-vrm) implement the op end-to-end; the swing-seq corpus from Phase 4 can drive a three-way consensus diff against the mock reference.

**Out of scope for this phase:**

- `apply_vrma` per-frame in either adapter (reject, same as VMK)
- ffmpeg mux for MP4/MOV (reject non-PNG, same as VMK)
- UniVRM (Phase 7 — bundled with the deferred L4-PlayMode work)

**Architectural notes:**

- **three-vrm**: the Node-side dispatcher calls `BrowserSession.renderSequence(...)`, which loops in the browser via a new `__renderSequence` JS function. The loop interpolates root translation per frame, drives physics via the existing per-frame `renderer.render()` (three.js's `springBoneManager.update(dt)` runs implicitly during render), and writes per-frame PNGs to `<output_dir>/<i:04>.png` via Node.
- **godot-vrm**: GDScript handler in `session.gd` loops, calling `vrm_secondary.do_process(physics_dt)` and capturing `viewport.get_texture().get_image()` per frame. Pattern is the union of `render` (line 163) and `animate_root_transform` (line 278) in the existing file.
- **BLAKE3**: both adapters return `"blake3:" + 64*"0"` sentinel per frame. Runner already re-hashes (Phase 5 Task 2 committed `59efb04`).

**Tech stack:** TypeScript / Playwright / three.js (three-vrm); GDScript / Godot 4 (godot-vrm); no upstream changes to either renderer library.

**Spec:** [`rfcs/0004-render-sequence-op.md`](../../../rfcs/0004-render-sequence-op.md).

---

## File structure

**Modify:**
- `adapters/three-vrm/src/operations.ts` — remove `render_sequence` from `PHASE_BY_RESERVED_METHOD`, add `case "render_sequence":` dispatch
- `adapters/three-vrm/src/browser-session.ts` — add `renderSequence(...)` method
- `adapters/three-vrm/src/renderer-host.html` — add `__renderSequence` JS function (where the existing `__render` / `__animateRootTransform` live)
- `adapters/three-vrm/test/contract.test.ts` — flip the existing "render_sequence returns Unimplemented" assertion to a success-path test
- `adapters/godot-vrm/src/session.gd` — add `render_sequence(...)` method
- `adapters/godot-vrm/src/operations.gd` — remove `render_sequence` from `PHASE_BY_RESERVED_METHOD`, add dispatch arm calling `session.render_sequence(...)`
- `crates/vrm-godot-shim/tests/contract.rs` — flip the existing `render_sequence` Unimplemented row in `reserved_ops_still_return_unimplemented` (or remove it and add a new success-path test)

**Create:**
- `crates/vrm-runner/tests/render_sequence_e2e_three_vrm.rs` — `#[ignore]`-gated E2E (requires Playwright Chromium)
- `crates/vrm-runner/tests/render_sequence_e2e_godot_vrm.rs` — `#[ignore]`-gated E2E (requires Godot 4 on PATH)

---

## Task 1: three-vrm `render_sequence` implementation

**Files:** `adapters/three-vrm/src/{operations.ts, browser-session.ts, renderer-host.html}`, plus the test file.

- [ ] **Step 1.1: Remove from reserved + add dispatch case**

Edit `adapters/three-vrm/src/operations.ts`:
- Remove `render_sequence: "v1.x-sequence",` from `PHASE_BY_RESERVED_METHOD` (around line 26).
- Add a new dispatch case after `case "animate_root_transform":` (around line 107):

```typescript
case "render_sequence": {
  const p = params as {
    session_id?: string;
    width?: number;
    height?: number;
    output_dir?: string;
    frame_count?: number;
    frame_hz?: number;
    physics_dt_seconds?: number;
    color_space?: string;
    output_format?: string;
    animate_root_transform?: {
      translation_start: [number, number, number];
      translation_end: [number, number, number];
    };
    apply_vrma?: unknown;
  };

  // Validation per RFC-0004.
  if (typeof p?.physics_dt_seconds === "number" && p.physics_dt_seconds > 1.0 / 60.0 + 1e-6) {
    return badParams(`physics_dt_seconds ${p.physics_dt_seconds} exceeds 60 Hz floor`);
  }
  if (p?.animate_root_transform != null && p?.apply_vrma != null) {
    return badParams("animate_root_transform and apply_vrma are mutually exclusive");
  }
  if (p?.apply_vrma != null) {
    return badParams("apply_vrma is not yet implemented in three-vrm (Phase 6 deferral)");
  }
  if (p?.output_format != null && p.output_format !== "png_sequence") {
    return badParams(`output_format "${p.output_format}" is not yet supported by three-vrm; only png_sequence`);
  }
  if (!p?.output_dir) return badParams("missing output_dir");
  if (!p?.frame_count || p.frame_count < 1) return badParams("frame_count must be >= 1");
  if (typeof p?.frame_hz !== "number") return badParams("frame_hz required");

  const result = await ctx.session.renderSequence({
    width: p.width ?? 512,
    height: p.height ?? 512,
    color_space: p.color_space ?? "Linear",
    output_dir: p.output_dir,
    frame_count: p.frame_count,
    frame_hz: p.frame_hz,
    physics_dt_seconds: p.physics_dt_seconds ?? 1.0 / 60.0,
    animate_root_transform: p.animate_root_transform,
  });
  return { ok: true, result };
}
```

Use whichever `badParams(...)` helper the existing handlers use — likely returns `{ ok: false, error: { code: -32602, ... } }`.

- [ ] **Step 1.2: Add `BrowserSession.renderSequence`**

Edit `adapters/three-vrm/src/browser-session.ts`:

```typescript
async renderSequence(params: {
  width: number;
  height: number;
  color_space: string;
  output_dir: string;
  frame_count: number;
  frame_hz: number;
  physics_dt_seconds: number;
  animate_root_transform?: {
    translation_start: [number, number, number];
    translation_end: [number, number, number];
  };
}): Promise<{
  frames: Array<{ index: number; timestamp_seconds: number; path: string; blake3: string }>;
  duration_seconds: number;
  actual_color_space: string;
  frame_hz_achieved: number;
}> {
  if (!this.page) throw new Error("BrowserSession not started");

  await fs.mkdir(params.output_dir, { recursive: true });

  // The browser-side __renderSequence returns N PNG data URLs in order.
  const dataUrls: string[] = await this.page.evaluate(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (p) => (window as any).__renderSequence(p),
    {
      width: params.width,
      height: params.height,
      color_space: params.color_space,
      frame_count: params.frame_count,
      physics_dt_seconds: params.physics_dt_seconds,
      animate_root_transform: params.animate_root_transform,
    },
  );

  if (dataUrls.length !== params.frame_count) {
    throw new Error(`__renderSequence returned ${dataUrls.length} URLs, expected ${params.frame_count}`);
  }

  const zeroHash = "blake3:" + "0".repeat(64);
  const frames: Array<{ index: number; timestamp_seconds: number; path: string; blake3: string }> = [];
  for (let i = 0; i < dataUrls.length; i++) {
    const dataUrl = dataUrls[i];
    const comma = dataUrl.indexOf(",");
    if (comma < 0) throw new Error(`malformed data URL at frame ${i}`);
    const png = Buffer.from(dataUrl.slice(comma + 1), "base64");
    const framePath = path.join(params.output_dir, `${String(i).padStart(4, "0")}.png`);
    await fs.writeFile(framePath, png);
    frames.push({
      index: i,
      timestamp_seconds: i / params.frame_hz,
      path: framePath,
      blake3: zeroHash,
    });
  }

  return {
    frames,
    duration_seconds: params.frame_count / params.frame_hz,
    actual_color_space: params.color_space,
    frame_hz_achieved: params.frame_hz,
  };
}
```

- [ ] **Step 1.3: Add `__renderSequence` to `renderer-host.html`**

Find the existing `__render` and `__animateRootTransform` globals in `adapters/three-vrm/src/renderer-host.html`. Add a sibling:

```javascript
window.__renderSequence = async function(p) {
  // Snapshot root translations for restoration after the loop.
  const rootBones = [];  // [{ node, originalTranslation }]
  for (const node of currentVrm.scene.children) {
    if (node.parent === currentVrm.scene) {
      rootBones.push({
        node,
        originalTranslation: node.position.clone(),
      });
    }
  }

  // Parse animate_root_transform (optional).
  const anim = p.animate_root_transform;
  const startV = anim ? anim.translation_start : [0, 0, 0];
  const endV = anim ? anim.translation_end : [0, 0, 0];

  const dataUrls = [];
  for (let i = 0; i < p.frame_count; i++) {
    const t = p.frame_count > 1 ? i / (p.frame_count - 1) : 0;
    const offset = [
      startV[0] + (endV[0] - startV[0]) * t,
      startV[1] + (endV[1] - startV[1]) * t,
      startV[2] + (endV[2] - startV[2]) * t,
    ];
    for (const { node, originalTranslation } of rootBones) {
      node.position.set(
        originalTranslation.x + offset[0],
        originalTranslation.y + offset[1],
        originalTranslation.z + offset[2],
      );
      node.updateMatrixWorld(true);
    }
    // Drive physics + render.
    if (currentVrm.springBoneManager) {
      currentVrm.springBoneManager.update(p.physics_dt_seconds);
    }
    const dataUrl = await window.__render({
      width: p.width,
      height: p.height,
      color_space: p.color_space,
    });
    dataUrls.push(dataUrl);
  }

  // Restore root translations.
  for (const { node, originalTranslation } of rootBones) {
    node.position.copy(originalTranslation);
    node.updateMatrixWorld(true);
  }

  return dataUrls;
};
```

Adapt to the actual three-vrm runtime references — `currentVrm` is likely the variable name the existing `__render` uses; `springBoneManager` exists on the `@pixiv/three-vrm` VRM object. Inspect the existing globals first.

- [ ] **Step 1.4: Flip the Unimplemented contract test**

Edit `adapters/three-vrm/test/contract.test.ts`:

Find the test asserting `render_sequence` returns `-32000` with `phase: "v1.x-sequence"` (added in Phase 1 Task 7, commit `227187c`). Replace its assertions with success-path checks:

```typescript
test("render_sequence with PngSequence + 2 frames produces frames on disk", async () => {
  // Setup harness — load a VRM via load_vrm + set_camera + set_lighting + set_post_processing
  // (mirror an existing success test if available).

  const outDir = await fs.mkdtemp(path.join(os.tmpdir(), "three-vrm-seq-"));
  const resp = await rpc(h, NEXT_ID, "render_sequence", {
    session_id: SESSION_ID,
    width: 64,
    height: 64,
    output_dir: outDir,
    frame_count: 2,
    frame_hz: 30.0,
    physics_dt_seconds: 1.0 / 60.0,
    color_space: "Linear",
    msaa: 1,
    output_type: "Color",
    output_format: "png_sequence",
  });

  assert.equal(resp.error, undefined, `unexpected error: ${JSON.stringify(resp.error)}`);
  const r = resp.result;
  assert.equal(r.frames.length, 2);
  for (let i = 0; i < 2; i++) {
    const f = r.frames[i];
    assert.equal(f.index, i);
    assert.ok(fs.existsSync(f.path), `frame ${i} not on disk: ${f.path}`);
    const size = fs.statSync(f.path).size;
    assert.ok(size > 100, `frame ${i} PNG too small (${size} bytes)`);
    assert.ok(f.blake3.startsWith("blake3:"));
  }
});
```

Adapt to the existing test file's harness (likely uses `rpc(h, id, method, params)` and a setup-by-load_vrm helper). If a sibling test already loads a VRM + sets camera/lighting, mirror its setup.

Also add 2-3 validation rejection tests (mutual exclusion, physics_dt floor, unsupported output_format). Mirror Phase 5 Task 3's Swift pattern.

- [ ] **Step 1.5: Build + test**

```
cd adapters/three-vrm && npm run build && npm test
```

All tests must pass.

- [ ] **Step 1.6: Commit**

```bash
git add adapters/three-vrm/
git commit -m "$(cat <<'EOF'
feat(adapters/three-vrm): implement render_sequence (PNG + root anim)

Node-side dispatch validates RFC-0004 rules then delegates to
BrowserSession.renderSequence, which calls the new __renderSequence
JS function. The browser-side loop interpolates root translation per
frame, drives physics via springBoneManager.update(dt), and returns
N PNG data URLs in order. Node writes them to <output_dir>/<i:04>.png.

BLAKE3 is populated with the 64-zero sentinel; runner re-hashes.
apply_vrma + non-PNG output_format + physics_dt > 1/60 all reject.

Promotes render_sequence out of PHASE_BY_RESERVED_METHOD; flips the
existing Unimplemented contract test to the success path.
EOF
)"
```

---

## Task 2: godot-vrm `render_sequence` implementation

**Files:** `adapters/godot-vrm/src/{session.gd, operations.gd}`, `crates/vrm-godot-shim/tests/contract.rs`.

- [ ] **Step 2.1: Add `render_sequence` to `session.gd`**

After `animate_root_transform` (around line 278), append:

```gdscript
# Phase 6 — RFC-0004 render_sequence.
# Loops frame_count times. Per frame: interpolate root translation,
# drive vrm_secondary.do_process(physics_dt), render via the existing
# viewport path, save PNG, append SequenceFrame.
#
# BLAKE3 is populated with the 64-zero sentinel — Rust runner re-hashes.
# apply_vrma is rejected (Phase 6 deferral); non-PNG output_format is
# rejected; physics_dt_seconds > 1/60 is rejected (60 Hz methodology pin).
func render_sequence(tree: SceneTree, params: Dictionary) -> Dictionary:
    if viewport == null:
        return _err(-32002, "RenderFailed", { "reason": "no session active; call load_vrm first" })

    var output_dir: String = params.get("output_dir", "")
    if output_dir == "":
        return _err(-32602, "missing output_dir")

    var frame_count: int = params.get("frame_count", 0)
    if frame_count < 1:
        return _err(-32602, "frame_count must be >= 1")

    var frame_hz: float = params.get("frame_hz", 30.0)
    var physics_dt: float = params.get("physics_dt_seconds", 1.0 / 60.0)
    if physics_dt > 1.0 / 60.0 + 1e-6:
        return _err(-32602, "physics_dt_seconds %f exceeds 60 Hz floor (1/60)" % physics_dt)

    var has_anim: bool = params.has("animate_root_transform") and params["animate_root_transform"] != null
    var has_vrma: bool = params.has("apply_vrma") and params["apply_vrma"] != null
    if has_anim and has_vrma:
        return _err(-32602, "animate_root_transform and apply_vrma are mutually exclusive")
    if has_vrma:
        return _err(-32602, "apply_vrma is not yet implemented in godot-vrm (Phase 6 deferral)")

    var output_format: String = params.get("output_format", "png_sequence")
    if output_format != "png_sequence":
        return _err(-32602, "output_format \"%s\" is not yet supported by godot-vrm; only png_sequence" % output_format)

    var width: int = params.get("width", 512)
    var height: int = params.get("height", 512)
    var msaa: int = params.get("msaa", 4)
    var color_space: String = params.get("color_space", "Srgb")

    # Configure viewport once.
    viewport.size = Vector2i(width, height)
    match msaa:
        0, 1: viewport.msaa_3d = Viewport.MSAA_DISABLED
        2:    viewport.msaa_3d = Viewport.MSAA_2X
        4:    viewport.msaa_3d = Viewport.MSAA_4X
        8:    viewport.msaa_3d = Viewport.MSAA_8X
        _:    viewport.msaa_3d = Viewport.MSAA_4X
    viewport.render_target_update_mode = SubViewport.UPDATE_WHEN_VISIBLE

    var start := Vector3.ZERO
    var end := Vector3.ZERO
    if has_anim:
        var anim: Dictionary = params["animate_root_transform"]
        var s = anim.get("translation_start", [0.0, 0.0, 0.0])
        var e = anim.get("translation_end", [0.0, 0.0, 0.0])
        start = Vector3(s[0], s[1], s[2])
        end = Vector3(e[0], e[1], e[2])

    # Ensure dir exists.
    DirAccess.make_dir_recursive_absolute(output_dir)

    var original_translation := scene.position
    var zero_hash := "blake3:" + "0".repeat(64)
    var frames: Array = []

    for i in frame_count:
        var t: float = float(i) / float(max(frame_count - 1, 1)) if frame_count > 1 else 0.0
        scene.position = original_translation + start.lerp(end, t)

        if vrm_secondary != null:
            vrm_secondary.do_process(physics_dt)

        # Let the viewport render the next frame.
        await tree.process_frame
        await tree.process_frame
        var img: Image = viewport.get_texture().get_image()
        if img == null:
            scene.position = original_translation
            return _err(-32002, "RenderFailed", { "reason": "frame %d: get_image returned null" % i })
        var frame_path := "%s/%04d.png" % [output_dir, i]
        var save_err := img.save_png(frame_path)
        if save_err != OK:
            scene.position = original_translation
            return _err(-32002, "RenderFailed", { "reason": "frame %d: save_png err %d" % [i, save_err] })

        frames.append({
            "index": i,
            "timestamp_seconds": float(i) / frame_hz,
            "path": frame_path,
            "blake3": zero_hash,
        })

    # Restore root.
    scene.position = original_translation

    var _declared = color_space  # we always write sRGB-encoded PNGs.
    return _ok({
        "frames": frames,
        "duration_seconds": float(frame_count) / frame_hz,
        "actual_color_space": "Srgb",
        "frame_hz_achieved": frame_hz,
    })
```

If `vrm_secondary` isn't always non-null (it's optional for non-spring-bone models), the `do_process` call should be skipped when null — the existing `step_physics` already handles this case.

- [ ] **Step 2.2: Wire dispatch in `operations.gd`**

Edit `adapters/godot-vrm/src/operations.gd`:
- Remove `"render_sequence": "v1.x-sequence",` from `PHASE_BY_RESERVED_METHOD`.
- Add `render_sequence` to `PHASE1_METHODS` (or wherever real-handler ops live in the dispatch).
- Add the dispatch arm in the match:
  ```gdscript
  "render_sequence":
      outcome = await session.render_sequence(tree, params if typeof(params) == TYPE_DICTIONARY else {})
  ```

- [ ] **Step 2.3: Update the contract test**

Edit `crates/vrm-godot-shim/tests/contract.rs`:

Find the `reserved_ops_still_return_unimplemented` test (line ~172). The `cases` array currently has a row for `render_sequence` asserting `-32000` with phase `v1.x-sequence`. Remove that row.

Optionally add a new test that exercises the success path against a real godot subprocess — but that's the e2e Task 3.x territory. For Task 2, just removing the row is sufficient.

- [ ] **Step 2.4: Build + test the shim**

```
cargo test -p vrm-godot-shim
```

Must pass. The `reserved_ops_still_return_unimplemented` test now expects one fewer reserved op.

- [ ] **Step 2.5: Commit**

```bash
git add adapters/godot-vrm/ crates/vrm-godot-shim/
git commit -m "$(cat <<'EOF'
feat(adapters/godot-vrm): implement render_sequence (PNG + root anim)

GDScript session.render_sequence loops frame_count times, per frame:
interpolates root translation, calls vrm_secondary.do_process(dt),
captures viewport texture to <output_dir>/<i:04>.png. Restores original
root after the loop.

BLAKE3 is populated with the 64-zero sentinel; runner re-hashes.
apply_vrma + non-PNG output_format + physics_dt > 1/60 all reject.

Promotes render_sequence out of PHASE_BY_RESERVED_METHOD; removes the
render_sequence row from the vrm-godot-shim contract test's reserved
list.
EOF
)"
```

---

## Task 3: E2E integration tests + workspace cleanup

**Files:**
- Create: `crates/vrm-runner/tests/render_sequence_e2e_three_vrm.rs`
- Create: `crates/vrm-runner/tests/render_sequence_e2e_godot_vrm.rs`

Both are `#[ignore]`-gated like the VMK E2E. Mirror `crates/vrm-runner/tests/render_sequence_e2e_vmk.rs` structurally:

- **three-vrm**: spawn `node dist/main.js` (or whatever the existing three-vrm adapter binary path is — check `adapters/three-vrm/package.json` `main`/`bin` and the `Adapter::spawn` invocations elsewhere). Skip with a clear error if `node` or Playwright Chromium isn't installed.
- **godot-vrm**: spawn the `vrm-godot-shim` binary (already in the cargo workspace, so `env!("CARGO_BIN_EXE_vrm-godot-shim")` works). Skip with a clear error if `godot` isn't on PATH.

Both tests should:
- Emit a small VRM via `emit_vrm`
- Build a default plan + inject `render_sequence: { frame_count: 2, frame_hz: 30, physics_dt: 1/60, output_format: PngSequence, animate_root_transform: [0,0,0]→[0.1,0,0] }`
- Drive `execute_plan` against the adapter
- Assert `SequenceStatus::Ok`, 2 frames on disk, BLAKE3 prefix + non-zero hash

- [ ] **Step 3.1: Add the two E2E tests**

- [ ] **Step 3.2: Run them with `--ignored`**

```
cd adapters/three-vrm && npm run build && cd ../..
cargo test -p vrm-runner --test render_sequence_e2e_three_vrm -- --ignored
cargo build -p vrm-godot-shim
cargo test -p vrm-runner --test render_sequence_e2e_godot_vrm -- --ignored
```

Both should pass given local prerequisites. If Playwright Chromium isn't installed, three-vrm test will fail with a clear error from the harness — not a Phase 6 bug.

- [ ] **Step 3.3: Workspace cleanup**

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd adapters/three-vrm && npm test && cd -
cd adapters/vrm-metal-kit && swift test && cd -
```

- [ ] **Step 3.4: Commit**

```bash
git add crates/vrm-runner/tests/
git commit -m "$(cat <<'EOF'
test(vrm-runner): render_sequence E2E against three-vrm + godot-vrm

Two #[ignore]-gated integration tests mirroring the VMK E2E from
Phase 5. Each drives the adapter through a 2-frame plan with
animate_root_transform, asserts the success path: real PNGs on disk
with runner-rehashed BLAKE3.

Locally exercises three-way coverage (VMK + three-vrm + godot-vrm);
CI without those toolchains skips cleanly.
EOF
)"
```

If fmt fixes are needed, separate commit per the established pattern.

---

## Phase 6 completion checklist

- [ ] three-vrm `render_sequence` no longer in `PHASE_BY_RESERVED_METHOD`; success-path test replaces Unimplemented test
- [ ] three-vrm `BrowserSession.renderSequence` + `__renderSequence` browser-side function
- [ ] godot-vrm `render_sequence` no longer in `PHASE_BY_RESERVED_METHOD`; shim contract test row removed
- [ ] godot-vrm `session.render_sequence` GDScript handler with viewport per-frame capture
- [ ] Both adapters validate mutual exclusion + 60Hz floor + apply_vrma reject + non-PNG output_format reject
- [ ] Both adapters restore original root translation after the loop
- [ ] Two new `#[ignore]`-gated E2E tests pass locally
- [ ] `cargo fmt --check` clean
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [ ] `cargo test --workspace` green
- [ ] three-vrm npm test green
- [ ] vmk swift test green

After Phase 6, three real renderers consume Phase 4's swing-seq corpus. Phase 7 (UniVRM PlayMode) is the last adapter — bundles with the deferred L4-PlayMode follow-up.

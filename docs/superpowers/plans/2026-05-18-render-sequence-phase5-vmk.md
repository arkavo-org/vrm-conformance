# `render_sequence` Phase 5 — vrm-metal-kit First Real Implementer

> **For agentic workers:** Use superpowers:subagent-driven-development. Phases 1–4 are landed (latest `35fedbe`). The mock renderer (Phase 3) is the deterministic reference; the swing-sequence corpus (Phase 4) is the input. Phase 5 makes VMK the **first real renderer** that consumes that corpus and produces real-pixel sequence outputs.

**Goal:** Implement `render_sequence` in the Swift VMK adapter (`adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift`) for PNG sequence output with `animate_root_transform` driving per-frame root translation. Promote `render_sequence` out of `reservedPhases`. Wire the runner to re-hash PNGs on receipt (so the BLAKE3 in `RenderSequenceResult` is trustworthy regardless of what the adapter returns).

**Out of scope for this phase (deferred to follow-ups):**

- `apply_vrma` driving — VRMA retarget per-frame depends on VMK's VRMAnimationLoader; defer until the corpus actually needs it (Phase 4 emitted only `animate_root_transform` plans).
- `ffmpeg` mux for MP4/MOV — orthogonal to the core render loop; can land separately.
- `bootstrap-goldens.sh` sequence path — touches shared infra (S3 push, manifest sequence entries); defer to Phase 5b once Phase 5a renders cleanly.
- `docs/findings.md` entry — depends on actually running the full corpus locally and seeing real numbers.

**Architecture:**

- Swift handler `handleRenderSequence` mirrors the existing `handleRender` shape (MSAA 4× color + depth + resolve → `drawOffscreenHeadless` → `writeTexturePng`) **per frame**, with `animate_root_transform` interpolation between captures.
- **BLAKE3 ownership shifts to the runner**: the adapter populates `blake3: "blake3:" + 64×"0"` as a sentinel; the runner re-hashes each frame's PNG bytes after `render_sequence` returns and overwrites the sentinel. This avoids adding a BLAKE3 Swift dep and centralizes hashing in one place (Rust, where the diff engine already uses it). The mock renderer's existing hashes get recomputed to the same value — BLAKE3 is deterministic.
- Validation: mutual exclusion of `animate_root_transform`/`apply_vrma`, 60Hz physics_dt floor. Same checks as the mock (Phase 3 Task 1).

**Tech stack:** Swift 6.3 / Xcode 26 (verified buildable on this host). No upstream VRMMetalKit revision bump.

**Spec:** [`rfcs/0004-render-sequence-op.md`](../../../rfcs/0004-render-sequence-op.md). The adapter contract from `docs/operation-contract.md` is the ground truth for params + result shapes.

---

## File structure

**Modify:**
- `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift` — promote `render_sequence` out of reservedPhases, add `handleRenderSequence`, wire dispatch
- `crates/vrm-runner/src/execute.rs` — re-hash sequence frames on receipt before populating `SequenceExecuteResult.result`
- `adapters/vrm-metal-kit/Tests/VRMMetalKitAdapterTests/` — extend the existing test file with sequence-validation tests (or add a new one)

**Create:**
- (none — all changes extend existing files)

---

## Task 1: Swift `handleRenderSequence` (PNG sequence + animate_root_transform)

**Files:**
- Modify: `adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift`

- [ ] **Step 1.1: Promote `render_sequence` out of `reservedPhases`**

Remove `"render_sequence": "v1.x-sequence",` from the `reservedPhases` map (around line 61). Update the doc comment above the map to drop the now-stale "VMK is the first real adapter" hint.

- [ ] **Step 1.2: Add the dispatch case**

In the dispatch switch (around line 147 — find the `case "render":` arm), add:

```swift
case "render_sequence":         return handleRenderSequence(params: params)
```

Place it after `case "animate_root_transform":` to keep related ops grouped.

- [ ] **Step 1.3: Add `handleRenderSequence`**

Append after `handleAnimateRootTransform` (around line 793). Sketch:

```swift
/// Multi-frame capture with optional root-transform animation.
///
/// Per RFC-0004: a sequence frame is rendered via the same MSAA 4× path
/// as the single-frame `render` op, but in a loop with the root
/// translation linearly interpolated between `animate_root_transform.
/// translation_start` and `translation_end` across `frame_count` frames.
/// Physics is driven implicitly by each frame's real render call
/// (VMK steps spring-bone physics during draw).
///
/// `apply_vrma` is rejected as invalid_params for now — VRMA retarget
/// per-frame is a follow-up.
///
/// `blake3` field on each frame is populated with a sentinel (64 zeros).
/// The runner re-hashes the on-disk PNG bytes after this method returns
/// so the SequenceFrame.blake3 carries the real hash before reaching
/// the manifest.
private func handleRenderSequence(params: JSONValue?) -> OpOutcome {
    guard case .object(let obj) = params,
          case .string(let sessionId) = obj["session_id"]
    else { return invalidParams("missing session_id") }

    guard case .number(let widthD) = obj["width"],
          let width = Int(exactly: widthD), width > 0,
          case .number(let heightD) = obj["height"],
          let height = Int(exactly: heightD), height > 0,
          case .string(let outputDir) = obj["output_dir"],
          case .number(let frameCountD) = obj["frame_count"],
          let frameCount = Int(exactly: frameCountD), frameCount > 0,
          case .number(let frameHz) = obj["frame_hz"],
          case .number(let physicsDt) = obj["physics_dt_seconds"],
          case .string(let colorSpace) = obj["color_space"],
          case .string(let outputFormat) = obj["output_format"]
    else { return invalidParams("missing or malformed required render_sequence fields") }

    // Validation per RFC-0004 failure-modes table.
    if physicsDt > 1.0 / 60.0 + 1e-9 {
        return invalidParams("physics_dt_seconds \(physicsDt) exceeds 60 Hz floor (1/60 ≈ 0.01667)")
    }
    let hasRootAnim = (obj["animate_root_transform"] != nil &&
                      !isJSONNull(obj["animate_root_transform"]))
    let hasVrma = (obj["apply_vrma"] != nil && !isJSONNull(obj["apply_vrma"]))
    if hasRootAnim && hasVrma {
        return invalidParams("animate_root_transform and apply_vrma are mutually exclusive")
    }
    if hasVrma {
        return invalidParams("apply_vrma is not yet implemented in vrm-metal-kit (Phase 5 deferral)")
    }

    // Output_format: PngSequence is the only path currently. Mp4/Mov mux
    // is deferred to a follow-up; reject explicitly so callers know.
    if outputFormat != "png_sequence" {
        return invalidParams("output_format \(outputFormat) is not yet supported by vrm-metal-kit; only png_sequence")
    }

    guard let session = lookupSession(sessionId) else {
        return invalidParams("unknown session_id: \(sessionId)")
    }
    guard let device = device, let commandQueue = commandQueue else {
        return renderFailed("no Metal device or command queue available")
    }

    // Parse optional animate_root_transform.
    var startV = SIMD3<Float>(0, 0, 0)
    var endV = SIMD3<Float>(0, 0, 0)
    if hasRootAnim, case .object(let anim) = obj["animate_root_transform"] {
        guard let s = parseVec3(anim["translation_start"]),
              let e = parseVec3(anim["translation_end"])
        else { return invalidParams("animate_root_transform: translation_start/end required") }
        startV = s
        endV = e
    }

    // Snapshot root translations for restoration after the loop.
    let rootNodes: [VRMNode] = session.model.nodes.filter { $0.parent == nil }
    let originalTranslations: [SIMD3<Float>] = rootNodes.map { $0.translation }

    // Camera setup — same as handleRender. Persist across frames so the
    // GPU pipelines don't churn.
    let position = session.cameraPosition ?? SIMD3<Float>(0, 1.4, 1.5)
    let target = session.cameraTarget ?? SIMD3<Float>(0, 1.4, 0)
    let up = session.cameraUp ?? SIMD3<Float>(0, 1, 0)
    let fov = session.cameraFovDegrees ?? 30.0
    let aspect = Float(width) / Float(height)
    MainActor.assumeIsolated {
        session.renderer.projectionMatrix = perspective(
            fovRadians: fov * .pi / 180.0,
            aspect: aspect,
            near: 0.01,
            far: 100.0
        )
        session.renderer.viewMatrix = lookAt(eye: position, center: target, up: up)
    }

    // Lighting — same as handleRender.
    if let dir = session.directionalDir,
       let color = session.directionalColor,
       let intensity = session.directionalIntensity {
        MainActor.assumeIsolated {
            session.renderer.setLight(0, direction: dir, color: color, intensity: intensity)
            session.renderer.disableLight(1)
            session.renderer.disableLight(2)
        }
    }
    if let ambColor = session.ambientColor, let ambIntensity = session.ambientIntensity {
        MainActor.assumeIsolated {
            session.renderer.setAmbientColor(ambColor * ambIntensity)
        }
    }

    // Ensure output_dir exists.
    let fm = FileManager.default
    do {
        try fm.createDirectory(atPath: outputDir, withIntermediateDirectories: true)
    } catch {
        return renderFailed("create output_dir \(outputDir): \(error)")
    }

    // Per-frame render targets are allocated inside the loop so resource
    // lifetime is bounded — Metal textures are reference-counted, but the
    // resolve target's .shared bytes are mapped until the texture
    // deallocates. Allocating per-frame keeps memory pressure flat.
    let colorPixelFormat: MTLPixelFormat =
        (colorSpace.lowercased() == "srgb") ? .rgba8Unorm_srgb : .rgba8Unorm
    let sampleCount = Operations.msaaSampleCount

    var frames: [JSONValue] = []
    let zeroHash = "blake3:" + String(repeating: "0", count: 64)

    for i in 0..<frameCount {
        // Interpolate root translation. For frameCount==1, t=0 (no animation).
        let t: Float = frameCount > 1 ? Float(i) / Float(frameCount - 1) : 0
        let offset = startV + (endV - startV) * t
        for (idx, root) in rootNodes.enumerated() {
            root.translation = originalTranslations[idx] + offset
            MainActor.assumeIsolated {
                root.updateWorldTransform()
            }
        }

        // Build per-frame MSAA targets (same shape as handleRender).
        let msColorDesc = MTLTextureDescriptor()
        msColorDesc.textureType = .type2DMultisample
        msColorDesc.pixelFormat = colorPixelFormat
        msColorDesc.width = width
        msColorDesc.height = height
        msColorDesc.sampleCount = sampleCount
        msColorDesc.usage = [.renderTarget]
        msColorDesc.storageMode = .private
        guard let msColorTex = device.makeTexture(descriptor: msColorDesc) else {
            return renderFailed("frame \(i): failed to create MS color texture")
        }

        let resolveDesc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: colorPixelFormat,
            width: width, height: height, mipmapped: false
        )
        resolveDesc.usage = [.renderTarget, .shaderRead]
        resolveDesc.storageMode = .shared
        guard let resolveTex = device.makeTexture(descriptor: resolveDesc) else {
            return renderFailed("frame \(i): failed to create resolve texture")
        }

        let depthDesc = MTLTextureDescriptor()
        depthDesc.textureType = .type2DMultisample
        depthDesc.pixelFormat = .depth32Float
        depthDesc.width = width
        depthDesc.height = height
        depthDesc.sampleCount = sampleCount
        depthDesc.usage = [.renderTarget]
        depthDesc.storageMode = .private
        guard let msDepthTex = device.makeTexture(descriptor: depthDesc) else {
            return renderFailed("frame \(i): failed to create MS depth texture")
        }

        let rpd = MTLRenderPassDescriptor()
        rpd.colorAttachments[0].texture = msColorTex
        rpd.colorAttachments[0].resolveTexture = resolveTex
        rpd.colorAttachments[0].loadAction = .clear
        rpd.colorAttachments[0].storeAction = .multisampleResolve
        rpd.colorAttachments[0].clearColor = MTLClearColor(red: 1.0, green: 0.0, blue: 1.0, alpha: 1.0)
        rpd.depthAttachment.texture = msDepthTex
        rpd.depthAttachment.loadAction = .clear
        rpd.depthAttachment.storeAction = .dontCare
        rpd.depthAttachment.clearDepth = 1.0

        guard let commandBuffer = commandQueue.makeCommandBuffer() else {
            return renderFailed("frame \(i): failed to make command buffer")
        }

        MainActor.assumeIsolated {
            session.renderer.drawOffscreenHeadless(
                to: msColorTex,
                depth: msDepthTex,
                commandBuffer: commandBuffer,
                renderPassDescriptor: rpd
            )
        }
        let sem = DispatchSemaphore(value: 0)
        commandBuffer.addCompletedHandler { _ in sem.signal() }
        commandBuffer.commit()
        sem.wait()
        if let err = commandBuffer.error {
            return renderFailed("frame \(i): GPU error: \(err)")
        }

        // Export PNG.
        let framePath = "\(outputDir)/\(String(format: "%04d", i)).png"
        do {
            try writeTexturePng(resolveTex, to: framePath)
        } catch {
            return renderFailed("frame \(i): PNG export failed: \(error)")
        }

        frames.append(.object([
            "index": .number(Double(i)),
            "timestamp_seconds": .number(Double(i) / Double(frameHz)),
            "path": .string(framePath),
            "blake3": .string(zeroHash),
        ]))
    }

    // Restore original root translations so subsequent ops don't see drift.
    for (idx, root) in rootNodes.enumerated() {
        root.translation = originalTranslations[idx]
        MainActor.assumeIsolated {
            root.updateWorldTransform()
        }
    }

    let durationSeconds = frameHz > 0 ? Double(frameCount) / frameHz : 0.0

    return .ok(.object([
        "frames": .array(frames),
        "duration_seconds": .number(durationSeconds),
        "actual_color_space": .string(colorSpace),
        "frame_hz_achieved": .number(frameHz),
    ]))
}

/// Tiny helper — JSONValue's `.null` check is internal; expose locally.
private func isJSONNull(_ v: JSONValue?) -> Bool {
    if case .null = v { return true }
    return v == nil
}
```

Notes for the implementer:
- `JSONValue` is the adapter's local JSON-like enum. Check its variant names (`.object`, `.string`, `.number`, `.array`, `.null`) — the snippet assumes these but they may differ slightly.
- `MainActor.assumeIsolated` is the existing pattern for `drawOffscreenHeadless` — preserved.
- If `isJSONNull` is already defined, drop the local helper.
- The Mp4/Mov rejection is explicit so callers fail loud rather than silently producing PNG-only output.

- [ ] **Step 1.4: Build + cursory verify**

```
cd adapters/vrm-metal-kit && swift build
```

Must complete with "Build complete!" — no errors.

- [ ] **Step 1.5: Commit**

```bash
git add adapters/vrm-metal-kit/Sources/VRMMetalKitAdapter/Operations.swift
git commit -m "$(cat <<'EOF'
feat(adapters/vrm-metal-kit): implement render_sequence (PNG + root anim)

handleRenderSequence loops frame_count times, each iteration rendering
through the same MSAA 4× path as handleRender after applying the
animate_root_transform interpolation. Physics is driven implicitly by
each frame's draw call (VMK steps spring-bone during render).

BLAKE3 is populated with a 64-zero sentinel — the runner re-hashes
on receipt so the manifest gets real hashes. This avoids adding a
BLAKE3 Swift dep; hashing centralizes in Rust where the diff engine
already uses blake3.

apply_vrma is rejected (deferred); output_format mp4/mov is rejected
(deferred); physics_dt_seconds > 1/60 and animate_root_transform +
apply_vrma both set are rejected per RFC-0004.

Promotes render_sequence out of reservedPhases.
EOF
)"
```

## Reporting

Standard format. Include commit SHA. The build verification is the primary signal at this step — full runtime verification happens in Task 4.

---

## Task 2: Runner re-hashes sequence frames on receipt

**Files:**
- Modify: `crates/vrm-runner/src/execute.rs`

- [ ] **Step 2.1: Add the re-hash pass**

In the sequence-mode branch of `execute_plan` (Phase 2 Task 10 — around the `SequenceStatus::Ok` arm where `RenderSequenceResult` is captured), iterate the frames and recompute BLAKE3 from on-disk PNG bytes. Replace whatever blake3 the adapter returned with the recomputed value. Place the re-hash AFTER the result is captured but BEFORE wrapping it in `SequenceExecuteResult`.

Sketch:

```rust
// Re-hash frames from on-disk PNG bytes. Adapter-returned blake3 is
// treated as advisory only — the runner is the authority for what
// lands in the manifest. This handles the case where an adapter
// (e.g., vrm-metal-kit) hasn't yet added a BLAKE3 Swift dep and
// returns a placeholder hash.
fn rehash_frames(result: &mut ops::RenderSequenceResult) -> Result<(), String> {
    for frame in result.frames.iter_mut() {
        let bytes = std::fs::read(&frame.path)
            .map_err(|e| format!("re-hash {}: {e}", frame.path))?;
        let hash = blake3::hash(&bytes);
        frame.blake3 = format!("blake3:{}", hash.to_hex());
    }
    Ok(())
}
```

Add `blake3.workspace = true` to `crates/vrm-runner/Cargo.toml` `[dependencies]` if absent — verify with `grep blake3 crates/vrm-runner/Cargo.toml`.

Invoke `rehash_frames` in the Ok arm of the sequence dispatch:

```rust
Ok(mut r) => {
    if let Err(e) = rehash_frames(&mut r) {
        return SequenceExecuteResult {
            status: SequenceStatus::Error,
            result: None,
            unimplemented_phase: None,
            error_message: Some(format!("frame re-hash failed: {e}")),
        };
    }
    SequenceExecuteResult {
        status: SequenceStatus::Ok,
        result: Some(r),
        unimplemented_phase: None,
        error_message: None,
    }
}
```

(Adapt the exact match-arm shape to the actual code — Phase 2 Task 10's commit `555e1b1` introduced it.)

- [ ] **Step 2.2: Update the existing E2E test if needed**

`crates/vrm-runner/tests/render_sequence_e2e_mock.rs` asserts the blake3 prefix + length on each frame. Since the runner re-hashes, this assertion still holds (the recomputed hash is identical to what the mock returned). No assertion changes expected.

If a test specifically asserted the adapter's returned hash vs the re-hashed value, adjust the assertion.

- [ ] **Step 2.3: Build + test + clippy**

```
cargo test -p vrm-runner --test render_sequence_e2e_mock
cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 2.4: Commit**

```bash
git add crates/vrm-runner/src/execute.rs crates/vrm-runner/Cargo.toml
git commit -m "$(cat <<'EOF'
feat(vrm-runner): re-hash sequence frames on receipt

The runner is the authority for the blake3 column in the manifest, not
the adapter. Re-hash each frame's PNG bytes after render_sequence
returns and overwrite whatever the adapter reported. This lets
adapters return placeholder hashes (e.g., vrm-metal-kit's 64-zero
sentinel) without poisoning the manifest.

Mock adapter's hashes are unchanged because BLAKE3 is deterministic —
the re-hash produces the same value.
EOF
)"
```

---

## Task 3: Swift validation tests

**Files:**
- Modify: `adapters/vrm-metal-kit/Tests/VRMMetalKitAdapterTests/` — find the existing test file via `ls adapters/vrm-metal-kit/Tests/VRMMetalKitAdapterTests/`; if there's an `OperationsTests.swift` or similar, extend it.

- [ ] **Step 3.1: Add validation tests**

Cover the three rejection paths:

```swift
func testRenderSequenceRejectsMutualExclusion() {
    let ops = Operations()
    let params: JSONValue = .object([
        "session_id": .string("nonexistent"),  // validation happens before session lookup
        "width": .number(64), "height": .number(64),
        "output_dir": .string("/tmp/x"),
        "frame_count": .number(1),
        "frame_hz": .number(30.0),
        "physics_dt_seconds": .number(1.0 / 60.0),
        "color_space": .string("linear"),
        "msaa": .number(1),
        "output_type": .string("color"),
        "output_format": .string("png_sequence"),
        "animate_root_transform": .object([
            "translation_start": .array([.number(0), .number(0), .number(0)]),
            "translation_end": .array([.number(1), .number(0), .number(0)]),
        ]),
        "apply_vrma": .object([
            "vrma_handle": .number(1),
            "start_seconds": .number(0),
        ]),
    ])
    let outcome = ops.dispatch(method: "render_sequence", params: params)
    // Expect invalidParams (-32602)
    // Match against the OpOutcome shape used by the existing tests.
}

func testRenderSequenceRejectsPhysicsDtAbove60Hz() {
    // similar, with physics_dt_seconds = 0.1
}

func testRenderSequenceRejectsUnsupportedOutputFormat() {
    // similar, with output_format = "mp4"
}
```

Adapt to the existing tests' style — read whichever test file exists in `adapters/vrm-metal-kit/Tests/` first and mirror its conventions (XCTest vs Swift Testing, OpOutcome enum matching, error code extraction).

- [ ] **Step 3.2: Run Swift tests**

```
cd adapters/vrm-metal-kit && swift test
```

All three new tests must pass. Existing tests must continue to pass.

- [ ] **Step 3.3: Commit**

```bash
git add adapters/vrm-metal-kit/Tests/
git commit -m "$(cat <<'EOF'
test(adapters/vrm-metal-kit): render_sequence validation rejections

Three rejection-path tests for the new render_sequence handler:
animate_root_transform + apply_vrma mutually exclusive,
physics_dt_seconds > 1/60 over the methodology floor, and unsupported
output_format (mp4/mov deferred to a follow-up).
EOF
)"
```

---

## Task 4: Runner E2E against VMK with a swing-seq plan

**Files:**
- Create: `crates/vrm-runner/tests/render_sequence_e2e_vmk.rs`

- [ ] **Step 4.1: Add an integration test**

Drive the VMK adapter through a swing-sequence plan and assert the runner produces a non-empty `RenderSequenceResult` with on-disk PNG frames and runner-recomputed BLAKE3 hashes.

The test should:
- Build the VMK adapter binary (mirror `mock_bin()` from `render_sequence_e2e_mock.rs` but pointing at `target/debug/vrm-metal-kit-adapter` after invoking `swift build` if it doesn't exist)
- Emit a 1-2 frame sequence plan (small for speed) via the asset generator's emit functions, OR construct one inline
- Run `execute_plan` against the VMK binary
- Assert `result.sequence.status == SequenceStatus::Ok`
- Assert each frame's PNG exists on disk and has non-trivial size (> 100 bytes)
- Assert each frame's blake3 is a real (non-zero) hash

**Mark the test `#[ignore]`** because it requires Xcode 26 + macOS 26 platform availability AND a `swift build` step that's not free. CI without those won't run it; local dev does.

```rust
//! End-to-end sequence dispatch test against the VMK adapter.
//! Ignored by default because it requires Xcode 26 / macOS 26 + swift build.
//! Run with: cargo test -p vrm-runner --test render_sequence_e2e_vmk -- --ignored

// ... harness ...

#[test]
#[ignore = "requires Xcode 26 + macOS 26 + swift build"]
fn vmk_render_sequence_with_animate_root_transform_produces_frames() {
    // ...
}
```

The detailed test scaffolding follows the existing `render_sequence_e2e_mock.rs` pattern. Adapt the asset emission (use `emit_with_sidecars_spring_bone_swing_sequence` or build inline) and the adapter bin lookup.

- [ ] **Step 4.2: Run + commit**

```bash
cd adapters/vrm-metal-kit && swift build
cd ../..
cargo test -p vrm-runner --test render_sequence_e2e_vmk -- --ignored
```

Test must pass when run with `--ignored`. (Skip the assertion if the test environment can't build VMK — note that case in the report.)

```bash
git add crates/vrm-runner/tests/render_sequence_e2e_vmk.rs
git commit -m "$(cat <<'EOF'
test(vrm-runner): render_sequence end-to-end against vrm-metal-kit

Drives a 1-frame swing-seq plan through the VMK adapter and asserts
the full pipeline produces a real PNG with a non-zero BLAKE3 hash.

Ignored by default because the test requires Xcode 26 + macOS 26 +
swift build. Local dev exercises it; CI without those skips cleanly.
EOF
)"
```

---

## Task 5: Workspace cleanup

- [ ] **Step 5.1: fmt + clippy + workspace test + npm test**

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd adapters/three-vrm && npm test && cd -
cd adapters/vrm-metal-kit && swift build && cd ../..
```

- [ ] **Step 5.2: Commit any fmt fixes**

---

## Phase 5 completion checklist

- [ ] `render_sequence` no longer in `reservedPhases` map
- [ ] `handleRenderSequence` implements PNG-sequence output with MSAA 4× per frame
- [ ] `animate_root_transform` interpolation works frame-by-frame
- [ ] `apply_vrma` rejected as invalid_params (deferred)
- [ ] `output_format != png_sequence` rejected (deferred)
- [ ] `physics_dt_seconds > 1/60` rejected
- [ ] Mutual-exclusion of `animate_root_transform` + `apply_vrma` rejected
- [ ] Original root translations restored after the loop
- [ ] Runner re-hashes frames on receipt; manifest blake3 is trustworthy
- [ ] Swift validation tests pass
- [ ] Runner E2E against VMK (ignored by default) produces real PNGs + non-zero hashes
- [ ] `swift build` clean
- [ ] `cargo fmt --check` clean
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [ ] `cargo test --workspace` green
- [ ] three-vrm npm test green

After Phase 5, the swing-seq corpus has its first real renderer. Phase 6 (three-vrm + godot-vrm) follows naturally — same shape, different engine idioms. Phase 5b (bootstrap-goldens sequence path, manifest sequence entries, findings.md update) can land independently when desired.

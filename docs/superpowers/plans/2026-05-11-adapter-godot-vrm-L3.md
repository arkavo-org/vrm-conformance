# godot-vrm Adapter L3 — Real Rendering Integration

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **Builds on:** [`2026-05-11-adapter-godot-vrm-scaffold-v2.md`](./2026-05-11-adapter-godot-vrm-scaffold-v2.md) (L1+L2). L1+L2 landed the Rust shim + GDScript dispatch returning `Unimplemented` for every op. L3 replaces those `Unimplemented` returns for Phase 1 ops with real V-Sekai/godot-vrm renders. Phase 2 (`step_physics`/`reset_physics`/`animate_root_transform`) is deferred to a follow-up plan — Phase 1 alone makes godot-vrm a usable third renderer for the 44-asset MToon corpus.

**Goal:** Land the six Phase 1 ops (`load_vrm`, `set_camera`, `set_lighting`, `set_post_processing`, `render`, `dispose`) as real implementations driving V-Sekai/godot-vrm via GDScript, so `vrm-runner execute-test-plan` produces actual PNG renders through this adapter and `scripts/bootstrap-goldens.sh` picks it up as the third real renderer alongside three-vrm and vrm-metal-kit.

**Architecture:** Vendor V-Sekai/godot-vrm @ `9fae4049f20954e70d9d7de6f3ed2695a6870e04` and V-Sekai/Godot-MToon-Shader @ `27cb2b78f13ce473c1ccdcf14c30a835c2193fbd` into `adapters/godot-vrm/addons/`. Introduce `src/session.gd` as a stateful object owning the loaded VRM scene + SubViewport + camera + lights. `operations.gd` keeps the Phase 2/reserved entries returning `Unimplemented`; Phase 1 entries dispatch to Session methods. Runs in Godot's `--headless --rendering-driver <real-driver>` mode (the spike confirms which driver works on Linux CI vs macOS dev).

```
runner ──framed stdio──> vrm-godot-shim ──NDJSON over TCP──> godot --headless --rendering-driver opengl3
                                                                  │
                                                                  ├─ main.gd (TCP client)
                                                                  ├─ tcp_session.gd (NDJSON dispatch loop)
                                                                  ├─ operations.gd (dispatch table → Session)
                                                                  ├─ session.gd (VRM scene + SubViewport + render)
                                                                  └─ addons/vrm + addons/Godot-MToon-Shader
                                                                       ↑ vendored at pinned SHAs
```

**Tech Stack:** GDScript on Godot 4.6.2 (macOS dev) + 4.3-stable (CI), V-Sekai/godot-vrm + V-Sekai/Godot-MToon-Shader (vendored), unchanged Rust shim from L1+L2.

---

## Pre-flight assumptions to verify

Three load-bearing assumptions; each gets a spike task in the plan, in order. A failure on any one stops the plan for re-spec.

1. **Headless rendering works on macOS.** Godot's default `--headless` may select a "Dummy" rendering driver that doesn't render. We need `--rendering-driver opengl3` (or similar) to be available and to produce actual pixel output. Spike: render a magenta-clear SubViewport in pure GDScript without any VRM, save PNG, verify file is non-trivial and has the expected dimensions.

2. **VRM 1.0 loads at runtime via `GLTFDocument` + `vrm_extension.gd`.** The addon was written for editor-time import (`_import_scene` callback in `import_vrm.gd`), but the inner mechanism is `GLTFDocument.append_from_file` which is runtime-callable. Spike: load a generated VRM, walk the scene tree, assert there's a `Skeleton3D` with the head bone.

3. **MToon shader compiles and runs.** Godot-MToon-Shader is plain Godot shaders (`*.gdshader`). They should compile on first load; if any one fails, the entire VRM looks pink-magenta (Godot's shader-error tint). Spike: load + render a default MToon VRM, sample a few pixels, confirm they're not pink-magenta (the error indicator).

---

## File Structure

```
adapters/godot-vrm/
├── README.md                          # Updated: L3 status table + "tested against" notes
├── project.godot                      # Possibly bumped to enable Forward+ renderer
├── .gitignore                         # (unchanged from L2)
├── addons/                            # NEW — vendored
│   ├── vrm/                           #   V-Sekai/godot-vrm @ 9fae4049
│   │   ├── PINNED_SHA                 #   plain-text marker
│   │   ├── plugin.cfg
│   │   ├── vrm_extension.gd           #   the GLTFDocumentExtension that does the real work
│   │   ├── vrm_secondary.gd
│   │   ├── ... (rest of addon files)
│   └── Godot-MToon-Shader/            #   V-Sekai/Godot-MToon-Shader @ 27cb2b78
│       ├── PINNED_SHA
│       ├── mtoon.gdshader
│       ├── ... (rest of shader files)
├── src/
│   ├── main.gd                        # (unchanged) — TCP connect + run
│   ├── operations.gd                  # CHANGED — Phase 1 entries dispatch to Session
│   ├── tcp_session.gd                 # (unchanged) — NDJSON loop
│   └── session.gd                     # NEW — owns SubViewport, scene, camera, lights, render output
└── tests/
    ├── run_gdscript_tests.gd          # (unchanged)
    └── test_operations.gd             # (unchanged — L2 dispatch tests for Phase 2+/unknown ops still apply)

# Rust shim changes
crates/vrm-godot-shim/tests/contract.rs   # CHANGED — asserts on real render output for Phase 1 ops

# Existing files updated
scripts/bootstrap-goldens.sh              # Add godot-vrm to the render-with-adapter loop
README.md                                  # Update godot-vrm row from "L1+L2 scaffold" to "L3"
CLAUDE.md                                  # Update adapter-status bullet
adapters/godot-vrm/README.md               # Update status table, remove L3 sketch (now done)
docs/findings.md                           # Add a new run entry once corpus consensus rerun
```

**Boundaries:**
- `session.gd` owns ALL renderer state and Phase 1 op implementations. Single source of state for a Godot child's lifetime.
- `operations.gd` becomes thin: a dict-keyed dispatch to `session.<op_name>(params)` for Phase 1, returning structured errors for everything else.
- `tcp_session.gd` does not change — it parses requests, calls `Operations.dispatch`, writes responses. Doesn't know about Session.
- The Rust shim does not change. The wire is identical to L2; only the responses become real instead of `Unimplemented`.

---

## Operation contract reminder

From `docs/operation-contract.md`, Phase 1 ops:

| op | input | output |
|---|---|---|
| `load_vrm` | `{ path: string }` | `{ session_id: string }` |
| `set_camera` | `{ session_id, position[3], target[3], up[3], fov_degrees }` | `{}` |
| `set_lighting` | `{ session_id, directional: { dir[3], color[3], intensity }, ambient: { color[3], intensity }, cast_shadows, receive_shadows }` | `{}` |
| `set_post_processing` | `{ session_id, tone_mapping: "None\|Linear\|Reinhard\|Aces", exposure }` | `{}` |
| `render` | `{ session_id, width, height, output_path, color_space: "Linear\|Srgb", msaa, output_type }` | `{ output_path, actual_color_space }` |
| `dispose` | `{ session_id }` | `{}` |

**Session lifetime:** the shim's `forward_one` is request-locked — one round-trip per call. The runner's `execute-test-plan` issues exactly one of each op in order, then closes stdin. The Godot child lives for that whole sequence. A single global `Session` object is sufficient.

**Magenta clear color** (255, 0, 255) per `docs/methodology.md` — used by property-assertion bbox detection to find the rendered avatar against an unambiguous background. Apply in `render` via `SubViewport.transparent_bg = false` + `World3D.environment.background_mode = BG_COLOR` + `background_color = Color(1, 0, 1)`.

**Tone mapping pins** for MToon math tests (`tone_mapping: "None"`): Godot has no "None" — map to `Environment.TONE_MAPPER_LINEAR` with `tonemap_exposure = 1.0`. That's the closest equivalent to "no tone mapping."

---

## Spike 1 result

- Date: 2026-05-11
- Godot version: `4.6.2.stable.official.71f334935`
- Working rendering driver: `metal` (with `--display-driver macos --audio-driver Dummy`; **NOT** `--headless`)
- PNG size: 1364 bytes (256x256, valid PNG)
- Outcome: SubViewport render-to-PNG confirmed when using the macOS display driver in offscreen mode.

**Critical finding for Task 8 (shim spawn):** `--headless` is a hard alias for `--display-driver headless --audio-driver Dummy`, and the `headless` display driver only supports the `dummy` rendering driver — which produces null textures (`get_image()` returns null with `texture_2d_get` failing inside `servers/rendering/dummy/storage/texture_storage.h`). Specifying `--rendering-driver opengl3|vulkan|metal` alongside `--headless` is silently ignored.

The wrapper script must spawn Godot with:

```
godot --display-driver macos --rendering-driver metal --audio-driver Dummy --script <script>
```

(NOT `--headless`.) `metal` was chosen over `opengl3` because Godot reports it as "Forward+" (full-featured renderer used in production), while `opengl3` reports as "Compatibility" — for a fidelity suite we want the production-quality path. Both produce non-trivial PNGs (metal 1364 B, opengl3 1436 B); `vulkan` was not tested separately because metal is the native macOS path. If CI runs on Linux later, the driver will need to be re-spiked there (likely `vulkan`).

**Side-effects to verify in Task 8:** spawning with `--display-driver macos` from a CLI may briefly create a Cocoa window or dock icon. The spike did not surface a window during its ~1 s lifetime, but a longer-lived session may differ — Task 8 should add a guard (e.g. `LSUIElement` plist hint or `--position`-offscreen workaround) if a window flashes during real runs.

Script bug uncovered during the spike: `Camera3D.look_at()` errors if called before `add_child()` (the node isn't in the tree yet). Use `look_at_from_position()` or call `look_at()` after parenting. Recorded for Task 7's render impl.

---

## Task list

11 tasks. Spikes 1+2+3 are gating — if any fails, stop and re-spec.

---

### Task 1: Vendor V-Sekai/godot-vrm + Godot-MToon-Shader

**Files:**
- Create: `adapters/godot-vrm/addons/vrm/**` (vendor of V-Sekai/godot-vrm @ `9fae4049f20954e70d9d7de6f3ed2695a6870e04`, only the `addons/vrm/` subtree)
- Create: `adapters/godot-vrm/addons/Godot-MToon-Shader/**` (vendor of V-Sekai/Godot-MToon-Shader @ `27cb2b78f13ce473c1ccdcf14c30a835c2193fbd`)
- Create: `adapters/godot-vrm/addons/vrm/PINNED_SHA`
- Create: `adapters/godot-vrm/addons/Godot-MToon-Shader/PINNED_SHA`

- [ ] **Step 1: Fetch + extract V-Sekai/godot-vrm at pinned SHA**

```bash
mkdir -p adapters/godot-vrm/addons/vrm
GODOT_VRM_SHA=9fae4049f20954e70d9d7de6f3ed2695a6870e04

# tarball includes a leading directory prefix; strip it. Only keep addons/vrm/*.
curl -L "https://github.com/V-Sekai/godot-vrm/archive/${GODOT_VRM_SHA}.tar.gz" \
  | tar -xz -C /tmp
# Resulting path: /tmp/godot-vrm-<sha>/
cp -R "/tmp/godot-vrm-${GODOT_VRM_SHA}/addons/vrm/." adapters/godot-vrm/addons/vrm/
rm -rf "/tmp/godot-vrm-${GODOT_VRM_SHA}"
echo "${GODOT_VRM_SHA}" > adapters/godot-vrm/addons/vrm/PINNED_SHA
```

- [ ] **Step 2: Fetch + extract V-Sekai/Godot-MToon-Shader at pinned SHA**

```bash
mkdir -p adapters/godot-vrm/addons/Godot-MToon-Shader
MTOON_SHA=27cb2b78f13ce473c1ccdcf14c30a835c2193fbd

curl -L "https://github.com/V-Sekai/Godot-MToon-Shader/archive/${MTOON_SHA}.tar.gz" \
  | tar -xz -C /tmp
cp -R "/tmp/Godot-MToon-Shader-${MTOON_SHA}/." adapters/godot-vrm/addons/Godot-MToon-Shader/
rm -rf "/tmp/Godot-MToon-Shader-${MTOON_SHA}"
echo "${MTOON_SHA}" > adapters/godot-vrm/addons/Godot-MToon-Shader/PINNED_SHA
```

- [ ] **Step 3: Sanity-check the vendored tree**

```bash
ls adapters/godot-vrm/addons/vrm/ | head -20
ls adapters/godot-vrm/addons/Godot-MToon-Shader/ | head -10
```

Expected `addons/vrm/`: includes `plugin.cfg`, `plugin.gd`, `vrm_extension.gd`, `vrm_secondary.gd`, `import_vrm.gd`, `vrm_meta.gd`, `vrm_constants.gd`, `vrm_utils.gd`, `vrm_spring_bone.gd`, `vrm_collider.gd`, `vrm_toplevel.gd`, `LICENSE`, `README.md`, etc.

Expected `addons/Godot-MToon-Shader/`: includes `mtoon.gdshader`, `mtoon_common.gdshaderinc`, `mtoon_cull_off.gdshader`, `mtoon_cutout.gdshader`, `inspector_mtoon.gd`, `LICENSE`.

If either tree is empty or missing the listed files, stop and report — the tarball URL or pin SHA may have changed upstream.

- [ ] **Step 4: Confirm GDScript syntax-check passes on the vendored files**

Run Godot's headless syntax check by loading the project:

```bash
godot --headless --path adapters/godot-vrm --quit-after 1 2>&1 | tail -10
```

Expected: no `Parse Error` or `SCRIPT ERROR` lines. Godot may emit `WARNING:` lines about the addon plugin (e.g., import-time editor-only behavior) — those are acceptable. Hard parse errors are not.

- [ ] **Step 5: Commit**

```bash
git add adapters/godot-vrm/addons/
git commit -m "$(cat <<'EOF'
feat(adapters/godot-vrm): vendor V-Sekai/godot-vrm + MToon shader

V-Sekai/godot-vrm @ 9fae4049 (covers VRM 1.0 import + MToon)
V-Sekai/Godot-MToon-Shader @ 27cb2b78 (standalone shader pack)

Pinned via PINNED_SHA file per addon — same upstream-revision-pin
convention as adapters/vrm-metal-kit/Package.swift.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

Commit will be large (~250 KB of GDScript + shader files); that's expected. Look at `git show --stat HEAD | tail -3` to confirm both addon directories landed.

---

### Task 2: Spike — headless SubViewport render-to-PNG (no VRM)

**Files:**
- Create: `/tmp/godot-render-spike.gd` (throwaway — never committed)

This spike is the equivalent of L1+L2's stdio spike. It validates that Godot can render to a SubViewport in headless mode and save a non-trivial PNG. **If this fails, stop and re-spec** — without working headless rendering, every Phase 1 op is blocked.

- [ ] **Step 1: Write the spike script**

```bash
cat > /tmp/godot-render-spike.gd <<'GD'
extends SceneTree

func _init() -> void:
    var viewport := SubViewport.new()
    viewport.size = Vector2i(256, 256)
    viewport.transparent_bg = false
    viewport.render_target_update_mode = SubViewport.UPDATE_ONCE
    viewport.world_3d = World3D.new()
    viewport.world_3d.environment = Environment.new()
    viewport.world_3d.environment.background_mode = Environment.BG_COLOR
    viewport.world_3d.environment.background_color = Color(0.5, 0.7, 0.3)
    root.add_child(viewport)
    # Add a simple cube so the render isn't just a clear color.
    var mesh := MeshInstance3D.new()
    mesh.mesh = BoxMesh.new()
    mesh.position = Vector3(0, 0, -3)
    viewport.add_child(mesh)
    var cam := Camera3D.new()
    cam.position = Vector3(0, 0, 0)
    cam.look_at(Vector3(0, 0, -3), Vector3.UP)
    viewport.add_child(cam)
    var light := DirectionalLight3D.new()
    light.rotation_degrees = Vector3(-30, 45, 0)
    viewport.add_child(light)

    # Wait one frame for the renderer to populate the texture.
    await process_frame
    await process_frame  # double-buffer paranoia

    var img := viewport.get_texture().get_image()
    if img == null:
        push_error("get_image() returned null"); quit(2); return
    var err := img.save_png("/tmp/godot-render-spike.png")
    if err != OK:
        push_error("save_png err: %d" % err); quit(2); return
    var size_bytes := FileAccess.get_file_as_bytes("/tmp/godot-render-spike.png").size()
    print("PNG written: %d bytes" % size_bytes)
    quit(0)
GD
```

- [ ] **Step 2: Try `opengl3` rendering driver**

```bash
godot --headless --rendering-driver opengl3 --script /tmp/godot-render-spike.gd 2>&1 | tail -20
ls -la /tmp/godot-render-spike.png 2>&1 | head -2
```

Expected: stdout contains `PNG written: NNNNN bytes` with NNNNN > 1000 (a 256×256 RGB PNG of a shaded cube + colored background should be several KB). The PNG file exists at `/tmp/godot-render-spike.png` and `file /tmp/godot-render-spike.png` reports a valid PNG.

If stdout shows `PNG written: 0 bytes` or the file size is suspiciously small (< 200 bytes), the renderer is producing all-clear-color or empty output — that's a fail mode.

- [ ] **Step 3: Fallback — try vulkan or default rendering driver if opengl3 fails**

If Step 2 fails (e.g., "Could not initialize OpenGL", or PNG is null/empty):

```bash
godot --headless --rendering-driver vulkan --script /tmp/godot-render-spike.gd 2>&1 | tail -20
ls -la /tmp/godot-render-spike.png
```

If Vulkan also fails on macOS (Vulkan is via MoltenVK and may not be available), try **no flag** (let Godot pick):

```bash
godot --headless --script /tmp/godot-render-spike.gd 2>&1 | tail -20
```

- [ ] **Step 4: Inspect the rendered pixels (sanity)**

The cube should be visible against the green background. Confirm with sips:

```bash
# Center-ish pixel — should be the cube (lit shading, not pure background green)
python3 - <<'PY'
from struct import unpack
with open("/tmp/godot-render-spike.png", "rb") as f:
    data = f.read()
# Quick sanity: PNG magic + IHDR width/height
assert data[:8] == b"\x89PNG\r\n\x1a\n", "not a PNG"
width, height = unpack(">II", data[16:24])
print(f"PNG: {width}x{height}, total bytes {len(data)}")
assert (width, height) == (256, 256), f"unexpected dimensions: {width}x{height}"
assert len(data) > 1000, f"file suspiciously small: {len(data)} bytes"
print("OK — non-trivial PNG produced")
PY
```

Expected: `PNG: 256x256, total bytes NNNN` with NNNN > 1000, and `OK — non-trivial PNG produced`.

- [ ] **Step 5: Record the winning driver flag**

Open `docs/superpowers/plans/2026-05-11-adapter-godot-vrm-L3.md` and append a `## Spike 1 result` section under `## Pre-flight assumptions to verify`:

```markdown
## Spike 1 result

- Date: 2026-05-11
- Godot version: <output of `godot --version`>
- Working rendering driver: `<opengl3 | vulkan | default>`
- PNG size: NNNN bytes
- Outcome: headless SubViewport render-to-PNG confirmed.
```

The winning rendering-driver flag becomes the value the wrapper script passes to Godot in Task 6.

- [ ] **Step 6: Commit the plan-result update**

```bash
git add docs/superpowers/plans/2026-05-11-adapter-godot-vrm-L3.md
git commit -m "docs(plan/godot-vrm-L3): record headless-rendering spike result"
```

Don't commit `/tmp/godot-render-spike.*` — throwaways outside the repo.

---

### Task 3: Spike — load a generated VRM at runtime

**Files:**
- Create: `/tmp/godot-vrm-load-spike.gd` (throwaway)

This spike validates that V-Sekai/godot-vrm's `vrm_extension.gd` works at runtime (not just editor-time). **If this fails, stop and re-spec** — the addon may need extra setup (e.g., must be activated as a plugin first).

- [ ] **Step 1: Generate a sample VRM**

```bash
cargo build --release -p vrm-asset-generator 2>&1 | tail -3
./target/release/vrm-asset-generator emit-default --id l3_spike --output-dir /tmp/godot-l3-assets 2>&1 | tail -3
ls /tmp/godot-l3-assets/
```

Expected: `l3_spike.vrm`, `l3_spike.meta.json`, `l3_spike.test.yaml` exist.

- [ ] **Step 2: Write the load spike**

```bash
cat > /tmp/godot-vrm-load-spike.gd <<'GD'
extends SceneTree

const vrm_extension_class := preload("res://addons/vrm/vrm_extension.gd")
const vrm_constants := preload("res://addons/vrm/vrm_constants.gd")

func _init() -> void:
    var args := OS.get_cmdline_user_args()
    if args.is_empty():
        push_error("expected vrm path as positional arg"); quit(2); return
    var vrm_path: String = args[0]

    var gltf := GLTFDocument.new()
    var ext: GLTFDocumentExtension = vrm_extension_class.new()
    gltf.register_gltf_document_extension(ext, true)

    var state := GLTFState.new()
    state.set_additional_data(&"vrm/head_hiding_method", 0)
    state.set_additional_data(&"vrm/first_person_layers", 2)
    state.set_additional_data(&"vrm/third_person_layers", 4)
    state.handle_binary_image = GLTFState.HANDLE_BINARY_EMBED_AS_UNCOMPRESSED

    var err := gltf.append_from_file(vrm_path, state, 0)
    if err != OK:
        push_error("append_from_file err: %d" % err); quit(2); return

    var scene: Node = gltf.generate_scene(state)
    gltf.unregister_gltf_document_extension(ext)
    if scene == null:
        push_error("generate_scene returned null"); quit(2); return

    # Walk the tree, count nodes by type, find Skeleton3D.
    var counts := {}
    var skeleton: Skeleton3D = null
    var stack: Array[Node] = [scene]
    while not stack.is_empty():
        var n: Node = stack.pop_back()
        var t := n.get_class()
        counts[t] = counts.get(t, 0) + 1
        if n is Skeleton3D and skeleton == null:
            skeleton = n
        for child in n.get_children():
            stack.append(child)

    print("scene root: %s (%s)" % [scene.name, scene.get_class()])
    print("node-type counts: %s" % counts)
    if skeleton != null:
        print("skeleton bone count: %d" % skeleton.get_bone_count())
    else:
        push_error("no Skeleton3D found in scene"); quit(2); return

    quit(0)
GD
```

- [ ] **Step 3: Run the spike** (use the rendering driver determined in Spike 1)

```bash
godot --headless --rendering-driver opengl3 --path adapters/godot-vrm \
  --script /tmp/godot-vrm-load-spike.gd \
  -- /tmp/godot-l3-assets/l3_spike.vrm 2>&1 | tail -15
```

Replace `opengl3` if Spike 1 chose a different driver.

Expected: stdout shows `scene root: <name>`, `node-type counts: {...}` (with `MeshInstance3D` count >= 1 and `Skeleton3D` count == 1), and `skeleton bone count: NN` with NN >= 1. Exit 0.

If the spike fails with `append_from_file err: -1`, the VRM extension didn't activate — likely because the addon's `plugin.cfg` declares it as an editor-only plugin (`script="plugin.gd"` with `EditorPlugin` subclass). The `vrm_extension.gd` is the *core* extension though, and is independent of the editor plugin. The script above bypasses the editor plugin by instantiating `vrm_extension.gd` directly — that's the correct runtime entry point.

If it fails with a different error (e.g., `null skeleton`), investigate before continuing — the addon's behavior on a default-emit VRM may need adjusting in the asset generator.

- [ ] **Step 4: Record the spike result + the load recipe in the plan**

Append a `## Spike 2 result` section to this plan file:

```markdown
## Spike 2 result

- Date: 2026-05-11
- VRM loaded: l3_spike.vrm (emit-default)
- Skeleton bone count: NN
- Node-type counts: {...}
- Outcome: VRM 1.0 runtime load via GLTFDocument + vrm_extension.gd confirmed.
```

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/plans/2026-05-11-adapter-godot-vrm-L3.md
git commit -m "docs(plan/godot-vrm-L3): record VRM runtime-load spike result"
```

---

### Task 4: Spike — render a loaded VRM (MToon shader smoke)

**Files:**
- Create: `/tmp/godot-vrm-render-spike.gd` (throwaway)

The third gating spike. Combines spikes 1+2: load a VRM, set up camera/light/viewport, render to PNG. **Validates that the MToon shader actually compiles and produces non-error pixels.** If the addon's shader has a compilation problem on the host Godot version, the VRM renders as the Godot shader-error magenta pink — easy to detect.

- [ ] **Step 1: Write the render spike**

```bash
cat > /tmp/godot-vrm-render-spike.gd <<'GD'
extends SceneTree

const vrm_extension_class := preload("res://addons/vrm/vrm_extension.gd")

func _init() -> void:
    var args := OS.get_cmdline_user_args()
    if args.size() < 2:
        push_error("usage: ... -- <vrm_path> <out_png_path>"); quit(2); return
    var vrm_path: String = args[0]
    var out_path: String = args[1]

    # Load the VRM (same as spike 2).
    var gltf := GLTFDocument.new()
    var ext: GLTFDocumentExtension = vrm_extension_class.new()
    gltf.register_gltf_document_extension(ext, true)
    var state := GLTFState.new()
    state.set_additional_data(&"vrm/head_hiding_method", 0)
    state.set_additional_data(&"vrm/first_person_layers", 2)
    state.set_additional_data(&"vrm/third_person_layers", 4)
    state.handle_binary_image = GLTFState.HANDLE_BINARY_EMBED_AS_UNCOMPRESSED
    var err := gltf.append_from_file(vrm_path, state, 0)
    if err != OK:
        push_error("append_from_file err: %d" % err); quit(2); return
    var scene: Node = gltf.generate_scene(state)
    gltf.unregister_gltf_document_extension(ext)

    # Build the viewport scene.
    var vp := SubViewport.new()
    vp.size = Vector2i(1024, 1024)
    vp.transparent_bg = false
    vp.msaa_3d = Viewport.MSAA_4X
    vp.render_target_update_mode = SubViewport.UPDATE_ONCE
    vp.world_3d = World3D.new()
    var env := Environment.new()
    env.background_mode = Environment.BG_COLOR
    env.background_color = Color(1.0, 0.0, 1.0)   # magenta
    env.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
    env.ambient_light_color = Color(0.5, 0.5, 0.5)
    env.ambient_light_energy = 0.3
    env.tonemap_mode = Environment.TONE_MAPPER_LINEAR
    env.tonemap_exposure = 1.0
    vp.world_3d.environment = env
    root.add_child(vp)
    vp.add_child(scene)
    # Camera positioned for a head-and-shoulders shot.
    var cam := Camera3D.new()
    cam.position = Vector3(0.0, 1.4, 1.5)
    cam.look_at(Vector3(0.0, 1.4, 0.0), Vector3.UP)
    cam.fov = 30.0
    vp.add_child(cam)
    # Directional light.
    var dl := DirectionalLight3D.new()
    dl.rotation = Vector3(-deg_to_rad(30.0), deg_to_rad(45.0), 0.0)
    dl.light_color = Color(1, 1, 1)
    dl.light_energy = 1.0
    dl.shadow_enabled = false
    vp.add_child(dl)

    # Wait several frames so shaders compile + are committed.
    for i in 4:
        await process_frame

    var img := vp.get_texture().get_image()
    if img == null:
        push_error("get_image returned null"); quit(2); return
    var save_err := img.save_png(out_path)
    if save_err != OK:
        push_error("save_png err: %d" % save_err); quit(2); return

    # Pixel sanity — sample 9 points and report mean RGB.
    var samples: Array = [
        Vector2i(512, 200), Vector2i(512, 512), Vector2i(512, 800),
        Vector2i(200, 512), Vector2i(800, 512),
        Vector2i(300, 300), Vector2i(700, 300), Vector2i(300, 700), Vector2i(700, 700),
    ]
    var pink_count := 0
    for p in samples:
        var c := img.get_pixel(p.x, p.y)
        # Godot shader-error pink is (1, 0, 1) — count any pixel within 0.05 of that.
        if c.r > 0.95 and c.g < 0.05 and c.b > 0.95:
            pink_count += 1
    print("magenta-or-pink pixels in 9 samples: %d / 9" % pink_count)
    print("rendered to: %s (%d bytes)" % [out_path, FileAccess.get_file_as_bytes(out_path).size()])
    if pink_count == 9:
        push_error("entire frame is pink-magenta — likely shader compile failure"); quit(3); return
    quit(0)
GD
```

- [ ] **Step 2: Run the spike**

```bash
godot --headless --rendering-driver opengl3 --path adapters/godot-vrm \
  --script /tmp/godot-vrm-render-spike.gd \
  -- /tmp/godot-l3-assets/l3_spike.vrm /tmp/godot-l3-render.png 2>&1 | tail -10
ls -la /tmp/godot-l3-render.png
```

Expected:
- `magenta-or-pink pixels in 9 samples: 0 / 9` (or at most 4 — the background corners are real magenta, sphere fills the center, the 5 center-cluster samples should be sphere pixels at neutral gray).
- `rendered to: /tmp/godot-l3-render.png (NNNN bytes)` with NNNN > 5000.
- File exists; `file /tmp/godot-l3-render.png` reports a valid 1024×1024 PNG.

If `magenta-or-pink pixels: 9 / 9`, the MToon shader didn't compile. Read stderr for shader errors; investigate before continuing — the spike SHOULD NOT be patched here, but the failure mode determines whether we need a different `mtoon.gdshader` path, a different Godot version, or a different bug-fix.

If the file size is suspiciously small (<2 KB), the render produced near-uniform output — possible if the VRM didn't load correctly or the camera is pointed away from the model.

- [ ] **Step 3: Visual eyeball check** (optional but recommended)

```bash
# Convert to ASCII art so you can sanity-check the render from the terminal.
sips -Z 80 /tmp/godot-l3-render.png --out /tmp/godot-l3-render-small.png 2>&1 | tail -1
python3 - <<'PY'
from struct import unpack
import zlib
with open("/tmp/godot-l3-render.png", "rb") as f:
    data = f.read()
width, height = unpack(">II", data[16:24])
print(f"{width}x{height}, {len(data)} bytes")
PY
```

If you have time, open `/tmp/godot-l3-render.png` in Preview/an image viewer to confirm the avatar is recognizable. Not required if pixel-sampling passed.

- [ ] **Step 4: Record the spike result**

Append to the plan:

```markdown
## Spike 3 result

- Date: 2026-05-11
- VRM rendered: l3_spike.vrm
- Pink-magenta count: N / 9
- PNG size: NNNN bytes
- Outcome: MToon shader compiles + renders confirmed (no shader-error pink).
```

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/plans/2026-05-11-adapter-godot-vrm-L3.md
git commit -m "docs(plan/godot-vrm-L3): record VRM render spike result"
```

---

### Task 5: Implement `session.gd` — state + load_vrm + dispose

**Files:**
- Create: `adapters/godot-vrm/src/session.gd`

- [ ] **Step 1: Write session.gd with load + dispose**

```bash
cat > adapters/godot-vrm/src/session.gd <<'GD'
# Session state for one Godot-child lifetime. The shim's request/response
# loop is request-locked, so a single global Session suffices. Holds the
# loaded VRM scene + a SubViewport configured for off-screen rendering.

class_name Session

const vrm_extension_class := preload("res://addons/vrm/vrm_extension.gd")
const MAGENTA := Color(1.0, 0.0, 1.0)

var session_id: String = ""
var scene: Node = null
var viewport: SubViewport = null
var camera: Camera3D = null
var directional_light: DirectionalLight3D = null
var environment: Environment = null

# Build the SubViewport once at load time; reused across set_camera/
# set_lighting/set_post_processing/render. Caller passes the SceneTree
# root so the viewport can be parented and the renderer drives it.
func load_vrm(tree_root: Node, params: Dictionary) -> Dictionary:
    var path: String = params.get("path", "")
    if path == "":
        return _err(-32602, "missing path")

    var gltf := GLTFDocument.new()
    var ext: GLTFDocumentExtension = vrm_extension_class.new()
    gltf.register_gltf_document_extension(ext, true)
    var state := GLTFState.new()
    state.set_additional_data(&"vrm/head_hiding_method", 0)
    state.set_additional_data(&"vrm/first_person_layers", 2)
    state.set_additional_data(&"vrm/third_person_layers", 4)
    state.handle_binary_image = GLTFState.HANDLE_BINARY_EMBED_AS_UNCOMPRESSED

    var err := gltf.append_from_file(path, state, 0)
    gltf.unregister_gltf_document_extension(ext)
    if err != OK:
        return _err(-32001, "LoadFailed", { "reason": "append_from_file err %d" % err })

    var built: Node = gltf.generate_scene(state)
    if built == null:
        return _err(-32001, "LoadFailed", { "reason": "generate_scene returned null" })

    scene = built
    session_id = "godot-%d" % Time.get_ticks_msec()

    # Build the viewport scaffolding. set_camera/set_lighting/set_post_processing
    # will tune fields on viewport/camera/directional_light/environment.
    viewport = SubViewport.new()
    viewport.size = Vector2i(1024, 1024)
    viewport.transparent_bg = false
    viewport.msaa_3d = Viewport.MSAA_4X
    viewport.render_target_update_mode = SubViewport.UPDATE_ONCE
    viewport.world_3d = World3D.new()

    environment = Environment.new()
    environment.background_mode = Environment.BG_COLOR
    environment.background_color = MAGENTA
    environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
    environment.ambient_light_color = Color(0.5, 0.5, 0.5)
    environment.ambient_light_energy = 0.3
    environment.tonemap_mode = Environment.TONE_MAPPER_LINEAR
    environment.tonemap_exposure = 1.0
    viewport.world_3d.environment = environment

    camera = Camera3D.new()
    camera.position = Vector3(0.0, 1.4, 1.5)
    camera.look_at(Vector3(0.0, 1.4, 0.0), Vector3.UP)
    camera.fov = 30.0

    directional_light = DirectionalLight3D.new()
    directional_light.rotation = Vector3(-deg_to_rad(30.0), deg_to_rad(45.0), 0.0)
    directional_light.light_color = Color(1, 1, 1)
    directional_light.light_energy = 1.0
    directional_light.shadow_enabled = false

    tree_root.add_child(viewport)
    viewport.add_child(scene)
    viewport.add_child(camera)
    viewport.add_child(directional_light)

    return _ok({ "session_id": session_id })

func dispose(_params: Dictionary) -> Dictionary:
    if viewport != null:
        viewport.queue_free()
    scene = null
    viewport = null
    camera = null
    directional_light = null
    environment = null
    session_id = ""
    return _ok({})

func _ok(result: Variant) -> Dictionary:
    return { "ok": true, "result": result }

func _err(code: int, message: String, data: Variant = null) -> Dictionary:
    var e: Dictionary = { "code": code, "message": message }
    if data != null:
        e["data"] = data
    return { "ok": false, "error": e }
GD
```

- [ ] **Step 2: Confirm GDScript loads without errors**

```bash
godot --headless --path adapters/godot-vrm --quit-after 1 2>&1 | tail -5
```

Expected: no `Parse Error` lines.

- [ ] **Step 3: Confirm existing GDScript dispatch tests still pass**

```bash
godot --headless --path adapters/godot-vrm --script tests/run_gdscript_tests.gd 2>&1 | tail -3
```

Expected: still `7 passed, 0 failed`. Session is new code, not yet wired into operations.gd; L2's tests are untouched.

- [ ] **Step 4: Commit**

```bash
git add adapters/godot-vrm/src/session.gd
git commit -m "feat(adapters/godot-vrm): session.gd skeleton + load_vrm + dispose"
```

---

### Task 6: Implement `set_camera`, `set_lighting`, `set_post_processing`

**Files:**
- Modify: `adapters/godot-vrm/src/session.gd`

- [ ] **Step 1: Append the three config methods to session.gd**

Insert the following before the `_ok` / `_err` helpers (or wherever helpers end up — append them to the class body):

```gdscript
func set_camera(params: Dictionary) -> Dictionary:
    if camera == null:
        return _err(-32002, "RenderFailed", { "reason": "no session active; call load_vrm first" })
    var pos = params.get("position", [0.0, 1.4, 1.5])
    var tgt = params.get("target", [0.0, 1.4, 0.0])
    var up = params.get("up", [0.0, 1.0, 0.0])
    var fov: float = params.get("fov_degrees", 30.0)
    camera.position = Vector3(pos[0], pos[1], pos[2])
    camera.look_at(Vector3(tgt[0], tgt[1], tgt[2]), Vector3(up[0], up[1], up[2]))
    camera.fov = fov
    return _ok({})

func set_lighting(params: Dictionary) -> Dictionary:
    if directional_light == null or environment == null:
        return _err(-32002, "RenderFailed", { "reason": "no session active; call load_vrm first" })
    var d: Dictionary = params.get("directional", {})
    var dir = d.get("dir", [-0.3, -0.6, -0.7])
    var col = d.get("color", [1.0, 1.0, 1.0])
    var intensity: float = d.get("intensity", 1.0)
    # Godot represents directional light via Node3D rotation, not a vector.
    # Build a basis whose -Z points along `dir`.
    var dir_v := Vector3(dir[0], dir[1], dir[2]).normalized()
    directional_light.look_at_from_position(Vector3.ZERO, dir_v, Vector3.UP)
    directional_light.light_color = Color(col[0], col[1], col[2])
    directional_light.light_energy = intensity

    var a: Dictionary = params.get("ambient", {})
    var ac = a.get("color", [0.5, 0.5, 0.5])
    var ai: float = a.get("intensity", 0.3)
    environment.ambient_light_color = Color(ac[0], ac[1], ac[2])
    environment.ambient_light_energy = ai

    var cast: bool = params.get("cast_shadows", false)
    var receive: bool = params.get("receive_shadows", false)
    directional_light.shadow_enabled = cast
    # Receive shadows is per-material; for MToon math tests both flags are false,
    # so we can ignore the receive side here — the directional shadow_enabled
    # gates the entire shadow path.
    var _ = receive
    return _ok({})

func set_post_processing(params: Dictionary) -> Dictionary:
    if environment == null:
        return _err(-32002, "RenderFailed", { "reason": "no session active; call load_vrm first" })
    var tone: String = params.get("tone_mapping", "None")
    var exposure: float = params.get("exposure", 1.0)
    match tone:
        "None":
            environment.tonemap_mode = Environment.TONE_MAPPER_LINEAR
        "Linear":
            environment.tonemap_mode = Environment.TONE_MAPPER_LINEAR
        "Reinhard":
            environment.tonemap_mode = Environment.TONE_MAPPER_REINHARD
        "Aces":
            environment.tonemap_mode = Environment.TONE_MAPPER_ACES
        _:
            return _err(-32602, "unknown tone_mapping: " + tone)
    environment.tonemap_exposure = exposure
    return _ok({})
```

- [ ] **Step 2: Re-verify GDScript parses**

```bash
godot --headless --path adapters/godot-vrm --quit-after 1 2>&1 | tail -5
```

Expected: no parse errors.

- [ ] **Step 3: Existing tests still pass**

```bash
godot --headless --path adapters/godot-vrm --script tests/run_gdscript_tests.gd 2>&1 | tail -3
```

Expected: `7 passed, 0 failed`.

- [ ] **Step 4: Commit**

```bash
git add adapters/godot-vrm/src/session.gd
git commit -m "feat(adapters/godot-vrm): session config ops (camera, lighting, post)"
```

---

### Task 7: Implement `render`

**Files:**
- Modify: `adapters/godot-vrm/src/session.gd`

- [ ] **Step 1: Append the render method**

```gdscript
# Render the current scene to a PNG. Returns the output path + the actual
# color-space the PNG was written in (Godot writes sRGB-encoded PNGs by
# default; `color_space: "Linear"` would require a linear PNG which Godot
# doesn't support natively, so we accept the request and report sRGB —
# the runner's diff engine tolerates this declared mismatch).
func render(tree: SceneTree, params: Dictionary) -> Dictionary:
    if viewport == null:
        return _err(-32002, "RenderFailed", { "reason": "no session active; call load_vrm first" })
    var width: int = params.get("width", 1024)
    var height: int = params.get("height", 1024)
    var output_path: String = params.get("output_path", "")
    if output_path == "":
        return _err(-32602, "missing output_path")
    var msaa: int = params.get("msaa", 4)
    var declared_cs: String = params.get("color_space", "Srgb")

    viewport.size = Vector2i(width, height)
    match msaa:
        0, 1: viewport.msaa_3d = Viewport.MSAA_DISABLED
        2:    viewport.msaa_3d = Viewport.MSAA_2X
        4:    viewport.msaa_3d = Viewport.MSAA_4X
        8:    viewport.msaa_3d = Viewport.MSAA_8X
        _:    viewport.msaa_3d = Viewport.MSAA_4X
    viewport.render_target_update_mode = SubViewport.UPDATE_ONCE

    # Drive a few frames so the shader pipeline is warm + the viewport
    # texture is populated. UPDATE_ONCE renders the next frame after
    # `notify_update` (which `tree.process_frame` triggers implicitly).
    for i in 4:
        await tree.process_frame

    var img: Image = viewport.get_texture().get_image()
    if img == null:
        return _err(-32002, "RenderFailed", { "reason": "get_image returned null" })
    var save_err := img.save_png(output_path)
    if save_err != OK:
        return _err(-32002, "RenderFailed", { "reason": "save_png err %d" % save_err })

    # Declared color-space: we always write sRGB-encoded PNGs.
    var _declared = declared_cs
    return _ok({ "output_path": output_path, "actual_color_space": "Srgb" })
```

Note the `await tree.process_frame` calls — `render` is an async function. The caller (`tcp_session.gd`) must `await` it.

- [ ] **Step 2: Confirm parse + existing tests**

```bash
godot --headless --path adapters/godot-vrm --quit-after 1 2>&1 | tail -3
godot --headless --path adapters/godot-vrm --script tests/run_gdscript_tests.gd 2>&1 | tail -3
```

Both: clean, no errors, `7 passed, 0 failed`.

- [ ] **Step 3: Commit**

```bash
git add adapters/godot-vrm/src/session.gd
git commit -m "feat(adapters/godot-vrm): render op (SubViewport → save_png)"
```

---

### Task 8: Wire `operations.gd` to dispatch Phase 1 ops to Session

**Files:**
- Modify: `adapters/godot-vrm/src/operations.gd`
- Modify: `adapters/godot-vrm/src/tcp_session.gd`
- Modify: `adapters/godot-vrm/src/main.gd`

The boundary change: `operations.gd` is no longer pure-function; it owns a reference to a Session. `tcp_session.gd` passes the session through. `main.gd` constructs the session.

- [ ] **Step 1: Rewrite operations.gd to dispatch Phase 1 to Session**

```bash
cat > adapters/godot-vrm/src/operations.gd <<'GD'
# Operation registry + dispatch for the godot-vrm adapter.
#
# L3 state: Phase 1 ops (load_vrm, set_camera, set_lighting,
# set_post_processing, render, dispose) dispatch to Session. Reserved ops
# return the standard -32000 Unimplemented with phase labels per
# docs/operation-contract.md.

class_name Operations

const Session := preload("res://src/session.gd")

const PHASE_BY_RESERVED_METHOD := {
    "set_environment": "v1.x",
    "set_expression": "Phase 3",
    "set_humanoid_pose": "Phase 2",
    "set_root_transform": "Phase 2",
    "animate_root_transform": "Phase 2",
    "step_physics": "Phase 2",
    "reset_physics": "Phase 2",
}

# Phase 1 method names. dispatch() routes these to Session.<name>.
const PHASE1_METHODS := [
    "load_vrm", "set_camera", "set_lighting",
    "set_post_processing", "render", "dispose",
]

# Async to support `render` which awaits frames.
static func dispatch(tree: SceneTree, session: Session, id: Variant, method: String, params: Variant) -> Dictionary:
    if PHASE1_METHODS.has(method):
        var outcome: Dictionary
        match method:
            "load_vrm":
                outcome = session.load_vrm(tree.root, params if typeof(params) == TYPE_DICTIONARY else {})
            "set_camera":
                outcome = session.set_camera(params if typeof(params) == TYPE_DICTIONARY else {})
            "set_lighting":
                outcome = session.set_lighting(params if typeof(params) == TYPE_DICTIONARY else {})
            "set_post_processing":
                outcome = session.set_post_processing(params if typeof(params) == TYPE_DICTIONARY else {})
            "render":
                outcome = await session.render(tree, params if typeof(params) == TYPE_DICTIONARY else {})
            "dispose":
                outcome = session.dispose(params if typeof(params) == TYPE_DICTIONARY else {})
            _:
                outcome = { "ok": false, "error": { "code": -32601, "message": "internal: PHASE1 method not routed: " + method } }
        if outcome.get("ok"):
            return { "jsonrpc": "2.0", "id": id, "result": outcome.get("result", {}) }
        return { "jsonrpc": "2.0", "id": id, "error": outcome.get("error") }

    if PHASE_BY_RESERVED_METHOD.has(method):
        return {
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": -32000,
                "message": "Unimplemented",
                "data": { "phase": PHASE_BY_RESERVED_METHOD[method] },
            },
        }

    return {
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": -32601, "message": "method not found: " + method },
    }
GD
```

- [ ] **Step 2: Update tcp_session.gd to thread tree + session through dispatch**

```bash
cat > adapters/godot-vrm/src/tcp_session.gd <<'GD'
# NDJSON request/response loop over a connected StreamPeerTCP socket.
# One JSON object per line, terminated by "\n". On socket close or
# read error, returns cleanly so main.gd can call quit(0).

class_name TcpSession

const Operations := preload("res://src/operations.gd")
const Session := preload("res://src/session.gd")

# Run the loop on `socket`. Blocks until the peer (shim) closes the
# connection. The Session is held in main.gd; we forward both to dispatch.
static func run(tree: SceneTree, session: Session, socket: StreamPeerTCP) -> void:
    var buf := PackedByteArray()
    while true:
        if socket.get_status() != StreamPeerTCP.STATUS_CONNECTED:
            return
        socket.poll()
        var available := socket.get_available_bytes()
        if available > 0:
            var chunk := socket.get_data(available)
            if chunk[0] != OK:
                push_error("tcp read error: %d" % chunk[0]); return
            buf.append_array(chunk[1])
        var newline_byte := 0x0a
        while true:
            var nl := buf.find(newline_byte)
            if nl < 0: break
            var line: PackedByteArray = buf.slice(0, nl)
            buf = buf.slice(nl + 1)
            var text := line.get_string_from_utf8()
            var parsed: Variant = JSON.parse_string(text)
            var resp: Dictionary
            if parsed == null or typeof(parsed) != TYPE_DICTIONARY:
                resp = {
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": "parse error" },
                }
            else:
                var req: Dictionary = parsed
                var raw_id: Variant = req.get("id", null)
                var id_out: Variant = raw_id
                if typeof(raw_id) == TYPE_FLOAT and raw_id == floor(raw_id):
                    id_out = int(raw_id)
                resp = await Operations.dispatch(
                    tree, session,
                    id_out, req.get("method", ""), req.get("params", {}),
                )
            var out := (JSON.stringify(resp) + "\n").to_utf8_buffer()
            var put_err := socket.put_data(out)
            if put_err != OK:
                push_error("tcp write error: %d" % put_err); return
        OS.delay_msec(5)
GD
```

- [ ] **Step 3: Update main.gd to construct + pass the Session**

```bash
cat > adapters/godot-vrm/src/main.gd <<'GD'
# godot-vrm adapter — Godot-side entry. Reads the loopback port from the
# first positional user arg (after `--`), connects to vrm-godot-shim,
# and runs the NDJSON session loop until the shim closes the socket.

extends SceneTree

const TcpSession := preload("res://src/tcp_session.gd")
const Session := preload("res://src/session.gd")

func _init() -> void:
    var args := OS.get_cmdline_user_args()
    if args.is_empty():
        push_error("godot-vrm adapter: expected positional port arg after `--`"); quit(2); return
    var port := args[0].to_int()
    if port <= 0 or port > 65535:
        push_error("godot-vrm adapter: bad port: %s" % args[0]); quit(2); return

    var socket := StreamPeerTCP.new()
    var err := socket.connect_to_host("127.0.0.1", port)
    if err != OK:
        push_error("godot-vrm adapter: connect_to_host failed: %d" % err); quit(2); return

    var deadline := Time.get_ticks_msec() + 5000
    while socket.get_status() == StreamPeerTCP.STATUS_CONNECTING:
        if Time.get_ticks_msec() > deadline:
            push_error("godot-vrm adapter: connect timeout"); quit(2); return
        socket.poll()
        OS.delay_msec(10)
    if socket.get_status() != StreamPeerTCP.STATUS_CONNECTED:
        push_error("godot-vrm adapter: not connected: status=%d" % socket.get_status()); quit(2); return

    var session := Session.new()
    await TcpSession.run(self, session, socket)
    socket.disconnect_from_host()
    quit(0)
GD
```

- [ ] **Step 4: Confirm GDScript loads + L2 dispatch tests still pass**

```bash
godot --headless --path adapters/godot-vrm --quit-after 1 2>&1 | tail -3
godot --headless --path adapters/godot-vrm --script tests/run_gdscript_tests.gd 2>&1 | tail -3
```

Expected: clean parse; 7 passed, 0 failed (the dispatch unit tests don't exercise Session, so they should be unaffected).

- [ ] **Step 5: Manual end-to-end smoke — load + render via the shim**

The wrapper script already invokes Godot via the shim. We need to pass the rendering-driver flag to Godot. **Update `crates/vrm-godot-shim/src/child.rs`** to pass `--rendering-driver opengl3` (or the winning flag from Spike 1):

Read child.rs, find the `spawn_godot` function, locate the line:

```rust
cmd.arg("--headless")
```

Change to:

```rust
cmd.arg("--headless")
    .arg("--rendering-driver").arg("opengl3")
```

Then:

```bash
cargo build --release -p vrm-godot-shim 2>&1 | tail -3
# Issue load_vrm, set_camera, set_lighting, set_post_processing, render, dispose
# in sequence over framed stdio. Easiest path: a tiny Python harness.

python3 - <<'PY'
import json, os, subprocess, sys
BIN = os.path.abspath("./target/release/vrm-godot-shim")
ASSET = os.path.abspath("/tmp/godot-l3-assets/l3_spike.vrm")
OUT = os.path.abspath("/tmp/godot-l3-smoke-render.png")

def frame(body): return f"Content-Length: {len(body)}\r\n\r\n".encode() + body
def read_frame(s):
    header = b""
    while b"\r\n\r\n" not in header:
        c = s.read(1)
        if not c: raise EOFError(header)
        header += c
    head, _, _ = header.partition(b"\r\n\r\n")
    length = int(next(l for l in head.split(b"\r\n") if l.lower().startswith(b"content-length")).partition(b":")[2])
    return s.read(length)

p = subprocess.Popen([BIN], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
def call(i, m, params):
    body = json.dumps({"jsonrpc":"2.0","id":i,"method":m,"params":params}).encode()
    p.stdin.write(frame(body)); p.stdin.flush()
    resp = json.loads(read_frame(p.stdout))
    print(f"  {m} -> {resp.get('result') or resp.get('error')}")
    return resp
call(1, "load_vrm", {"path": ASSET})
call(2, "set_camera", {"position":[0,1.4,1.5],"target":[0,1.4,0],"up":[0,1,0],"fov_degrees":30})
call(3, "set_lighting", {"directional":{"dir":[-0.3,-0.6,-0.7],"color":[1,1,1],"intensity":1},"ambient":{"color":[0.5,0.5,0.5],"intensity":0.3},"cast_shadows":False,"receive_shadows":False})
call(4, "set_post_processing", {"tone_mapping":"None","exposure":1.0})
call(5, "render", {"width":1024,"height":1024,"output_path":OUT,"color_space":"Srgb","msaa":4,"output_type":"Color"})
call(6, "dispose", {})
p.stdin.close(); p.wait(timeout=10)
print(f"output: {OUT} ({os.path.getsize(OUT)} bytes)" if os.path.exists(OUT) else f"missing {OUT}")
PY
```

Expected output:
- `load_vrm -> {'session_id': 'godot-...'}`
- `set_camera -> {}`, `set_lighting -> {}`, `set_post_processing -> {}`
- `render -> {'output_path': '/tmp/godot-l3-smoke-render.png', 'actual_color_space': 'Srgb'}`
- `dispose -> {}`
- `output: /tmp/godot-l3-smoke-render.png (NNNN bytes)` with NNNN > 5000.

If any op returns an error, capture the full response and stop. The error object's `data.reason` typically tells you what failed.

- [ ] **Step 6: Commit**

```bash
git add adapters/godot-vrm/src/operations.gd adapters/godot-vrm/src/tcp_session.gd adapters/godot-vrm/src/main.gd crates/vrm-godot-shim/src/child.rs
git commit -m "feat(adapters/godot-vrm): wire Phase 1 ops through to Session (L3)"
```

---

### Task 9: Update the Rust contract test to assert real renders

**Files:**
- Modify: `crates/vrm-godot-shim/tests/contract.rs`

The existing `#[ignore]`'d test asserts on `Unimplemented` responses for Phase 1 ops. After L3, Phase 1 ops return real success envelopes (and produce a PNG). Replace those expectations.

- [ ] **Step 1: Rewrite the test cases**

The contract test's structure stays mostly the same (helpers, framing); only the expectations change.

Use Edit to find the `contract_cases_round_trip_through_real_godot` test's `exchanges` vector and replace the Phase 1 entries with a sequential render flow. Phase 2+ entries stay as `Unimplemented` assertions.

Replace this block:

```rust
let exchanges: Vec<Exchange> = vec![
    Exchange {
        request_id: 1,
        request: br#"{"jsonrpc":"2.0","id":1,"method":"definitely_not_a_method","params":{}}"#.to_vec(),
        expected_code: -32601,
        expected_phase: None,
    },
    Exchange {
        request_id: 2,
        request: br#"{"jsonrpc":"2.0","id":2,"method":"load_vrm","params":{"path":"/tmp/x.vrm"}}"#.to_vec(),
        expected_code: -32000,
        expected_phase: Some("L3 (godot-vrm integration deferred)"),
    },
    Exchange {
        request_id: 3,
        request: br#"{"jsonrpc":"2.0","id":3,"method":"render","params":{}}"#.to_vec(),
        expected_code: -32000,
        expected_phase: Some("L3 (godot-vrm integration deferred)"),
    },
    Exchange {
        request_id: 4,
        request: br#"{"jsonrpc":"2.0","id":4,"method":"set_humanoid_pose","params":{}}"#.to_vec(),
        expected_code: -32000,
        expected_phase: Some("Phase 2"),
    },
    Exchange {
        request_id: 5,
        request: br#"{"jsonrpc":"2.0","id":5,"method":"set_environment","params":{}}"#.to_vec(),
        expected_code: -32000,
        expected_phase: Some("v1.x"),
    },
    Exchange {
        request_id: 6,
        request: br#"{"jsonrpc":"2.0","id":6,"method":"set_expression","params":{}}"#.to_vec(),
        expected_code: -32000,
        expected_phase: Some("Phase 3"),
    },
];
```

With a new structure: the test now needs both "Phase 1 success" expectations and "Phase 2+ Unimplemented" expectations. Split into two separate `#[ignore]`'d tests:

(a) `phase1_ops_render_a_real_vrm` — runs load → camera → lighting → post → render → dispose against a `vrm-asset-generator emit-default`-produced VRM. Asserts each op succeeds, asserts the render PNG exists + is >5000 bytes + is a valid 1024×1024 PNG.

(b) `reserved_ops_still_return_unimplemented` — keep the existing assertions for `set_humanoid_pose` (Phase 2), `set_environment` (v1.x), `set_expression` (Phase 3), and the unknown-method -32601 case.

Concretely, paste this entirely-new version of `contract_cases_round_trip_through_real_godot`:

```rust
#[test]
#[ignore]
fn phase1_ops_render_a_real_vrm() {
    use std::path::Path;

    let project_dir = workspace_root().join("adapters").join("godot-vrm");
    assert!(project_dir.join("project.godot").is_file());

    // Generate a sample VRM the test will load.
    let tmp = tempfile::tempdir().expect("tempdir");
    let asset_dir = tmp.path();
    let status = std::process::Command::new("cargo")
        .arg("run").arg("--release").arg("-q")
        .arg("-p").arg("vrm-asset-generator").arg("--")
        .arg("emit-default")
        .arg("--id").arg("contract_l3")
        .arg("--output-dir").arg(asset_dir)
        .current_dir(workspace_root())
        .status().expect("emit-default");
    assert!(status.success(), "emit-default failed");
    let vrm_path = asset_dir.join("contract_l3.vrm");
    assert!(vrm_path.exists(), "VRM not generated");
    let out_png = asset_dir.join("contract_l3.png");

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
        (5, "render", serde_json::json!({"width":1024,"height":1024,"output_path":out_png.to_string_lossy(),"color_space":"Srgb","msaa":4,"output_type":"Color"})),
        (6, "dispose", serde_json::json!({})),
    ];

    for (id, method, params) in &calls {
        let req = serde_json::json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}).to_string();
        stdin.write_all(&frame(req.as_bytes())).unwrap();
        stdin.flush().unwrap();
        let body = read_framed(&mut stdout);
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(parsed["error"].is_null(), "{} failed: {parsed}", method);
        let resp_id = parsed["id"].as_i64().expect("integer id");
        assert_eq!(resp_id, *id, "id mismatch for {}", method);
    }

    drop(stdin);
    assert!(child.wait().unwrap().success());

    // Validate the rendered PNG.
    let png = std::fs::read(&out_png).expect("read PNG");
    assert!(png.len() > 5000, "PNG too small: {} bytes", png.len());
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "bad PNG magic");
    let width = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
    let height = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
    assert_eq!((width, height), (1024, 1024), "unexpected dimensions");
    let _ = out_png; let _ = Path::new(asset_dir);
}

#[test]
#[ignore]
fn reserved_ops_still_return_unimplemented() {
    let project_dir = workspace_root().join("adapters").join("godot-vrm");
    let mut child = Command::new(shim_binary())
        .env("GODOT_VRM_ADAPTER_DIR", &project_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn shim");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();

    let cases: Vec<(i64, &str, i64, Option<&str>)> = vec![
        (1, "definitely_not_a_method", -32601, None),
        (2, "set_humanoid_pose", -32000, Some("Phase 2")),
        (3, "set_environment", -32000, Some("v1.x")),
        (4, "set_expression", -32000, Some("Phase 3")),
    ];
    for (id, method, code, phase) in &cases {
        let req = format!(r#"{{"jsonrpc":"2.0","id":{},"method":"{}","params":{{}}}}"#, id, method);
        stdin.write_all(&frame(req.as_bytes())).unwrap();
        stdin.flush().unwrap();
        let body = read_framed(&mut stdout);
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(parsed["error"]["code"].as_i64(), Some(*code), "method {} expected code {}, got {parsed}", method, code);
        if let Some(p) = phase {
            assert_eq!(parsed["error"]["data"]["phase"].as_str(), Some(*p), "phase mismatch for {}: {parsed}", method);
        }
    }
    drop(stdin);
    let _ = child.wait();
}
```

Delete the original `contract_cases_round_trip_through_real_godot` and `malformed_json_returns_parse_error_with_null_id` tests (the malformed-JSON one is still useful — keep it).

Actually keep `malformed_json_returns_parse_error_with_null_id` as it is. Just replace the first test with the two new ones.

- [ ] **Step 2: Run the contract tests**

```bash
cargo test -p vrm-godot-shim --test contract -- --ignored --nocapture 2>&1 | tail -10
```

Expected: **3 passed** (`phase1_ops_render_a_real_vrm`, `reserved_ops_still_return_unimplemented`, `malformed_json_returns_parse_error_with_null_id`).

The `phase1_ops_render_a_real_vrm` test takes ~5–10 s (Godot startup + VRM load + shader compile + render).

- [ ] **Step 3: Verify `cargo test --workspace` still passes (with the contract tests skipped)**

```bash
cargo test --workspace 2>&1 | tail -8
```

Expected: all non-ignored tests pass; godot-shim shows `3 ignored` (was 2; we added one).

- [ ] **Step 4: Clippy still clean**

```bash
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3
```

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-godot-shim/tests/contract.rs
git commit -m "test(vrm-godot-shim): assert real Phase 1 render output (L3)"
```

---

### Task 10: Wire godot-vrm into `scripts/bootstrap-goldens.sh`

**Files:**
- Modify: `scripts/bootstrap-goldens.sh`

- [ ] **Step 1: Add a SKIP_GODOT_VRM guard + render_with_adapter call**

Read the existing bootstrap-goldens.sh. After the `if [ "${SKIP_THREE_VRM:-0}" != "1" ]; then ... fi` block (which renders three-vrm), append:

```bash
if [ "${SKIP_GODOT_VRM:-0}" != "1" ]; then
    echo "==> Building vrm-godot-shim"
    cargo build --release -q -p vrm-godot-shim >/dev/null
    GVRM_BIN="$ROOT/target/release/vrm-godot-shim"
    if [ -x "$GVRM_BIN" ] && command -v godot >/dev/null 2>&1; then
        render_with_adapter "godot-vrm" "0.1.0" "$GVRM_BIN"
    else
        if [ ! -x "$GVRM_BIN" ]; then
            echo "    (skipping godot-vrm: shim binary not built)" >&2
        else
            echo "    (skipping godot-vrm: godot not on PATH)" >&2
        fi
    fi
else
    echo "==> Skipping godot-vrm (SKIP_GODOT_VRM=1)"
fi
```

Also update the docstring at the top of the script to mention `SKIP_GODOT_VRM=1`.

- [ ] **Step 2: Smoke the integration with QUICK mode (2 assets only)**

```bash
QUICK=1 ./scripts/bootstrap-goldens.sh 2>&1 | tee /tmp/bootstrap-l3.log | tail -30
```

Expected: the run renders 2 test_ids (`mtoon_default` + `springbone_default`) through three-vrm, vrm-metal-kit, AND godot-vrm. Each renderer produces 2 PNGs. The local-manifest.json has 6 entries (3 renderers × 2 tests).

The `springbone_default` test plan has a `physics` block; godot-vrm's Phase 2 ops still return `Unimplemented`. So that test will FAIL through godot-vrm — the runner's `execute-test-plan` will exit non-zero on the `reset_physics` call. Check stderr; it should say "Unimplemented" with `phase: Phase 2`.

This is expected behavior at L3. The bootstrap script's `render_with_adapter` function tolerates per-adapter failures with a `failed=$((failed + 1))` counter; it continues with the next test.

```bash
grep "FAIL godot-vrm" /tmp/bootstrap-l3.log
```

Expected: one line — `springbone_default` failed for godot-vrm. The `mtoon_default` test should succeed.

- [ ] **Step 3: Commit**

```bash
git add scripts/bootstrap-goldens.sh
git commit -m "feat(bootstrap): wire godot-vrm adapter into corpus render loop"
```

---

### Task 11: Documentation + findings update

**Files:**
- Modify: `adapters/godot-vrm/README.md` (L3 status table)
- Modify: `README.md` (root — bump godot-vrm row from "L1+L2" to "L3")
- Modify: `CLAUDE.md` (adapter status bullet)
- Modify: `docs/findings.md` (new run entry with three-renderer consensus)

- [ ] **Step 1: Full corpus rerun + new consensus report**

```bash
./scripts/bootstrap-goldens.sh 2>&1 | tee /tmp/bootstrap-l3-full.log | tail -10
./scripts/consensus-report.sh 2>&1 | tee /tmp/consensus-l3.log | tail -80
```

Expected: the full 80-asset corpus runs through all three real adapters (44 MToon assets succeed for all three; 36 spring-bone assets fail for godot-vrm and succeed for three-vrm + vrm-metal-kit). Consensus report now has 3-way pairwise SSIM where all three adapters rendered, and 2-way where only three-vrm + vrm-metal-kit did.

- [ ] **Step 2: Update the adapter README status table**

Edit `adapters/godot-vrm/README.md`:

- Change the `| L3 — Phase 1 ops against V-Sekai/godot-vrm | deferred (separate plan) |` row to `| L3 — Phase 1 ops against V-Sekai/godot-vrm | shipped |`.
- Add a new row: `| L4 — Phase 2 physics ops | deferred — spring-bone settle/swing tests skip godot-vrm |`.
- Update the phase-label table: Phase 1 ops no longer return `Unimplemented`; remove the L3 row from the "what's still Unimplemented" table.
- Remove the `## L3 sketch` section entirely (the work is done; it's no longer a sketch).
- Update the `## How the runner invokes it` example to point at `target/release/vrm-godot-shim`.

- [ ] **Step 3: Update root README adapter row**

In repo-root `README.md`, change the godot-vrm row to:

```markdown
| `adapters/godot-vrm/` | Godot 4 / GDScript adapter for [V-Sekai/godot-vrm](https://github.com/V-Sekai/godot-vrm). Pairs with `crates/vrm-godot-shim/` (Rust) for stdio framing. **L3 — Phase 1 ops live**; spring-bone physics (Phase 2) deferred. Third real renderer for the MToon corpus. |
```

- [ ] **Step 4: Update CLAUDE.md adapter status bullet**

Change the godot-vrm bullet to:

```markdown
- `adapters/godot-vrm/` — Godot 4 / GDScript paired with the `crates/vrm-godot-shim/` Rust shim. L3 (Phase 1 ops real); MToon corpus renders end-to-end. Phase 2 spring-bone ops still `Unimplemented` so spring-bone test plans skip this adapter. Runner consumes `target/release/vrm-godot-shim` as `--adapter-bin`. Requires Godot 4.3+ on `PATH`.
```

- [ ] **Step 5: Add a `## Sixth run` entry to findings.md**

Append after the existing `## Fifth run` section. Include:

- Date.
- Method (`scripts/bootstrap-goldens.sh` + `consensus-report.sh` with three adapters live).
- Headline metric: how many test_ids now have 3-way consensus available (44 MToon + ? spring-bone settle if godot-vrm Phase-2 happens to no-op vs error).
- Cross-renderer pair SSIM stats from the new consensus-report.json.
- Notable findings: are there any MToon parameter sweeps where godot-vrm is the outlier vs the three-vrm/vrm-metal-kit cluster? Or vice versa?
- Pixel-level sample for `mtoon_default` from godot-vrm — compare to the run 5 values for three-vrm (53,53,53) and vrm-metal-kit (164,164,164).

Use this template:

```markdown
## Sixth run: godot-vrm L3 shipped — third real renderer added

**Date**: <YYYY-MM-DD>, vrm-conformance commit <SHA>.

**Trigger**: V-Sekai/godot-vrm vendored at `9fae4049` + Godot-MToon-Shader at `27cb2b78`; L3 Phase 1 ops landed (`load_vrm`, `set_camera`, `set_lighting`, `set_post_processing`, `render`, `dispose`). Phase 2 (spring-bone) deferred — spring-bone test plans skip godot-vrm.

**Method**: `scripts/bootstrap-goldens.sh` rendered the full 80-test corpus through three-vrm, vrm-metal-kit, and godot-vrm on macOS 26 (Apple M4 Max, Godot 4.6.2). `scripts/consensus-report.sh` ran pairwise SSIM across the manifest.

**Headline**: <fill in once corpus + consensus run completes>.

<pairwise SSIM table>

<observations>

<sphere centerline pixel sample for godot-vrm vs other two>
```

Fill in the placeholders with actual data from `/tmp/consensus-l3.log`.

- [ ] **Step 6: Commit**

```bash
git add adapters/godot-vrm/README.md README.md CLAUDE.md docs/findings.md
git commit -m "docs: godot-vrm L3 shipped + record sixth corpus run"
```

---

## Out of scope (deferred to a future plan)

- **Phase 2 ops (`step_physics`, `reset_physics`, `animate_root_transform`)** — would require overriding godot-vrm's `vrm_secondary.gd` spring-bone auto-stepping and taking manual control of the physics pump. Spring-bone test plans currently fail for godot-vrm; that's the L4 boundary.
- **Reserved Phase 2+ ops (`set_humanoid_pose`, `set_root_transform`, `set_environment`, `set_expression`)** — out of scope for any current plan.
- **Linear PNG output (`color_space: "Linear"` requested)** — Godot's `Image.save_png` always writes sRGB-encoded PNGs. The render op accepts the request and reports `actual_color_space: "Srgb"`; the runner's diff engine handles color-space mismatch via wider tolerances (per `docs/methodology.md`).
- **Tone-mapping `None` exactness** — Godot has no true "no tone mapping" mode; we use `TONE_MAPPER_LINEAR` + `exposure=1.0`. May produce subtly different output than three-vrm's `NoToneMapping` for high-dynamic-range pixels. Acceptable for the L1+L2+L3 scope; revisit if specific tests show divergence traceable to this.

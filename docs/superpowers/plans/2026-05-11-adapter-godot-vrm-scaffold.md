# godot-vrm Adapter L1+L2 Scaffold Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land `adapters/godot-vrm/` as a third VRM 1.0 renderer adapter scaffold — package skeleton + LSP-style Content-Length JSON-RPC stdio framing + dispatch table returning structured `Unimplemented` for every Phase 1 / Phase 2+ op — so consensus diff has a viable third real renderer once L3 wires in V-Sekai/godot-vrm.

**Architecture:** Mirrors the `adapters/babylon-vrm/` L1+L2 milestone: same operation contract, same framing wire (LSP `Content-Length` headers, identical bytes to the Rust/Swift/TS adapters), same `data.phase` deferral labels. Diverges on host language only — the adapter binary is a shell wrapper that invokes Godot 4 headless (`godot --headless --script src/main.gd`) and the framing + dispatch live in GDScript. Wire-level contract tests are Python subprocess tests (matching the project's existing `python3` smoke dependency); in-process unit tests are GDScript scripts run by Godot itself. L3 (real `V-Sekai/godot-vrm` rendering) is a separate plan.

**Tech Stack:** Godot 4.3 (pinned, headless), GDScript, Python 3 (subprocess tests, stdlib only — no pytest dep), bash wrapper, GitHub Actions Linux runner.

---

## File Structure

```
adapters/godot-vrm/
├── README.md                 # Status table, build/test commands, L3 sketch
├── project.godot             # Godot 4.3 project descriptor (headless-compatible)
├── .gitignore                # .godot/, exports, editor scratch
├── bin/
│   └── godot-vrm-adapter     # Shell wrapper: `exec godot --headless ...`
├── src/
│   ├── main.gd               # Entry script: instantiates Server, runs loop
│   ├── framing.gd            # Content-Length read/write over OS stdin/stdout
│   ├── operations.gd         # Phase-by-method table + dispatch()
│   └── server.gd             # JSON-RPC request → dispatch → response loop
└── tests/
    ├── run_gdscript_tests.gd # GDScript test runner (built-in, no GUT dep)
    ├── test_framing.gd       # Round-trip + edge cases for framing.gd
    ├── test_operations.gd    # Dispatch table coverage
    └── test_contract.py      # Python subprocess test — wire-level contract
.github/workflows/
└── godot-vrm.yml             # Install Godot 4.3 + run both test layers

# Edits to existing files
README.md                     # Add adapters/godot-vrm/ row + update Acks
CLAUDE.md                     # Adapter status section adds godot-vrm
adapters/babylon-vrm/README.md # Replace "Alternative third adapter" hint with cross-link
```

**Boundaries:**
- `framing.gd` owns byte-level wire framing only — no JSON, no method names.
- `operations.gd` owns the method → phase-label table and dispatch — no I/O.
- `server.gd` owns the request/response loop — knows about JSON, doesn't know what methods exist.
- `main.gd` owns process startup — wires stdin/stdout into `server.gd` and waits.

Same separation as `adapters/babylon-vrm/src/{framing,operations,server,main}.ts` so future maintainers can grep across adapters.

---

## Pre-flight assumption to verify

Godot 4.3 exposes `OS.read_buffer_from_stdin(amount: int) -> PackedByteArray` and `OS.write_buffer_to_stdout(buffer: PackedByteArray) -> void` for byte-safe stdio in headless mode. Task 1 verifies this before committing to GDScript framing. If verification fails, **stop and revise this plan** to introduce a thin Rust or Node shim that does framing and forwards decoded JSON to Godot — do not improvise that pivot inside subsequent tasks.

---

### Task 1: Verify Godot 4.3 stdio byte-safety (spike)

**Files:**
- Create: `/tmp/godot-stdio-spike.gd` (throwaway — never committed)

- [ ] **Step 1: Install Godot 4.3 headless locally**

```bash
# macOS:
brew install --cask godot   # accept whatever 4.x ships; 4.3 minimum
# Linux:
curl -L -o /tmp/godot.zip \
  https://github.com/godotengine/godot/releases/download/4.3-stable/Godot_v4.3-stable_linux.x86_64.zip
unzip -p /tmp/godot.zip > /tmp/godot && chmod +x /tmp/godot
godot --version    # confirm >= 4.3
```

Expected: `4.3.stable.official.<hash>` or newer.

- [ ] **Step 2: Write the spike script**

```bash
cat > /tmp/godot-stdio-spike.gd <<'GD'
extends SceneTree

func _init() -> void:
    # Read 5 bytes, write them back, exit. Round-trip proves byte-safe stdio.
    var buf := OS.read_buffer_from_stdin(5)
    OS.write_buffer_to_stdout(buf)
    quit(0)
GD
```

- [ ] **Step 3: Run the spike**

```bash
printf 'hello' | godot --headless --no-window --script /tmp/godot-stdio-spike.gd | xxd
```

Expected stdout (5 bytes): `00000000: 6865 6c6c 6f                             hello`

If the output is missing, truncated, or has extra bytes (e.g., trailing newline), **stop**. GDScript stdio is not byte-safe on this Godot version; revise the plan to introduce a host-language shim before continuing.

- [ ] **Step 4: Note the result in the plan**

Append a one-line decision record to this plan file under a new `## Spike result` section:

```markdown
## Spike result

- Date: <YYYY-MM-DD>
- Godot version: <output of `godot --version`>
- Outcome: byte-safe stdio confirmed (5/5 bytes round-tripped) — proceeding with GDScript framing.
```

No commit yet (no repo files changed).

---

### Task 2: Adapter package skeleton

**Files:**
- Create: `adapters/godot-vrm/README.md`
- Create: `adapters/godot-vrm/project.godot`
- Create: `adapters/godot-vrm/.gitignore`
- Modify: `.gitignore` (add Godot patterns to repo root if not already covered)

- [ ] **Step 1: Create the project.godot manifest**

```bash
mkdir -p adapters/godot-vrm/{src,tests,bin}
cat > adapters/godot-vrm/project.godot <<'GD'
; Godot 4.3 project descriptor for the godot-vrm conformance adapter.
; Headless-only — no main scene, no display server. The adapter is
; invoked via `godot --headless --path adapters/godot-vrm --script src/main.gd`.

config_version=5

[application]

config/name="vrm-godot-vrm-adapter"
config/description="V-Sekai/godot-vrm renderer adapter for arkavo-org/vrm-conformance. L1+L2: JSON-RPC scaffold; renderer integration deferred to L3."
config/features=PackedStringArray("4.3")

[debug]

settings/stdout/print_fps=false
settings/stdout/verbose_stdout=false
GD
```

- [ ] **Step 2: Create the adapter .gitignore**

```bash
cat > adapters/godot-vrm/.gitignore <<'GD'
# Godot editor cache + import metadata.
.godot/
.import/
# Export artifacts.
export.cfg
export_presets.cfg
*.import
GD
```

- [ ] **Step 3: Create the README in its initial form**

```bash
cat > adapters/godot-vrm/README.md <<'MD'
# godot-vrm renderer adapter

A renderer adapter that bridges [V-Sekai/godot-vrm](https://github.com/V-Sekai/godot-vrm) to the project's renderer-agnostic operation contract documented at [`docs/operation-contract.md`](../../docs/operation-contract.md).

Architecturally similar to the [three-vrm](../three-vrm/README.md) and [babylon-vrm](../babylon-vrm/README.md) adapters: a small executable that speaks JSON-RPC over stdio with LSP-style `Content-Length` framing. Host language differs — this adapter runs inside `godot --headless`, with framing + dispatch implemented in GDScript.

## Why a third adapter

vrm-conformance currently has two real adapters (three-vrm + vrm-metal-kit). The [N-way consensus diff](../../crates/vrm-diff-engine/src/consensus.rs) needs three or more independent renderers to produce a real majority-vs-outlier signal. The natural third candidate — `virtual-cast/babylon-vrm-loader` via [`adapters/babylon-vrm/`](../babylon-vrm/) — is upstream-blocked on VRM 1.0 support. `V-Sekai/godot-vrm` already implements VRMC_vrm, VRMC_materials_mtoon, VRMC_springBone, and VRMC_node_constraint, so it is the realistic next adapter for closing the third-renderer gap.

## Status

| Phase | Status |
|---|---|
| L1 — package skeleton                         | scaffolded |
| L2 — JSON-RPC stdio framing + dispatcher      | scaffolded (all ops return `Unimplemented`) |
| L3 — Phase 1 ops against V-Sekai/godot-vrm    | deferred (separate plan) |

Through L2, every operation returns a structured `Unimplemented` error (JSON-RPC code `-32000`):

| Method | `data.phase` |
|---|---|
| `load_vrm`, `set_camera`, `set_lighting`, `set_post_processing`, `render`, `dispose` | `L3 (godot-vrm integration deferred)` |
| `set_humanoid_pose`, `set_root_transform`, `animate_root_transform`, `step_physics`, `reset_physics` | `Phase 2` |
| `set_environment` | `v1.x` |
| `set_expression` | `Phase 3` |
| (unknown) | `-32601 method not found` |

## Runtime dependency

Godot 4.3 must be on `PATH` as `godot`. The adapter is a thin wrapper around `godot --headless --path adapters/godot-vrm --script src/main.gd`. No compile step.

- macOS: `brew install --cask godot` (4.3+).
- Linux: download `Godot_v4.3-stable_linux.x86_64.zip` from the [Godot releases page](https://github.com/godotengine/godot/releases/tag/4.3-stable) and put the binary on `PATH`.

## Build

There is no build step. The wrapper script is executable as shipped:

```bash
adapters/godot-vrm/bin/godot-vrm-adapter
```

## Tests

```bash
# GDScript in-process tests (framing + dispatch table)
godot --headless --path adapters/godot-vrm --script tests/run_gdscript_tests.gd

# Python wire-level contract tests (spawn the binary, exchange framed JSON-RPC)
python3 adapters/godot-vrm/tests/test_contract.py
```

Both layers run in CI (`.github/workflows/godot-vrm.yml`).

## How the runner invokes it

Same wire as the other adapters. The runner spawns the binary as a long-lived child and pipes framed JSON-RPC requests/responses:

```
Content-Length: NNN\r\n
\r\n
{"jsonrpc":"2.0","id":1,"method":"load_vrm","params":{"path":"…"}}
```

Once L3 lands, `scripts/bootstrap-goldens.sh` picks up this adapter automatically alongside three-vrm and vrm-metal-kit, contributing a third entry per `test_id` to the manifest. The consensus-diff command's `--render` flag then takes three or more `name=path` pairs and gains real majority-vs-outlier signal.

## L3 sketch

L3 lives in a separate plan. Implementation outline:

1. Add `addons/godot-vrm/` as a [Godot asset library](https://godotengine.org/asset-library/asset) install pinned to a specific commit (parity with the upstream-revision pin in `adapters/vrm-metal-kit/Package.swift`).
2. Replace the `Unimplemented` returns for Phase 1 ops with passthroughs that drive a hidden `SubViewport` rendering the loaded `.vrm` to a `ViewportTexture` and saving via `Image.save_png`.
3. Set the SubViewport clear color to magenta `(255, 0, 255)` for property-assertion bbox detection (matches three-vrm + vrm-metal-kit convention).
4. Pin `Environment.tone_mapper = TONE_MAPPER_LINEAR` and disable shadows for MToon math tests (per `docs/methodology.md`).
5. Lock `Engine.physics_ticks_per_second = 60` and expose `reset_physics(settle_steps)` via `addons/godot-vrm`'s spring-bone API.

Issues to file upstream will accumulate in `docs/findings.md` once renders flow.
MD
```

- [ ] **Step 4: Verify gitignore patterns aren't already broken**

```bash
grep -E '^adapters/\*/' .gitignore
```

Expected: existing patterns `adapters/*/node_modules/`, `adapters/*/dist/`, `adapters/*/.build/`, `adapters/*/.swiftpm/`, `adapters/*/Package.resolved` are present. No edit needed — Godot's `.godot/` is handled by the per-adapter `.gitignore` created in Step 2.

- [ ] **Step 5: Commit**

```bash
git add adapters/godot-vrm/README.md adapters/godot-vrm/project.godot adapters/godot-vrm/.gitignore
git commit -m "feat(adapters/godot-vrm): L1 package skeleton"
```

---

### Task 3: Framing module — failing tests first

**Files:**
- Create: `adapters/godot-vrm/tests/test_framing.gd`
- Create: `adapters/godot-vrm/tests/run_gdscript_tests.gd`

- [ ] **Step 1: Write the test runner**

```bash
cat > adapters/godot-vrm/tests/run_gdscript_tests.gd <<'GD'
# Minimal GDScript test runner. No external deps (GUT would pull in a plugin).
# Discovers test_*.gd files in tests/, instantiates each, calls every method
# starting with `test_`, and reports pass/fail counts. Exits non-zero on
# any failure so CI can gate.

extends SceneTree

const TESTS_DIR := "res://tests/"

var _passed := 0
var _failed := 0
var _failures: Array[String] = []

func _init() -> void:
    var dir := DirAccess.open(TESTS_DIR)
    if dir == null:
        push_error("cannot open " + TESTS_DIR)
        quit(2)
        return
    dir.list_dir_begin()
    var names: Array[String] = []
    while true:
        var name := dir.get_next()
        if name == "":
            break
        if name.begins_with("test_") and name.ends_with(".gd"):
            names.append(name)
    names.sort()
    for name in names:
        _run_file(TESTS_DIR + name)
    print("\n%d passed, %d failed" % [_passed, _failed])
    for f in _failures:
        print("  FAIL: " + f)
    quit(0 if _failed == 0 else 1)

func _run_file(path: String) -> void:
    var script: GDScript = load(path)
    if script == null:
        push_error("cannot load " + path)
        _failed += 1
        _failures.append(path + " (load failed)")
        return
    var inst: Object = script.new()
    var method_list := inst.get_method_list()
    for m in method_list:
        var mname: String = m["name"]
        if not mname.begins_with("test_"):
            continue
        var ok := true
        var failure_msg := ""
        # Each test method should call self.fail(msg) or self.assert_eq(...).
        # We capture failures via a `_test_failure` field on the instance.
        inst.set("_test_failure", "")
        inst.call(mname)
        var captured: String = inst.get("_test_failure")
        if captured != "":
            ok = false
            failure_msg = captured
        if ok:
            _passed += 1
        else:
            _failed += 1
            _failures.append("%s::%s — %s" % [path, mname, failure_msg])
GD
```

- [ ] **Step 2: Write the framing tests (will fail — framing.gd doesn't exist yet)**

```bash
cat > adapters/godot-vrm/tests/test_framing.gd <<'GD'
extends RefCounted

const Framing := preload("res://src/framing.gd")

var _test_failure: String = ""

func _fail(msg: String) -> void:
    if _test_failure == "":
        _test_failure = msg

func _assert_eq(actual, expected, label: String) -> void:
    if actual != expected:
        _fail("%s: expected %s, got %s" % [label, str(expected), str(actual)])

# encode/decode round-trip on a normal JSON-RPC body.
func test_round_trip_basic() -> void:
    var body := '{"jsonrpc":"2.0","id":1,"method":"ping","params":{}}'.to_utf8_buffer()
    var wire := Framing.encode(body)
    var decoded: PackedByteArray = Framing.decode(wire)
    _assert_eq(decoded, body, "round-trip body")

# encode produces literal "Content-Length: N\r\n\r\n<body>" bytes.
func test_encode_header_format() -> void:
    var wire := Framing.encode('{"ok":true}'.to_utf8_buffer())
    var expected := 'Content-Length: 11\r\n\r\n{"ok":true}'.to_utf8_buffer()
    _assert_eq(wire, expected, "encode wire bytes")

# decode tolerates lowercase header name and extra headers.
func test_decode_case_insensitive_and_extra_headers() -> void:
    var wire := 'content-length: 5\r\nX-Trace-Id: abc\r\n\r\nhello'.to_utf8_buffer()
    var body: PackedByteArray = Framing.decode(wire)
    _assert_eq(body.get_string_from_utf8(), "hello", "case-insensitive decode")

# decode rejects messages missing Content-Length.
func test_decode_missing_content_length() -> void:
    var wire := '\r\n\r\n{}'.to_utf8_buffer()
    var body = Framing.decode(wire)
    if body != null:
        _fail("expected null on missing Content-Length, got " + str(body))

# encode handles empty body.
func test_encode_empty_body() -> void:
    var wire := Framing.encode(PackedByteArray())
    var expected := 'Content-Length: 0\r\n\r\n'.to_utf8_buffer()
    _assert_eq(wire, expected, "empty body wire bytes")
GD
```

- [ ] **Step 3: Run tests to confirm they fail with "framing.gd not found"**

```bash
cd adapters/godot-vrm
godot --headless --script tests/run_gdscript_tests.gd
cd ../..
```

Expected: non-zero exit, output containing `cannot load res://tests/test_framing.gd` or `preload("res://src/framing.gd") failed`. This proves the test harness runs and the missing module is detected.

- [ ] **Step 4: Commit failing tests**

```bash
git add adapters/godot-vrm/tests/test_framing.gd adapters/godot-vrm/tests/run_gdscript_tests.gd
git commit -m "test(adapters/godot-vrm): framing round-trip + header tests"
```

---

### Task 4: Framing module — minimal implementation

**Files:**
- Create: `adapters/godot-vrm/src/framing.gd`

- [ ] **Step 1: Implement framing.gd**

```bash
cat > adapters/godot-vrm/src/framing.gd <<'GD'
# LSP-style Content-Length framing for JSON-RPC over stdio.
#
# Wire format (identical to vrm-ops::stdio (Rust), Swift's Framing.swift,
# and the three-vrm + babylon-vrm TypeScript adapters):
#
#   Content-Length: NNN\r\n
#   \r\n
#   <NNN bytes of body>
#
# Case-insensitive header parsing; additional headers (e.g., X-Trace-Id)
# are accepted and ignored. Decode returns null when the input is
# malformed — callers convert that to an RPC error or EOF as appropriate.

class_name Framing

const CRLF := "\r\n"
const HEADER_TERMINATOR := "\r\n\r\n"

# Build a wire-frame from a body. Always succeeds.
static func encode(body: PackedByteArray) -> PackedByteArray:
    var header := "Content-Length: %d%s%s" % [body.size(), CRLF, CRLF]
    var out := header.to_utf8_buffer()
    out.append_array(body)
    return out

# Decode one frame from a complete byte buffer. Returns the body, or null
# if the header is malformed or the body length is short. This is the
# "all bytes already in hand" variant used by unit tests.
static func decode(wire: PackedByteArray) -> Variant:
    var text := wire.get_string_from_utf8()
    var sep := text.find(HEADER_TERMINATOR)
    if sep < 0:
        return null
    var header_text := text.substr(0, sep)
    var content_length := -1
    for line in header_text.split(CRLF, false):
        if line == "":
            continue
        var colon := line.find(":")
        if colon < 0:
            return null
        var key := line.substr(0, colon).strip_edges().to_lower()
        var value := line.substr(colon + 1).strip_edges()
        if key == "content-length":
            content_length = value.to_int()
            if content_length < 0:
                return null
    if content_length < 0:
        return null
    var body_start := sep + HEADER_TERMINATOR.length()
    var body_end := body_start + content_length
    if body_end > wire.size():
        return null
    return wire.slice(body_start, body_end)

# Read one frame from OS stdin in a blocking loop. Returns the body, or
# null on EOF / framing error. Used by server.gd at runtime; not by tests.
static func read_from_stdin() -> Variant:
    var header_bytes := PackedByteArray()
    var terminator := HEADER_TERMINATOR.to_utf8_buffer()
    while true:
        var chunk := OS.read_buffer_from_stdin(1)
        if chunk.size() == 0:
            return null  # EOF
        header_bytes.append_array(chunk)
        if _ends_with(header_bytes, terminator):
            break
        if header_bytes.size() > 4096:
            return null  # runaway header; treat as framing error
    var header_text := header_bytes.slice(0, header_bytes.size() - terminator.size()).get_string_from_utf8()
    var content_length := -1
    for line in header_text.split(CRLF, false):
        if line == "":
            continue
        var colon := line.find(":")
        if colon < 0:
            return null
        var key := line.substr(0, colon).strip_edges().to_lower()
        if key == "content-length":
            content_length = line.substr(colon + 1).strip_edges().to_int()
    if content_length < 0:
        return null
    var body := PackedByteArray()
    while body.size() < content_length:
        var chunk2 := OS.read_buffer_from_stdin(content_length - body.size())
        if chunk2.size() == 0:
            return null  # truncated body
        body.append_array(chunk2)
    return body

static func _ends_with(buf: PackedByteArray, suffix: PackedByteArray) -> bool:
    if buf.size() < suffix.size():
        return false
    var offset := buf.size() - suffix.size()
    for i in suffix.size():
        if buf[offset + i] != suffix[i]:
            return false
    return true
GD
```

- [ ] **Step 2: Run framing tests — expect all five to pass**

```bash
cd adapters/godot-vrm
godot --headless --script tests/run_gdscript_tests.gd
cd ../..
```

Expected: stdout contains `5 passed, 0 failed`. Exit code 0.

- [ ] **Step 3: Commit**

```bash
git add adapters/godot-vrm/src/framing.gd
git commit -m "feat(adapters/godot-vrm): Content-Length framing module"
```

---

### Task 5: Operations dispatch — failing tests first

**Files:**
- Create: `adapters/godot-vrm/tests/test_operations.gd`

- [ ] **Step 1: Write dispatch tests**

```bash
cat > adapters/godot-vrm/tests/test_operations.gd <<'GD'
extends RefCounted

const Operations := preload("res://src/operations.gd")

var _test_failure: String = ""

func _fail(msg: String) -> void:
    if _test_failure == "":
        _test_failure = msg

func _assert_eq(actual, expected, label: String) -> void:
    if actual != expected:
        _fail("%s: expected %s, got %s" % [label, str(expected), str(actual)])

# Unknown methods get JSON-RPC -32601.
func test_unknown_method_returns_minus_32601() -> void:
    var result: Dictionary = Operations.dispatch("definitely_not_a_method", {})
    _assert_eq(result.get("ok"), false, "ok flag")
    _assert_eq(result.get("error", {}).get("code"), -32601, "error code")

# load_vrm returns the L3 deferral phase.
func test_load_vrm_returns_l3_deferral() -> void:
    var result: Dictionary = Operations.dispatch("load_vrm", {"path": "/tmp/x.vrm"})
    _assert_eq(result.get("ok"), false, "ok flag")
    var err: Dictionary = result.get("error", {})
    _assert_eq(err.get("code"), -32000, "error code")
    _assert_eq(err.get("message"), "Unimplemented", "error message")
    _assert_eq(err.get("data", {}).get("phase"), "L3 (godot-vrm integration deferred)", "phase label")

# render returns the L3 deferral phase.
func test_render_returns_l3_deferral() -> void:
    var result: Dictionary = Operations.dispatch("render", {})
    var err: Dictionary = result.get("error", {})
    _assert_eq(err.get("data", {}).get("phase"), "L3 (godot-vrm integration deferred)", "phase label")

# set_humanoid_pose returns Phase 2.
func test_set_humanoid_pose_returns_phase_2() -> void:
    var result: Dictionary = Operations.dispatch("set_humanoid_pose", {})
    var err: Dictionary = result.get("error", {})
    _assert_eq(err.get("data", {}).get("phase"), "Phase 2", "phase label")

# set_environment returns v1.x.
func test_set_environment_returns_v1x() -> void:
    var result: Dictionary = Operations.dispatch("set_environment", {})
    var err: Dictionary = result.get("error", {})
    _assert_eq(err.get("data", {}).get("phase"), "v1.x", "phase label")

# set_expression returns Phase 3.
func test_set_expression_returns_phase_3() -> void:
    var result: Dictionary = Operations.dispatch("set_expression", {})
    var err: Dictionary = result.get("error", {})
    _assert_eq(err.get("data", {}).get("phase"), "Phase 3", "phase label")
GD
```

- [ ] **Step 2: Run tests — expect six failures (operations.gd missing)**

```bash
cd adapters/godot-vrm
godot --headless --script tests/run_gdscript_tests.gd
cd ../..
```

Expected: `5 passed, 6 failed` (framing still passes; operations all fail to load).

- [ ] **Step 3: Commit failing tests**

```bash
git add adapters/godot-vrm/tests/test_operations.gd
git commit -m "test(adapters/godot-vrm): dispatch table phase-label tests"
```

---

### Task 6: Operations dispatch — minimal implementation

**Files:**
- Create: `adapters/godot-vrm/src/operations.gd`

- [ ] **Step 1: Implement operations.gd**

```bash
cat > adapters/godot-vrm/src/operations.gd <<'GD'
# Operation registry + dispatch for the godot-vrm adapter.
#
# L1 + L2 state: every Phase 1 op and every reserved op returns a structured
# `Unimplemented` error. L3 will replace the Phase 1 entries with real
# implementations driven by V-Sekai/godot-vrm + a hidden SubViewport,
# mirroring how the three-vrm adapter is organized.

class_name Operations

const PHASE_BY_METHOD := {
    # Phase 1 ops — declared, unimplemented until L3.
    "load_vrm": "L3 (godot-vrm integration deferred)",
    "set_camera": "L3 (godot-vrm integration deferred)",
    "set_lighting": "L3 (godot-vrm integration deferred)",
    "set_post_processing": "L3 (godot-vrm integration deferred)",
    "render": "L3 (godot-vrm integration deferred)",
    "dispose": "L3 (godot-vrm integration deferred)",
    # Reserved ops with the canonical phase labels from
    # docs/operation-contract.md.
    "set_environment": "v1.x",
    "set_expression": "Phase 3",
    "set_humanoid_pose": "Phase 2",
    "set_root_transform": "Phase 2",
    "animate_root_transform": "Phase 2",
    "step_physics": "Phase 2",
    "reset_physics": "Phase 2",
}

# Returns a dict shaped like the JSON-RPC server expects:
#   { "ok": true, "result": <value> }   on success
#   { "ok": false, "error": { code, message, data? } }   on failure
static func dispatch(method: String, _params: Variant) -> Dictionary:
    if PHASE_BY_METHOD.has(method):
        return {
            "ok": false,
            "error": {
                "code": -32000,
                "message": "Unimplemented",
                "data": { "phase": PHASE_BY_METHOD[method] },
            },
        }
    return {
        "ok": false,
        "error": {
            "code": -32601,
            "message": "method not found: " + method,
        },
    }

static func known_methods() -> Array:
    return PHASE_BY_METHOD.keys()
GD
```

- [ ] **Step 2: Run tests — expect 11 passed, 0 failed**

```bash
cd adapters/godot-vrm
godot --headless --script tests/run_gdscript_tests.gd
cd ../..
```

Expected: `11 passed, 0 failed`.

- [ ] **Step 3: Commit**

```bash
git add adapters/godot-vrm/src/operations.gd
git commit -m "feat(adapters/godot-vrm): dispatch table with phase labels"
```

---

### Task 7: Server stdio loop

**Files:**
- Create: `adapters/godot-vrm/src/server.gd`

No unit tests at this layer — the Python contract test in Task 9 exercises the full server end-to-end. Splitting an in-process server test would duplicate that coverage.

- [ ] **Step 1: Implement server.gd**

```bash
cat > adapters/godot-vrm/src/server.gd <<'GD'
# Stdio JSON-RPC loop. One request → one response. EOF on stdin ends the
# loop cleanly. Wire format matches Framing (LSP-style Content-Length),
# identical to the three-vrm, babylon-vrm, and vrm-metal-kit adapters so
# the same runner code drives all four.

class_name Server

const Framing := preload("res://src/framing.gd")
const Operations := preload("res://src/operations.gd")

static func run() -> void:
    while true:
        var body: Variant = Framing.read_from_stdin()
        if body == null:
            return  # EOF or framing error; clean shutdown.
        var text: String = (body as PackedByteArray).get_string_from_utf8()
        var parsed: Variant = JSON.parse_string(text)
        if parsed == null or typeof(parsed) != TYPE_DICTIONARY:
            _write_response({
                "jsonrpc": "2.0",
                "id": null,
                "error": {
                    "code": -32700,
                    "message": "parse error",
                },
            })
            continue
        var req: Dictionary = parsed
        var id: Variant = req.get("id", null)
        var method: String = req.get("method", "")
        var params: Variant = req.get("params", {})
        var outcome: Dictionary = Operations.dispatch(method, params)
        var resp: Dictionary = {
            "jsonrpc": "2.0",
            "id": id,
        }
        if outcome.get("ok"):
            resp["result"] = outcome.get("result", {})
        else:
            resp["error"] = outcome.get("error")
        _write_response(resp)

static func _write_response(resp: Dictionary) -> void:
    var body := JSON.stringify(resp).to_utf8_buffer()
    var wire := Framing.encode(body)
    OS.write_buffer_to_stdout(wire)
GD
```

- [ ] **Step 2: Confirm existing tests still pass (server.gd should not regress framing or operations)**

```bash
cd adapters/godot-vrm
godot --headless --script tests/run_gdscript_tests.gd
cd ../..
```

Expected: `11 passed, 0 failed`.

- [ ] **Step 3: Commit**

```bash
git add adapters/godot-vrm/src/server.gd
git commit -m "feat(adapters/godot-vrm): JSON-RPC stdio server loop"
```

---

### Task 8: Entry script + wrapper binary

**Files:**
- Create: `adapters/godot-vrm/src/main.gd`
- Create: `adapters/godot-vrm/bin/godot-vrm-adapter`

- [ ] **Step 1: Implement main.gd**

```bash
cat > adapters/godot-vrm/src/main.gd <<'GD'
# godot-vrm adapter — executable entry point.
#
# L1 + L2: wires stdin/stdout into the JSON-RPC server. Every known method
# returns -32000 Unimplemented with `data.phase` indicating the L3
# deferral; unknown methods return -32601. No real renderer yet.
# L3 will keep this entry point and replace the operations dispatch.

extends SceneTree

const Server := preload("res://src/server.gd")

func _init() -> void:
    push_warning("godot-vrm adapter: starting (L1+L2 scaffold; ops return Unimplemented)")
    Server.run()
    quit(0)
GD
```

- [ ] **Step 2: Write the wrapper shell script**

```bash
cat > adapters/godot-vrm/bin/godot-vrm-adapter <<'SH'
#!/usr/bin/env bash
# Spawn Godot headless with the adapter as main script. Forwards stdin/stdout
# bytewise so the conformance runner sees an identical wire to the
# three-vrm + babylon-vrm + vrm-metal-kit adapters.
set -euo pipefail
HERE=$(cd "$(dirname "$0")/.." && pwd -P)
GODOT_BIN="${GODOT_BIN:-godot}"
exec "$GODOT_BIN" --headless --path "$HERE" --script src/main.gd "$@"
SH
chmod +x adapters/godot-vrm/bin/godot-vrm-adapter
```

- [ ] **Step 3: Smoke-test the wrapper manually**

```bash
printf 'Content-Length: 60\r\n\r\n{"jsonrpc":"2.0","id":1,"method":"load_vrm","params":{"p":""}}' \
  | adapters/godot-vrm/bin/godot-vrm-adapter
```

Expected (modulo Godot's own startup banner on stderr, which the wrapper does not suppress):

```
Content-Length: 155\r\n\r\n{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"Unimplemented","data":{"phase":"L3 (godot-vrm integration deferred)"}}}
```

The exact response length may differ by a few bytes (JSON key order is stable in GDScript). Confirm `Content-Length` matches the body bytes and the response contains `"phase":"L3 (godot-vrm integration deferred)"`.

- [ ] **Step 4: Commit**

```bash
git add adapters/godot-vrm/src/main.gd adapters/godot-vrm/bin/godot-vrm-adapter
git commit -m "feat(adapters/godot-vrm): entry script + wrapper binary"
```

---

### Task 9: Python wire-level contract test

**Files:**
- Create: `adapters/godot-vrm/tests/test_contract.py`

- [ ] **Step 1: Write the contract test**

Python stdlib only — matches `scripts/smoke.sh`'s existing `python3` dependency. No pytest.

```bash
cat > adapters/godot-vrm/tests/test_contract.py <<'PY'
#!/usr/bin/env python3
"""Wire-level contract tests for the godot-vrm adapter.

Spawns the adapter wrapper as a subprocess, exchanges framed JSON-RPC
requests over stdin/stdout, and asserts the response shape. Phase 1 ops
should return -32000 Unimplemented with `data.phase = "L3 (godot-vrm
integration deferred)"`. Phase 2+ reserved ops should return -32000 with
their canonical phase labels. Unknown methods should return -32601.

Mirrors adapters/babylon-vrm/test/contract.test.ts. L3 will replace the
Phase 1 expectations here with real-render assertions against a generated
VRM (same pattern as three-vrm's render.test.ts).
"""

import json
import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
BIN = HERE.parent / "bin" / "godot-vrm-adapter"


def frame(body: bytes) -> bytes:
    return f"Content-Length: {len(body)}\r\n\r\n".encode("ascii") + body


def read_frame(stream) -> bytes:
    """Read one Content-Length framed message from a binary stream."""
    header = b""
    while b"\r\n\r\n" not in header:
        chunk = stream.read(1)
        if not chunk:
            raise EOFError(f"stream ended mid-header; got {header!r}")
        header += chunk
        if len(header) > 4096:
            raise RuntimeError(f"runaway header: {header!r}")
    head, _, _ = header.partition(b"\r\n\r\n")
    content_length = None
    for line in head.split(b"\r\n"):
        if not line:
            continue
        key, _, value = line.partition(b":")
        if key.strip().lower() == b"content-length":
            content_length = int(value.strip())
    if content_length is None:
        raise RuntimeError(f"missing Content-Length in {head!r}")
    body = b""
    while len(body) < content_length:
        chunk = stream.read(content_length - len(body))
        if not chunk:
            raise EOFError(f"stream ended mid-body; got {len(body)} of {content_length}")
        body += chunk
    return body


def rpc(child, request_id, method, params):
    req = json.dumps(
        {"jsonrpc": "2.0", "id": request_id, "method": method, "params": params},
        separators=(",", ":"),
    ).encode("utf-8")
    child.stdin.write(frame(req))
    child.stdin.flush()
    body = read_frame(child.stdout)
    return json.loads(body)


def spawn():
    return subprocess.Popen(
        [str(BIN)],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        env={**os.environ},
    )


def close(child):
    try:
        child.stdin.close()
    except BrokenPipeError:
        pass
    try:
        child.wait(timeout=5)
    except subprocess.TimeoutExpired:
        child.kill()
        child.wait()


CASES = [
    # (id, method, params, expected_code, expected_phase_or_None)
    (1, "definitely_not_a_method", {}, -32601, None),
    (2, "load_vrm", {"path": "/tmp/x.vrm"}, -32000, "L3 (godot-vrm integration deferred)"),
    (3, "render", {}, -32000, "L3 (godot-vrm integration deferred)"),
    (4, "set_humanoid_pose", {}, -32000, "Phase 2"),
    (5, "set_environment", {}, -32000, "v1.x"),
    (6, "set_expression", {}, -32000, "Phase 3"),
]


def main():
    if not BIN.exists():
        print(f"FAIL: adapter binary not found at {BIN}", file=sys.stderr)
        sys.exit(2)

    failures = []
    for request_id, method, params, expected_code, expected_phase in CASES:
        child = spawn()
        try:
            resp = rpc(child, request_id, method, params)
        finally:
            close(child)

        error = resp.get("error") or {}
        if error.get("code") != expected_code:
            failures.append(
                f"{method}: expected code {expected_code}, got {error.get('code')} ({resp!r})"
            )
            continue
        if expected_phase is not None:
            actual_phase = (error.get("data") or {}).get("phase")
            if actual_phase != expected_phase:
                failures.append(
                    f"{method}: expected phase {expected_phase!r}, got {actual_phase!r}"
                )
                continue
        print(f"  ok  {method} -> code={expected_code} phase={expected_phase}")

    # Malformed JSON case (separate flow — we send raw garbage in the body).
    child = spawn()
    try:
        garbage = b"not json at all }}}"
        child.stdin.write(frame(garbage))
        child.stdin.flush()
        body = read_frame(child.stdout)
        resp = json.loads(body)
        if resp.get("error", {}).get("code") != -32700:
            failures.append(f"malformed JSON: expected -32700, got {resp!r}")
        elif resp.get("id") is not None:
            failures.append(f"malformed JSON: expected id=null, got {resp.get('id')!r}")
        else:
            print("  ok  malformed JSON -> -32700 with id=null")
    finally:
        close(child)

    if failures:
        print(f"\n{len(failures)} failure(s):", file=sys.stderr)
        for f in failures:
            print(f"  FAIL: {f}", file=sys.stderr)
        sys.exit(1)
    print(f"\n{len(CASES) + 1} passed")


if __name__ == "__main__":
    main()
PY
chmod +x adapters/godot-vrm/tests/test_contract.py
```

- [ ] **Step 2: Run the contract test**

```bash
python3 adapters/godot-vrm/tests/test_contract.py
```

Expected stdout: 7 `ok` lines, ending with `7 passed`. Exit code 0.

- [ ] **Step 3: Commit**

```bash
git add adapters/godot-vrm/tests/test_contract.py
git commit -m "test(adapters/godot-vrm): wire-level contract tests"
```

---

### Task 10: CI workflow

**Files:**
- Create: `.github/workflows/godot-vrm.yml`

- [ ] **Step 1: Author the workflow**

```bash
cat > .github/workflows/godot-vrm.yml <<'YML'
name: godot-vrm

# L1+L2 scaffold: no Rust deps, no real renderer integration. Installs
# Godot 4.3 headless on Ubuntu, runs both the in-process GDScript tests
# and the Python wire-level contract test.
#
# No untrusted-input usage: this workflow does not read PR titles, commit
# messages, issue bodies, or any other user-controlled fields into run:
# commands.

on:
  pull_request:
    paths:
      - 'adapters/godot-vrm/**'
      - '.github/workflows/godot-vrm.yml'
  push:
    branches: [main]
    paths:
      - 'adapters/godot-vrm/**'
      - '.github/workflows/godot-vrm.yml'

jobs:
  test:
    runs-on: ubuntu-latest
    env:
      GODOT_VERSION: 4.3-stable
    steps:
      - uses: actions/checkout@v4

      - name: Cache Godot binary
        id: cache-godot
        uses: actions/cache@v4
        with:
          path: ~/.local/bin/godot
          key: godot-${{ env.GODOT_VERSION }}-linux-x86_64

      - name: Install Godot headless
        if: steps.cache-godot.outputs.cache-hit != 'true'
        run: |
          mkdir -p ~/.local/bin
          curl -L -o /tmp/godot.zip \
            "https://github.com/godotengine/godot/releases/download/${GODOT_VERSION}/Godot_v${GODOT_VERSION}_linux.x86_64.zip"
          unzip -p /tmp/godot.zip > ~/.local/bin/godot
          chmod +x ~/.local/bin/godot

      - name: Verify Godot version
        run: |
          echo "$HOME/.local/bin" >> "$GITHUB_PATH"
          ~/.local/bin/godot --version

      - name: Run GDScript in-process tests
        run: godot --headless --path adapters/godot-vrm --script tests/run_gdscript_tests.gd

      - name: Run Python wire-level contract tests
        run: python3 adapters/godot-vrm/tests/test_contract.py
YML
```

- [ ] **Step 2: Validate YAML locally**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/godot-vrm.yml'))"
```

Expected: no output, exit 0. (If PyYAML is unavailable, `python3 -c "import json,yaml" 2>&1` errors with `ModuleNotFoundError`; in that case skip — CI will validate.)

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/godot-vrm.yml
git commit -m "ci(adapters/godot-vrm): build + test workflow"
```

---

### Task 11: Cross-link from existing docs

**Files:**
- Modify: `README.md` (repo root) — add adapter table row
- Modify: `CLAUDE.md` — add adapter status entry
- Modify: `adapters/babylon-vrm/README.md` — replace "Alternative third adapter" prose with cross-link

- [ ] **Step 1: Add the adapter to the root README table**

Edit `README.md`. After the `adapters/babylon-vrm/` row in the repository-layout table (around line 44), insert:

```markdown
| `adapters/godot-vrm/` | Godot 4 / GDScript adapter for [V-Sekai/godot-vrm](https://github.com/V-Sekai/godot-vrm) — L1+L2 scaffold; renderer integration deferred to L3. The realistic third real renderer once L3 lands (babylon-vrm is upstream-blocked on VRM 1.0 support). |
```

Run:

```bash
grep -n 'godot-vrm' README.md
```

Expected: at least three lines — the new table row, the existing Acknowledgements line, and any other mentions.

- [ ] **Step 2: Add the adapter status to CLAUDE.md**

Edit `CLAUDE.md`. In the `### Adapter status` section, after the `adapters/babylon-vrm/` bullet, insert:

```markdown
- `adapters/godot-vrm/` — Godot 4 / GDScript via `godot --headless`. L1+L2 scaffolded; all ops return `Unimplemented` with `L3 (godot-vrm integration deferred)` phase label. Wrapper binary at `adapters/godot-vrm/bin/godot-vrm-adapter`. Requires Godot 4.3+ on `PATH`.
```

Run:

```bash
grep -n 'godot-vrm' CLAUDE.md
```

Expected: at least one matching line — the new bullet.

- [ ] **Step 3: Update babylon-vrm README's "Alternative third adapter" prose**

Edit `adapters/babylon-vrm/README.md`. Replace the existing `### Alternative third adapter` paragraph with a one-line pointer:

```markdown
### Alternative third adapter

[V-Sekai/godot-vrm](https://github.com/V-Sekai/godot-vrm) is now scaffolded at [`adapters/godot-vrm/`](../godot-vrm/) (L1+L2; renderer integration deferred to L3). Once that L3 lands, the consensus diff will have its third independent renderer regardless of the babylon-vrm-loader VRM 1.0 timeline.
```

Run:

```bash
grep -n 'godot-vrm' adapters/babylon-vrm/README.md
```

Expected: the new cross-link line plus any historical mentions.

- [ ] **Step 4: Commit**

```bash
git add README.md CLAUDE.md adapters/babylon-vrm/README.md
git commit -m "docs: cross-link godot-vrm adapter scaffold from root + babylon-vrm"
```

---

### Task 12: End-to-end verification

**Files:** none (verification only)

- [ ] **Step 1: Clean-run every test from a fresh terminal**

```bash
cd /Users/arkavo/Projects/vrm-conformance
godot --headless --path adapters/godot-vrm --script tests/run_gdscript_tests.gd
python3 adapters/godot-vrm/tests/test_contract.py
```

Expected:
- GDScript runner: `11 passed, 0 failed`.
- Python contract: `7 passed`.
- Both exit 0.

- [ ] **Step 2: Confirm the wrapper is invokable by name on `PATH`-like access**

```bash
./adapters/godot-vrm/bin/godot-vrm-adapter < /dev/null
echo "exit: $?"
```

Expected: clean exit 0 within ~3 seconds (Godot startup + EOF on stdin → server loop returns → `quit(0)`).

- [ ] **Step 3: Verify the runner *would* be able to spawn this adapter**

This is a paper check — Phase 1 ops still return `Unimplemented`, so the runner cannot complete an `execute-test-plan` against godot-vrm. But the spawn shape matches every other adapter: a single executable path plus optional args. Confirm by reading:

```bash
grep -n 'render_with_adapter' scripts/bootstrap-goldens.sh
```

Expected: the existing `render_with_adapter` function signature `(renderer_name, renderer_version, adapter_bin, ...args)`. The godot-vrm adapter slots in as:

```bash
render_with_adapter "godot-vrm" "0.1.0" "$ROOT/adapters/godot-vrm/bin/godot-vrm-adapter"
```

No code change yet — that wiring is part of L3, not the scaffold. Note this in a one-line addendum to the godot-vrm README under a new `## Bootstrap wiring (L3)` section:

```markdown
## Bootstrap wiring (L3)

`scripts/bootstrap-goldens.sh` will gain a `SKIP_GODOT_VRM` env knob and a `render_with_adapter "godot-vrm" "<version>" "$ROOT/adapters/godot-vrm/bin/godot-vrm-adapter"` call once L3 produces real renders. Not wired during L1+L2 because Phase 1 ops return `Unimplemented`, so the runner cannot complete an `execute-test-plan` against this adapter yet.
```

Then commit the README addendum:

```bash
git add adapters/godot-vrm/README.md
git commit -m "docs(adapters/godot-vrm): note L3 bootstrap-script wiring"
```

---

## Out of scope (deferred to L3 plan)

- Installing/pinning `addons/godot-vrm/` (V-Sekai asset).
- Real `load_vrm` parsing the VRM 1.0 GLB extensions Godot doesn't natively understand.
- SubViewport + ViewportTexture render path with magenta clear color.
- `Engine.physics_ticks_per_second = 60` and the `step_physics` / `reset_physics` / `animate_root_transform` op set against godot-vrm's spring-bone implementation.
- `scripts/bootstrap-goldens.sh` integration (`SKIP_GODOT_VRM`, `render_with_adapter` call).
- `docs/findings.md` entries from a three-renderer consensus run.

A separate plan (`YYYY-MM-DD-adapter-godot-vrm-L3.md`) covers those.

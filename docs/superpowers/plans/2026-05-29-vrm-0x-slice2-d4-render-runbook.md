# Slice 2 — Task D4 render runbook (deferred to a machine with adapters)

**Status:** Code complete (Phases A–C, D1–D3 merged). This task is the *execution-time deliverable*: render the 0.x corpus through the real adapters and record the cross-version findings. It was deferred from the implementation session because that environment had only `godot` on PATH (no Unity/UniVRM, no Xcode 26/VMK, no Playwright/three-vrm).

**Run this on a machine where the real adapters are built** (per `CLAUDE.md` adapter status). 0.x conformance is **validator-structural + render-consensus**; recall the v0 assets are sphere-bodied (spring-bone adds the skinned chain), so the VMK 180° humanoid flip (VMK#299) does **not** confound these comparisons.

## 1. Generate the 0.x corpus

Either via the new bootstrap knob (renders too — needs adapters):
```bash
SPEC_VERSION=0.x scripts/bootstrap-goldens.sh        # writes under <GOLDENS_DIR>_v0/
```
…or just stage the assets (no render) for manual driving:
```bash
cargo build --release -p vrm-asset-generator
GEN=target/release/vrm-asset-generator
OUT=/tmp/corpus_v0
# Applicable sweeps (mirror scripts/bootstrap-goldens.sh's Applicable set):
for sub in emit-sweep emit-emissive-sweep emit-shade-multiply-texture-sweep \
           emit-matcap-texture-sweep emit-outline-width-multiply-texture-sweep \
           emit-pbr-textures-sweep emit-first-person-sweep \
           emit-springbone-sweep emit-springbone-swing-sweep \
           emit-springbone-collider-sweep emit-springbone-gravity-dir-sweep \
           emit-springbone-coupling-sweep emit-springbone-multichain-sweep \
           emit-sequence-sweep; do
  "$GEN" "$sub" --spec-version 0.x --output-dir "$OUT/$sub" --json >/dev/null && echo "OK $sub"
done
# (The 5 NotApplicable sweeps — shading-shift, rim-multiply, texture-transform,
#  extended-collider, taper — correctly reject --spec-version 0.x; do not run them.)
```

## 2. Render through each available adapter

Per-op adapters (three-vrm, VMK, godot) via `execute-test-plan`; batch adapter (UniVRM) via `execute-test-batch`. Example for one variant + one renderer:
```bash
cargo run -p vrm-runner -- execute-test-plan \
  --plan "$OUT/emit-sweep/mtoon_default.test.yaml" \
  --adapter-bin <ADAPTER_BIN> --adapter-args <...> \
  --asset-dir "$OUT/emit-sweep" --output-dir /tmp/render_v0/<renderer> \
  --renderer-name <renderer> --json
```
Adapter bins (build per each adapter's README):
- **godot**: `target/release/vrm-godot-shim` (Godot 4.x on PATH).
- **UniVRM**: `adapters/univrm/launcher.sh` via `execute-test-batch` (batched one-shot; Unity 6000.4.6f1 + Personal license).
- **three-vrm**: the adapter's node entry (after `npx playwright install chromium`).
- **VMK**: the built `vrm-metal-kit` adapter (Xcode 26 / macOS 26).

## 3. Diff: within-renderer cross-version FIRST, then cross-renderer

Per the methodology pin added in D3 (`docs/methodology.md` → "Spring-bone cross-version triage order"):

- **Spring-bone sweeps:** for each renderer, first compare its **0.x render vs its 1.0 render** of the same axis (same `SpringBoneParams`, `VRMC_springBone` vs `secondaryAnimation` emit). A within-renderer cross-version delta isolates a coordinate/unit/field-mapping bug in one of our two emit paths (`gravityDir` sign, `stiffiness` typo handling, deg/rad). Only after that, run cross-renderer `consensus-diff`.
- **MToon material sweeps:** the question is whether 0.x-delivered material params match 1.0 within a version (cross-renderer read). Use `consensus-diff`:
```bash
cargo run -p vrm-runner -- consensus-diff \
  --plan "$OUT/emit-sweep/mtoon_default.test.yaml" \
  --render univrm=/tmp/render_v0/univrm/mtoon_default_univrm.png \
  --render three-vrm=/tmp/render_v0/three-vrm/mtoon_default_three-vrm.png \
  --render vmk=/tmp/render_v0/vmk/mtoon_default_vmk.png --json
```

## 4. Record findings

Append a `docs/findings.md` entry: which 0.x sweep variants diverge from their 1.0 counterparts, and which renderers cluster. Note the sphere-geometry caveat (180° flip does not confound). Commit the findings entry. **Do NOT commit `goldens-cache/` PNGs** — goldens go to S3 per the manifest trust model.

## What "done" looks like
- A `docs/findings.md` slice-2 entry naming the divergent 0.x variants + renderer clusters.
- Validator already confirms 0-error structural conformance (run locally: `cargo test -p vrm-asset-generator -- --ignored`).
- Optional: push the 0.x golden PNGs to S3 + extend `goldens/manifest.json` (separate, gated by `manifest-validate.yml`).

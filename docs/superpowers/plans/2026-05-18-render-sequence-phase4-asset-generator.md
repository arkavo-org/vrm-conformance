# `render_sequence` Phase 4 — Asset Generator + Sequence-Capable Test Plans

> **For agentic workers:** Use superpowers:subagent-driven-development. RFC-0004 is Accepted; Phases 1–3 are landed (latest SHA `8407262`). The op surface, runner integration, diff aggregator, mock renderer reference impl are all in place — Phase 4 produces the test plans that finally exercise the sequence path through real corpus.

**Goal:** Add an `emit-sequence-sweep` CLI subcommand to `vrm-asset-generator` that emits the swing-sweep corpus as **sequence-mode test plans**. Each plan declares a `render_sequence:` block (no `animation:` block per Phase 2's validator rule). Existing single-frame swing plans (emitted via `emit-springbone-swing-sweep`) stay — Phase 5+ adapter work uses sequence plans; the single-frame variants stay alive until adapters drop them.

**Architecture:**

- **New plan builder** in `crates/vrm-asset-generator/src/sidecar.rs`: `build_spring_bone_swing_sequence_test_plan(params, asset_relpath)` — mirrors `build_spring_bone_swing_test_plan` but emits `render_sequence:` with `animate_root_transform` instead of `animation: { root_transform }`.
- **New emit helper** in `crates/vrm-asset-generator/src/emit.rs`: `emit_with_sidecars_spring_bone_swing_sequence` — same VRM body as `emit_with_sidecars_spring_bone_swing`, different test plan.
- **New CLI subcommand** in `crates/vrm-asset-generator/src/cli.rs`: `Cmd::EmitSequenceSweep` — emits the 18 swing variants as sequence plans, distinguishing IDs with a `swing_seq_` prefix.
- **Tests** anchor the plan structure and the validator-acceptance (no animation + render_sequence ambiguity).

**Capture rate choice (60 frames @ 30 Hz):**

The single-frame swing plan animates `[0,0,0] → [0.15, 0, 0]` over 0.25 s. The sequence variant spreads the same translation across **60 frames at 30 Hz capture (2.0 s total animation)** with `physics_dt_seconds = 1/60`. This trades a slower head-turn (less physics excitation) for a much richer temporal signal: 60 frames of captured trajectory vs. 1 single end-frame. Cross-renderer divergence becomes visible as per-frame SSIM drift rather than collapsed into one number. The conformance question shifts from "do renderers agree on the swing-peak pose?" to "do renderers agree on the trajectory shape?" — strictly more informative.

**Tech stack:** Rust only. No adapter or runner changes.

**Spec:** [`rfcs/0004-render-sequence-op.md`](../../../rfcs/0004-render-sequence-op.md) — op contract. [`docs/methodology.md`](../../methodology.md) — sequence-capture pins.

---

## File structure

**Modify:**
- `crates/vrm-asset-generator/src/sidecar.rs` — new `build_spring_bone_swing_sequence_test_plan`
- `crates/vrm-asset-generator/src/emit.rs` — new `emit_with_sidecars_spring_bone_swing_sequence`
- `crates/vrm-asset-generator/src/cli.rs` — new `Cmd::EmitSequenceSweep` variant + match arm

**Create:**
- `crates/vrm-asset-generator/tests/sequence_sweep.rs` — emission + plan-validation integration test

---

## Task 1: `build_spring_bone_swing_sequence_test_plan` in sidecar.rs

**Files:**
- Modify: `crates/vrm-asset-generator/src/sidecar.rs`

- [ ] **Step 1.1: Read the existing swing plan builder**

Read `crates/vrm-asset-generator/src/sidecar.rs:155-168` (the `build_spring_bone_swing_test_plan` function). The new builder follows the same shape — settle steps, MToon defaults, spec_section labelling — but replaces the `animation:` block with `render_sequence:`.

- [ ] **Step 1.2: Add the new builder**

Append after `build_spring_bone_swing_test_plan`:

```rust
/// Sequence variant of the swing plan. Emits a `render_sequence:` block
/// instead of `animation: { root_transform }` so the runner dispatches
/// `render_sequence` (multi-frame capture) rather than the single-frame
/// `render` path.
///
/// Capture rate: 60 frames @ 30 Hz with `physics_dt_seconds = 1/60`.
/// Animation: `[0,0,0] → [0.15, 0, 0]` linearly across all 60 frames.
/// Total animation duration: 2.0 s. This is slower than the single-frame
/// swing plan's 0.25 s, trading reduced physics excitation for a much
/// richer temporal signal — 60 frames of captured trajectory vs. 1
/// single end-frame.
///
/// The `physics` settle (30 steps before frame 0) runs first; then the
/// adapter steps physics per frame while interpolating the root
/// translation. Per RFC-0004's failure-modes table, `physics_dt_seconds
/// > 1/60` is a methodology-pin SHOULD-reject; adapters validate.
pub fn build_spring_bone_swing_sequence_test_plan(
    params: &MToonParams,
    asset_relpath: &str,
) -> TestPlan {
    // Start from the SETTLE plan (NOT the swing plan): the swing plan
    // carries an `animation:` block which is mutually exclusive with
    // `render_sequence:` per TestPlan::validate.
    let mut plan = build_spring_bone_test_plan(params, asset_relpath);
    plan.render_sequence = Some(RenderSequenceBlock {
        frame_count: 60,
        frame_hz: 30.0,
        physics_dt_seconds: 1.0 / 60.0,
        output_format: SequenceFormat::PngSequence,
        animate_root_transform: Some(SequenceRootTransformAnimation {
            translation_start: [0.0, 0.0, 0.0],
            translation_end: [0.15, 0.0, 0.0],
        }),
        apply_vrma: None,
        temporal_ssim_threshold: None,  // uses RFC-0004 default 0.90
    });
    plan.spec_section = "VRMC_materials_mtoon + VRMC_springBone (sequence)".into();
    plan
}
```

`SequenceRootTransformAnimation` is the type name used in `crates/vrm-test-plan/src/lib.rs` (Phase 2 Task 5 renamed it from `RootTransformAnimation` to avoid colliding with the existing single-frame `RootTransformAnimation`). Verify the import path — likely already in scope via `use vrm_test_plan::*` or similar.

- [ ] **Step 1.3: Verify imports**

At the top of `sidecar.rs`, check that `RenderSequenceBlock`, `SequenceFormat`, `SequenceRootTransformAnimation` are imported. The file's existing `use` for `vrm_test_plan` types should be extended.

- [ ] **Step 1.4: Add a unit test**

Append a `#[cfg(test)] mod render_sequence_tests` block (or inline test if the file's convention is inline):

```rust
#[cfg(test)]
mod render_sequence_tests {
    use super::*;

    #[test]
    fn swing_sequence_plan_uses_render_sequence_not_animation() {
        let params = MToonParams::defaults("swing_seq_test");
        let plan = build_spring_bone_swing_sequence_test_plan(&params, "swing_seq_test.vrm");

        // Sequence plan must NOT carry animation (validator would reject).
        assert!(plan.animation.is_none(), "sequence plan must omit animation");
        // Must carry render_sequence.
        let seq = plan.render_sequence.as_ref().expect("render_sequence required");
        assert_eq!(seq.frame_count, 60);
        assert!((seq.frame_hz - 30.0).abs() < 1e-6);
        assert!((seq.physics_dt_seconds - 1.0 / 60.0).abs() < 1e-9);
        assert!(matches!(seq.output_format, SequenceFormat::PngSequence));

        let anim = seq.animate_root_transform.as_ref().expect("translation required");
        assert_eq!(anim.translation_start, [0.0, 0.0, 0.0]);
        assert_eq!(anim.translation_end, [0.15, 0.0, 0.0]);

        // Physics settle preserved from the settle plan.
        assert_eq!(plan.physics.as_ref().unwrap().settle_steps, 30);

        // Validator accepts the plan (no animation + render_sequence collision).
        assert!(plan.validate().is_ok());
    }
}
```

- [ ] **Step 1.5: Build + clippy + run test**

```
cargo test -p vrm-asset-generator --lib render_sequence_tests
cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 1.6: Commit**

```bash
git add crates/vrm-asset-generator/src/sidecar.rs
git commit -m "$(cat <<'EOF'
feat(vrm-asset-generator): build_spring_bone_swing_sequence_test_plan

Sequence variant of the swing plan: 60 frames @ 30 Hz with
physics_dt_seconds = 1/60, root translation [0,0,0] → [0.15,0,0]
linearly across all 60 frames. Total animation duration: 2.0 s.

Trades reduced physics excitation (slower head-turn than the 0.25 s
single-frame swing plan) for a much richer temporal signal — 60 frames
of captured trajectory vs 1 single end-frame.
EOF
)"
```

---

## Task 2: `emit_with_sidecars_spring_bone_swing_sequence` in emit.rs

**Files:**
- Modify: `crates/vrm-asset-generator/src/emit.rs`

- [ ] **Step 2.1: Add the emit helper**

After `emit_with_sidecars_spring_bone_swing` (around line 493), append:

```rust
/// Same VRM body as `emit_with_sidecars_spring_bone_swing`, but the
/// `.test.yaml` carries a `render_sequence:` block (sequence-mode) instead
/// of `animation: { root_transform }` (single-frame mode). Used by the
/// `emit-sequence-sweep` CLI subcommand.
pub fn emit_with_sidecars_spring_bone_swing_sequence(
    mtoon: &MToonParams,
    spring_bone: &SpringBoneParams,
    stem: &Utf8Path,
) -> Result<()> {
    let vrm_path = stem.with_extension("vrm");
    emit_vrm_with_spring_bone(mtoon, spring_bone, &vrm_path)?;

    let meta_path = stem.with_extension("meta.json");
    write_meta_json(mtoon, Some(spring_bone), &vrm_path, &meta_path)?;

    let yaml_path = stem.with_extension("test.yaml");
    let asset_relpath = vrm_path
        .file_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let plan = crate::sidecar::build_spring_bone_swing_sequence_test_plan(mtoon, &asset_relpath);
    write_test_yaml(&plan, &yaml_path)?;

    Ok(())
}
```

The helper is identical to `emit_with_sidecars_spring_bone_swing` except for the plan builder call.

- [ ] **Step 2.2: Build + clippy**

```
cargo build -p vrm-asset-generator
cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 2.3: Commit**

```bash
git add crates/vrm-asset-generator/src/emit.rs
git commit -m "$(cat <<'EOF'
feat(vrm-asset-generator): emit_with_sidecars_spring_bone_swing_sequence

Mirror of emit_with_sidecars_spring_bone_swing but the test plan uses
render_sequence: instead of animation: { root_transform }. Same VRM
body — only the test plan differs.
EOF
)"
```

---

## Task 3: `Cmd::EmitSequenceSweep` CLI subcommand

**Files:**
- Modify: `crates/vrm-asset-generator/src/cli.rs`

- [ ] **Step 3.1: Add the subcommand variant**

Find the `Cmd` enum in `cli.rs` and add a new variant near `EmitSpringboneSwingSweep`:

```rust
/// Emit the spring-bone sequence-mode sweep (18 assets). Each asset's
/// `.test.yaml` carries a `render_sequence:` block instead of an
/// `animation:` block, dispatching the runner's render_sequence path
/// instead of the single-frame render path.
///
/// Asset IDs are prefixed `swing_seq_` to keep them distinct from the
/// existing single-frame `swing_` variants in the cross-renderer
/// goldens manifest (both can coexist).
EmitSequenceSweep {
    #[arg(long)]
    output_dir: Utf8PathBuf,
    #[arg(long)]
    json: bool,
},
```

Match the position-in-enum and `#[command(...)]` attributes of the existing `EmitSpringboneSwingSweep` variant so the help output groups them naturally.

- [ ] **Step 3.2: Add the dispatch arm**

Append a new arm to the `match cli.command` block, modeled after the existing `Cmd::EmitSpringboneSwingSweep` arm (around line 375). The change is: call `emit_with_sidecars_spring_bone_swing_sequence` instead of `emit_with_sidecars_spring_bone_swing`, and use ID prefix `swing_seq_` instead of `swing_`.

```rust
Cmd::EmitSequenceSweep {
    output_dir,
    json: emit_json,
} => {
    use crate::emit::emit_with_sidecars_spring_bone_swing_sequence;
    use crate::spring_bone::spring_bone_basic_sweep;

    std::fs::create_dir_all(&output_dir)?;
    let variants = spring_bone_basic_sweep();
    let total = variants.len();

    let mut emitted = Vec::new();
    for (i, spring) in variants.iter().enumerate() {
        let seq_id = format!("swing_seq_{}", spring.id);
        if emit_json {
            let evt = json!({
                "event": "progress",
                "op": "emit-sequence-sweep",
                "index": i,
                "total": total,
                "id": seq_id
            });
            eprintln!("{}", serde_json::to_string(&evt)?);
        } else {
            eprintln!("[{:3}/{}] {}", i + 1, total, seq_id);
        }

        let mut prefixed = spring.clone();
        prefixed.id = seq_id.clone();
        prefixed.spring_name = format!("{seq_id}_chain");
        let stem = output_dir.join(&seq_id);
        let mtoon = MToonParams::defaults(&seq_id);
        emit_with_sidecars_spring_bone_swing_sequence(&mtoon, &prefixed, &stem)?;
        emitted.push(stem);
    }

    if emit_json {
        let summary = json!({
            "ok": true,
            "count": emitted.len(),
            "output_dir": output_dir,
            "assets": emitted
        });
        println!("{}", serde_json::to_string(&summary)?);
    } else {
        println!(
            "emitted {} sequence-mode spring-bone assets to {}",
            emitted.len(),
            output_dir
        );
    }
    Ok(())
}
```

- [ ] **Step 3.3: Smoke-run the new command**

```bash
mkdir -p /tmp/seq-sweep
cargo run --release -p vrm-asset-generator -- emit-sequence-sweep --output-dir /tmp/seq-sweep
ls /tmp/seq-sweep | wc -l    # should be 18 * 3 = 54 files
head -50 /tmp/seq-sweep/swing_seq_stiffness_0p1.test.yaml  # verify render_sequence block
```

The 18-asset count comes from `spring_bone_basic_sweep().len()`; verify with the run.

- [ ] **Step 3.4: Build + clippy**

```
cargo build -p vrm-asset-generator
cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 3.5: Commit**

```bash
git add crates/vrm-asset-generator/src/cli.rs
git commit -m "$(cat <<'EOF'
feat(vrm-asset-generator): emit-sequence-sweep CLI subcommand

Emits the 18 spring-bone basic-sweep variants as sequence-mode test
plans. Each .test.yaml carries a render_sequence: block — runner
dispatches render_sequence instead of single-frame render.

Asset IDs prefixed swing_seq_ to coexist with the existing swing_
single-frame variants in the goldens manifest.
EOF
)"
```

---

## Task 4: Integration test for emission + plan validation

**Files:**
- Create: `crates/vrm-asset-generator/tests/sequence_sweep.rs`

- [ ] **Step 4.1: Add the test**

```rust
//! Integration test for the emit-sequence-sweep subcommand.
//! Runs the binary, asserts the 18 triplets emit correctly, and that
//! each plan validates (no animation + render_sequence collision).

use camino::Utf8PathBuf;
use vrm_test_plan::TestPlan;

#[test]
fn emit_sequence_sweep_produces_18_valid_triplets() {
    let dir = tempfile::tempdir().unwrap();
    let out = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_vrm-asset-generator"))
        .args([
            "emit-sequence-sweep",
            "--output-dir", out.as_str(),
        ])
        .status()
        .expect("asset-generator must be runnable");
    assert!(status.success(), "emit-sequence-sweep exited non-zero");

    // Each asset = 3 files (.vrm + .meta.json + .test.yaml). 18 variants ⇒ 54 files.
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(entries.len(), 54, "expected 18 triplets = 54 files, got {}", entries.len());

    // Every test.yaml in the dir must:
    //   - validate (validator accepts the plan)
    //   - declare render_sequence
    //   - NOT declare animation (mutual exclusion)
    //   - use the swing_seq_ id prefix
    let yamls: Vec<_> = entries
        .iter()
        .filter(|f| f.ends_with(".test.yaml"))
        .collect();
    assert_eq!(yamls.len(), 18, "expected 18 test.yaml files");

    for yaml_name in &yamls {
        let yaml_path = out.join(yaml_name);
        let raw = std::fs::read_to_string(yaml_path.as_std_path()).unwrap();
        let plan: TestPlan = serde_yml::from_str(&raw)
            .unwrap_or_else(|e| panic!("{yaml_name} failed to parse: {e}"));

        assert!(plan.id.starts_with("swing_seq_"), "{yaml_name}: id should start with swing_seq_, got {}", plan.id);
        assert!(plan.render_sequence.is_some(), "{yaml_name}: render_sequence missing");
        assert!(plan.animation.is_none(), "{yaml_name}: animation must be absent (mutually exclusive with render_sequence)");
        assert!(plan.validate().is_ok(), "{yaml_name}: validator rejected");

        let seq = plan.render_sequence.unwrap();
        assert_eq!(seq.frame_count, 60);
        assert!((seq.frame_hz - 30.0).abs() < 1e-6);
        assert!((seq.physics_dt_seconds - 1.0 / 60.0).abs() < 1e-9);
        let anim = seq.animate_root_transform.unwrap();
        assert_eq!(anim.translation_end, [0.15, 0.0, 0.0]);
    }
}
```

If `serde_yml` isn't a dev-dependency of `vrm-asset-generator`, add it. Inspect `crates/vrm-asset-generator/Cargo.toml`.

The exact count `18` depends on `spring_bone_basic_sweep().len()`. If it's actually 17 or 20 (sweep generator may have evolved), update the assertion to match. The test should fail at the assertion with a clear message if the count diverges.

- [ ] **Step 4.2: Run + commit**

```
cargo test -p vrm-asset-generator --test sequence_sweep
cargo clippy --workspace --all-targets -- -D warnings
```

```bash
git add crates/vrm-asset-generator/tests/sequence_sweep.rs crates/vrm-asset-generator/Cargo.toml
git commit -m "$(cat <<'EOF'
test(vrm-asset-generator): integration test for emit-sequence-sweep

Runs the binary, asserts 18 triplets emit correctly, each test.yaml
parses, validates, declares render_sequence (not animation), uses the
swing_seq_ id prefix, and carries the expected 60-frame / 30 Hz /
1/60-physics / [0,0,0]→[0.15,0,0] configuration.
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
```

- [ ] **Step 5.2: Commit any fmt fixes (if any)**

---

## Phase 4 completion checklist

- [ ] `build_spring_bone_swing_sequence_test_plan` in sidecar.rs with unit test
- [ ] `emit_with_sidecars_spring_bone_swing_sequence` in emit.rs
- [ ] `Cmd::EmitSequenceSweep` in cli.rs with the dispatch arm
- [ ] Integration test asserts 18 valid sequence plans emit
- [ ] Each emitted plan: declares render_sequence, omits animation, validates, uses swing_seq_ prefix
- [ ] `cargo fmt --check` clean
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [ ] `cargo test --workspace` green
- [ ] three-vrm npm test green

After Phase 4, the runner can drive the full 18-variant sequence corpus through any sequence-capable adapter. Phase 5 (vrm-metal-kit as first real implementer) is unblocked — VMK's `render_sequence` impl will be diffed against the mock (Phase 3) using the corpus emitted here.

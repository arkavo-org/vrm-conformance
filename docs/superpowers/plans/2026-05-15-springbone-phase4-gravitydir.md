# VRMC_springBone Phase 4 — gravityDir Sweep Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development.

**Goal:** Ship a 4-direction gravity sweep (8 plans = 4 directions × settle/swing) that flushes out adapters hard-coding `gravity_dir = [0,-1,0]`. No new types needed — `SpringBoneParams.gravity_dir: [f32; 3]` already supports this.

**Spec:** `docs/superpowers/specs/2026-05-15-springbone-conformance-closure-design.md` §6.

---

## Single task (gravity is small enough)

**Files modified:**
- `crates/vrm-asset-generator/src/sweep.rs` — add `spring_bone_gravity_dir_sweep()` returning 4 variants
- `crates/vrm-asset-generator/src/cli.rs` — add `emit-springbone-gravity-dir-sweep` subcommand
- `crates/vrm-asset-generator/src/emit.rs` — if a new sidecar emit wrapper is needed (likely just reuse existing `emit_with_sidecars_spring_bone` + `_swing` since gravity_dir is already a SpringBoneParams field, no scene-shape change)
- `docs/findings.md` — phase 4 entry

### Steps

- [ ] **Step 1: Write failing tests** in `crates/vrm-asset-generator/src/sweep.rs`:

```rust
#[cfg(test)]
mod gravity_dir_sweep_tests {
    use super::*;

    #[test]
    fn gravity_dir_sweep_produces_4_variants() {
        let variants = spring_bone_gravity_dir_sweep();
        assert_eq!(variants.len(), 4);
    }

    #[test]
    fn gravity_dir_sweep_covers_4_distinct_directions() {
        let variants = spring_bone_gravity_dir_sweep();
        let dirs: std::collections::HashSet<[i32; 3]> = variants
            .iter()
            .map(|p| {
                let g = p.gravity_dir;
                // Multiply by 10 to compare with tolerance via integer hashing.
                [(g[0] * 10.0) as i32, (g[1] * 10.0) as i32, (g[2] * 10.0) as i32]
            })
            .collect();
        assert_eq!(dirs.len(), 4, "all four directions must be distinct");
    }

    #[test]
    fn gravity_dir_sweep_baseline_first() {
        let variants = spring_bone_gravity_dir_sweep();
        assert_eq!(variants[0].gravity_dir, [0.0, -1.0, 0.0],
            "first variant should be the baseline -Y direction");
    }

    #[test]
    fn gravity_dir_sweep_includes_anti_sideways_oblique() {
        let variants = spring_bone_gravity_dir_sweep();
        let has_antigravity = variants.iter().any(|p| p.gravity_dir == [0.0, 1.0, 0.0]);
        let has_sideways_x = variants.iter().any(|p| p.gravity_dir == [1.0, 0.0, 0.0]);
        let has_oblique = variants.iter().any(|p| {
            (p.gravity_dir[0] - 0.7).abs() < 1e-6 && (p.gravity_dir[1] - (-0.7)).abs() < 1e-6
        });
        assert!(has_antigravity && has_sideways_x && has_oblique,
            "must include anti, sideways, and oblique");
    }

    #[test]
    fn gravity_dir_sweep_uses_unique_ids() {
        let variants = spring_bone_gravity_dir_sweep();
        let ids: std::collections::HashSet<_> = variants.iter().map(|p| p.id.clone()).collect();
        assert_eq!(ids.len(), 4);
    }
}
```

- [ ] **Step 2: Run, expect failure:**
  ```
  cd /Users/arkavo/Projects/vrm-conformance && cargo test -p vrm-asset-generator gravity_dir_sweep
  ```

- [ ] **Step 3: Implement `spring_bone_gravity_dir_sweep()`** in `sweep.rs`:

```rust
pub fn spring_bone_gravity_dir_sweep() -> Vec<SpringBoneParams> {
    let directions = [
        ("default",  [0.0_f32, -1.0,  0.0]),
        ("anti",     [0.0,      1.0,  0.0]),
        ("sideways", [1.0,      0.0,  0.0]),
        ("oblique",  [0.7,     -0.7,  0.0]),
    ];

    directions
        .iter()
        .map(|(name, dir)| {
            let mut p = SpringBoneParams::defaults(format!("springbone_gravity_dir_{}", name));
            p.gravity_dir = *dir;
            p
        })
        .collect()
}
```

(`SpringBoneParams` is already imported in `sweep.rs` — verify.)

- [ ] **Step 4: Add CLI subcommand `emit-springbone-gravity-dir-sweep`** in `cli.rs`. Pattern: same as `emit-springbone-swing-sweep`, but iterating `spring_bone_gravity_dir_sweep()`. The subcommand emits BOTH settle and swing for each direction — 4 × 2 = 8 plans. Reuse existing `emit_with_sidecars_spring_bone` and `emit_with_sidecars_spring_bone_swing` (no new emit function needed; these already accept `SpringBoneParams` with custom gravity_dir).

- [ ] **Step 5: Smoke test:**
  ```
  cd /Users/arkavo/Projects/vrm-conformance
  cargo build -p vrm-asset-generator --release
  mkdir -p /tmp/phase4-sweep && ./target/release/vrm-asset-generator emit-springbone-gravity-dir-sweep --output-dir /tmp/phase4-sweep
  ls /tmp/phase4-sweep/*.vrm | wc -l  # expect 8
  ls /tmp/phase4-sweep/*.test.yaml | wc -l  # expect 8
  ```

- [ ] **Step 6: Validate one asset** if the validator is installed:
  ```
  ls .tools/vrm-validator-cli 2>/dev/null && .tools/vrm-validator-cli /tmp/phase4-sweep/springbone_gravity_dir_default_settle.vrm 2>&1 | tail -10
  ```

- [ ] **Step 7: docs/findings.md phase-4 entry:**

```markdown
## Phase 4 — gravityDir sweep landed (8 plans)

**Trigger:** Phase 3 extended-collider sweep merged. Phase 4 closes the gravity-direction axis: prior sweeps held `gravity_dir = [0,-1,0]` constant, so any adapter hard-coding -Y would pass cross-renderer diff silently.

**Shipped:** `emit-springbone-gravity-dir-sweep` subcommand emitting 8 plans (4 directions × settle/swing): default (-Y), anti (+Y), sideways (+X), oblique (+0.7, -0.7, 0). All other SpringBoneParams (joint_count, stiffness, drag, gravity_power) held at defaults so the gravity-direction axis is unconfounded.

**Forward:** Phase 5 — per-joint parameter taper (JointVec refactor).
```

- [ ] **Step 8: Run, lint, commit (single commit for this whole phase):**
  ```
  cd /Users/arkavo/Projects/vrm-conformance
  cargo test -p vrm-asset-generator gravity_dir_sweep
  cargo clippy --workspace --all-targets -- -D warnings
  cargo fmt --all -- --check
  git add crates/vrm-asset-generator/src/sweep.rs crates/vrm-asset-generator/src/cli.rs docs/findings.md && \
    git commit -m "$(cat <<'EOF'
  feat(vrm-asset-generator): gravity_dir 4-direction sweep (8 plans)

  emit-springbone-gravity-dir-sweep produces 4 baseline-only variants
  (default -Y, anti +Y, sideways +X, oblique +0.7/-0.7) × settle/swing
  = 8 plans. Flushes adapters that hard-code -Y in shortcut paths.
  Phase 4 of the seven-phase springbone gap closure design.

  Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Acceptance

- [ ] `cargo test --workspace` green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` green
- [ ] `cargo fmt --all -- --check` green
- [ ] 8 vrms + 8 test plans emitted
- [ ] findings.md entry present

//! Absolute non-penetration metric for spring-bone vs world-fixed colliders.
//! A conformant solver keeps joints outside the collider surface. Tunneling =
//! a joint inside the surface beyond tolerance on any captured frame.
//! Unlike `positions::diff_positions` (drift vs a reference) this is an
//! absolute geometric invariant — no oracle.
use serde::{Deserialize, Serialize};
use vrm_ops::tools::SpringPositions;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ColliderSpec {
    Sphere {
        center: [f32; 3],
        radius: f32,
    },
    Capsule {
        a: [f32; 3],
        b: [f32; 3],
        radius: f32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PenetrationReport {
    pub max_penetration_depth_m: f32,
    pub epsilon_m: f32,
    pub worst_frame: usize,
    pub worst_spring: usize,
    pub worst_joint: usize,
    pub passed: bool,
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    let (dx, dy, dz) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn dist_point_segment(p: [f32; 3], a: [f32; 3], b: [f32; 3]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ap = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
    let ab2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    let t = if ab2 <= 0.0 {
        0.0
    } else {
        ((ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2]) / ab2).clamp(0.0, 1.0)
    };
    let proj = [a[0] + ab[0] * t, a[1] + ab[1] * t, a[2] + ab[2] * t];
    dist(p, proj)
}

/// Signed distance from point `p` to the collider surface. Negative = inside.
fn signed_distance(p: [f32; 3], c: &ColliderSpec) -> f32 {
    match c {
        ColliderSpec::Sphere { center, radius } => dist(p, *center) - radius,
        ColliderSpec::Capsule { a, b, radius } => dist_point_segment(p, *a, *b) - radius,
    }
}

/// Worst (deepest) penetration of any joint into any collider across all
/// frames. `frames[f]` is the per-spring positions captured at frame `f`.
/// `max_penetration_depth_m = max(0, -min signed_distance)`; passes iff that
/// depth <= `epsilon_m`. Empty input passes with depth 0.
pub fn worst_penetration(
    frames: &[Vec<SpringPositions>],
    colliders: &[ColliderSpec],
    epsilon_m: f32,
) -> PenetrationReport {
    let mut deepest = 0.0_f32;
    let (mut wf, mut ws, mut wj) = (0usize, 0usize, 0usize);
    for (fi, springs) in frames.iter().enumerate() {
        for (si, spring) in springs.iter().enumerate() {
            for (ji, &p) in spring.joint_positions.iter().enumerate() {
                for c in colliders {
                    let depth = -signed_distance(p, c);
                    if depth > deepest {
                        deepest = depth;
                        wf = fi;
                        ws = si;
                        wj = ji;
                    }
                }
            }
        }
    }
    PenetrationReport {
        max_penetration_depth_m: deepest,
        epsilon_m,
        worst_frame: wf,
        worst_spring: ws,
        worst_joint: wj,
        passed: deepest <= epsilon_m,
    }
}

/// Like [`worst_penetration`] but the colliders move per frame: `frames[i]`
/// joints are tested only against `colliders_per_frame[i]`. Iterates the
/// shorter of the two lengths. Used for bone-attached (synthetic) colliders
/// captured alongside positions. Empty input passes with depth 0.
///
/// When `exclude_root_joints` is `true`, joint index 0 of each spring chain is
/// skipped. Root joints (index 0) are kinematically driven by their parent bone
/// and are never pushed out by the collision solver, so for bone-attached
/// (synthetic) colliders they sit inside the collider by construction and would
/// dominate the metric — matching VMK's `HairHeadCollisionTests` which excludes
/// root joints.
pub fn worst_penetration_per_frame(
    frames: &[Vec<SpringPositions>],
    colliders_per_frame: &[Vec<ColliderSpec>],
    epsilon_m: f32,
    exclude_root_joints: bool,
) -> PenetrationReport {
    let mut deepest = 0.0_f32;
    let (mut wf, mut ws, mut wj) = (0usize, 0usize, 0usize);
    let n = frames.len().min(colliders_per_frame.len());
    for fi in 0..n {
        for (si, spring) in frames[fi].iter().enumerate() {
            for (ji, &p) in spring.joint_positions.iter().enumerate() {
                if exclude_root_joints && ji == 0 {
                    continue;
                }
                for c in &colliders_per_frame[fi] {
                    let depth = -signed_distance(p, c);
                    if depth > deepest {
                        deepest = depth;
                        wf = fi;
                        ws = si;
                        wj = ji;
                    }
                }
            }
        }
    }
    PenetrationReport {
        max_penetration_depth_m: deepest,
        epsilon_m,
        worst_frame: wf,
        worst_spring: ws,
        worst_joint: wj,
        passed: deepest <= epsilon_m,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sp(joints: Vec<[f32; 3]>) -> SpringPositions {
        SpringPositions {
            name: "c".into(),
            joint_positions: joints,
        }
    }

    #[test]
    fn joint_outside_sphere_passes() {
        let c = ColliderSpec::Sphere {
            center: [0.0, 0.0, 0.0],
            radius: 0.05,
        };
        let frames = vec![vec![sp(vec![[0.10, 0.0, 0.0]])]];
        let r = worst_penetration(&frames, &[c], 0.002);
        assert!(r.passed);
        assert_eq!(r.max_penetration_depth_m, 0.0);
    }

    #[test]
    fn joint_inside_sphere_beyond_epsilon_fails_and_locates() {
        let c = ColliderSpec::Sphere {
            center: [0.0, 0.0, 0.0],
            radius: 0.05,
        };
        let frames = vec![
            vec![sp(vec![[0.10, 0.0, 0.0]])],
            vec![sp(vec![[0.10, 0.0, 0.0], [0.02, 0.0, 0.0]])],
        ];
        let r = worst_penetration(&frames, &[c], 0.002);
        assert!(!r.passed);
        assert!((r.max_penetration_depth_m - 0.03).abs() < 1e-5);
        assert_eq!(r.worst_frame, 1);
        assert_eq!(r.worst_joint, 1);
    }

    #[test]
    fn shallow_penetration_within_epsilon_passes() {
        let c = ColliderSpec::Sphere {
            center: [0.0, 0.0, 0.0],
            radius: 0.05,
        };
        let frames = vec![vec![sp(vec![[0.049, 0.0, 0.0]])]]; // 1 mm in, eps 2 mm
        let r = worst_penetration(&frames, &[c], 0.002);
        assert!(r.passed);
    }

    #[test]
    fn capsule_distance_is_to_segment() {
        let c = ColliderSpec::Capsule {
            a: [0.0, -0.1, 0.0],
            b: [0.0, 0.1, 0.0],
            radius: 0.03,
        };
        let frames = vec![vec![sp(vec![[0.02, 0.0, 0.0]])]];
        let r = worst_penetration(&frames, &[c], 0.002);
        assert!(!r.passed);
        assert!((r.max_penetration_depth_m - 0.01).abs() < 1e-5);
    }

    #[test]
    fn capsule_endpoint_cap_is_spherical() {
        // point beyond endpoint b along +Y: distance is to the cap (endpoint), not the infinite line
        let c = ColliderSpec::Capsule {
            a: [0.0, -0.1, 0.0],
            b: [0.0, 0.1, 0.0],
            radius: 0.03,
        };
        // point at (0, 0.2, 0): nearest segment point is endpoint b (0,0.1,0); dist 0.1; pen = 0.1-0.03 <0 → outside
        let frames = vec![vec![sp(vec![[0.0, 0.2, 0.0]])]];
        let r = worst_penetration(&frames, &[c], 0.002);
        assert!(r.passed, "point beyond cap is outside");
    }

    #[test]
    fn empty_frames_pass_zero_depth() {
        let c = ColliderSpec::Sphere {
            center: [0.0, 0.0, 0.0],
            radius: 0.05,
        };
        let r = worst_penetration(&[], &[c], 0.002);
        assert!(r.passed);
        assert_eq!(r.max_penetration_depth_m, 0.0);
    }

    #[test]
    fn per_frame_collider_moves_with_frame() {
        // Joint fixed at x=0.10. Collider sphere (r=0.05) sits at x=0.20 in
        // frame 0 (joint outside) and sweeps to x=0.12 in frame 1 (joint 0.02
        // inside the surface → 0.03 penetration).
        let frames = vec![
            vec![sp(vec![[0.10, 0.0, 0.0]])],
            vec![sp(vec![[0.10, 0.0, 0.0]])],
        ];
        let colliders_per_frame = vec![
            vec![ColliderSpec::Sphere {
                center: [0.20, 0.0, 0.0],
                radius: 0.05,
            }],
            vec![ColliderSpec::Sphere {
                center: [0.12, 0.0, 0.0],
                radius: 0.05,
            }],
        ];
        let r = worst_penetration_per_frame(&frames, &colliders_per_frame, 0.002, false);
        assert!(!r.passed);
        assert!((r.max_penetration_depth_m - 0.03).abs() < 1e-5);
        assert_eq!(r.worst_frame, 1);
    }

    #[test]
    fn per_frame_excludes_root_joint_when_requested() {
        // Joint 0 (root) is deep inside the collider; joint 1 is outside.
        let frames = vec![vec![sp(vec![[0.0, 0.0, 0.0], [0.10, 0.0, 0.0]])]];
        let colliders_per_frame = vec![vec![ColliderSpec::Sphere {
            center: [0.0, 0.0, 0.0],
            radius: 0.05,
        }]];
        // Including root: deep penetration at joint 0.
        let incl = worst_penetration_per_frame(&frames, &colliders_per_frame, 0.002, false);
        assert!(!incl.passed);
        assert_eq!(incl.worst_joint, 0);
        // Excluding root: only joint 1 considered → outside → passes.
        let excl = worst_penetration_per_frame(&frames, &colliders_per_frame, 0.002, true);
        assert!(excl.passed);
        assert_eq!(excl.max_penetration_depth_m, 0.0);
    }

    #[test]
    fn per_frame_empty_colliders_for_a_frame_is_skipped() {
        let frames = vec![
            vec![sp(vec![[0.0, 0.0, 0.0]])],
            vec![sp(vec![[0.0, 0.0, 0.0]])],
        ];
        let colliders_per_frame = vec![
            vec![],
            vec![ColliderSpec::Sphere {
                center: [0.0, 0.0, 0.0],
                radius: 0.05,
            }],
        ];
        let r = worst_penetration_per_frame(&frames, &colliders_per_frame, 0.002, false);
        assert!(!r.passed);
        assert_eq!(r.worst_frame, 1);
    }
}

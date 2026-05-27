//! Humanoid bone-name normalization v0 ↔ v1.
//!
//! Bone names are mostly identical between specs. This module exists for
//! API symmetry and to flag any renames that emerge (currently: none
//! confirmed in slice 1; module is mostly identity).

pub fn normalize_bone_name_v0_to_v1(v0_name: &str) -> String {
    // Slice 1: identity mapping. If specific renames surface during
    // adapter testing, they get added here as named match arms with
    // citations to the relevant spec section.
    v0_name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_bone_names_pass_through() {
        for name in &[
            "hips",
            "spine",
            "chest",
            "neck",
            "head",
            "leftUpperArm",
            "rightUpperLeg",
            "leftEye",
            "rightEye",
            "leftIndexProximal",
            "rightThumbDistal",
        ] {
            assert_eq!(normalize_bone_name_v0_to_v1(name), *name);
        }
    }
}

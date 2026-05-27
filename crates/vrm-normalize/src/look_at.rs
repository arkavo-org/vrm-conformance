//! `look_at` state normalization v0 ↔ v1.
//!
//! Both spec versions ultimately express look_at as yaw + pitch angles
//! (degrees). Normalization is a passthrough of those scalars; the
//! difference is in how the *adapter* computes them from the asset
//! definition (curve-based in 0.x, range-mapping in 1.0), which is the
//! adapter's responsibility, not this crate's.

pub fn normalize_look_at_state_v0_to_v1(yaw_degrees: f32, pitch_degrees: f32) -> (f32, f32) {
    (yaw_degrees, pitch_degrees)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaw_pitch_pass_through() {
        let (yaw, pitch) = normalize_look_at_state_v0_to_v1(10.0, -5.0);
        assert_eq!(yaw, 10.0);
        assert_eq!(pitch, -5.0);
    }

    #[test]
    fn zero_pass_through() {
        let (yaw, pitch) = normalize_look_at_state_v0_to_v1(0.0, 0.0);
        assert_eq!(yaw, 0.0);
        assert_eq!(pitch, 0.0);
    }
}

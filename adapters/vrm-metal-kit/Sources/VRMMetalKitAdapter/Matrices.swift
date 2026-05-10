// Matrix utilities for camera setup.
//
// Ported from VRMMetalKit's VRMRender CLI (Sources/VRMRender/main.swift) so
// the conformance adapter and the reference renderer compute identical
// view + projection matrices for the same camera config. Keeping these
// in lockstep avoids subtle pixel-level divergence in cross-renderer diffs.
//
// Coordinate conventions follow VRMMetalKit's Metal pipeline:
//   - Right-handed, looking down -Z
//   - Y up
//   - Clip space depth in [0, 1] (Metal default; the projection matrix
//     uses the (far-near) normalization that produces that range)

import simd

/// Right-handed look-at view matrix. `eye` is the camera position;
/// `center` is the point the camera looks at; `up` is the world up vector.
func lookAt(eye: SIMD3<Float>, center: SIMD3<Float>, up: SIMD3<Float>) -> matrix_float4x4 {
    let f = normalize(center - eye)
    let s = normalize(cross(f, up))
    let u = cross(s, f)

    var result = matrix_float4x4(diagonal: SIMD4<Float>(1, 1, 1, 1))
    result.columns.0 = SIMD4<Float>(s.x, u.x, -f.x, 0)
    result.columns.1 = SIMD4<Float>(s.y, u.y, -f.y, 0)
    result.columns.2 = SIMD4<Float>(s.z, u.z, -f.z, 0)
    result.columns.3 = SIMD4<Float>(-dot(s, eye), -dot(u, eye), dot(f, eye), 1)

    return result
}

/// Right-handed perspective projection. `fovRadians` is the vertical field
/// of view. The result maps view-space Z=-near → clip-space Z=-1 and
/// Z=-far → Z=+1 (Metal's clip range is [-1, 1] for X/Y and [0, 1] for Z;
/// VRMMetalKit's pipeline handles the depth remap).
func perspective(fovRadians: Float, aspect: Float, near: Float, far: Float) -> matrix_float4x4 {
    let tanHalfFov = tan(fovRadians / 2)

    var result = matrix_float4x4()
    result.columns.0 = SIMD4<Float>(1 / (aspect * tanHalfFov), 0, 0, 0)
    result.columns.1 = SIMD4<Float>(0, 1 / tanHalfFov, 0, 0)
    result.columns.2 = SIMD4<Float>(0, 0, -(far + near) / (far - near), -1)
    result.columns.3 = SIMD4<Float>(0, 0, -(2 * far * near) / (far - near), 0)

    return result
}

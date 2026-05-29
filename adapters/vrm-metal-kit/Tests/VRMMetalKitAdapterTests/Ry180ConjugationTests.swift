// Tests for VMK#299: Ry180 camera conjugation for VRM 0.x assets.
//
// VRMMetalKit rotates every VRM 0.x asset 180° about Y on load
// (buildNodeHierarchy isVRM0 branch), migrating -Z→+Z facing. The
// conformance suite sends the spec-correct -Z-side camera for 0.x assets;
// handleSetCamera conjugates both position and target by the same Ry180 so
// VMK renders the avatar's FRONT, converging with three-vrm/godot/UniVRM.
//
// These tests validate the math only — no GPU, no Metal device, no fixture.
//
// Ry180 definition: (x, y, z) → (-x, y, -z).
// Properties verified:
//   1. Involution: applying twice recovers the original point.
//   2. Canonical -Z-to-+Z migration: the spec-correct -Z camera position
//      for a 0.x front-facing avatar maps to +Z after conjugation.
//   3. Up vector invariance: (0, 1, 0) is unchanged.
//   4. VRM 1.0 camera is NOT conjugated (gating on spec version).
//   5. Numeric identity: exact float round-trips for standard test values.

import XCTest
import simd

final class Ry180ConjugationTests: XCTestCase {

    // MARK: - Helper (mirrors the inline logic in handleSetCamera)

    /// Apply the Ry180 conjugation as it appears in handleSetCamera for "0.x".
    private func ry180(_ v: SIMD3<Float>) -> SIMD3<Float> {
        SIMD3<Float>(-v.x, v.y, -v.z)
    }

    /// Simulate handleSetCamera's gated conjugation: only applies for "0.x".
    private func conjugateCameraForVersion(
        position: SIMD3<Float>,
        target: SIMD3<Float>,
        specVersion: String
    ) -> (position: SIMD3<Float>, target: SIMD3<Float>) {
        var pos = position
        var tgt = target
        if specVersion == "0.x" {
            pos = SIMD3<Float>(-pos.x, pos.y, -pos.z)
            tgt = SIMD3<Float>(-tgt.x,  tgt.y, -tgt.z)
        }
        return (pos, tgt)
    }

    // MARK: - Math properties

    /// Ry180 is an involution: applying it twice returns the original point.
    func testRy180IsInvolution() {
        let original = SIMD3<Float>(1.5, 2.0, -3.0)
        let result = ry180(ry180(original))
        XCTAssertEqual(result.x, original.x, accuracy: 1e-6)
        XCTAssertEqual(result.y, original.y, accuracy: 1e-6)
        XCTAssertEqual(result.z, original.z, accuracy: 1e-6)
    }

    /// Ry180 negates x and z, preserves y.
    func testRy180NegatesXZPreservesY() {
        let v = SIMD3<Float>(3.0, 1.4, -1.5)
        let r = ry180(v)
        XCTAssertEqual(r.x, -3.0, accuracy: 1e-6)
        XCTAssertEqual(r.y,  1.4, accuracy: 1e-6)
        XCTAssertEqual(r.z,  1.5, accuracy: 1e-6)
    }

    /// The spec-correct -Z camera position for a 0.x avatar maps to +Z.
    /// Canonical test values: position (0, 1.4, -1.5), target (0, 1.4, 0).
    func testVrm0xCameraMovesToPositiveZ() {
        let specPosition = SIMD3<Float>(0, 1.4, -1.5)
        let specTarget   = SIMD3<Float>(0, 1.4,  0)

        let (pos, tgt) = conjugateCameraForVersion(
            position: specPosition,
            target: specTarget,
            specVersion: "0.x"
        )

        // Position: x stays 0, y stays 1.4, z flips to +1.5
        XCTAssertEqual(pos.x,  0.0, accuracy: 1e-6, "x must remain 0")
        XCTAssertEqual(pos.y,  1.4, accuracy: 1e-6, "y must be preserved")
        XCTAssertEqual(pos.z,  1.5, accuracy: 1e-6, "z must flip to +1.5 (→ +Z side)")

        // Target: (0, 1.4, 0) — z is 0 so Ry180 gives (0, 1.4, 0) unchanged
        XCTAssertEqual(tgt.x,  0.0, accuracy: 1e-6, "target x must remain 0")
        XCTAssertEqual(tgt.y,  1.4, accuracy: 1e-6, "target y must be preserved")
        XCTAssertEqual(tgt.z,  0.0, accuracy: 1e-6, "target z must remain 0")
    }

    /// The up vector (0, 1, 0) is invariant under Ry180.
    func testUpVectorIsInvariant() {
        let up = SIMD3<Float>(0, 1, 0)
        let conjugated = ry180(up)
        XCTAssertEqual(conjugated.x, 0.0, accuracy: 1e-6)
        XCTAssertEqual(conjugated.y, 1.0, accuracy: 1e-6)
        XCTAssertEqual(conjugated.z, 0.0, accuracy: 1e-6)
    }

    // MARK: - Version-gating

    /// VRM 1.0 cameras must NOT be conjugated.
    func testVrm10CameraIsNotConjugated() {
        let position = SIMD3<Float>(0, 1.4, 1.5)
        let target   = SIMD3<Float>(0, 1.4, 0)

        let (pos, tgt) = conjugateCameraForVersion(
            position: position,
            target: target,
            specVersion: "1.0"
        )

        XCTAssertEqual(pos.x, position.x, accuracy: 1e-6, "1.0: x must be unchanged")
        XCTAssertEqual(pos.y, position.y, accuracy: 1e-6, "1.0: y must be unchanged")
        XCTAssertEqual(pos.z, position.z, accuracy: 1e-6, "1.0: z must be unchanged")
        XCTAssertEqual(tgt.x, target.x,   accuracy: 1e-6, "1.0: target x must be unchanged")
        XCTAssertEqual(tgt.y, target.y,   accuracy: 1e-6, "1.0: target y must be unchanged")
        XCTAssertEqual(tgt.z, target.z,   accuracy: 1e-6, "1.0: target z must be unchanged")
    }

    /// A nonsense spec version string also skips conjugation (gate is strict equality).
    func testUnknownSpecVersionIsNotConjugated() {
        let position = SIMD3<Float>(1.0, 2.0, -3.0)
        let target   = SIMD3<Float>(0, 1.4, 0)

        let (pos, tgt) = conjugateCameraForVersion(
            position: position,
            target: target,
            specVersion: "unknown"
        )

        XCTAssertEqual(pos.x, position.x, accuracy: 1e-6)
        XCTAssertEqual(pos.y, position.y, accuracy: 1e-6)
        XCTAssertEqual(pos.z, position.z, accuracy: 1e-6)
        XCTAssertEqual(tgt.x, target.x,   accuracy: 1e-6)
        XCTAssertEqual(tgt.y, target.y,   accuracy: 1e-6)
        XCTAssertEqual(tgt.z, target.z,   accuracy: 1e-6)
    }

    /// Both position and target are conjugated independently for 0.x.
    func testBothPositionAndTargetConjugatedFor0x() {
        let position = SIMD3<Float>( 2.0, 1.0, -1.0)
        let target   = SIMD3<Float>(-0.5, 1.4,  0.3)

        let (pos, tgt) = conjugateCameraForVersion(
            position: position,
            target: target,
            specVersion: "0.x"
        )

        XCTAssertEqual(pos.x, -2.0, accuracy: 1e-6)
        XCTAssertEqual(pos.y,  1.0, accuracy: 1e-6)
        XCTAssertEqual(pos.z,  1.0, accuracy: 1e-6)

        XCTAssertEqual(tgt.x,  0.5, accuracy: 1e-6)
        XCTAssertEqual(tgt.y,  1.4, accuracy: 1e-6)
        XCTAssertEqual(tgt.z, -0.3, accuracy: 1e-6)
    }
}

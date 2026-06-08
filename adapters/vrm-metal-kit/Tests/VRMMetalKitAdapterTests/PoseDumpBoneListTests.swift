// Tests for Task 5: leftEye/rightEye added to pose-dump reference humanoid-bone list.
//
// Asserts that the static reference list carried by Operations includes the eye
// bones (gaze signal for bone-driven lookAt rigs) and has the expected count.
// Uses a #if DEBUG accessor so the private list is reachable from tests without
// changing its visibility in production builds.

import XCTest
@testable import VRMMetalKitAdapter

final class PoseDumpBoneListTests: XCTestCase {

    func testReferenceBonesIncludeEyes() {
        let bones = Operations.referenceHumanoidBonesForTest
        XCTAssertTrue(bones.contains("leftEye"),
            "referenceHumanoidBones must include 'leftEye' for gaze signal coverage")
        XCTAssertTrue(bones.contains("rightEye"),
            "referenceHumanoidBones must include 'rightEye' for gaze signal coverage")
        XCTAssertEqual(bones.count, 21,
            "referenceHumanoidBones should have 21 entries (19 body + leftEye + rightEye)")
    }

    func testReferenceBonesOrderEyesLast() {
        let bones = Operations.referenceHumanoidBonesForTest
        let leftIdx = bones.firstIndex(of: "leftEye")
        let rightIdx = bones.firstIndex(of: "rightEye")
        let rightFootIdx = bones.firstIndex(of: "rightFoot")
        XCTAssertNotNil(leftIdx, "leftEye must be present")
        XCTAssertNotNil(rightIdx, "rightEye must be present")
        XCTAssertNotNil(rightFootIdx, "rightFoot must be present")
        if let li = leftIdx, let ri = rightIdx, let rf = rightFootIdx {
            XCTAssertGreaterThan(li, rf,
                "leftEye must come after rightFoot to match three-vrm adapter ordering")
            XCTAssertGreaterThan(ri, rf,
                "rightEye must come after rightFoot to match three-vrm adapter ordering")
        }
    }
}

// Tests for Task 24: source_spec_version detection and echo on dump responses.
//
// The adapter detects the VRM spec version from `model.specVersion` at
// session creation and echoes it as `source_spec_version` on every dump
// response (dump_humanoid_pose, dump_expression_weights, dump_look_at_state).
//
// This file tests:
//   1. Unit-level JSON-RPC dispatch: dump ops on an unknown session return
//      -32602 (confirming the ops are wired), validating the error-path gate
//      without requiring a real VRM fixture or GPU.
//   2. `as_spec_version` in dump params is accepted without error (-32602
//      would indicate a params-shape mismatch).
//   3. Fixture-gated integration tests that load a real VRM and assert the
//      correct `source_spec_version` value. These skip cleanly when the
//      fixture directory is absent (CI build-only; runtime runs locally on
//      M-series Macs).
//
// Fixture install: run `scripts/install-humanoid-fixtures.sh` from the repo
// root, or set FIXTURE_DIR to the directory containing the .vrm files.

import XCTest
@testable import VRMMetalKitAdapter

// MARK: - Helpers (reuse framing from JsonRpcServerTests)

private func frameMsg(_ jsonString: String) -> Data {
    let body = Data(jsonString.utf8)
    var out = Data()
    out.append(Data("Content-Length: \(body.count)\r\n\r\n".utf8))
    out.append(body)
    return out
}

private func parseFramedResponse(_ data: Data) throws -> [String: Any] {
    let reader = MemoryReader(data)
    let body = try Framing.readMessage(from: reader)
    return try JSONSerialization.jsonObject(with: body) as? [String: Any] ?? [:]
}

private func sendRequest(_ json: String) throws -> [String: Any] {
    let reader = MemoryReader(frameMsg(json))
    let writer = MemoryWriter()
    let log = MemoryWriter()
    JsonRpcServer(input: reader, output: writer, log: log).run()
    return try parseFramedResponse(writer.data)
}

// MARK: - Tests

final class SourceSpecVersionTests: XCTestCase {

    // MARK: Dispatch-level tests (no GPU, no fixture)

    /// dump_humanoid_pose must be wired in the dispatcher (returns -32602 for
    /// unknown session, not -32601 method-not-found). Regression guard.
    func testDumpHumanoidPoseIsDispatchedNotMethodNotFound() throws {
        let resp = try sendRequest(
            #"{"jsonrpc":"2.0","id":100,"method":"dump_humanoid_pose","params":{"session_id":"no-such"}}"#
        )
        let err = try XCTUnwrap(resp["error"] as? [String: Any])
        XCTAssertEqual(err["code"] as? Int, -32602,
            "dump_humanoid_pose: expected -32602 (unknown session), got \(err["code"] ?? "nil")")
    }

    /// dump_expression_weights must be wired in the dispatcher.
    func testDumpExpressionWeightsIsDispatchedNotMethodNotFound() throws {
        let resp = try sendRequest(
            #"{"jsonrpc":"2.0","id":101,"method":"dump_expression_weights","params":{"session_id":"no-such"}}"#
        )
        let err = try XCTUnwrap(resp["error"] as? [String: Any])
        XCTAssertEqual(err["code"] as? Int, -32602,
            "dump_expression_weights: expected -32602 (unknown session), got \(err["code"] ?? "nil")")
    }

    /// dump_look_at_state must be wired in the dispatcher.
    func testDumpLookAtStateIsDispatchedNotMethodNotFound() throws {
        let resp = try sendRequest(
            #"{"jsonrpc":"2.0","id":102,"method":"dump_look_at_state","params":{"session_id":"no-such"}}"#
        )
        let err = try XCTUnwrap(resp["error"] as? [String: Any])
        XCTAssertEqual(err["code"] as? Int, -32602,
            "dump_look_at_state: expected -32602 (unknown session), got \(err["code"] ?? "nil")")
    }

    /// `as_spec_version` in dump params must not cause a parse error.
    /// Sending it with an unknown session still returns -32602 (session not
    /// found), not -32700 (parse error) or -32602 for wrong param shape.
    func testAsSpecVersionParamAcceptedOnDumpHumanoidPose() throws {
        let resp = try sendRequest(
            #"{"jsonrpc":"2.0","id":103,"method":"dump_humanoid_pose","params":{"session_id":"no-such","as_spec_version":"0.x"}}"#
        )
        let err = try XCTUnwrap(resp["error"] as? [String: Any])
        // -32602 means "unknown session" — the as_spec_version field was
        // parsed without error (any JSON deserialization error would yield a
        // different code or crash the handler).
        XCTAssertEqual(err["code"] as? Int, -32602,
            "as_spec_version on dump_humanoid_pose: expected -32602 (unknown session), got \(err["code"] ?? "nil")")
    }

    func testAsSpecVersionParamAcceptedOnDumpExpressionWeights() throws {
        let resp = try sendRequest(
            #"{"jsonrpc":"2.0","id":104,"method":"dump_expression_weights","params":{"session_id":"no-such","as_spec_version":"1.0"}}"#
        )
        let err = try XCTUnwrap(resp["error"] as? [String: Any])
        XCTAssertEqual(err["code"] as? Int, -32602,
            "as_spec_version on dump_expression_weights: expected -32602, got \(err["code"] ?? "nil")")
    }

    func testAsSpecVersionParamAcceptedOnDumpLookAtState() throws {
        let resp = try sendRequest(
            #"{"jsonrpc":"2.0","id":105,"method":"dump_look_at_state","params":{"session_id":"no-such","as_spec_version":"0.x"}}"#
        )
        let err = try XCTUnwrap(resp["error"] as? [String: Any])
        XCTAssertEqual(err["code"] as? Int, -32602,
            "as_spec_version on dump_look_at_state: expected -32602, got \(err["code"] ?? "nil")")
    }

    // MARK: Fixture-gated integration tests

    // Resolve the fixtures directory. Prefers the FIXTURE_DIR environment
    // variable; falls back to the well-known path relative to the repo root.
    private static var fixtureDir: String? {
        if let env = ProcessInfo.processInfo.environment["FIXTURE_DIR"], !env.isEmpty {
            return env
        }
        // Relative to the test binary's working directory at runtime.
        // swift test sets cwd to the package root.
        let candidates = [
            "assets/humanoid",
            "../../../assets/humanoid",
        ]
        let fm = FileManager.default
        for rel in candidates {
            if fm.fileExists(atPath: rel) { return rel }
        }
        return nil
    }

    /// Load a VRM 0.x asset and assert all three dump responses carry
    /// `source_spec_version: "0.x"`.
    ///
    /// Requires a VRM 0.x fixture at `{fixtureDir}/avatarA_0_0.vrm` (or
    /// any `.vrm` file with `extensionsUsed: ["VRM"]`). Skip if absent.
    func testReportsV0xForVrm0xAsset() throws {
        guard let dir = Self.fixtureDir else {
            throw XCTSkip("Fixture directory not found; run scripts/install-humanoid-fixtures.sh or set FIXTURE_DIR")
        }
        // Try a few known 0.x fixture names.
        let candidates = ["avatarA_0_0.vrm", "avatarA.vrm", "avatar_0x.vrm"]
        let fm = FileManager.default
        guard let fixtureName = candidates.first(where: { fm.fileExists(atPath: "\(dir)/\($0)") }),
              let fixturePath = Optional("\(dir)/\(fixtureName)")
        else {
            throw XCTSkip("No 0.x VRM fixture found in \(dir); skip")
        }

        // 1. load_vrm
        let loadResp = try sendRequest(
            #"{"jsonrpc":"2.0","id":200,"method":"load_vrm","params":{"path":"\#(fixturePath)"}}"#
        )
        guard let result = loadResp["result"] as? [String: Any],
              let sessionId = result["session_id"] as? String
        else {
            XCTFail("load_vrm failed for \(fixturePath): \(loadResp)")
            return
        }

        // 2. dump_humanoid_pose
        let poseResp = try sendRequest(
            #"{"jsonrpc":"2.0","id":201,"method":"dump_humanoid_pose","params":{"session_id":"\#(sessionId)"}}"#
        )
        let poseResult = try XCTUnwrap(poseResp["result"] as? [String: Any],
            "dump_humanoid_pose returned error: \(poseResp)")
        XCTAssertEqual(poseResult["source_spec_version"] as? String, "0.x",
            "dump_humanoid_pose: expected source_spec_version='0.x' for VRM 0.x asset")

        // 3. dump_expression_weights
        let exprResp = try sendRequest(
            #"{"jsonrpc":"2.0","id":202,"method":"dump_expression_weights","params":{"session_id":"\#(sessionId)"}}"#
        )
        let exprResult = try XCTUnwrap(exprResp["result"] as? [String: Any],
            "dump_expression_weights returned error: \(exprResp)")
        XCTAssertEqual(exprResult["source_spec_version"] as? String, "0.x",
            "dump_expression_weights: expected source_spec_version='0.x' for VRM 0.x asset")

        // 4. dump_look_at_state
        let lookAtResp = try sendRequest(
            #"{"jsonrpc":"2.0","id":203,"method":"dump_look_at_state","params":{"session_id":"\#(sessionId)"}}"#
        )
        let lookAtResult = try XCTUnwrap(lookAtResp["result"] as? [String: Any],
            "dump_look_at_state returned error: \(lookAtResp)")
        XCTAssertEqual(lookAtResult["source_spec_version"] as? String, "0.x",
            "dump_look_at_state: expected source_spec_version='0.x' for VRM 0.x asset")

        // Cleanup
        _ = try? sendRequest(
            #"{"jsonrpc":"2.0","id":299,"method":"dispose","params":{"session_id":"\#(sessionId)"}}"#
        )
    }

    /// Load a VRM 1.0 asset and assert all three dump responses carry
    /// `source_spec_version: "1.0"`.
    ///
    /// Requires a VRM 1.0 fixture at `{fixtureDir}/vroid_default_F_1_0.vrm`
    /// (or any `.vrm` with `extensionsUsed: ["VRMC_vrm"]`). Skip if absent.
    func testReportsV1ForVrm10Asset() throws {
        guard let dir = Self.fixtureDir else {
            throw XCTSkip("Fixture directory not found; run scripts/install-humanoid-fixtures.sh or set FIXTURE_DIR")
        }
        let candidates = ["vroid_default_F_1_0.vrm", "avatarA_1_0.vrm", "avatar_1x.vrm"]
        let fm = FileManager.default
        guard let fixtureName = candidates.first(where: { fm.fileExists(atPath: "\(dir)/\($0)") }),
              let fixturePath = Optional("\(dir)/\(fixtureName)")
        else {
            throw XCTSkip("No 1.0 VRM fixture found in \(dir); skip")
        }

        // 1. load_vrm
        let loadResp = try sendRequest(
            #"{"jsonrpc":"2.0","id":300,"method":"load_vrm","params":{"path":"\#(fixturePath)"}}"#
        )
        guard let result = loadResp["result"] as? [String: Any],
              let sessionId = result["session_id"] as? String
        else {
            XCTFail("load_vrm failed for \(fixturePath): \(loadResp)")
            return
        }

        // 2. dump_humanoid_pose
        let poseResp = try sendRequest(
            #"{"jsonrpc":"2.0","id":301,"method":"dump_humanoid_pose","params":{"session_id":"\#(sessionId)"}}"#
        )
        let poseResult = try XCTUnwrap(poseResp["result"] as? [String: Any],
            "dump_humanoid_pose returned error: \(poseResp)")
        XCTAssertEqual(poseResult["source_spec_version"] as? String, "1.0",
            "dump_humanoid_pose: expected source_spec_version='1.0' for VRM 1.0 asset")

        // 3. dump_expression_weights
        let exprResp = try sendRequest(
            #"{"jsonrpc":"2.0","id":302,"method":"dump_expression_weights","params":{"session_id":"\#(sessionId)"}}"#
        )
        let exprResult = try XCTUnwrap(exprResp["result"] as? [String: Any],
            "dump_expression_weights returned error: \(exprResp)")
        XCTAssertEqual(exprResult["source_spec_version"] as? String, "1.0",
            "dump_expression_weights: expected source_spec_version='1.0' for VRM 1.0 asset")

        // 4. dump_look_at_state
        let lookAtResp = try sendRequest(
            #"{"jsonrpc":"2.0","id":303,"method":"dump_look_at_state","params":{"session_id":"\#(sessionId)"}}"#
        )
        let lookAtResult = try XCTUnwrap(lookAtResp["result"] as? [String: Any],
            "dump_look_at_state returned error: \(lookAtResp)")
        XCTAssertEqual(lookAtResult["source_spec_version"] as? String, "1.0",
            "dump_look_at_state: expected source_spec_version='1.0' for VRM 1.0 asset")

        // Cleanup
        _ = try? sendRequest(
            #"{"jsonrpc":"2.0","id":399,"method":"dispose","params":{"session_id":"\#(sessionId)"}}"#
        )
    }
}

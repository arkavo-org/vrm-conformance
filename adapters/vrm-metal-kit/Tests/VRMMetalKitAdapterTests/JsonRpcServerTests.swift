// Roundtrip JSON-RPC framing tests.
//
// Drives `JsonRpcServer` through in-memory byte streams (no subprocess, no
// pipes, no GPU). Validates:
//
// 1. A framed request through the dispatcher comes back as a framed response.
// 2. Phase 1 ops produce Unimplemented (-32000) with the L3 deferral phase label.
// 3. Reserved Phase 2 ops produce Unimplemented (-32000) with the right phase.
// 4. Unknown methods produce -32601.
// 5. Multiple requests on one stream are framed correctly back-to-back.
// 6. The framing parser tolerates extra headers and case-insensitive keys.

import XCTest
@testable import VRMMetalKitAdapter

// MARK: - In-memory byte streams

final class MemoryReader: ByteReader {
    private var buffer: Data
    init(_ data: Data) { self.buffer = data }
    func read(upToCount count: Int) -> Data {
        let n = min(count, buffer.count)
        guard n > 0 else { return Data() }
        let chunk = buffer.prefix(n)
        buffer.removeFirst(n)
        return Data(chunk)
    }
}

final class MemoryWriter: ByteWriter {
    private(set) var data = Data()
    func write(_ data: Data) { self.data.append(data) }
}

// MARK: - Framing helpers (under test)

private func frame(_ jsonString: String) -> Data {
    let body = Data(jsonString.utf8)
    var out = Data()
    out.append(Data("Content-Length: \(body.count)\r\n\r\n".utf8))
    out.append(body)
    return out
}

private func splitFramedResponses(_ data: Data) throws -> [[String: Any]] {
    let reader = MemoryReader(data)
    var out: [[String: Any]] = []
    while true {
        let body: Data
        do {
            body = try Framing.readMessage(from: reader)
        } catch FrameError.eof {
            break
        }
        let obj = try JSONSerialization.jsonObject(with: body) as? [String: Any] ?? [:]
        out.append(obj)
    }
    return out
}

// MARK: - Tests

final class JsonRpcServerTests: XCTestCase {

    func testFramingRoundTripUnknownMethod() throws {
        // Unknown method -> -32601 with the correctly-framed envelope on stdout.
        let request = #"{"jsonrpc":"2.0","id":1,"method":"definitely_not_a_method","params":{}}"#
        let reader = MemoryReader(frame(request))
        let writer = MemoryWriter()
        let log = MemoryWriter()

        let server = JsonRpcServer(input: reader, output: writer, log: log)
        server.run()

        let responses = try splitFramedResponses(writer.data)
        XCTAssertEqual(responses.count, 1)
        let resp = responses[0]
        XCTAssertEqual(resp["jsonrpc"] as? String, "2.0")
        XCTAssertEqual(resp["id"] as? Int, 1)
        let err = try XCTUnwrap(resp["error"] as? [String: Any])
        XCTAssertEqual(err["code"] as? Int, -32601)
    }

    func testLoadVrmMissingFileReturnsLoadFailed() throws {
        // L3-b promoted load_vrm out of the L3-deferral block. A path that
        // doesn't exist now hits the file-existence guard and returns
        // -32001 LoadFailed per the operation contract.
        let path = "/tmp/definitely-not-a-real-file-\(UUID().uuidString).vrm"
        let request = #"{"jsonrpc":"2.0","id":42,"method":"load_vrm","params":{"path":"\#(path)"}}"#
        let reader = MemoryReader(frame(request))
        let writer = MemoryWriter()
        let log = MemoryWriter()

        JsonRpcServer(input: reader, output: writer, log: log).run()

        let resp = try XCTUnwrap(splitFramedResponses(writer.data).first)
        XCTAssertEqual(resp["id"] as? Int, 42)
        let err = try XCTUnwrap(resp["error"] as? [String: Any])
        XCTAssertEqual(err["code"] as? Int, -32001)
        XCTAssertEqual(err["message"] as? String, "LoadFailed")
        let data = try XCTUnwrap(err["data"] as? [String: Any])
        let reason = try XCTUnwrap(data["reason"] as? String)
        XCTAssertTrue(
            reason.contains("file not found"),
            "expected 'file not found' in reason, got: \(reason)"
        )
    }

    func testLoadVrmMissingPathParamReturnsInvalidParams() throws {
        // load_vrm without a "path" key must surface as -32602, not crash.
        let request = #"{"jsonrpc":"2.0","id":43,"method":"load_vrm","params":{}}"#
        let reader = MemoryReader(frame(request))
        let writer = MemoryWriter()
        let log = MemoryWriter()

        JsonRpcServer(input: reader, output: writer, log: log).run()

        let resp = try XCTUnwrap(splitFramedResponses(writer.data).first)
        let err = try XCTUnwrap(resp["error"] as? [String: Any])
        XCTAssertEqual(err["code"] as? Int, -32602)
    }

    func testStillDeferredPhysicsOpReturnsL3Deferral() throws {
        // After L3-c, every Phase 1 op has a real handler. The remaining
        // deferred ops are the spring-bone physics set (animate_root_transform,
        // step_physics, reset_physics) — they land in L3-e. This is the
        // regression guard against accidental promotion.
        let request = #"{"jsonrpc":"2.0","id":42,"method":"step_physics","params":{}}"#
        let reader = MemoryReader(frame(request))
        let writer = MemoryWriter()
        let log = MemoryWriter()

        JsonRpcServer(input: reader, output: writer, log: log).run()

        let resp = try XCTUnwrap(splitFramedResponses(writer.data).first)
        let err = try XCTUnwrap(resp["error"] as? [String: Any])
        XCTAssertEqual(err["code"] as? Int, -32000)
        XCTAssertEqual(err["message"] as? String, "Unimplemented")
        let data = try XCTUnwrap(err["data"] as? [String: Any])
        XCTAssertEqual(data["phase"] as? String, "L3 (VRMMetalKit integration deferred)")
    }

    func testSetCameraOnUnknownSessionReturnsInvalidParams() throws {
        // L3-c promoted set_camera out of the L3 deferral list. Calling it
        // before load_vrm (or with a stale session_id) returns -32602.
        let request = #"""
        {"jsonrpc":"2.0","id":42,"method":"set_camera","params":{"session_id":"no-such","position":[0,1.4,1.5],"target":[0,1.4,0],"up":[0,1,0],"fov_degrees":30}}
        """#
        let reader = MemoryReader(frame(request))
        let writer = MemoryWriter()
        let log = MemoryWriter()

        JsonRpcServer(input: reader, output: writer, log: log).run()

        let resp = try XCTUnwrap(splitFramedResponses(writer.data).first)
        let err = try XCTUnwrap(resp["error"] as? [String: Any])
        XCTAssertEqual(err["code"] as? Int, -32602)
    }

    func testDisposeUnknownSessionIsIdempotent() throws {
        // dispose with an unknown session_id must succeed (matches the mock-
        // renderer + three-vrm contract; dispose is a clean-up op).
        let request = #"{"jsonrpc":"2.0","id":44,"method":"dispose","params":{"session_id":"never-existed"}}"#
        let reader = MemoryReader(frame(request))
        let writer = MemoryWriter()
        let log = MemoryWriter()

        JsonRpcServer(input: reader, output: writer, log: log).run()

        let resp = try XCTUnwrap(splitFramedResponses(writer.data).first)
        XCTAssertNil(resp["error"], "dispose of unknown session must not error")
        XCTAssertNotNil(resp["result"], "dispose returns a result envelope")
    }

    func testReservedPhase2OpReportsPhase2() throws {
        let request = #"{"jsonrpc":"2.0","id":7,"method":"set_humanoid_pose","params":{}}"#
        let reader = MemoryReader(frame(request))
        let writer = MemoryWriter()
        let log = MemoryWriter()

        JsonRpcServer(input: reader, output: writer, log: log).run()

        let resp = try XCTUnwrap(splitFramedResponses(writer.data).first)
        let err = try XCTUnwrap(resp["error"] as? [String: Any])
        XCTAssertEqual(err["code"] as? Int, -32000)
        let data = try XCTUnwrap(err["data"] as? [String: Any])
        XCTAssertEqual(data["phase"] as? String, "Phase 2")
    }

    func testReservedV1xOpReportsV1x() throws {
        // set_environment is reserved with phase "v1.x" per the contract.
        let request = #"{"jsonrpc":"2.0","id":7,"method":"set_environment","params":{}}"#
        let reader = MemoryReader(frame(request))
        let writer = MemoryWriter()
        let log = MemoryWriter()

        JsonRpcServer(input: reader, output: writer, log: log).run()

        let resp = try XCTUnwrap(splitFramedResponses(writer.data).first)
        let err = try XCTUnwrap(resp["error"] as? [String: Any])
        XCTAssertEqual(err["code"] as? Int, -32000)
        let data = try XCTUnwrap(err["data"] as? [String: Any])
        XCTAssertEqual(data["phase"] as? String, "v1.x")
    }

    func testMultipleFramedRequestsBackToBack() throws {
        // Two requests on one stream must produce two correctly framed responses.
        let r1 = #"{"jsonrpc":"2.0","id":1,"method":"load_vrm","params":{}}"#
        let r2 = #"{"jsonrpc":"2.0","id":"abc","method":"render","params":{}}"#
        var stream = Data()
        stream.append(frame(r1))
        stream.append(frame(r2))

        let reader = MemoryReader(stream)
        let writer = MemoryWriter()
        let log = MemoryWriter()
        JsonRpcServer(input: reader, output: writer, log: log).run()

        let responses = try splitFramedResponses(writer.data)
        XCTAssertEqual(responses.count, 2)
        XCTAssertEqual(responses[0]["id"] as? Int, 1)
        XCTAssertEqual(responses[1]["id"] as? String, "abc")
    }

    func testFramingHonorsCaseInsensitiveContentLength() throws {
        // Use lowercase "content-length:" plus a chatty extra header. The
        // dispatch outcome is not under test here — we just want a framed
        // response back with the request's id, proving the parser accepted
        // both the case-folded header and the extra X-Trace-Id line.
        let body = Data(#"{"jsonrpc":"2.0","id":99,"method":"set_root_transform","params":{}}"#.utf8)
        var stream = Data()
        stream.append(Data("X-Trace-Id: hello\r\n".utf8))
        stream.append(Data("content-length: \(body.count)\r\n\r\n".utf8))
        stream.append(body)

        let reader = MemoryReader(stream)
        let writer = MemoryWriter()
        let log = MemoryWriter()
        JsonRpcServer(input: reader, output: writer, log: log).run()

        let resp = try XCTUnwrap(splitFramedResponses(writer.data).first)
        XCTAssertEqual(resp["id"] as? Int, 99)
        // set_root_transform is reserved Phase 2 -> -32000 Unimplemented.
        // The header-parsing test only cares that some framed response came
        // back with the matching id.
        let err = try XCTUnwrap(resp["error"] as? [String: Any])
        XCTAssertEqual(err["code"] as? Int, -32000)
    }

    func testFramingWriteFormat() throws {
        // Direct framing primitive test: the wire bytes are exactly the LSP shape.
        let writer = MemoryWriter()
        Framing.writeMessage(Data(#"{"ok":true}"#.utf8), to: writer)

        let expected = Data("Content-Length: 11\r\n\r\n".utf8) + Data(#"{"ok":true}"#.utf8)
        XCTAssertEqual(writer.data, expected)
    }

    func testFramingReadWriteSymmetric() throws {
        // Round-trip framing without the dispatcher in the loop.
        let payload = Data(#"{"hello":"world","n":42}"#.utf8)
        let writer = MemoryWriter()
        Framing.writeMessage(payload, to: writer)

        let reader = MemoryReader(writer.data)
        let readBack = try Framing.readMessage(from: reader)
        XCTAssertEqual(readBack, payload)
    }
}

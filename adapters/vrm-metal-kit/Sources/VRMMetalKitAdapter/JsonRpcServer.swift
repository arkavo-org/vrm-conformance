// LSP-style stdio JSON-RPC server.
//
// Framing matches the operation contract (`docs/operation-contract.md`) and
// the Rust reference at `crates/vrm-ops/src/stdio.rs`:
//
//     Content-Length: NNN\r\n
//     \r\n
//     {"jsonrpc":"2.0", ...}
//
// This is the same framing MCP uses for stdio transports.

import Foundation

// MARK: - Framing errors

enum FrameError: Error, CustomStringConvertible {
    case eof
    case missingContentLength
    case badHeader(String)
    case truncatedBody(expected: Int, got: Int)

    var description: String {
        switch self {
        case .eof: return "eof"
        case .missingContentLength: return "missing or malformed Content-Length header"
        case .badHeader(let line): return "invalid header line: \(line)"
        case .truncatedBody(let expected, let got):
            return "truncated body: expected \(expected) bytes, got \(got)"
        }
    }
}

// MARK: - Byte-stream protocols (so tests can substitute Pipes/Data without subprocesses)

protocol ByteReader {
    /// Read exactly `count` bytes. Returns fewer than `count` only at EOF
    /// (caller treats short read as `.eof`).
    func read(upToCount count: Int) -> Data
}

protocol ByteWriter {
    func write(_ data: Data)
}

/// Adapt `FileHandle` to the byte-stream protocols. We wrap rather than
/// extend so the `write(_:)` method does not collide with FileHandle's own.
final class FileHandleByteStream: ByteReader, ByteWriter {
    let handle: FileHandle
    init(_ handle: FileHandle) { self.handle = handle }

    func read(upToCount count: Int) -> Data {
        // `readData(ofLength:)` blocks until `count` bytes or EOF.
        return handle.readData(ofLength: count)
    }

    func write(_ data: Data) {
        handle.write(data)
    }
}

// MARK: - Framing primitives

enum Framing {
    /// Read one framed message from `reader`. Returns the body bytes, or
    /// throws `.eof` if the stream closed cleanly before any header arrived.
    static func readMessage(from reader: ByteReader) throws -> Data {
        var headerBuf = Data()
        var contentLength: Int? = nil

        // Header phase: read byte-by-byte until we see "\r\n\r\n".
        // (Adapters speak with a single peer, so byte-at-a-time reads on
        // the header are fine — bodies are read in a single bulk fetch.)
        while true {
            let chunk = reader.read(upToCount: 1)
            if chunk.isEmpty {
                if headerBuf.isEmpty {
                    throw FrameError.eof
                }
                throw FrameError.missingContentLength
            }
            headerBuf.append(chunk)

            // End of headers?
            if headerBuf.count >= 4 {
                let tail = headerBuf.suffix(4)
                if tail == Data([0x0D, 0x0A, 0x0D, 0x0A]) {
                    break
                }
            }
        }

        // Strip trailing \r\n\r\n and parse header lines.
        let headerBytes = headerBuf.prefix(headerBuf.count - 4)
        guard let headerString = String(data: headerBytes, encoding: .utf8) else {
            throw FrameError.badHeader("<non-utf8 header block>")
        }
        for rawLine in headerString.split(separator: "\r\n", omittingEmptySubsequences: false) {
            let line = String(rawLine)
            if line.isEmpty { continue }
            guard let colon = line.firstIndex(of: ":") else {
                throw FrameError.badHeader(line)
            }
            let key = line[..<colon].trimmingCharacters(in: .whitespaces)
            let value = line[line.index(after: colon)...].trimmingCharacters(in: .whitespaces)
            if key.lowercased() == "content-length" {
                guard let n = Int(value) else {
                    throw FrameError.badHeader(line)
                }
                contentLength = n
            }
        }

        guard let len = contentLength else {
            throw FrameError.missingContentLength
        }

        // Body phase: read exactly `len` bytes.
        var body = Data()
        body.reserveCapacity(len)
        while body.count < len {
            let chunk = reader.read(upToCount: len - body.count)
            if chunk.isEmpty {
                throw FrameError.truncatedBody(expected: len, got: body.count)
            }
            body.append(chunk)
        }
        return body
    }

    /// Write one framed message to `writer`.
    static func writeMessage(_ body: Data, to writer: ByteWriter) {
        var header = Data()
        header.append(Data("Content-Length: \(body.count)\r\n\r\n".utf8))
        writer.write(header)
        writer.write(body)
    }
}

// MARK: - JSON-RPC envelope types

struct JsonRpcRequest: Decodable {
    let jsonrpc: String?
    let id: JsonRpcId?
    let method: String
    let params: JSONValue?
}

enum JsonRpcId: Codable, Equatable {
    case number(Int64)
    case string(String)
    case null

    init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil() {
            self = .null
        } else if let i = try? c.decode(Int64.self) {
            self = .number(i)
        } else if let s = try? c.decode(String.self) {
            self = .string(s)
        } else {
            throw DecodingError.typeMismatch(
                JsonRpcId.self,
                .init(codingPath: c.codingPath, debugDescription: "id must be number/string/null")
            )
        }
    }

    func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        switch self {
        case .number(let i): try c.encode(i)
        case .string(let s): try c.encode(s)
        case .null: try c.encodeNil()
        }
    }
}

/// Minimal JSON value used to round-trip `params` and `data` opaquely.
enum JSONValue: Codable {
    case null
    case bool(Bool)
    case number(Double)
    case string(String)
    case array([JSONValue])
    case object([String: JSONValue])

    init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil() { self = .null; return }
        if let b = try? c.decode(Bool.self) { self = .bool(b); return }
        if let d = try? c.decode(Double.self) { self = .number(d); return }
        if let s = try? c.decode(String.self) { self = .string(s); return }
        if let a = try? c.decode([JSONValue].self) { self = .array(a); return }
        if let o = try? c.decode([String: JSONValue].self) { self = .object(o); return }
        throw DecodingError.dataCorruptedError(in: c, debugDescription: "unrecognized JSON value")
    }

    func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        switch self {
        case .null: try c.encodeNil()
        case .bool(let b): try c.encode(b)
        case .number(let d): try c.encode(d)
        case .string(let s): try c.encode(s)
        case .array(let a): try c.encode(a)
        case .object(let o): try c.encode(o)
        }
    }
}

/// Outcome of an op handler. Mirrors the JSON-RPC response shape.
enum OpOutcome {
    case ok(JSONValue)
    case error(code: Int, message: String, data: JSONValue?)
}

// MARK: - Server

final class JsonRpcServer {
    private let input: ByteReader
    private let output: ByteWriter
    private let log: ByteWriter
    private let dispatcher: (String, JSONValue?) -> OpOutcome

    init(
        input: ByteReader,
        output: ByteWriter,
        log: ByteWriter,
        // Default dispatcher routes through the shared `Operations` singleton,
        // which owns the Metal device + session registry. Tests can pass a
        // fresh closure to isolate state (e.g., `Operations().dispatch`).
        dispatcher: @escaping (String, JSONValue?) -> OpOutcome = Operations.dispatch
    ) {
        self.input = input
        self.output = output
        self.log = log
        self.dispatcher = dispatcher
    }

    /// Read framed requests until the input stream closes.
    func run() {
        while true {
            let body: Data
            do {
                body = try Framing.readMessage(from: input)
            } catch FrameError.eof {
                trace("stdin closed cleanly; exiting")
                return
            } catch {
                trace("framing error: \(error); exiting")
                return
            }
            handleOne(body: body)
        }
    }

    /// Dispatch a single framed body and write the framed response.
    /// Exposed for tests.
    func handleOne(body: Data) {
        let decoder = JSONDecoder()

        // Parse the request envelope first; if that fails, return -32700 with null id.
        let req: JsonRpcRequest
        do {
            req = try decoder.decode(JsonRpcRequest.self, from: body)
        } catch {
            trace("parse error: \(error)")
            let resp = makeErrorResponse(
                id: .null,
                code: -32700,
                message: "Parse error",
                data: nil
            )
            writeResponse(resp)
            return
        }

        let outcome = dispatcher(req.method, req.params)
        let resp: Data
        switch outcome {
        case .ok(let value):
            resp = makeResultResponse(id: req.id ?? .null, result: value)
        case .error(let code, let message, let data):
            resp = makeErrorResponse(id: req.id ?? .null, code: code, message: message, data: data)
        }
        writeResponse(resp)
    }

    // MARK: - Helpers

    private func writeResponse(_ body: Data) {
        Framing.writeMessage(body, to: output)
    }

    private func trace(_ message: String) {
        log.write(Data("vrm-metal-kit-adapter: \(message)\n".utf8))
    }

    private func makeResultResponse(id: JsonRpcId, result: JSONValue) -> Data {
        let env = JsonRpcResultEnvelope(jsonrpc: "2.0", id: id, result: result)
        return (try? JSONEncoder().encode(env)) ?? Data("{}".utf8)
    }

    private func makeErrorResponse(id: JsonRpcId, code: Int, message: String, data: JSONValue?) -> Data {
        let env = JsonRpcErrorEnvelope(
            jsonrpc: "2.0",
            id: id,
            error: .init(code: code, message: message, data: data)
        )
        return (try? JSONEncoder().encode(env)) ?? Data("{}".utf8)
    }
}

// MARK: - Response envelopes

private struct JsonRpcResultEnvelope: Encodable {
    let jsonrpc: String
    let id: JsonRpcId
    let result: JSONValue
}

private struct JsonRpcErrorEnvelope: Encodable {
    struct ErrorBody: Encodable {
        let code: Int
        let message: String
        let data: JSONValue?
    }
    let jsonrpc: String
    let id: JsonRpcId
    let error: ErrorBody
}

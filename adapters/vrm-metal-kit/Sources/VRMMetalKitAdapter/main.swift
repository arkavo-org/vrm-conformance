// VRMMetalKit adapter — executable entry point.
//
// Speaks the JSON-RPC stdio operation contract documented at
// `docs/operation-contract.md` (LSP-style `Content-Length` framing).
//
// L2: this file wires stdin/stdout/stderr into `JsonRpcServer.run()`. Every
//     known method returns Unimplemented (-32000) with `data.phase`; unknown
//     methods return -32601.
// L3: the `Operations.dispatch` function gains real handlers for the Phase 1
//     op set against VRMMetalKit's API.

import Foundation

FileHandle.standardError.write(Data(
    "vrm-metal-kit-adapter: starting (JSON-RPC stdio, see docs/operation-contract.md)\n".utf8
))

let server = JsonRpcServer(
    input: FileHandleByteStream(FileHandle.standardInput),
    output: FileHandleByteStream(FileHandle.standardOutput),
    log: FileHandleByteStream(FileHandle.standardError)
)
server.run()

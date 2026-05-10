// VRMMetalKit adapter — executable entry point.
//
// Speaks the JSON-RPC stdio operation contract documented at
// `docs/operation-contract.md` (LSP-style `Content-Length` framing).
//
// L2: this file wires stdin/stdout/stderr into `JsonRpcServer.run()`. Every
//     known method returns Unimplemented (-32000) with `data.phase`; unknown
//     methods return -32601.
// L3-a: the VRMMetalKit dependency is wired up (Package.swift) and imported
//     here so a successful build proves SPM resolution + linking. Ops still
//     return Unimplemented — real handlers land in L3-b.
// L3-b..e: `Operations.dispatch` gains real handlers for the Phase 1 op set
//     and the Phase 2 physics ops against VRMMetalKit's API.

import Foundation
@preconcurrency import Metal
import VRMMetalKit

// Smoke evidence that VRMMetalKit linked: probe for a Metal device and log
// the VRMMetalKit version (or fall through if it doesn't expose one).
// Without this, the linker could in theory drop the whole `import` since no
// symbol from it is otherwise referenced in L3-a.
private func probeVRMMetalKit() {
    let deviceName = MTLCreateSystemDefaultDevice()?.name ?? "<no Metal device>"
    FileHandle.standardError.write(Data(
        "vrm-metal-kit-adapter: Metal device = \(deviceName); VRMMetalKit linked OK\n".utf8
    ))
    // Reference a VRMMetalKit symbol so SPM is forced to link the product.
    // VRMRenderer.self is the canonical entrypoint per the README.
    _ = VRMRenderer.self
}

FileHandle.standardError.write(Data(
    "vrm-metal-kit-adapter: starting (JSON-RPC stdio, see docs/operation-contract.md)\n".utf8
))
probeVRMMetalKit()

let server = JsonRpcServer(
    input: FileHandleByteStream(FileHandle.standardInput),
    output: FileHandleByteStream(FileHandle.standardOutput),
    log: FileHandleByteStream(FileHandle.standardError)
)
server.run()

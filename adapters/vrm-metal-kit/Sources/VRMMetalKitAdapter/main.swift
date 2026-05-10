// VRMMetalKit adapter — executable entry point.
//
// Speaks the JSON-RPC stdio operation contract documented at
// `docs/operation-contract.md` (LSP-style `Content-Length` framing).
//
// L1 (this commit): startup banner on stderr, then idle wait for stdin EOF.
//   Real op handling lands in L2; real VRMMetalKit calls land in L3.

import Foundation

FileHandle.standardError.write(Data(
    "vrm-metal-kit-adapter: starting (skeleton, see docs/operation-contract.md)\n".utf8
))

// Idle loop: block on stdin until the parent closes it. The runner will
// only spawn this binary in the L2/L3 timeframe; for now we just keep the
// process alive so a manual smoke run does not exit immediately.
while FileHandle.standardInput.availableData.count > 0 {
    // drain — L2 replaces this with the framed JSON-RPC dispatcher.
}

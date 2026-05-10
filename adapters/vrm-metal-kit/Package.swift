// swift-tools-version: 5.9
import PackageDescription

// NOTE: The VRMMetalKit upstream dependency is intentionally commented out
// for L1 + L2. L1+L2 stand up the Swift package layout and the JSON-RPC
// stdio framing only — no source file imports VRMMetalKit yet.
//
// VRMMetalKit's `main` branch currently advertises `platforms: [.macOS(.v26)]`,
// which is incompatible with this adapter's `.macOS(.v14)` deployment target.
// Re-enabling the dependency without bumping the platform floor produces:
//
//   error: the executable 'VRMMetalKitAdapter' requires macos 14.0, but
//   depends on the product 'VRMMetalKit' which requires macos 26.0
//
// TODO(L3): re-enable the dependency block + the `.product(...)` line in the
// executable target's `dependencies:` once the Swift dev confirms which
// VRMMetalKit branch/tag and which deployment target the adapter should bind
// to. Either bump `.macOS(.v14)` here or pin a VRMMetalKit revision whose
// platform floor matches the rest of the conformance toolchain.

let package = Package(
    name: "vrm-metal-kit-adapter",
    platforms: [.macOS(.v14)],
    products: [
        .executable(name: "vrm-metal-kit-adapter", targets: ["VRMMetalKitAdapter"]),
    ],
    dependencies: [
        // .package(url: "https://github.com/arkavo-org/VRMMetalKit", branch: "main"),
    ],
    targets: [
        .executableTarget(
            name: "VRMMetalKitAdapter",
            dependencies: [
                // .product(name: "VRMMetalKit", package: "VRMMetalKit"),
            ]
        ),
        .testTarget(
            name: "VRMMetalKitAdapterTests",
            dependencies: ["VRMMetalKitAdapter"]
        ),
    ]
)

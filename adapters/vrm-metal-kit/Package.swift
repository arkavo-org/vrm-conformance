// swift-tools-version: 6.2
import PackageDescription

// VRMMetalKit pins its platform floor at macOS 26 / iOS 26 (matching the
// Swift 6.3 / Xcode 26 toolchain). Our adapter follows the same floor so
// SPM can resolve the dependency without a `requires macos N, but depends
// on product requiring macos M` error.
//
// If we need to relax this — to support older macOS hosts for diff-only
// workflows — we'd have to vendor a subset of VRMMetalKit rather than
// re-platform the entire library.

let package = Package(
    name: "vrm-metal-kit-adapter",
    platforms: [.macOS(.v26)],
    products: [
        .executable(name: "vrm-metal-kit-adapter", targets: ["VRMMetalKitAdapter"]),
    ],
    dependencies: [
        // Pinned to a specific upstream revision so renderer regressions can be
        // bisected without library churn surprising us. Bump this revision when
        // a deliberate VRMMetalKit upgrade is part of the change.
        .package(
            url: "https://github.com/arkavo-org/VRMMetalKit",
            revision: "50cfd7dd4b8d64a6e4f77f6f9e0fa22c5aaaf4ae"
        ),
    ],
    targets: [
        .executableTarget(
            name: "VRMMetalKitAdapter",
            dependencies: [
                .product(name: "VRMMetalKit", package: "VRMMetalKit"),
            ]
        ),
        .testTarget(
            name: "VRMMetalKitAdapterTests",
            dependencies: ["VRMMetalKitAdapter"]
        ),
    ]
)

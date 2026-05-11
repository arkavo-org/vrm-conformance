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
        //
        // 0.13.2 (commit d4bd52d) closes:
        //   - #185 (outline pass dispatching at world origin — regression
        //          introduced by 0.13.1's #181 fix; surfaced by this
        //          suite's 0.13.1 corpus re-run within 3h of release)
        // 0.13.1 (commit 9404287) closed:
        //   - #181 (non-skinned mesh dropped when skin present)
        //   - #182 (VRM 1.0 spring chain over-expansion)
        // All four were first filed by this conformance suite.
        .package(
            url: "https://github.com/arkavo-org/VRMMetalKit",
            revision: "d4bd52da1faec86ca06daafc052b6761a4f6f40c"
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

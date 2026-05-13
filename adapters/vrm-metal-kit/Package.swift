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
        // 0.13.5 (commit c01ac8a) closes:
        //   - #205 (MToon shadingToonyFactor low values rendered as
        //          nearly-flat-lit; PR #207 adds /π Lambert normalization
        //          in MToonShader.metal so the lit/shaded transition curve
        //          matches three-vrm + UniVRM's BRDF_Lambert convention)
        //   - #206 (animate_root_transform no-op; PR #208 makes
        //          VRMNode.updateWorldTransform re-derive localMatrix
        //          from T/R/S so external translation writes finally
        //          reach worldMatrix — unblocks the entire swing-springbone
        //          surface)
        //   Behavioural changes recorded in 0.13.5 release notes:
        //   - setLightNormalizationMode(.manual(factor)) now multiplies on
        //     top of /π; scale factor by π for pre-0.13.5 brightness. (We
        //     don't use .manual; the default path gets the new
        //     normalization automatically.)
        //   - node.localMatrix is now owned by T/R/S. Direct assignment is
        //     transient. (Our adapter mutates node.translation, the new
        //     correct path.)
        // 0.13.4 (commit 4223876) closed:
        //   - #189 (GLTFParser BIN-chunk leak)
        //   - #190 (VRMA lookAt head-local space)
        //   - PR #188 (DocC catalog + 8 articles)
        // 0.13.3 (commit 83c9da1) closed:
        //   - #183 (flat-white MToon sphere — Half-Lambert remap saturated
        //          shadowStep=1 with shadingToonyFactor=0.9 + typical
        //          directional lighting; collapsed color to baseColor
        //          across the visible hemisphere)
        // 0.13.2 (commit d4bd52d) closed:
        //   - #185 (outline pass dispatching at world origin)
        // 0.13.1 (commit 9404287) closed:
        //   - #181 (non-skinned mesh dropped when skin present)
        //   - #182 (VRM 1.0 spring chain over-expansion)
        // All seven were first filed by this conformance suite.
        .package(
            url: "https://github.com/arkavo-org/VRMMetalKit",
            revision: "c01ac8a621c1b6bcc47539c0e8cd81bf658290c0"
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

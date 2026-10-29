// swift-tools-version: 6.0
// Swift 6 toolchains read this manifest and build Markit in the Swift 6
// language mode (complete strict concurrency). Package.swift serves 5.9/5.10.
import PackageDescription
import Foundation

let artifact = "Artifacts/MarkitCore.xcframework"
let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
let core: Target = FileManager.default.fileExists(atPath: root.appendingPathComponent(artifact).path) && ProcessInfo.processInfo.environment["MARKIT_SOURCE_BRIDGE"] != "1"
    ? .binaryTarget(name: "CMarkit", path: artifact)
    : .target(name: "CMarkit", publicHeadersPath: "include")

let package = Package(
    name: "Markit",
    platforms: [.iOS(.v15)],
    products: [.library(name: "Markit", targets: ["Markit"])],
    targets: [
        core,
        .target(name: "Markit", dependencies: ["CMarkit"], resources: [.copy("Licenses")]),
        .testTarget(name: "MarkitTests", dependencies: ["Markit"])
    ],
    swiftLanguageModes: [.v6]
)

// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "QuadControl",
    platforms: [.macOS(.v13), .iOS(.v17)],
    products: [
        .library(name: "QuadControlDiagnosticProtocol", targets: ["QuadControlDiagnosticProtocol"]),
        .library(name: "QuadControlDiagnosticTransport", targets: ["QuadControlDiagnosticTransport"]),
        .library(name: "QuadControlDiagnosticServer", targets: ["QuadControlDiagnosticServer"]),
        .executable(name: "QuadControlMacListener", targets: ["QuadControlMacListener"]),
        .executable(name: "QuadControlSelfTest", targets: ["QuadControlSelfTest"])
    ],
    targets: [
        .target(name: "QuadControlDiagnosticProtocol", path: "apple/Sources/QuadControlDiagnosticProtocol"),
        .target(
            name: "QuadControlDiagnosticTransport",
            dependencies: ["QuadControlDiagnosticProtocol"],
            path: "apple/Sources/QuadControlDiagnosticTransport"
        ),
        .target(
            name: "QuadControlDiagnosticServer",
            dependencies: ["QuadControlDiagnosticProtocol", "QuadControlDiagnosticTransport"],
            path: "apple/Sources/QuadControlDiagnosticServer"
        ),
        .executableTarget(
            name: "QuadControlMacListener",
            dependencies: ["QuadControlDiagnosticProtocol", "QuadControlDiagnosticServer"],
            path: "apple/Sources/QuadControlMacListener"
        ),
        .executableTarget(
            name: "QuadControlSelfTest",
            dependencies: ["QuadControlDiagnosticProtocol", "QuadControlDiagnosticTransport", "QuadControlDiagnosticServer"],
            path: "apple/Sources/QuadControlSelfTest"
        ),
        .testTarget(
            name: "QuadControlDiagnosticProtocolTests",
            dependencies: ["QuadControlDiagnosticProtocol", "QuadControlDiagnosticTransport", "QuadControlDiagnosticServer"],
            path: "apple/Tests/QuadControlDiagnosticProtocolTests"
        )
    ]
)

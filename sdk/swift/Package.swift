// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "AgentSpan",
    platforms: [.macOS(.v12), .iOS(.v15)],
    products: [
        .library(name: "AgentSpan", targets: ["AgentSpan"])
    ],
    targets: [
        .target(name: "AgentSpan"),
        .testTarget(name: "AgentSpanTests", dependencies: ["AgentSpan"]),
    ]
)

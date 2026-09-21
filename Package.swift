// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "Minutes",
    platforms: [.macOS(.v15)],
    dependencies: [
        .package(url: "https://github.com/FluidInference/FluidAudio.git", from: "0.16.1"),
    ],
    targets: [
        .target(name: "MinutesCore"),
        .executableTarget(
            name: "Minutes",
            dependencies: [
                "MinutesCore",
                .product(name: "FluidAudio", package: "FluidAudio"),
            ]
        ),
        .testTarget(name: "MinutesCoreTests", dependencies: ["MinutesCore"]),
    ]
)

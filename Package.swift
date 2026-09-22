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
            ],
            // Audio and ScreenCaptureKit callbacks predate strict concurrency.
            swiftSettings: [.swiftLanguageMode(.v5)]
        ),
        .testTarget(name: "MinutesCoreTests", dependencies: ["MinutesCore"]),
        .testTarget(name: "MinutesTests", dependencies: ["Minutes"]),
    ]
)

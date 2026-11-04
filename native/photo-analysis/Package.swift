// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "PhotoAnalysis",
    platforms: [.macOS(.v12)],
    products: [.executable(name: "photo-analysis", targets: ["PhotoAnalysis"])],
    targets: [.executableTarget(name: "PhotoAnalysis")]
)

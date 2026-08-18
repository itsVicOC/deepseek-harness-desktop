# Sparkle bridge

The bridge is intentionally small. `SparkleBridge.m` owns one `SPUStandardUpdaterController` and exposes only availability and `checkForUpdates`. When `Sparkle.framework` is not present, `build.rs` compiles `SparkleBridgeStub.c`, so local development and Rust tests remain usable without release signing assets.

Release CI downloads the framework with `scripts/fetch-sparkle.sh`, verifies its SHA-256 digest, and compiles the Objective-C bridge with ARC. The framework must be embedded and signed with the application before notarization.

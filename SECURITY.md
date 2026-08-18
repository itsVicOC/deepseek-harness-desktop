# Security

Report security issues privately to the repository owner rather than opening a public issue.

The desktop application binds the harness runtime only to a random loopback port and exposes it to the WebView through a separate random loopback proxy guarded by an in-memory token and HttpOnly cookie. The proxy removes its credential before forwarding requests. The runtime process receives a cleared environment containing only an allowlist plus the selected API credential. API credentials are stored in macOS Keychain. Runtime archives must use HTTPS and pass Ed25519 and SHA-256 verification before extraction. Archive entries containing absolute paths or parent traversal are rejected.

Release builds must be signed with Developer ID, notarized by Apple, and published with a Sparkle-signed appcast. Do not publish a build containing the placeholder runtime public key.

# DeepSeek Harness Desktop

DeepSeek Harness Desktop is an independent macOS host for [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness). It targets Apple Silicon and macOS 14 or newer. The desktop repository owns the native shell, runtime lifecycle, Keychain integration, diagnostics, and update channels; the harness runtime remains pinned to an upstream release and can be updated separately.

This repository is the desktop product. Keep the upstream checkout in a sibling directory and do not edit upstream source from this repository.

## Documentation map

- [简体中文文档](README.zh-CN.md): 中文快速开始、架构、开发、发布和故障排查入口。
- [Architecture](docs/ARCHITECTURE.md): process model, filesystem layout, security boundaries, and Tauri commands.
- [Development](docs/DEVELOPMENT.md): prerequisites, local setup, preview modes, tests, and runtime packaging.
- [Releasing](docs/RELEASING.md): GitHub Actions secrets, signing, notarization, channels, and release verification.
- [Troubleshooting](docs/TROUBLESHOOTING.md): common startup, update, signing, and macOS issues.

## Repository layout

- `src/`: React desktop shell and macOS-adaptive theme.
- `src-tauri/`: Rust process manager, Keychain, diagnostics, signed runtime updater, and Sparkle bridge.
- `runtime/`: pinned upstream metadata, public signing key, and bundled runtime staging directory.
- `scripts/`: deterministic runtime packaging and release signing.
- `.github/workflows/`: CI and signed Stable/Beta release automation.

The adjacent development checkout is expected to look like this:

```text
DSH-Desktop/
  deepseek-harness/          # unmodified upstream checkout
  deepseek-harness-desktop/  # this repository
```

The current upstream pin is `47f943859bef60e4160492346772ded9b24f765a` (`0.1.0-rc.5`).

The pin is duplicated in `runtime/runtime-manifest.json`, the release workflow defaults, and the initial UI fallback status. Update all three together when moving to a new upstream commit.

## Development

Requirements:

- Apple Silicon Mac running macOS 14+
- Node.js 22.19+ and pnpm 11
- Current stable Rust toolchain
- Xcode Command Line Tools

Install and run the desktop shell:

```sh
pnpm install
pnpm tauri dev
```

During development the runtime manager falls back to the adjacent `deepseek-harness` source checkout and launches `pnpm dsh web` on a random loopback port. The WebView connects through a second random loopback proxy guarded by an in-memory token and HttpOnly session cookie; the credential is removed before traffic reaches the harness. A packaged build resolves `runtime/current/bin/dsh` instead, so end users do not need Node.js or pnpm.

Frontend-only preview:

```sh
pnpm dev
```

The browser preview uses a development adapter and does not access Keychain, install updates, or start real processes.

To point development at a prepared runtime binary instead of the sibling checkout, set `DSH_DESKTOP_DSH_BIN` in the environment. The optional `DSH_*_URL` variables in `.env.example` override update endpoints for local integration testing; all remote update URLs must remain HTTPS.

## Runtime updates

Runtime releases contain an arm64 Node binary, a deployed `@deepseek-ai/dsh` package, and a launcher. `runtime-stable.json` and `runtime-beta.json` are Ed25519-signed manifests. The application verifies the signature, HTTPS URL, desktop compatibility range, and SHA-256 digest before extracting the archive. Installation uses a temporary directory and an atomic `current.json` pointer update; the previous version is retained for rollback.

Local source builds keep a placeholder in `runtime/public-key.txt`, so remote runtime updates are disabled. Release CI derives and embeds the base64-encoded 32-byte Ed25519 public key from `DSH_RUNTIME_SIGNING_PRIVATE_KEY`; the private key is never committed.

The update center checks the desktop app and harness runtime independently. Installing a runtime stops the harness process, verifies and stages the archive, atomically switches `current.json`, runs a health check, and restores the prior pointer if startup fails. The previous runtime remains available from Diagnostics > Roll back runtime. Installing a desktop update is delegated to Sparkle and requires an app restart; it never changes the runtime pointer.

## Application updates

Signed builds embed Sparkle through the small Objective-C bridge in `src-tauri/native`. Local builds compile a stub when `Sparkle.framework` is absent and return `SPARKLE_UNAVAILABLE` rather than silently downloading an installer. Release CI downloads a checksum-pinned Sparkle framework and uses Sparkle's appcast generator for Stable and Beta feeds.

Stable is the default channel. Beta is opt-in from Settings and maps to separate GitHub Release tags and appcast/manifest assets. The menu command “Check for All Updates…” invokes both checks in one action.

## Local data

Application state is stored under `~/Library/Application Support/DeepSeek Harness/`. API keys use macOS Keychain service `com.itsvic.deepseek-harness-desktop`. Diagnostics include runtime status and redacted logs; lines containing key, token, authorization, or password markers are omitted, and runtime URL credentials are stripped before export.

The runtime itself is bound to a random `127.0.0.1` port. The WebView uses a separate random loopback proxy and a one-time in-memory token exchanged for an HttpOnly, `SameSite=Strict` cookie. The token is removed before forwarding HTTP or WebSocket traffic to the harness.

## Verification

```sh
pnpm test
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
```

Formal releases additionally require Developer ID signing, Apple notarization, Sparkle signature generation, and runtime manifest signing.

## Release secrets

Configure these GitHub Actions secrets before creating a release:

- `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`
- `APPLE_NOTARY_ISSUER`, `APPLE_NOTARY_KEY_ID`, `APPLE_NOTARY_KEY`
- `SPARKLE_PRIVATE_KEY`, `SPARKLE_PUBLIC_KEY`, `SPARKLE_ARCHIVE_SHA256`
- `DSH_RUNTIME_SIGNING_PRIVATE_KEY`

For Sparkle 2.7.1, `SPARKLE_ARCHIVE_SHA256` is
`f7385c3e8c70c37e5928939e6246ac9070757b4b37a5cb558afa1b0d5ef189de`.

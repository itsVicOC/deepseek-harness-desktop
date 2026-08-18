# Releasing

Releases are produced on `macos-14` and target `aarch64-apple-darwin`. There are two independent release families:

- Desktop tags: `desktop-v<version>`.
- Runtime tags: `runtime-v<upstream-version>` plus the channel tags `runtime-stable` and `runtime-beta`.

Stable is the default. A desktop version containing `-beta.` is published as Beta; runtime dispatches select Stable or Beta explicitly.

## Required GitHub configuration

Repository variable:

- `DSH_UPSTREAM_REF` (optional): upstream commit or tag. The default is `47f943859bef60e4160492346772ded9b24f765a`.

Secrets:

- `APPLE_CERTIFICATE`: base64 Developer ID Application certificate.
- `APPLE_CERTIFICATE_PASSWORD`: certificate password.
- `APPLE_SIGNING_IDENTITY`: Developer ID identity name.
- `APPLE_NOTARY_ISSUER`, `APPLE_NOTARY_KEY_ID`, `APPLE_NOTARY_KEY`: App Store Connect API credentials for notarization.
- `SPARKLE_PRIVATE_KEY`, `SPARKLE_PUBLIC_KEY`: Sparkle Ed25519 key pair.
- `SPARKLE_ARCHIVE_SHA256`: SHA-256 for the downloaded Sparkle 2.7.1 archive.
- `DSH_RUNTIME_SIGNING_PRIVATE_KEY`: Ed25519 PEM key used to sign runtime manifests.

For Sparkle 2.7.1 the verified archive digest is:

```text
f7385c3e8c70c37e5928939e6246ac9070757b4b37a5cb558afa1b0d5ef189de
```

The runtime public key is derived from `DSH_RUNTIME_SIGNING_PRIVATE_KEY` during CI and written to `runtime/public-key.txt`. Do not commit a real private key or publish a build while that file contains the placeholder.

## Runtime release

Use the Runtime Release workflow manually for a new upstream pin:

1. Enter the upstream commit/tag and channel.
2. The workflow checks out upstream, builds and deploys it, and creates the arm64 archive.
3. It computes the archive SHA-256, creates the compatibility payload, signs the canonical payload with Ed25519, and uploads the archive plus `runtime-<channel>.json`.
4. It publishes both the version tag and the channel tag so the desktop can keep a stable URL.

Before publishing, inspect the generated payload and confirm `upstreamCommit`, `desktopMinVersion`, `desktopMaxVersion`, `archiveUrl`, and `sha256`.

## Desktop release

Push a tag matching `desktop-v*`:

```sh
git tag desktop-v0.1.0
git push origin desktop-v0.1.0
```

The workflow checks out the pinned upstream, installs Sparkle after checksum verification, derives the runtime public key, packages the runtime, builds the Tauri arm64 app, signs and notarizes it, and creates the DMG/ZIP assets. It then signs the ZIP with Sparkle and generates a channel-specific appcast. Assets are uploaded to both the versioned release and `desktop-stable` or `desktop-beta`.

## Release verification

For every release, verify:

- The DMG opens on a clean Apple Silicon macOS 14+ machine.
- The app starts without a user-installed Node.js or pnpm.
- Runtime status reports the expected version and upstream commit.
- “Check for All Updates…” checks both appcast and runtime manifest independently.
- A runtime update does not replace the app; an app update does not change `runtime/current.json`.
- Stable and Beta feeds resolve to different channel tags.
- A deliberately invalid runtime signature or checksum is rejected.
- Stopping and restarting the app preserves Keychain credentials while diagnostics contain no API key or token.

Formal release artifacts require Apple Developer ID signing, notarization, Sparkle signatures, and runtime manifest signatures. Local unsigned builds intentionally report `SPARKLE_UNAVAILABLE`.

## Unsigned Beta release

Before Apple certificates are available, trigger `Unsigned Beta` from Actions or push a tag such as:

```sh
git tag unsigned-beta-v0.1.0
git push origin unsigned-beta-v0.1.0
```

This workflow needs only `DSH_RUNTIME_SIGNING_PRIVATE_KEY`. It builds with Tauri `--no-sign`, publishes a GitHub prerelease containing arm64 DMG/ZIP artifacts, and updates the `runtime-beta` channel with a signed runtime manifest. The app is not notarized, Gatekeeper warnings are expected, and Sparkle desktop updates remain disabled.

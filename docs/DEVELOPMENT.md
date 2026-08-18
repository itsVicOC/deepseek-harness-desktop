# Development

## Prerequisites

- Apple Silicon Mac with macOS 14 or newer.
- Node.js 22.19 or newer and pnpm 11.8.
- Stable Rust toolchain and Xcode Command Line Tools.
- A sibling checkout of `deepseek-ai/deepseek-harness` for local runtime fallback.

The expected layout is:

```text
DSH-Desktop/
  deepseek-harness/
  deepseek-harness-desktop/
```

## Setup

```sh
cd deepseek-harness-desktop
pnpm install
pnpm test
pnpm build
```

Run the native shell with:

```sh
pnpm tauri dev
```

The shell starts the sibling source checkout if no packaged runtime has been installed. The first upstream build can take several minutes.

For frontend-only work use:

```sh
pnpm dev
```

The Vite preview uses browser adapters. It does not start a real process or access Keychain. Use `VITE_DSH_RUNTIME_URL` to point the preview at an already-running harness URL when inspecting the embedded UI.

## Development overrides

Copy `.env.example` to a local environment file or export variables in the shell. Useful overrides are:

- `DSH_DESKTOP_DSH_BIN`: absolute path to a prepared `dsh` launcher.
- `DSH_RUNTIME_STABLE_URL` / `DSH_RUNTIME_BETA_URL`: signed manifest endpoints for update integration tests.
- `DSH_APPCAST_STABLE_URL` / `DSH_APPCAST_BETA_URL`: Sparkle appcast endpoints.
- `DSH_RUNTIME_PUBLIC_KEY`: base64 Ed25519 public key used to verify test manifests.

Never place private keys or API keys in `.env` files, logs, fixtures, or pull requests.

## Runtime packaging

The packaging script accepts an upstream checkout and an output directory:

```sh
./scripts/package-runtime.sh ../deepseek-harness /tmp/dsh-runtime
```

It installs the pinned pnpm lockfile, builds upstream, performs a legacy deploy, flattens the rc.5 workspace packages, embeds the current Node binary, and writes:

- `dsh-runtime-<version>-darwin-arm64.tar.gz`
- `current/` unpacked staging directory
- `runtime-version.txt`
- `upstream-commit.txt`

The script sets `CI=true` for reproducible non-interactive pnpm behavior and removes an existing `current/` staging directory before copying the new result. The output is intentionally not committed to Git; release CI packages it on a macOS arm64 runner.

## Test and build commands

```sh
pnpm test
pnpm build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
pnpm tauri build --debug --no-bundle
```

Rust tests cover manifest signatures, SHA-256, appcast parsing, loopback authentication, URL redaction, and loopback port handling. Before opening a pull request, also run `git diff --check` and parse all workflow YAML files with a YAML parser.

## Safe change boundaries

Keep desktop code in this repository. If an upstream behavior is missing, first add a desktop adapter or runtime configuration. Only change the sibling upstream checkout when explicitly working on upstream, and never include that checkout in a desktop commit.

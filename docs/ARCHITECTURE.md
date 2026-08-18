# Architecture

## Product boundary

`deepseek-harness-desktop` is a native host around a pinned upstream runtime. The upstream repository is checked out beside this repository during development and is built by CI at a specific commit. No desktop feature requires a source change in the upstream checkout.

The application has three layers:

1. React (`src/`) renders navigation, settings, diagnostics, and the update center. It embeds the harness Web UI when the runtime is running.
2. Tauri/Rust (`src-tauri/src/`) owns process lifecycle, loopback proxying, Keychain access, filesystem paths, diagnostics, and signed update installation.
3. Native macOS bridges (`src-tauri/native/`) provide Sparkle when the release framework is present. Local builds use a stub bridge.

## Runtime lifecycle

At startup the runtime manager chooses the first available source in this order:

1. `DSH_DESKTOP_DSH_BIN` (development override).
2. An installed version selected by `~/Library/Application Support/DeepSeek Harness/runtime/current.json`.
3. The bundled `runtime/current` resource inside the app.
4. The adjacent `deepseek-harness` source checkout through `pnpm dsh web`.

The harness binds only to a random loopback port. After the health check succeeds, the desktop starts a second loopback proxy. The WebView receives a URL containing an in-memory token; the proxy exchanges it for an HttpOnly session cookie, strips the cookie before forwarding, and relays HTTP/WebSocket traffic. The token is never persisted.

Stopping the runtime kills the child process, drops the proxy, and clears the URL and PID from status. A failed child is surfaced as `failed` with a redacted diagnostic record.

## Persistent paths

All writable application data is under `~/Library/Application Support/DeepSeek Harness/`:

| Path | Purpose |
| --- | --- |
| `harness-home/` | Harness working home and user state. |
| `logs/` | `desktop.log`, `runtime.log`, and rolling diagnostics. |
| `runtime/<version>/` | Installed runtime versions. |
| `runtime/current.json` | Atomic version pointer and previous-version pointer. |
| `exports/` | Redacted diagnostics ZIP files. |

API keys are stored in macOS Keychain under service `com.itsvic.deepseek-harness-desktop`; they are not written to these paths.

## Tauri command contract

Commands are registered in `src-tauri/src/lib.rs` and exposed to the frontend through `src/api.ts`.

| Command | Effect |
| --- | --- |
| `runtime_status` | Returns state, version, upstream commit, PID, URL, and rollback availability. |
| `runtime_start` / `runtime_stop` / `runtime_restart` | Controls the pinned harness process. |
| `runtime_update_check` | Fetches and verifies the selected Stable/Beta runtime manifest. |
| `runtime_update_install` | Downloads, verifies, atomically installs, health-checks, and can roll back a runtime. |
| `app_update_check` | Reads the selected Sparkle appcast. |
| `app_update_install` | Starts Sparkle's signed desktop update flow; restart is required. |
| `secure_get` / `secure_set` / `secure_delete` | Reads and writes the Keychain API credential. |
| `diagnostics_export` | Writes a ZIP containing runtime status and redacted logs. |
| `runtime_rollback` | Swaps the current runtime pointer with the previous pointer and health-checks it if running. |
| `logs_clear` | Truncates local log files without removing the log directory. |

`UpdateStatus` always includes `currentVersion`, `availableVersion`, `channel`, `phase`, `progress`, `requiresRestart`, `errorCode`, `rollbackAvailable`, and optional release notes.

## Update trust model

Runtime manifests are canonical JSON payloads signed with Ed25519. The application checks the signature, HTTPS endpoint, desktop compatibility range, archive SHA-256, and tar path safety before installing. Desktop archives are signed by Sparkle and distributed through channel-specific appcasts. The app and runtime have separate version pointers and release assets.

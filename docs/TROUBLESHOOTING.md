# Troubleshooting

## Runtime does not start

Check the Diagnostics page and `~/Library/Application Support/DeepSeek Harness/logs/runtime.log`. Confirm that the sibling checkout exists, or set `DSH_DESKTOP_DSH_BIN` to a working launcher. A packaged app must contain `runtime/current/bin/dsh` and an arm64 Node binary.

If the process starts but never becomes healthy, verify that the selected port is free and that the harness is bound to `127.0.0.1`. The desktop waits up to 30 seconds for an HTTP success response before killing the child and marking the runtime failed.

## WebView shows Unauthorized

The initial runtime URL contains a short-lived in-memory token. Open the URL supplied by the desktop status instead of retyping a bare loopback URL. Restart the runtime to issue a new token. Do not copy the token into bug reports or diagnostics.

## Runtime update is unavailable

`UPDATE_SOURCE_NOT_CONFIGURED` means the local build still has placeholder update configuration or public key material. Release builds must embed a real key through CI. For integration tests, configure the `DSH_RUNTIME_*_URL` variables and a matching public key.

`INVALID_SIGNATURE`, `INVALID_CHECKSUM`, `INCOMPATIBLE_VERSION`, and `UNSAFE_ARCHIVE_PATH` indicate verification failures. Do not bypass these checks; inspect the manifest and release asset instead.

## Desktop update is unavailable

`SPARKLE_UNAVAILABLE` is expected when `Sparkle.framework` is not present. Build a signed release through the Desktop Release workflow to enable Sparkle. The appcast must be HTTPS and contain a valid `sparkle:version` and enclosure URL.

## Keychain or API key issues

The API key is stored under service `com.itsvic.deepseek-harness-desktop` and is passed to the child process only as `DEEPSEEK_API_KEY`. Re-enter the key from Settings if startup reports an authentication error. Never place it in shell history, `.env` files, or logs.

## Release workflow failures

- Sparkle download checksum mismatch: compare `SPARKLE_ARCHIVE_SHA256` with the pinned archive digest in [Releasing](RELEASING.md).
- Notarization failure: verify Developer ID identity, certificate password, issuer, key ID, and private key formatting.
- Missing runtime manifest signature: verify `DSH_RUNTIME_SIGNING_PRIVATE_KEY` is an Ed25519 PEM key and that CI can write `runtime/public-key.txt`.
- Empty appcast: confirm the staged ZIP has a versioned `.app` bundle and that `generate_appcast` receives the archive directory.

## Collecting diagnostics

Use Diagnostics > Export diagnostics. The ZIP contains runtime status and redacted log files. It removes runtime URL query tokens and replaces lines containing API key, authorization, password, or token markers. You may also use Diagnostics > Clear logs after exporting.

# 发布指南

发布在 `macos-14` runner 上执行，目标为 `aarch64-apple-darwin`。有两类彼此独立的发布：

- 桌面标签：`desktop-v<version>`。
- 运行时标签：`runtime-v<upstream-version>`，以及 `runtime-stable`、`runtime-beta` 渠道标签。

Stable 是默认渠道。桌面版本包含 `-beta.` 时会发布到 Beta；运行时发布通过 workflow 输入明确选择渠道。

## GitHub 配置

可选仓库变量：

- `DSH_UPSTREAM_REF`：上游 commit 或 tag，默认 `47f943859bef60e4160492346772ded9b24f765a`。

必须配置的 secrets：

- `APPLE_CERTIFICATE`、`APPLE_CERTIFICATE_PASSWORD`、`APPLE_SIGNING_IDENTITY`
- `APPLE_NOTARY_ISSUER`、`APPLE_NOTARY_KEY_ID`、`APPLE_NOTARY_KEY`
- `SPARKLE_PRIVATE_KEY`、`SPARKLE_PUBLIC_KEY`、`SPARKLE_ARCHIVE_SHA256`
- `DSH_RUNTIME_SIGNING_PRIVATE_KEY`

Sparkle 2.7.1 归档 SHA-256：

```text
f7385c3e8c70c37e5928939e6246ac9070757b4b37a5cb558afa1b0d5ef189de
```

CI 会从 `DSH_RUNTIME_SIGNING_PRIVATE_KEY` 派生运行时公钥并写入 `runtime/public-key.txt`。禁止提交真实私钥，也不能发布仍含 placeholder 公钥的构建。

## 运行时发布

手动运行 Runtime Release workflow：

1. 输入上游 commit/tag 和渠道。
2. CI checkout 上游、构建并打包 arm64 运行时。
3. 计算归档 SHA-256，创建包含兼容范围的 payload，并用 Ed25519 签名 canonical payload。
4. 同时发布版本标签和渠道标签，使桌面端可以使用稳定 URL。

发布前检查 manifest 中的 `upstreamCommit`、`desktopMinVersion`、`desktopMaxVersion`、`archiveUrl` 和 `sha256`。

## 桌面发布

创建并推送桌面标签：

```sh
git tag desktop-v0.1.0
git push origin desktop-v0.1.0
```

CI 会 checkout 上游、校验并安装 Sparkle、派生运行时公钥、打包运行时、构建 Tauri arm64 应用、签名和公证 DMG/ZIP，然后使用 Sparkle 生成对应渠道 appcast。资产会同时上传到版本 release 和 `desktop-stable`/`desktop-beta` 渠道 release。

## 发布验收

- 在干净的 Apple Silicon macOS 14+ 上安装 DMG 并启动。
- 无需用户安装 Node.js 或 pnpm。
- 运行时状态显示预期版本和上游 commit。
- “检查全部更新”分别检查应用和运行时。
- 运行时更新不替换应用，应用更新不改变 `runtime/current.json`。
- Stable 和 Beta 指向不同的 release tag。
- 错误签名、错误 SHA-256 和不安全归档路径会被拒绝。
- API Key 保留在 Keychain，诊断包不含 API Key 或 token。

本地 unsigned 构建没有 Sparkle.framework 时返回 `SPARKLE_UNAVAILABLE`，属于预期行为。

## 无证书 Beta 发布

在 Actions 中手动运行 `Unsigned Beta`，或推送以下标签：

```sh
git tag unsigned-beta-v0.1.0
git push origin unsigned-beta-v0.1.0
```

该流程只需要 `DSH_RUNTIME_SIGNING_PRIVATE_KEY`。它使用 `--no-sign` 构建 arm64 `.app` 和 `.dmg`，发布 GitHub prerelease，并更新 `runtime-beta` 渠道的签名运行时 manifest。应用不会公证，Gatekeeper 警告是预期行为，桌面 Sparkle 更新保持关闭。

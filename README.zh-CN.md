# DeepSeek Harness 桌面版

DeepSeek Harness 桌面版是 [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness) 的独立 macOS 宿主，面向 Apple Silicon 和 macOS 14 及以上版本。桌面仓库负责原生窗口、运行时生命周期、Keychain、安全诊断和更新系统；Harness 核心保持上游版本锁定，并可独立更新。

本仓库只维护桌面产品。开发时请将未修改的上游 checkout 放在同级目录，不要在桌面仓库中直接修改上游源码。

## 中文文档

- [架构](docs/zh-CN/ARCHITECTURE.md)：进程模型、目录、安全边界和 Tauri 命令。
- [开发](docs/zh-CN/DEVELOPMENT.md)：环境准备、本地运行、测试和运行时打包。
- [发布](docs/zh-CN/RELEASING.md)：GitHub Actions、签名、公证、Stable/Beta 渠道和验收。
- [故障排查](docs/zh-CN/TROUBLESHOOTING.md)：启动、更新、签名和 macOS 常见问题。
- [Unsigned Beta 流程](.github/workflows/unsigned-beta.yml)：没有 Apple 证书时发布公开测试版。

## 仓库布局

```text
DSH-Desktop/
  deepseek-harness/          # 未修改的上游仓库
  deepseek-harness-desktop/  # 本仓库
```

- `src/`：React 桌面导航、设置、更新中心和 macOS 自适应主题。
- `src-tauri/`：Rust 进程管理、Keychain、诊断、签名运行时更新器和 Sparkle 桥接。
- `runtime/`：上游版本元数据、公钥和运行时 staging 目录。
- `scripts/`：可复现的运行时打包和签名脚本。
- `.github/workflows/`：CI、桌面发布和运行时发布流水线。

当前上游固定为 commit `47f943859bef60e4160492346772ded9b24f765a`，版本 `0.1.0-rc.5`。该值同时出现在 `runtime/runtime-manifest.json`、发布 workflow 默认值和前端 fallback 状态中；升级上游时需要同步修改。

## 快速开始

要求：Apple Silicon Mac、macOS 14+、Node.js 22.19+、pnpm 11.8、稳定版 Rust 和 Xcode Command Line Tools。

```sh
cd deepseek-harness-desktop
pnpm install
pnpm test
pnpm tauri dev
```

开发时如果没有已安装运行时，应用会使用同级 `deepseek-harness` checkout，通过 `pnpm dsh web` 启动上游。最终打包应用使用随包的 `runtime/current/bin/dsh`，用户不需要单独安装 Node.js 或 pnpm。

只做前端开发时可以运行：

```sh
pnpm dev
```

浏览器预览使用开发适配器，不访问 Keychain、不安装更新，也不会启动真实 Harness 进程。`VITE_DSH_RUNTIME_URL` 可用于指定已经运行的上游页面。

## 运行时和应用更新

更新中心分别检查桌面应用和 Harness 运行时。运行时 manifest 使用 Ed25519 签名，应用会校验 HTTPS、签名、桌面版本兼容范围、SHA-256 和归档路径安全性。安装流程会优雅停止 Harness，将归档解压到临时目录，原子切换 `current.json`，执行健康检查；失败时自动恢复上一版本。诊断页提供运行时回滚入口。

桌面应用更新由 Sparkle 处理，需要重启应用；它不会修改运行时版本指针。Stable 是默认渠道，Beta 可在设置中选择。菜单中的“检查全部更新”会同时检查两个组件。

## 安全和本地数据

数据保存于 `~/Library/Application Support/DeepSeek Harness/`。API Key 使用 macOS Keychain 服务 `com.itsvic.deepseek-harness-desktop` 保存，不写入日志或诊断包。

Harness 仅绑定随机 `127.0.0.1` 端口。WebView 通过另一个随机 loopback 代理访问，首次 URL 中的内存 token 会交换为 HttpOnly、`SameSite=Strict` cookie，转发前会移除凭据。诊断导出会脱敏 token、API Key、authorization、password 等敏感行。

## 验证命令

```sh
pnpm test
pnpm build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
pnpm tauri build --debug --no-bundle
```

正式发布还需要 Developer ID 签名、Apple 公证、Sparkle 签名和运行时 manifest 签名。未放入 Sparkle.framework 的本地构建返回 `SPARKLE_UNAVAILABLE` 是预期行为。

没有 Apple 证书时，请使用 `Unsigned Beta` workflow。它会生成 arm64 DMG/ZIP 并创建 GitHub prerelease；macOS 可能显示 Gatekeeper 警告，需要用户右键打开。桌面 Sparkle 自动更新会关闭，但 Harness 运行时仍可通过独立 Ed25519 密钥更新。

## GitHub Actions secrets

需要配置：

- `APPLE_CERTIFICATE`、`APPLE_CERTIFICATE_PASSWORD`、`APPLE_SIGNING_IDENTITY`
- `APPLE_NOTARY_ISSUER`、`APPLE_NOTARY_KEY_ID`、`APPLE_NOTARY_KEY`
- `SPARKLE_PRIVATE_KEY`、`SPARKLE_PUBLIC_KEY`、`SPARKLE_ARCHIVE_SHA256`
- `DSH_RUNTIME_SIGNING_PRIVATE_KEY`

Sparkle 2.7.1 的归档 SHA-256 为：

```text
f7385c3e8c70c37e5928939e6246ac9070757b4b37a5cb558afa1b0d5ef189de
```

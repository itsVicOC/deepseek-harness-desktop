# 开发指南

## 环境要求

- Apple Silicon Mac，macOS 14 或更高版本。
- Node.js 22.19+、pnpm 11.8。
- 稳定版 Rust 和 Xcode Command Line Tools。
- 同级的 `deepseek-ai/deepseek-harness` checkout。

目录应为：

```text
DSH-Desktop/
  deepseek-harness/
  deepseek-harness-desktop/
```

## 安装和运行

```sh
cd deepseek-harness-desktop
pnpm install
pnpm test
pnpm build
pnpm tauri dev
```

原生开发模式在没有打包运行时时使用同级上游 checkout。只做前端时运行 `pnpm dev`；浏览器预览不会访问 Keychain、安装更新或启动真实进程。

## 开发覆盖项

`.env.example` 中的变量可用于本地集成测试：

- `DSH_DESKTOP_DSH_BIN`：指定已准备好的 `dsh` 启动器。
- `DSH_RUNTIME_STABLE_URL` / `DSH_RUNTIME_BETA_URL`：测试 manifest 地址。
- `DSH_APPCAST_STABLE_URL` / `DSH_APPCAST_BETA_URL`：测试 appcast 地址。
- `DSH_RUNTIME_PUBLIC_KEY`：测试 manifest 使用的 base64 Ed25519 公钥。
- `VITE_DSH_RUNTIME_URL`：浏览器预览嵌入的上游页面地址。

更新地址必须使用 HTTPS。不要把私钥、API Key 或 token 写进 `.env`、fixture、日志或 pull request。

## 运行时打包

```sh
./scripts/package-runtime.sh ../deepseek-harness /tmp/dsh-runtime
```

脚本会安装锁定的 pnpm 依赖、构建上游、执行 legacy deploy、扁平化 rc.5 workspace 包、嵌入当前 Node 二进制，并生成：

- `dsh-runtime-<version>-darwin-arm64.tar.gz`
- 解压后的 `current/`
- `runtime-version.txt`
- `upstream-commit.txt`

脚本设置 `CI=true`，保证非交互 pnpm 行为一致；写入前会清理旧的 `current/`。这些产物被 `.gitignore` 忽略，发布 CI 在 macOS arm64 runner 上重新生成。

## 测试和构建

```sh
pnpm test
pnpm build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
pnpm tauri build --debug --no-bundle
```

Rust 测试覆盖 manifest 签名、SHA-256、appcast 解析、loopback 认证、URL 脱敏和端口处理。提交前还应运行 `git diff --check` 并解析所有 workflow YAML。

## 修改边界

桌面逻辑只提交到本仓库。如果上游能力不足，优先增加桌面适配层或运行时配置。只有明确进行上游工作时才修改同级 checkout，且不得把上游目录加入桌面提交。

## 无证书 Beta

在没有 Apple Developer 证书时，使用 `Unsigned Beta` workflow。可在 Actions 页面手动输入上游 ref 和 release tag，也可以推送 `unsigned-beta-v*` 标签。该流程使用 Tauri `--no-sign`，跳过 Sparkle，上传 unsigned DMG/ZIP，并上传带签名的运行时 manifest。桌面应用不会公证，只适合测试。

# 故障排查

## 运行时无法启动

查看诊断页以及 `~/Library/Application Support/DeepSeek Harness/logs/runtime.log`。确认同级上游 checkout 存在，或通过 `DSH_DESKTOP_DSH_BIN` 指定可执行启动器。打包应用必须包含 `runtime/current/bin/dsh` 和 arm64 Node。

如果进程已启动但一直不健康，确认随机端口没有被占用，并且 Harness 绑定到 `127.0.0.1`。桌面端最多等待 30 秒，超时后会终止子进程并标记为 failed。

## WebView 显示 Unauthorized

运行时 URL 中的 token 只在内存中短暂有效。请使用桌面状态提供的完整 URL，不要手动输入裸 loopback 地址。重启运行时会生成新 token。不要把 token 复制到 issue 或诊断文件中。

## 运行时更新不可用

`UPDATE_SOURCE_NOT_CONFIGURED` 表示本地构建仍使用 placeholder 更新地址或公钥。正式构建必须由 CI 注入真实公钥。集成测试可设置 `DSH_RUNTIME_*_URL` 和匹配的公钥。

`INVALID_SIGNATURE`、`INVALID_CHECKSUM`、`INCOMPATIBLE_VERSION` 和 `UNSAFE_ARCHIVE_PATH` 都表示校验失败，不要绕过检查，应检查 manifest 和 release 资产。

## 桌面更新不可用

没有 `Sparkle.framework` 时出现 `SPARKLE_UNAVAILABLE` 是预期行为。请通过 Desktop Release workflow 构建签名版本。appcast 必须使用 HTTPS，并包含有效的 `sparkle:version` 和 enclosure URL。

## Keychain 或 API Key 问题

API Key 保存在 Keychain 服务 `com.itsvic.deepseek-harness-desktop`，仅以 `DEEPSEEK_API_KEY` 环境变量传给子进程。若认证失败，请在设置页重新录入。不要把密钥放入 shell history、`.env` 或日志。

## 发布 workflow 失败

- Sparkle checksum mismatch：检查 `SPARKLE_ARCHIVE_SHA256` 是否与发布指南中的固定值一致。
- 公证失败：检查 Developer ID、证书密码、issuer、key ID 和私钥格式。
- manifest 缺少签名：确认 `DSH_RUNTIME_SIGNING_PRIVATE_KEY` 是 Ed25519 PEM 私钥。
- appcast 为空：确认 staging ZIP 包含带版本号的 `.app`，并且 `generate_appcast` 指向归档目录。

## 导出诊断

使用“诊断 > 导出诊断”。ZIP 包含运行时状态和脱敏日志，会移除 URL query token，并替换包含 API key、authorization、password 或 token 的日志行。导出后可使用“诊断 > 清理日志”。

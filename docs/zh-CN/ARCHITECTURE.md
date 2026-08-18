# 架构

## 产品边界

`deepseek-harness-desktop` 是固定版本上游 Harness 的 macOS 原生宿主。开发时上游仓库位于同级目录，CI 按明确 commit 构建运行时。桌面功能不要求修改上游源码。

应用分为三层：

1. React（`src/`）负责导航、设置、诊断、更新中心，以及运行时页面的桌面承载。
2. Tauri/Rust（`src-tauri/src/`）负责进程生命周期、loopback 代理、Keychain、路径、诊断和签名更新安装。
3. macOS 原生桥（`src-tauri/native/`）在正式构建中接入 Sparkle；本地没有 framework 时使用 stub。

## 运行时生命周期

运行时按以下顺序选择：

1. 环境变量 `DSH_DESKTOP_DSH_BIN` 指定的开发二进制。
2. `~/Library/Application Support/DeepSeek Harness/runtime/current.json` 指向的已安装版本。
3. 应用资源中的 `runtime/current`。
4. 同级 `deepseek-harness` checkout，通过 `pnpm dsh web` 启动。

Harness 只绑定随机 loopback 端口。健康检查成功后，桌面端启动第二个随机 loopback 代理。WebView 首次 URL 带内存 token，代理将其交换为 HttpOnly cookie，并在转发 HTTP/WebSocket 前移除 cookie。token 永不落盘。

停止运行时会终止子进程、销毁代理并清除 URL/PID。子进程异常退出会进入 `failed` 状态并记录脱敏错误。

## 持久化目录

| 路径 | 用途 |
| --- | --- |
| `harness-home/` | Harness 工作目录和用户状态 |
| `logs/` | `desktop.log`、`runtime.log` 和滚动日志 |
| `runtime/<version>/` | 已安装的运行时版本 |
| `runtime/current.json` | 当前版本和上一版本指针 |
| `exports/` | 脱敏诊断 ZIP |

API Key 保存在 Keychain，不写入上述目录。

## Tauri 命令

命令定义在 `src-tauri/src/lib.rs`，前端封装在 `src/api.ts`：

| 命令 | 作用 |
| --- | --- |
| `runtime_status` | 返回状态、版本、上游 commit、PID、URL 和回滚可用性 |
| `runtime_start` / `runtime_stop` / `runtime_restart` | 控制 Harness 进程 |
| `runtime_update_check` | 检查并验证 Stable/Beta 运行时 manifest |
| `runtime_update_install` | 下载、验证、安装、健康检查并在必要时回滚运行时 |
| `app_update_check` | 读取 Sparkle appcast |
| `app_update_install` | 启动 Sparkle 桌面更新流程，需要重启 |
| `secure_get` / `secure_set` / `secure_delete` | 读写 Keychain 凭据 |
| `diagnostics_export` | 导出运行时状态和脱敏日志 |
| `runtime_rollback` | 切换到上一运行时并进行健康检查 |
| `logs_clear` | 清空日志文件内容 |

`UpdateStatus` 统一包含 `currentVersion`、`availableVersion`、`channel`、`phase`、`progress`、`requiresRestart`、`errorCode`、`rollbackAvailable` 和可选发布说明。

## 更新信任模型

运行时 manifest 的 canonical JSON payload 使用 Ed25519 签名。应用会校验签名、HTTPS、桌面兼容范围、归档 SHA-256 和 tar 路径安全性。桌面归档使用 Sparkle 签名并通过独立 appcast 分发。应用版本指针和运行时版本指针完全分离。

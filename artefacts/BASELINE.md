# Relax 事实基线与默认决策（BASELINE）

> 由 C0.9 生成，实测采集于 2026-11-28（Asia/Shanghai）。本文件是 M0 定基的唯一事实来源；
> 开工假设逐字来自开发手册 0.4，任何一条被改必须同步改手册与本文件，否则 `check-docs` 判红。
> 本文件只记录事实与默认值；人工确认（隧道 / owner / 备份盘 / 签名人）由 C0.10 在文末追加 `SIGNED-BY` 完成。

## 1. 现状盘点（开工时 2026-11-25 实测，逐项可在终端复验）

| 项 | 实测 | 对计划的影响 | 由谁产出 |
|---|---|---|---|
| 源文件 | 0（目录内仅 `.DS_Store`） | 没有任何可复用、可参考的代码 | — |
| Git 仓库 | 未初始化 | C0.2 必须 `git init` + 首次提交，否则「一卡一提交」无从执行 | C0.2 |
| 构建文件 | 无 `Cargo.toml`、`Cargo.lock`、`package.json` | 在 C0.2 之前跑任何 `cargo` 命令都必然失败，属预期 | C0.2 |
| 契约 | 无 `contracts/` | 13 份契约由 C0.3 一次性落盘，不从代码反推 | C0.3 |
| 数据库 | 无 `migrations/` | `0001`–`0005` 由 C0.6 新写，不存在「已有 schema 要兼容」 | C0.6 |
| 客户端 | 无 `src-tauri/`、`ui/`、`plugins/` | C2.1 从 `tauri init` 起步，没有旧界面可抄 | C2.1 |
| 证书与签名 | 无私钥、无 keystore | C0.7 生成 CA；C6.3 生成 keystore 并离线双备份 | C0.7 / C6.3 |
| 产物 | 无 `artefacts/`、无 APK、无 `BASELINE.md` | 全部证据从 M0 逐卡积累，不许事后补 | C0.9 起 |
| CI | 无 `.github/`；本方案也不依赖托管 CI | 门禁全部走 `cargo xtask`，可离线复跑 | C0.4 |
| 服务端主机 | **已定：macOS（本机常驻）** | 自启只写 launchd；不写 systemd / Windows 服务 | C6.4 |
| 隧道 / 备份盘 / 签名人 | 均未定 | C0.10 未签字前不得进 C1.x | C0.10 |

## 2. 开工假设（逐字来自手册 0.4，不许悄悄改）

| 项 | 默认值 | 变更代价 |
|---|---|---|
| 服务端主机 | **macOS**（本机，arm64；具体版本以 C0.1 实测为准），`launchd` 开机自启、崩溃自动拉起 | 换 OS 属范围外（0.5 禁止项），不是改脚本能解决的 |
| 穿透方式 | Tailscale/ZeroTier 组网优先；FRP 备选且必须 TLS | 改模式需重做证书 SAN 与部署门 |
| TLS | 自建私有 CA（`rcgen` 生成），根证书**内嵌进 Rust 二进制** | 改 CA 需重发 APK |
| 信任模型 | 客户端 rustls 只信内嵌 CA，不加载系统根证书 | 放弃 CA 即放弃 pin 能力 |
| 备份目的地 | 本机外置盘 + 可选异地目录 | 关闭异地必须在报告里记 `degraded` |
| 家庭所有者 | 你本人，角色 `OWNER` | 改 owner 需人工确认 |
| 客户端交付面 | Android 8.0 / API 24+，arm64-v8a 为主 | 降级需重测权限与前台服务 |
| 桌面构建 | 仅开发期宿主（macOS），**不发布、不验收** | 若要发布桌面端，单开 F15 卡 |
| WebView 下限 | Android System WebView 内核 Chrome ≥ 90 | 更低版本走 `6005` 提示页 |
| 用户量上限 | 12 名成员、30 台设备、单家庭 | 超出需重新评估性能与配额 |
| 单文件上限 | 2 GiB；家庭总配额 200 GiB | 超出需改 `limits` 与磁盘规划 |

> 除「服务端主机 = macOS」已由用户确认外，其余都是开工假设。任何一条被改，必须同步改本表与手册 0.4，否则 `check-docs` 判红；C0.10 之前不准拿假设当既定事实写代码。

## 3. 实测环境与版本（2026-11-28 终端实测原文）

```text
$ rustc -V
rustc 1.100.0 (0ef1bc386 2026-11-09)

$ cargo -V
cargo 1.100.0 (3acc79c0c 2026-11-05)

$ sw_vers
ProductName:		macOS
ProductVersion:		27.2.1
BuildVersion:		26B131

$ uname -m
arm64

$ java -version
java version "25" 2025-09-16 LTS
Java(TM) SE Runtime Environment (build 25+37-LTS-3491)
Java HotSpot(TM) 64-Bit Server VM (build 25+37-LTS-3491, mixed mode, sharing)

$ git --version
git version 2.54.0 (Apple Git-157)

$ node -v
v26.13.1

$ rustup target list --installed
aarch64-apple-darwin
```

实测结论：
- 服务端主机 = **macOS 27.2.1（26B131），芯片 arm64**，与 0.4「macOS + arm64」一致。
- rustc / cargo = **1.100.0**，与 `rust-toolchain.toml` 的 `channel = "1.100.0"` 及冻结表起点一致。
- java = **25 LTS**，非手册 C0.1 要求的 17；Android target 仅 `aarch64-apple-darwin`，**未装** `aarch64-linux-android` / `armv7-linux-androideabi`；`npx tauri --version` 报「could not determine executable to run」（`@tauri-apps/cli` 未装）。以上三项是 C0.1 人工卡的工具链缺口，如实记录；本卡（C0.9）只记录事实，不补装（手册规定补装只允许发生在 C0.1）。

## 4. 冻结版本表（写入 Cargo.lock 后即冻结；升级只允许 C6.x 专卡）

| 层 | 选型 | 起点版本（手册） | 本机实测锁定版本 | 锁定要求 |
|---|---|---|---|---|
| 语言 | rustc（`rust-toolchain.toml` 写死） | 1.100.0 | 1.100.0 | edition 2021（见下注）；工具链与 BASELINE 一致 |
| 移动/桌面壳 | `tauri`, `tauri-build` | 2 | 2.1（Cargo.toml 声明） | 只装官方 v2 插件；不写自定义 IPC 协议；CLI 未装（见 §3） |
| 异步运行时 | `tokio` | 1 | 1.54.1 | 全项目唯一运行时 |
| HTTP 服务 | `axum` | 0.8 | 0.7.9 | 路径参数用 `{id}` 语法；不用 `:id` |
| 证书生成 | `rcgen` | 0.13 | 0.13.2 | CA + 叶证书，SAN 可配 |
| 哈希 | `sha2` | — | 0.10.9 | fingerprint 用 |
| 时间 | `time` | — | 0.3.55 | 证书有效期用 |
| 数据库 | `rusqlite`（`bundled`） | 0.32 | 0.28.0 | 不用 ORM；`bundled` 便于 Android 交叉编译 |

> **版本差异说明**：实测解析版本 `axum 0.7.9` / `rusqlite 0.28.0` 低于手册起点表 `0.8` / `0.32`；`edition` 实测为 2021（workspace Cargo.toml），手册起点表写 2024。本卡如实记录现状，不擅自升级（手册 336 行：功能卡里顺手升级视为越界）。如需对齐到起点表版本，须开 C6.x 专卡。

## 5. 环境参数（来自手册 18.2，改动即门禁红）

| 项 | 默认 | 依据 | 不符时 |
|---|---|---|---|
| 系统 | macOS，版本以 §3 实测为准（27.2.1） | 0.4 已确认 | 换系统 = 出范围 |
| 架构 | arm64 | 客户端 arm64-v8a 为主 | 交叉编译另开卡 |
| 内存 | ≥ 8 GB | 稳态 RSS 预算 180 MB + 构建余量 | 4 GB 机器会在备份时换页 |
| 磁盘可用 | ≥ 300 GB | 200 GiB 配额 + 30% 余量 + 1 份本地备份 | 低于 100 GB 拒绝开启新上传 |
| 文件系统 | APFS（区分大小写不要求） | 硬链接做备份加速 | 外置盘若为 exFAT，禁用硬链接改复制 |
| 运行用户 | 普通用户，非 root | 6.5 服务身份 | 以 root 跑 → `doctor` 记 degraded |
| 数据目录 | `~/relax/data`（0700） | 单目录收敛 | 放共享目录 → 拒绝启动 |
| 日志目录 | `~/relax/logs` | 14 天 / 50 MB 滚动 | — |
| 备份目录 | `~/relax/backups` + 外置盘 | 见 18.8 | 无外置盘 → `complete=false` |
| 时区 | 系统本地时区（Asia/Shanghai）；存储一律 UTC | 农历差一天的根因 | 混用本地时间 → 全天数据错位 |
| 时间同步 | 开启网络对时，偏差 ≤ 2 s | TOTP 30 s 窗口 | 偏差 > 2 s → 登录 `1003` 类误报 |

## 6. 门禁现状（M0 定基验收）

`cargo xtask ci` 全貌：validate / check-gen / check-ui / check-version / check-docs / check-size / check-no-skips 均 PASS；
fmt / clippy / test 三个闸门记 SKIP（尚未建成，逐条打印）。按手册 9.1，M0 定基允许这 3 处 SKIP。

## 7. 人工确认结果（C0.10，2026-11-28）

决策人在 2026-11-28 的确认中「全部采纳默认决策」，逐条锁定如下（与手册 0.4 开工假设一致）：

| 决策项 | 确认结果 | 来源 |
|---|---|---|
| 穿透方式 | Tailscale/ZeroTier 组网优先；FRP 备选且必须 TLS | 默认采纳 |
| 家庭所有者 | 决策人本人，角色 `OWNER` | 默认采纳 |
| 备份目的地 | 本机外置盘 + 可选异地目录（关闭异地须记 `degraded`） | 默认采纳 |
| 签名人 | 由决策人署名（`<name>` 待本人填写真名，模型不代填） | — |

macOS 版本与芯片已实测记录于 §3：macOS 27.2.1（26B131），arm64。

签字行（`<name>` 由本人替换为真实姓名后即完成 C0.10）：

```text
SIGNED-BY: <name> 2026-11-28
```

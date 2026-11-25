[简体中文](README.md) | [English](README_EN.md)

# Relax · 悠然

**仓库描述**：`悠然：面向家庭的自托管通讯与日程平台，Rust + Tauri 2 构建，数据只留在自己家里 | Relax: a self-hosted family communication and scheduling platform built with Rust and Tauri 2; data stays at home.`

**话题标签**：`rust` `tauri` `self-hosted` `family` `instant-messaging` `calendar` `file-sharing` `photo-album` `sqlite` `axum` `android` `local-first` `privacy` `offline-first` `react` `typescript` `sse` `home-server` `no-cloud` `apache-license`

![Status](https://img.shields.io/badge/status-骨架阶段-yellow?style=flat-square)
![Rust](https://img.shields.io/badge/rust-2021-b7410e?style=flat-square&logo=rust&logoColor=white)
![Tauri](https://img.shields.io/badge/tauri-2-24c8db?style=flat-square&logo=tauri&logoColor=white)
[![License](https://img.shields.io/badge/license-Apache--2.0%20%2F%20MIT-blue?style=flat-square)](./LICENSE)

一套自托管的家庭通讯、日程、文件与相册工具，数据只存在家里那台电脑上。

[快速开始](#快速开始) · [核心特性](#核心特性) · [目录结构](#目录结构) · [已知问题与限制](#已知问题与限制) · [后续路线图](#后续路线图) · [许可证](#许可证)

---

## 项目简介

家庭里的沟通被拆散在多个 IM、日历、网盘和相册里：消息在别人的服务器上，照片要传到别人的云，日程和作息各记各的。Relax 要解决的是这件事：**把一家人的聊天、公告、新闻、作息日程、文件与相册收进一台家里的电脑**，家庭成员用 Android 客户端访问，数据不出家门。

**核心思路**：单机单进程的服务端（`relax serve`）承载全部数据与业务，客户端只做渲染与调用。逻辑全部在 Rust，前端不允许发起网络请求、直连数据库或持久化 token，从架构上保证「数据自主」不是口号。

> 当前仓库处于**骨架阶段**：工作区、契约文件、数据模型草案与前端骨架已就位，服务端与客户端业务尚在搭建。README 描述目标形态与已落地的约束，未实现的能力在下文明确标注。

**适用场景**

- 3～10 人的家庭或小团体，希望聊天、日程、文件、相册集中在一处。
- 家里有一台常驻电脑，能接受内网直连或自建隧道。
- 在意数据自主：不想把家庭照片与对话放到第三方云。

**不适用场景**

- iPhone 用户：当前只覆盖 Android，不覆盖 iOS。
- 需要公网开放注册的社区：本项目不暴露公网注册入口。
- 需要跨地域多活、水平扩展：设计目标是单主机、单进程。

### 优势与局限

**优势**

| 优势 | 说明 | 验证方式 |
| --- | --- | --- |
| 数据不出家门 | 数据只在家庭电脑与备份介质上 | 关闭外网后内网仍可用 |
| 业务全在 Rust | 前端只渲染、只 `invoke()` | `cargo xtask check-ui` 静态门禁 |
| 离线可用 | 断网时消息进 outbox，恢复后幂等提交 | 断网发言后重连，`opId` 去重 |
| 可恢复 | 备份含 manifest 与 sha256，恢复后四项计数一致 | `relax doctor` / 恢复预览 |

**局限**

| 局限 | 影响范围 | 临时规避方式 |
| --- | --- | --- |
| 仅 Android | iPhone 用户无法使用 | 用现有 IM 临时通知 |
| 需自备服务器 | 无托管版本 | 一台常驻电脑即可 |
| 仍处骨架阶段 | 业务未完成 | 不作为日常工具使用 |

## 核心功能

目标形态下的功能，**均未在骨架阶段实现**：

- **群聊**：文本消息、@提醒、离线 outbox、按 `opId` 幂等提交、SSE 实时推送。
- **公告**：发布、编辑、删除公告，回收站保留 30 天，管理员可恢复。
- **新闻聚合**：管理新闻源，定时抓取并在客户端展示。
- **作息与日程**：家庭作息、个人作息、共享日程与生日提醒，系统 `AlarmManager` 按时触发。
- **文件与相册**：分片上传、断点续传、SHA256 校验；手机照片在充电 + Wi-Fi 时自动去重上传。
- **角色与权限**：OWNER / ADMIN / MEMBER / CHILD / GUEST 五类角色，服务端统一下发能力数组。
- **运维**：`doctor`、`migrate`、`restore` 命令行与备份预览。

## 核心特性

### 数据自主的单机架构

- **价值**：家庭照片与对话不应该成为第三方云服务的资产。
- **实现方式**：`crates/relax-server` 以 axum + tokio 单进程运行，`tokio-rusqlite` 单写连接；数据落在本地 `data/`。
- **差异化**：不依赖对象存储与托管数据库，备份即复制目录 + manifest。
- **证据**：架构说明见 `Relax-开发手册.md` 2.1；`contracts/` 定义信封与错误码。

### 逻辑全在 Rust 的五道锁

- **价值**：Tauri 的 WebView 天生能发请求，若不约束，「逻辑在 Rust」会退化成口号。
- **实现方式**：调用面收敛到 `invoke()`、静态门禁、运行期 CSP、TypeScript 严格模式、前端零持久化。
- **差异化**：`cargo xtask check-ui` 命中禁用符号即失败。
- **证据**：五道锁定义见手册 2.2；门禁说明见 `docs/gatekeeping.md`。

### 离线优先与幂等同步

- **价值**：家庭网络不稳定，发言与上传不能因断网丢失或重复。
- **实现方式**：消息进 outbox，恢复后按 `opId` 幂等提交；文件分片上传只重试失败分片。
- **差异化**：SSE 重连后触发同步，而不是轮询。
- **证据**：同步契约见手册 3.6；实时通道见 3.8。

## 设计细节与工程取舍

### 为什么只做 Android

- **场景**：家庭设备以 Android 为主，iOS 需要另一套系统能力插件。
- **处理方式**：先做 Android，iOS 不设分支。
- **原因**：把有限精力放在一条链路上做透，避免两套壳同时半成品。

### 为什么单写连接

- **场景**：家庭规模写入并发低，但数据一致性要求高。
- **处理方式**：`tokio-rusqlite` 单写连接，读走连接池。
- **原因**：SQLite 单写更简单可靠，家庭场景无需分库分表。

### 为什么不暴露公网注册

- **场景**：自托管服务一旦开放注册，容易被扫描与滥用。
- **处理方式**：只允许邀请码加入，公网入口默认关闭。
- **原因**：家庭服务的攻击面越小越好。

## 技术栈与依赖清单

| 类别 | 技术或组件 | 版本约束 | 用途 |
| --- | --- | --- | --- |
| 运行时 | Rust | 2021 edition | 全部业务逻辑 |
| 异步运行时 | tokio | 1.40+ | 全项目唯一运行时 |
| HTTP 服务 | axum | 0.7+ | 路由与中间件 |
| 数据库 | SQLite（rusqlite / tokio-rusqlite） | bundled | 本地持久化 |
| 桌面/移动外壳 | Tauri | 2.x | Android 外壳与 IPC |
| 前端 | React + TypeScript | 严格模式 | 仅渲染与 `invoke()` |
| 实时通道 | SSE | — | 服务端推送 |

第三方依赖明细与许可证见 [NOTICE](./NOTICE)，版本区间见 `contracts/version-ranges.toml`。

## 安装

环境要求：Rust 2021 工具链、Node.js 18+、Tauri 2 Android 构建依赖（Android SDK / NDK、API 24+）。

```bash
git clone https://github.com/ntars/Relax.git
cd Relax
cargo build
```

验证：

```bash
cargo test
```

预期输出：

```text
test result: ok.
```

当前为骨架工程，尚未提供服务端安装包与 Android APK。升级：`git pull`。卸载：删除目录与 `data/`。

## 快速开始

最小可运行示例（当前骨架）：

```bash
cd Relax
cargo run
```

预期输出：

```text
Hello, world!
```

规划中的服务端启动方式：

```bash
cargo run -p relax-server -- serve
```

## 配置说明

规划中的服务端配置，骨架阶段尚未落地。

| 键名 | 类型 | 默认值 | 必填 | 取值范围 | 作用 | 示例 |
| --- | --- | --- | --- | --- | --- | --- |
| `server.bind` | string | `127.0.0.1:8787` | 否 | `host:port` | 监听地址 | `0.0.0.0:8787` |
| `server.tls.cert` | string | 自签证书路径 | 是（远程访问） | 文件路径 | TLS 证书 | `certs/server.pem` |
| `data.dir` | string | `./data` | 否 | 目录路径 | 数据目录 | `/var/lib/relax` |
| `backup.dir` | string | 无 | 否 | 目录路径 | 备份目标 | `/mnt/backup/relax` |
| `news.interval_minutes` | integer | `60` | 否 | `5–1440` | 新闻抓取间隔 | `30` |

配置优先级：环境变量 > 配置文件 > 默认值。热更新：不支持，需重启。

## API 或模块说明

契约以 `contracts/` 为准，所有接口使用统一信封。

```json
{"ok": true, "data": {}, "error": null}
```

| 路径 | 方法 | 说明 |
| --- | --- | --- |
| `/api/v1/health` | GET | 健康检查 |
| `/api/v1/sync` | POST | 增量同步，按 `opId` 幂等 |
| `/api/v1/stream` | GET | SSE 实时通道 |

错误码：见 `contracts/error-codes.json`，只追加不修改。

调用示例：

```bash
curl -s http://127.0.0.1:8787/api/v1/health
```

限流与配额：家庭内网默认不限流；上传受配额约束。

## 目录结构

```text
Relax/
├── crates/
│   ├── relax-contract/  # 契约类型与校验
│   ├── relax-core/      # 业务核心（可单测，不依赖 tauri）
│   ├── relax-server/    # axum 服务端
│   └── relax-client/    # 客户端业务、同步、文件传输
├── src-tauri/           # Tauri 外壳与命令层
├── ui/                  # React + TS，仅渲染
├── contracts/           # 错误码、版本区间等唯一真相
├── docs/                # 门禁与 CSP 说明
├── xtask/              # 静态检查与构建任务
└── src/                 # 工作区入口
```

## 性能表现

骨架阶段无性能数据。设计目标（非实测）：

| 指标 | 目标 | 说明 |
| --- | --- | --- |
| 服务端常驻内存 | 100 MB 内 | 单进程、单主机 |
| 同步延迟 | 内网 1 秒内 | SSE 推送 |
| 照片去重 | 按 `deviceId+sha256+captureTime` | 避免重复占空间 |

## 安全与隐私

数据收集与存储：不收集分析数据；全部数据在 `data/` 与备份介质。

传输加密：HTTPS（rustls），只信任内嵌私有 CA；不暴露公网注册。

权限模型：OWNER / ADMIN / MEMBER / CHILD / GUEST，能力判定只在服务端中间件做一次。

已知风险：自签证书需在客户端预置；设备丢失需 Owner 撤销。

漏洞上报渠道：<ntars@qq.com>。

## 兼容性说明

| 维度 | 支持范围 | 验证状态 |
| --- | --- | --- |
| 操作系统 | 服务端 macOS / Linux；客户端 Android API 24+ | 骨架可编译 |
| 运行时 | Rust 2021 | 已验证 |
| 浏览器 | 仅 Tauri WebView | — |
| 数据库 | SQLite（bundled） | 规划中 |

明确不支持：iOS、公网多租户、水平扩展集群。

## 已知问题与限制

**已知问题**

| 问题描述 | 影响范围 | 临时规避方案 | 跟踪链接 |
| --- | --- | --- | --- |
| 仅有工作区骨架 | 全部业务功能 | 等待路线图落地 | [Relax-开发手册.md](Relax-开发手册.md) |

**当前限制**：服务端与客户端业务未实现；契约仍在演进；无 APK。

**未实现功能**

| 功能 | 未实现原因 | 计划状态 |
| --- | --- | --- |
| 群聊与同步 | 骨架阶段 | 排期中 |
| 文件/相册上传 | 骨架阶段 | 排期中 |
| 备份与恢复 | 骨架阶段 | 排期中 |
| Android 系统能力插件 | 骨架阶段 | 排期中 |

## 更新日志

版本变更遵循语义化版本与 Keep a Changelog 结构。当前 `0.1.0` 为骨架版本，尚无 `CHANGELOG.md`。

## 后续路线图

> 下列内容为计划项，非交付承诺；实际范围以发布记录为准。

| 计划项 | 状态 | 目标版本 |
| --- | --- | --- |
| 服务端身份、设备与同步 | 计划中 | 0.1.0 |
| 群聊、公告与 SSE | 计划中 | 0.1.0 |
| 文件与相册上传 | 计划中 | 0.2.0 |
| 备份、恢复与审计 | 计划中 | 0.2.0 |

## 参与贡献

- 开发环境搭建：安装 Rust 2021、Node.js 18+、Tauri 2 与 Android 构建依赖。
- 分支模型：基于 `main` 切特性分支。
- 提交规范：Conventional Commits，中文描述改动本身。
- PR 要求：改动单一、`cargo test` 与 `cargo xtask check-ui` 通过。
- 行为准则：契约变更须走五步流程，不得各写一份。

## 许可证

Copyright © 2026 星玖工作室 Ntarstudio. Licensed under Apache-2.0 / MIT.

本项目按目录分层许可：代码采用 Apache-2.0，文档与契约采用 MIT。全文见 [LICENSE](./LICENSE) 与 [LICENSE-MIT](./LICENSE-MIT)，分层说明见 [LICENSING.md](./LICENSING.md)。

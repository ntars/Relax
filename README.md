[简体中文](README.md) | [English](README_EN.md)

# 悠然 Relax

**仓库描述**：`悠然 Relax：面向家庭的聊天、日程、相册与网盘——装在家中常开的电脑上，每位家庭成员用手机接入，消息、照片、日程与文件只存在这台电脑里，不经过任何云。 | Relax: chat, calendar, albums and file storage for the household — runs on the always-on home computer, every family member joins from a phone, nothing goes through a cloud.`

**话题标签**（20 个，仅英文）： `android`、`axum`、`calendar`、`family`、`family-chat`、`file-sharing`、`home-cloud`、`home-server`、`household`、`instant-messaging`、`local-first`、`nas-alternative`、`no-cloud`、`photo-album`、`privacy`、`react`、`rust`、`self-hosted`、`sqlite`、`tauri`

![Status](https://img.shields.io/badge/status-早期开发-yellow?style=flat-square)
![Rust](https://img.shields.io/badge/rust-2021-b7410e?style=flat-square&logo=rust&logoColor=white)
![Tauri](https://img.shields.io/badge/tauri-2-24c8db?style=flat-square&logo=tauri&logoColor=white)
[![License](https://img.shields.io/badge/license-Apache--2.0%20%2F%20MIT-blue?style=flat-square)](./LICENSE)

面向家庭的聊天、日程、相册与网盘——安装在家中那台常开的电脑上，每位家庭成员均可通过手机接入，消息、照片、日程与文件仅保存在这台电脑中，不经过任何云。

[快速开始](#快速开始) · [核心功能](#核心功能) · [技术栈](#技术栈) · [目录结构](#目录结构) · [已知问题](#已知问题与限制) · [路线图](#路线图) · [许可证](#许可证)

---

## 项目简介

家庭沟通散落在多个即时通讯、日历、网盘与相册服务里：消息在别人的服务器上，照片在别人的云里，日程各记各的。Relax 把这些能力收拢到**一台家用电脑**：Android 客户端访问，数据只留在家里。

**核心思路**：单进程、单主机的服务端（`cargo run -p relax-server`）持有全部数据与业务逻辑，客户端只负责渲染与调用。业务逻辑全部在 Rust 侧，前端不得发起网络请求、不得直接读写 SQLite、不得持久化令牌——「数据主权」由架构保证，而不是靠承诺。

> 当前处于**早期开发**：工作区、契约、数据模型与前端骨架已就位；服务端已完成鉴权、同步、聊天、公告、新闻、日程、文件、相册、例行事项与审计等路由骨架，权限矩阵已按 OWNER / ADMIN / MEMBER / CHILD / GUEST 落地。尚未发布可日常使用的版本，未实现能力在下方明确标注。

**适用场景**

- 3–10 人的家庭或小团体，需要把聊天、日程、文件与照片放在一处。
- 有一台常开电脑，能接受局域网访问或自建隧道。
- 在意数据主权，不希望家庭照片与对话进入第三方云。

**不适用场景**

- iPhone 用户：当前只做 Android，不覆盖 iOS。
- 开放注册的公共社区：没有公开注册入口。
- 多地域、水平扩展部署：设计目标是单主机、单进程。

### 优势与局限

**优势**

| 优势 | 说明 | 验证方式 |
| --- | --- | --- |
| 数据留在家 | 数据只在家用电脑与备份介质上 | 断掉外网，局域网内仍可用 |
| 逻辑在 Rust | 前端只渲染并调用 `invoke()` | `cargo xtask check-ui` 静态门禁 |
| 离线可用 | 消息进入 outbox，按 `opId` 幂等提交 | 断网发送、重连后去重 |
| 可恢复 | 备份带 manifest 与 sha256，恢复后四类计数一致 | `relax doctor` / 恢复预览 |

**局限**

| 局限 | 影响范围 | 规避方式 |
| --- | --- | --- |
| 仅 Android | iPhone 用户无法使用 | 通知暂用现有即时通讯 |
| 需自备服务端 | 没有托管版 | 任意常开电脑均可 |
| 仍在早期 | 业务能力未完整 | 暂不作为日常主力 |

## 核心功能

目标能力；已落地路由骨架的条目单独标注：

- **群聊**：文本消息、@ 提及、离线 outbox、按 `opId` 幂等提交、SSE 实时推送（路由已接入）。
- **公告**：发布、编辑、删除；30 天回收站与管理员恢复（路由已接入）。
- **新闻聚合**：源管理、定时抓取、客户端展示（路由已接入）。
- **日程**：家庭与个人例行事项、共享日历、生日提醒（路由已接入）。
- **文件与相册**：分片上传、断点续传、SHA256 校验；充电 + Wi-Fi 时自动备份照片并去重（路由已接入）。
- **角色与权限**：OWNER / ADMIN / MEMBER / CHILD / GUEST，服务端中间件统一鉴权，`contracts/capabilities.json` 为能力真源。
- **运维**：`doctor`、`migrate`、`restore` 与备份预览。

## 核心特性

### 数据主权：单主机单进程

- **价值**：家庭照片与对话不应成为云厂商的资产。
- **实现**：`crates/relax-server` 以 axum + tokio 单进程运行，单个 `tokio-rusqlite` 写者；数据落在本地 `data/`。
- **差异**：没有对象存储与托管数据库；备份就是目录复制加一份 manifest。
- **证据**：`crates/relax-server/src/db.rs`、`contracts/` 下的信封与错误码。

### 五道锁把逻辑关在 Rust 侧

- **价值**：Tauri WebView 原生就能发请求；没有约束时「逻辑在 Rust」会退化成口号。
- **实现**：调用面收窄到 `invoke()`、静态门禁、运行时 CSP、严格 TypeScript、前端零持久化。
- **差异**：`cargo xtask check-ui` 命中禁用符号即失败。
- **证据**：`docs/gatekeeping.md` 与 `docs/CSP.md`。

### 离线优先的幂等同步

- **价值**：家庭网络不稳定，消息与上传不能丢也不能重。
- **实现**：消息进 outbox，按 `opId` 幂等提交；分片上传只重试失败分片。
- **差异**：SSE 重连触发同步，而不是轮询。
- **证据**：`crates/relax-server/src/routes/sync.rs` 与 `sse.rs`。

## 设计细节与工程取舍

### 为什么只做 Android

- **场景**：家庭设备以 Android 为主；iOS 需要另一套系统能力插件。
- **做法**：先做 Android，不铺 iOS 分支。
- **理由**：把一条路径做完整，而不是交付两个半成品外壳。

### 为什么单写者

- **场景**：家庭写入并发低，但一致性要求高。
- **做法**：一个 `tokio-rusqlite` 写者，读走连接池。
- **理由**：单写者 SQLite 更简单可靠；家庭场景不需要分片。

### 为什么没有公开注册

- **场景**：自托管服务开放注册等于邀请扫描与滥用。
- **做法**：只发邀请码，公网入口默认关闭。
- **理由**：家庭服务应尽量缩小攻击面。

## 技术栈与依赖清单

| 类别 | 技术 | 版本 | 用途 |
| --- | --- | --- | --- |
| 运行时 | Rust | 2021 edition | 全部业务逻辑 |
| 异步运行时 | tokio | 1.40+ | 项目内唯一异步运行时 |
| HTTP 服务 | axum | 0.7+ | 路由与中间件 |
| 数据库 | SQLite（rusqlite / tokio-rusqlite） | bundled | 本地持久化 |
| 桌面 / 移动外壳 | Tauri | 2.x | Android 外壳与 IPC |
| 前端 | React + TypeScript | strict | 只渲染与 `invoke()` |
| 实时通道 | SSE | — | 服务端推送 |

依赖明细与许可见 [NOTICE](./NOTICE)；版本范围见 `contracts/version-ranges.toml`。

## 安装与部署

环境要求：Rust 2021 工具链、Node.js 18+、Tauri 2 的 Android 构建依赖（Android SDK / NDK，API 24+）。

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

当前没有服务端安装包与 Android APK。升级：`git pull`。卸载：删除仓库目录与 `data/`。

## 快速开始

当前最小可运行路径：

```bash
cd Relax
cargo run -p relax-server
```

服务端监听 `0.0.0.0:3000`，数据写入 `data/relax.db`。健康检查返回纯文本：

```bash
curl -s http://127.0.0.1:3000/api/v1/health
# ok
```

## 配置说明

服务端配置仍在收敛，以下为当前约定：

| 键 | 类型 | 默认值 | 必填 | 取值范围 | 作用 | 示例 |
| --- | --- | --- | --- | --- | --- | --- |
当前没有配置文件，监听地址与数据库路径硬编码在 `crates/relax-server/src/main.rs`：

| 键 | 类型 | 当前值 | 必填 | 取值范围 | 作用 |
| --- | --- | --- | --- | --- | --- |
| `server.bind` | string | `0.0.0.0:3000` | 是（硬编码） | `host:port` | 监听地址 |
| `data.db` | string | `data/relax.db` | 是（硬编码） | 文件路径 | SQLite 数据库 |

配置化与热更新未实现，修改需改源码后重启。后续计划见路线图。

## API 与模块说明

`contracts/` 下的契约是唯一真源，所有接口使用统一信封。

```json
{"ok": true, "data": {}, "error": null}
```

| 路径 | 方法 | 用途 |
| --- | --- | --- |
| `/api/v1/health` | GET | 健康检查，返回 `ok` |
| `/api/v1/auth/*` | POST | 登录、设备注册与会话 |
| `/api/v1/sync` | POST | 增量同步，按 `opId` 幂等 |
| `/api/v1/stream` | GET | SSE 实时通道 |
| `/api/v1/chat/*` | GET/POST | 会话、消息与已读 |
| `/api/v1/files/*` | GET/POST | 文件与上传 |
| `/api/v1/folders/*` | GET/POST | 文件夹 |
| `/api/v1/announcements/*` | GET/POST | 公告 |
| `/api/v1/news/*` | GET/POST | 新闻源与条目 |
| `/api/v1/routines/*` | GET/POST | 例行事项 |
| `/api/v1/schedules/*` | GET/POST | 日程 |

错误码见 `contracts/error-codes.json`，只增不改。示例：

```bash
curl -s http://127.0.0.1:3000/api/v1/health
```

家庭局域网内不限流；上传受配额约束。

## 目录结构

```text
Relax/
├── crates/
│   ├── relax-contract/  # 契约类型与校验
│   ├── relax-core/      # 业务核心（可单测，不依赖 tauri）
│   ├── relax-server/    # axum 服务端
│   │   ├── db.rs        # 连接与迁移
│   │   ├── modules/     # 公告、聊天、文件、新闻、例行、日程等模块
│   │   └── routes/      # HTTP 路由与 SSE
│   └── relax-client/    # 客户端逻辑、同步与文件传输
├── src-tauri/           # Tauri 外壳与命令层
├── ui/                  # React + TS，只渲染
├── contracts/           # 错误码、能力矩阵、版本范围
├── docs/                # 门禁与 CSP 说明
├── xtask/               # 静态检查与构建任务
└── src/                 # 工作区入口
```

## 性能表现

早期阶段尚无实测数据。设计目标（未测量）：

| 指标 | 目标 | 说明 |
| --- | --- | --- |
| 服务端内存 | 100 MB 以内 | 单进程、单主机 |
| 同步延迟 | 局域网 1 s 内 | SSE 推送 |
| 照片去重 | 按 `deviceId+sha256+captureTime` | 避免重复存储 |

## 安全与隐私

数据收集：无埋点；全部数据在 `data/` 与备份介质。

传输：rustls 提供 HTTPS，只信任内嵌私有 CA；不开放公开注册。

权限模型：OWNER / ADMIN / MEMBER / CHILD / GUEST；能力检查在服务端中间件统一完成。

已知风险：自签证书需在客户端预置；设备丢失需 Owner 撤销。

漏洞上报：<ntars@qq.com>。

## 兼容性说明

| 维度 | 支持 | 验证状态 |
| --- | --- | --- |
| 操作系统 | 服务端：macOS / Linux；客户端：Android API 24+ | 服务端可构建 |
| 运行时 | Rust 2021 | 是 |
| 浏览器 | 仅 Tauri WebView | — |
| 数据库 | SQLite（bundled） | 路由层已接入 |

不支持：iOS、公开多租户托管、水平扩展集群。

## 已知问题与限制

**已知问题**

| 问题 | 影响范围 | 临时规避 | 跟踪 |
| --- | --- | --- | --- |
| 尚未发布可用版本 | 全部业务功能 | 等待路线图 | 本仓库 Issues |

**当前限制**：服务端路由已成型但业务闭环未完成；契约仍在演进；无 APK。

**未实现功能**

| 功能 | 未实现原因 | 计划状态 |
| --- | --- | --- |
| 聊天与同步闭环 | 早期开发 | 排期中 |
| 文件 / 照片上传 | 早期开发 | 排期中 |
| 备份与恢复 | 早期开发 | 排期中 |
| Android 系统插件 | 早期开发 | 排期中 |

## 更新日志

版本遵循语义化版本与 Keep a Changelog。`0.1.0` 为当前开发版本，尚无独立 `CHANGELOG.md`。

## 后续路线图

> 以下为计划，不构成交付承诺；实际范围以发布记录为准。

| 事项 | 状态 | 目标版本 |
| --- | --- | --- |
| 身份、设备与同步 | 路由骨架已落地 | 0.1.0 |
| 群聊、公告、SSE | 路由骨架已落地 | 0.1.0 |
| 文件与照片上传 | 路由骨架已落地 | 0.2.0 |
| 备份、恢复与审计 | 路由骨架已落地 | 0.2.0 |

## 参与贡献

- 环境：Rust 2021、Node.js 18+、Tauri 2 与 Android 构建依赖。
- 分支：从 `main` 切功能分支。
- 提交：Conventional Commits，主题写清改动。
- PR：一次一个改动；`cargo test` 与 `cargo xtask check-ui` 通过。
- 契约：变更须走五步流程，不允许分叉副本。

## 许可证

Copyright © 2026 星玖工作室 Ntarstudio. Licensed under Apache-2.0 / MIT.

按目录分层：`crates/`、`src-tauri/`、`ui/src/` 等代码目录为 Apache-2.0；`README.md`、`docs/`、`contracts/` 等文档与契约为 MIT。全文见 [LICENSE](./LICENSE) 与 [LICENSE-MIT](./LICENSE-MIT)，路由表见 [LICENSING.md](./LICENSING.md)。

## NOTICE 合规声明

第三方依赖均为 MIT / Apache-2.0 / BSD / ISC / Unicode-3.0，明细与商标声明见 [NOTICE](./NOTICE)。

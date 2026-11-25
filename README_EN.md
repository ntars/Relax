[简体中文](README.md) | [English](README_EN.md)

# Relax

**Description**: `悠然：面向家庭的自托管通讯与日程平台，Rust + Tauri 2 构建，数据只留在自己家里 | Relax: a self-hosted family communication and scheduling platform built with Rust and Tauri 2; data stays at home.`

**Topics**: `rust` `tauri` `self-hosted` `family` `instant-messaging` `calendar` `file-sharing` `photo-album` `sqlite` `axum` `android` `local-first` `privacy` `offline-first` `react` `typescript` `sse` `home-server` `no-cloud` `apache-license`

![Status](https://img.shields.io/badge/status-scaffold-yellow?style=flat-square)
![Rust](https://img.shields.io/badge/rust-2021-b7410e?style=flat-square&logo=rust&logoColor=white)
![Tauri](https://img.shields.io/badge/tauri-2-24c8db?style=flat-square&logo=tauri&logoColor=white)
[![License](https://img.shields.io/badge/license-Apache--2.0%20%2F%20MIT-blue?style=flat-square)](./LICENSE)

A self-hosted family hub for chat, schedules, files, and photos — all data stays on your home computer.

[Quick Start](#quick-start) · [Features](#key-features) · [Layout](#project-layout) · [Known Issues](#known-issues-and-limitations) · [Roadmap](#roadmap) · [License](#license)

---

## Introduction

Family communication is scattered across several IMs, calendars, cloud drives, and photo services: messages sit on someone else's server, photos are uploaded to someone else's cloud, and schedules are kept separately. Relax fixes that by **bringing a family's chat, announcements, news, schedules, files, and photos onto one home computer**, accessed from Android clients, with data never leaving the house.

**Core idea**: a single-process, single-host server (`relax serve`) owns all data and business logic; clients only render and call. Business logic lives entirely in Rust — the frontend may not make network requests, touch SQLite, or persist tokens, so "data sovereignty" is guaranteed by architecture rather than by promise.

> This repository is at **scaffold stage**: the workspace, contracts, data-model drafts, and frontend skeleton are in place, while server and client logic are still being built. This README describes the target shape and the constraints already in place; unimplemented capabilities are explicitly labeled below.

**When it fits**

- A household or small group of 3–10 people that wants chat, schedules, files, and photos in one place.
- A home with an always-on computer that can accept LAN access or a self-hosted tunnel.
- Anyone who cares about data sovereignty and does not want family photos and conversations in a third-party cloud.

**When it does not fit**

- iPhone users: the project targets Android only and does not cover iOS.
- A public community with open registration: there is no public sign-up endpoint.
- Multi-region, horizontally scaled deployments: the design target is one host, one process.

### Strengths and Limits

**Strengths**

| Strength | Detail | Verification |
| --- | --- | --- |
| Data stays home | Data lives only on the home computer and backup media | LAN still works with the WAN off |
| Business logic in Rust | Frontend only renders and calls `invoke()` | `cargo xtask check-ui` static gate |
| Offline-capable | Messages queue in an outbox and submit idempotently | Speak offline, reconnect, `opId` dedupes |
| Restorable | Backups carry a manifest and sha256; four counts match after restore | `relax doctor` / restore preview |

**Limits**

| Limit | Scope | Workaround |
| --- | --- | --- |
| Android only | iPhone users cannot use it | Use an existing IM for notifications |
| Needs your own server | No hosted version | Any always-on computer works |
| Still scaffold | Business logic incomplete | Not for daily use yet |

## Key Features

Target capabilities; **none are implemented at scaffold stage**:

- **Group chat**: text messages, @mentions, offline outbox, idempotent submission by `opId`, SSE realtime push.
- **Announcements**: publish, edit, delete; 30-day recycle bin with admin restore.
- **News aggregation**: manage sources, fetch on a schedule, display in the client.
- **Schedules**: family and personal routines, shared calendar, birthday reminders via `AlarmManager`.
- **Files and albums**: chunked upload, resumable transfer, SHA256 verification; automatic photo backup on charge + Wi-Fi with deduplication.
- **Roles and permissions**: OWNER / ADMIN / MEMBER / CHILD / GUEST, with a capability array issued by the server.
- **Operations**: `doctor`, `migrate`, `restore` commands and backup preview.

## Core Features

### Data-sovereign single-host architecture

- **Value**: family photos and conversations should not become a cloud provider's asset.
- **Implementation**: `crates/relax-server` runs axum + tokio in one process with a single `tokio-rusqlite` writer; data lives under local `data/`.
- **Difference**: no object storage or managed database; a backup is a directory copy plus a manifest.
- **Evidence**: architecture in `Relax-开发手册.md` section 2.1; envelope and error codes in `contracts/`.

### Five locks keeping logic in Rust

- **Value**: a Tauri WebView can make requests natively; without constraints, "logic in Rust" degrades into a slogan.
- **Implementation**: call surface narrowed to `invoke()`, a static gate, runtime CSP, strict TypeScript, and zero frontend persistence.
- **Difference**: `cargo xtask check-ui` fails on banned symbols.
- **Evidence**: five locks in the manual section 2.2; gates in `docs/gatekeeping.md`.

### Offline-first idempotent sync

- **Value**: home networks are flaky; messages and uploads must not be lost or duplicated.
- **Implementation**: messages enter an outbox and submit idempotently by `opId`; chunked uploads retry only failed chunks.
- **Difference**: SSE reconnection triggers sync instead of polling.
- **Evidence**: sync contract in manual section 3.6; realtime channel in 3.8.

## Design Details and Trade-offs

### Why Android only

- **Scenario**: household devices are predominantly Android; iOS needs another system-capability plugin.
- **Approach**: build Android first, with no iOS branch.
- **Reason**: finish one path well instead of shipping two half-done shells.

### Why a single writer

- **Scenario**: a household has low write concurrency but high consistency requirements.
- **Approach**: one `tokio-rusqlite` writer, reads from a pool.
- **Reason**: single-writer SQLite is simpler and more reliable; a household needs no sharding.

### Why no public registration

- **Scenario**: an open sign-up on a self-hosted service invites scanning and abuse.
- **Approach**: invitation codes only, public entry closed by default.
- **Reason**: a family service should minimize its attack surface.

## Tech Stack and Dependencies

| Category | Technology | Version | Purpose |
| --- | --- | --- | --- |
| Runtime | Rust | 2021 edition | All business logic |
| Async runtime | tokio | 1.40+ | The only runtime in the project |
| HTTP server | axum | 0.7+ | Routing and middleware |
| Database | SQLite (rusqlite / tokio-rusqlite) | bundled | Local persistence |
| Desktop/mobile shell | Tauri | 2.x | Android shell and IPC |
| Frontend | React + TypeScript | strict mode | Render and `invoke()` only |
| Realtime | SSE | — | Server push |

Dependency details and licenses: see [NOTICE](./NOTICE); version ranges: `contracts/version-ranges.toml`.

## Installation

Requirements: Rust 2021 toolchain, Node.js 18+, and Tauri 2 Android build dependencies (Android SDK / NDK, API 24+).

```bash
git clone https://github.com/ntars/Relax.git
cd Relax
cargo build
```

Verify:

```bash
cargo test
```

Expected output:

```text
test result: ok.
```

No server package or Android APK yet; this is a scaffold. Upgrade: `git pull`. Uninstall: delete the directory and `data/`.

## Quick Start

Minimal runnable example (current scaffold):

```bash
cd Relax
cargo run
```

Expected output:

```text
Hello, world!
```

Planned server start:

```bash
cargo run -p relax-server -- serve
```

## Configuration

Planned server configuration; not implemented at scaffold stage.

| Key | Type | Default | Required | Range | Purpose | Example |
| --- | --- | --- | --- | --- | --- | --- |
| `server.bind` | string | `127.0.0.1:8787` | No | `host:port` | Listen address | `0.0.0.0:8787` |
| `server.tls.cert` | string | self-signed path | Yes (remote) | file path | TLS certificate | `certs/server.pem` |
| `data.dir` | string | `./data` | No | directory | Data directory | `/var/lib/relax` |
| `backup.dir` | string | none | No | directory | Backup target | `/mnt/backup/relax` |
| `news.interval_minutes` | integer | `60` | No | `5–1440` | News fetch interval | `30` |

Precedence: environment variables > config file > defaults. Hot reload: not supported; restart required.

## API and Modules

Contracts under `contracts/` are authoritative; every endpoint uses a uniform envelope.

```json
{"ok": true, "data": {}, "error": null}
```

| Path | Method | Purpose |
| --- | --- | --- |
| `/api/v1/health` | GET | Health check |
| `/api/v1/sync` | POST | Incremental sync, idempotent by `opId` |
| `/api/v1/stream` | GET | SSE realtime channel |

Error codes: see `contracts/error-codes.json`, append-only.

Example call:

```bash
curl -s http://127.0.0.1:8787/api/v1/health
```

Rate limits: none on the home LAN; uploads are quota-bound.

## Project Layout

```text
Relax/
├── crates/
│   ├── relax-contract/  # Contract types and validation
│   ├── relax-core/      # Business core (unit-testable, no tauri)
│   ├── relax-server/    # axum server
│   └── relax-client/    # Client logic, sync, file transfer
├── src-tauri/           # Tauri shell and command layer
├── ui/                  # React + TS, render only
├── contracts/           # Error codes, version ranges, single source of truth
├── docs/                # Gate and CSP notes
├── xtask/               # Static checks and build tasks
└── src/                 # Workspace entry point
```

## Performance

No measurements at scaffold stage. Design goals (not measured):

| Metric | Goal | Note |
| --- | --- | --- |
| Server memory | Under 100 MB | Single process, single host |
| Sync latency | Under 1 s on LAN | SSE push |
| Photo dedupe | By `deviceId+sha256+captureTime` | Avoids duplicate storage |

## Security and Privacy

Data collection: no analytics; all data lives in `data/` and on backup media.

Transport: HTTPS via rustls, trusting only an embedded private CA; no public registration.

Permission model: OWNER / ADMIN / MEMBER / CHILD / GUEST; capability checks happen once in server middleware.

Known risk: the self-signed certificate must be provisioned on clients; a lost device needs Owner revocation.

Reporting: <ntars@qq.com>.

## Compatibility

| Dimension | Supported | Verified |
| --- | --- | --- |
| OS | Server: macOS / Linux; client: Android API 24+ | Scaffold builds |
| Runtime | Rust 2021 | Yes |
| Browser | Tauri WebView only | — |
| Database | SQLite (bundled) | Planned |

Not supported: iOS, public multi-tenant hosting, horizontally scaled clusters.

## Known Issues and Limitations

**Known issues**

| Issue | Scope | Workaround | Tracker |
| --- | --- | --- | --- |
| Workspace scaffold only | All business features | Wait for the roadmap | [Relax-开发手册.md](Relax-开发手册.md) |

**Current limitations**: server and client logic not implemented; contracts still evolving; no APK.

**Unimplemented**

| Feature | Reason | Status |
| --- | --- | --- |
| Chat and sync | Scaffold stage | Scheduled |
| File/photo upload | Scaffold stage | Scheduled |
| Backup and restore | Scaffold stage | Scheduled |
| Android system plugin | Scaffold stage | Scheduled |

## Changelog

Versioning follows Semantic Versioning and Keep a Changelog. `0.1.0` is the scaffold version; there is no `CHANGELOG.md` yet.

## Roadmap

> The following are plans, not delivery commitments; the actual scope follows release records.

| Item | Status | Target |
| --- | --- | --- |
| Server identity, devices, and sync | Planned | 0.1.0 |
| Group chat, announcements, SSE | Planned | 0.1.0 |
| File and photo upload | Planned | 0.2.0 |
| Backup, restore, and audit | Planned | 0.2.0 |

## Contributing

- Setup: install Rust 2021, Node.js 18+, Tauri 2, and Android build dependencies.
- Branching: feature branches off `main`.
- Commits: Conventional Commits with a descriptive subject.
- PRs: one change per PR; `cargo test` and `cargo xtask check-ui` green.
- Conduct: contract changes must follow the five-step process; no divergent copies.

## License

Copyright © 2026 Ntarstudio. Licensed under Apache-2.0 / MIT.

Licenses are layered by directory: code under Apache-2.0, docs and contracts under MIT. See [LICENSE](./LICENSE) and [LICENSE-MIT](./LICENSE-MIT); the mapping is in [LICENSING.md](./LICENSING.md).

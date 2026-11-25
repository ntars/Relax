# 许可分层说明（LICENSING）

本项目采用**按目录分层**的许可：代码用 Apache-2.0，文档与契约用 MIT。

## 一、分层映射

| 范围 | 许可证 | 理由 |
| --- | --- | --- |
| `crates/`、`src-tauri/`、`ui/src/`、`xtask/` | Apache-2.0 | 含专利授权（§3）与商标保留（§6） |
| `README.md`、`README_EN.md`、`docs/`、`contracts/`、`LICENSING.md` | MIT | 契约与文档要能被自由复用 |

## 二、我要用你的代码，该看哪份

| 我要做什么 | 看哪份 |
| --- | --- |
| 复制、修改、分发源码 | [LICENSE](./LICENSE)（Apache-2.0） |
| 引用契约、错误码或文档 | [LICENSE-MIT](./LICENSE-MIT)（MIT） |
| 使用 `Relax` / `悠然` 名称或图标 | 不在许可范围内，需另行授权 |

## 三、硬规则

1. 每个源文件头必须有 SPDX 标识，且与所在目录映射一致。
2. 依赖白名单只放 MIT / Apache-2.0 / BSD / ISC / Unicode-3.0；不引入 GPL / AGPL / SSPL。
3. `Relax` 与 `悠然` 名称、图标不随代码授权。
4. 贡献采用 DCO（提交带 `Signed-off-by:`）。

Copyright © 2026 星玖工作室 Ntarstudio.

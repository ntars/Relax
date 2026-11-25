# Relax - 家庭内部系统

本地模型按卡执行的完整家庭内部系统，支持 APK + 服务端部署。

## 特性

- 📱 完整 Android APK
- 🖥️ 本地常驻服务端
- 🔒 端到端加密同步
- 📸 相片自动备份
- 🗓️ 农历计算支持
- 💾 完整备份恢复

## 开发

```bash
# 构建服务端
cargo build --release

# 构建 Tauri 应用（macOS）
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer cargo build --release

# 构建 APK（需要 Android SDK）
cargo tauri build --target aarch64-linux-android
```

## 项目结构

```
├── Cargo.toml                    # Rust workspace 配置
├── crates/
│   ├── relax-contract/          # 契约定义（错误码、权限矩阵）
│   ├── relax-core/              # 核心业务逻辑
│   ├── relax-server/            # 服务端实现
│   └── relax-client/            # 客户端逻辑
├── ui/                          # React + TypeScript UI
├── src-tauri/                   # Tauri 配置
└── contracts/                   # 契约文件
```

## 开发阶段

- **M0 定基**：工具链齐、契约与门禁可复跑
- **M1 服务端切片**：auth/chat/sync 完整闭环
- **M2 客户端切片**：登录/聊天/SSE 在真机 APK 上成立
- **M3 六功能服务端**：公告/新闻/作息/日程/文件/相册
- **M4 六功能客户端**：六功能可在 APK 使用
- **M5 增强必备**：照片备份/农历/备份恢复等
- **M6 硬化发布候选**：安全/性能/签名/打包
- **M7 发布运营**：家人可用、故障可归因

## 许可证

MIT

# 卸载说明（C6.4）

本目录是本项目**唯一**的部署形态：macOS 用户级 launchd Agent。不写 systemd、不写 Windows 服务、不装 LaunchDaemon（root 常驻）。

## 装 / 卸 / 升级

```bash
# 安装（首次或重装）：前置 migrate + doctor 全绿
sh deploy/launchd/install.sh

# 卸载（只停服务 + 移除 Agent，保留数据）
sh deploy/launchd/uninstall.sh

# 升级：禁止运行中覆盖二进制（KeepAlive 会拉起旧句柄，版本显示新但行为旧）
launchctl bootout gui/$(id -u)/dev.relax.server
# 解包新版到 ~/relax/app（旧目录留作 app.prev）
~/relax/app/relax migrate && ~/relax/app/relax doctor
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/dev.relax.server.plist
launchctl kickstart -k gui/$(id -u)/dev.relax.server
```

## 卸载后数据保留与彻底清除

`uninstall.sh` **只**停服务并删除 `~/Library/LaunchAgents/dev.relax.server.plist`，以下一律**保留**，以便重装后恢复：

- `~/relax/data`（SQLite 库）
- `~/relax/files`（上传文件）
- `~/relax/backups`（备份）
- `~/relax/deploy/certs`（CA 与服务端证书）
- `~/relax/config.local.json`（含 secret）

彻底清除 = 数据不可恢复，必须人工显式执行（脚本不代劳）：

```bash
sh deploy/launchd/uninstall.sh
rm -rf ~/relax   # 不可恢复：先确认已有可用备份并 verify-only 通过
```

## 关键判定

- `plutil -lint <plist>` 通过（install.sh 内置）
- `launchctl print gui/$(id -u)/dev.relax.server | grep -E 'state = running|pid ='` 有输出
- 日志：`~/relax/logs/relax.out.log`、`relax.err.log`
- 服务重启失败排查：plist 语法、路径、`bootstrap` 是否被误写成 `load`

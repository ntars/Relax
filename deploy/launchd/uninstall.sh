#!/bin/sh
# C6.4 launchd 卸载（只停服务 + 移除 Agent，不删数据）。
# 用法：sh deploy/launchd/uninstall.sh
# 数据保留：~/relax/data、files、backups、deploy/certs、config.local.json 全部保留，
# 以便重装后恢复。彻底清除需人工显式删除这些目录（见 README.md 卸载说明）。
set -eu

LABEL="dev.relax.server"
AGENT="$HOME/Library/LaunchAgents/${LABEL}.plist"

# 停服务（未装时忽略错误，保持幂等）。
if launchctl print "gui/$(id -u)/${LABEL}" >/dev/null 2>&1; then
  launchctl bootout "gui/$(id -u)/${LABEL}"
fi

# 只删 Agent plist，不碰 ~/relax 数据。
if [ -f "$AGENT" ]; then
  rm -f "$AGENT"
fi

echo "已停用 ${LABEL} 并移除 LaunchAgent；数据目录 ~/relax/ 保留。"
echo "如需彻底清除（不可恢复），按 README.md 卸载说明显式删除数据目录。"

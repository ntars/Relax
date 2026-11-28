#!/bin/sh
# C6.4 launchd 安装（用户级 Agent，不用 sudo）。
# 用法：sh deploy/launchd/install.sh
# 前置：~/relax/{data,logs,backups,deploy/certs} 已建、config.local.json 0600、
#       relax 二进制已解包到 ~/relax/app/relax、migrate + doctor 已全绿。
set -eu

LABEL="dev.relax.server"
AGENT="$HOME/Library/LaunchAgents/${LABEL}.plist"
SRC="$(cd "$(dirname "$0")" && pwd)/${LABEL}.plist"

# 幂等：已装则先 bootout，避免重复 bootstrap 报 37。
if launchctl print "gui/$(id -u)/${LABEL}" >/dev/null 2>&1; then
  launchctl bootout "gui/$(id -u)/${LABEL}"
fi

mkdir -p "$HOME/relax/logs" "$HOME/Library/LaunchAgents"
# plist 内 __HOME__ 占位替换为当前用户 home（不硬编码路径）。
sed "s|__HOME__|$HOME|g" "$SRC" > "$AGENT"
chmod 644 "$AGENT"

# 语法自检：plutil 通过才 bootstrap。
plutil -lint "$AGENT"

launchctl bootstrap "gui/$(id -u)" "$AGENT"
launchctl kickstart -k "gui/$(id -u)/${LABEL}"

# 判定：有 pid 即拉起成功。
if launchctl print "gui/$(id -u)/${LABEL}" 2>/dev/null | grep -qE 'state = running|pid = '; then
  echo "launchd 安装完成：${LABEL} 已运行；日志在 ~/relax/logs/"
else
  echo "launchd 已 bootstrap 但未确认运行，检查 ~/relax/logs/ 与 launchctl print" >&2
  exit 7
fi

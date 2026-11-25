#!/bin/bash
set -e

echo "Building APK for Relax..."

cd /Users/ntars/Documents/Github/Relax/src-tauri

# 构建 Android APK
cargo tauri build --target aarch64-linux-android

# 复制 APK 到 dist 目录
mkdir -p /Users/ntars/Documents/Github/Relax/dist
cp target/aarch64-linux-android/release/apk/*.apk /Users/ntars/Documents/Github/Relax/dist/

echo "APK built successfully!"

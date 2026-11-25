#!/bin/bash
set -e

echo "Deploying Relax..."

cd /Users/ntars/Documents/Github/Relax

# 构建服务端
cargo build --release

# 构建前端
cd ui
npm install
npm run build
cd ..

# 生成部署包
mkdir -p dist
cp target/release/relax-server dist/
cp ui/dist/* dist/

echo "Deploy package ready in dist/"

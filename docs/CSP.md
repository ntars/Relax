# Relax Content Security Policy (CSP)

## Policy Overview

Relax 使用严格的 CSP 防止 XSS 和数据泄露。前端（TypeScript）只能通过 Tauri 命令访问后端逻辑，**禁止直接网络请求、SQL 查询或加密操作**。

## CSP Header

```
Content-Security-Policy: 
  default-src 'self';
  script-src 'self' 'wasm-unsafe-eval';
  style-src 'self' 'unsafe-inline';
  img-src 'self' data: blob:;
  font-src 'self';
  connect-src 'none';
  frame-ancestors 'none';
  base-uri 'self';
  form-action 'self';
```

## 禁止的模式

前端代码中禁止以下模式：

1. **网络请求**: `fetch()`、`XMLHttpRequest`
2. **数据库**: `sql`、`sqlite`、`openDatabase`
3. **加密**: `crypto`、`subtle`、`WebCrypto`

## 允许的模式

前端仅允许通过 Tauri 命令访问后端。

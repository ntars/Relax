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
4. **外部资源**: 仅允许 `data:` 和 `blob:` URI

## 门禁实现

### Rust-side Validation

```rust
// crates/relax-server/src/middleware/csp.rs
pub fn enforce_csp(req: &Request) -> Result<(), ApiError> {
    if req.uri().path().starts_with("/api/") {
        return Err(ApiError::new(403, "CSP violation", "network access forbidden"));
    }
    Ok(())
}
```

### TypeScript-side Guard

```typescript
// ui/src/guards/csp.ts
export function assertCspCompliant() {
    if (typeof fetch !== 'undefined') {
        console.error('CSP violation: fetch() is forbidden');
    }
}
```

## 允许的模式

前端仅允许通过 Tauri 命令访问后端：

```typescript
// ui/src/services/api.ts
import { invoke } from '@tauri-apps/api/tao';

export async function login(username: string, password: string): Promise<AuthToken> {
    return await invoke('login', { username, password });
}
```

## 验证

运行 `cargo xtask check-ui` 验证前端无 CSP 违规。

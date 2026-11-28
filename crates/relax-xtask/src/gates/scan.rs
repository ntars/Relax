//! 门禁共用的源码文本工具：剥离字符串字面量，避免扫描器扫到自身的禁用 token。

/// 把 Rust/TS 源码里的字符串字面量内容替换为空格，保留结构性字符与换行。
/// 这样门禁扫描 `.unwrap()` / `todo!()` / `#[allow]` 时不会被字符串里的示例文本误伤。
pub(crate) fn strip_strings(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars = text.chars();
    let mut in_str = false;
    let mut in_char = false;
    let mut escaped = false;
    for c in chars {
        if in_str {
            if escaped {
                escaped = false;
                out.push(' ');
            } else if c == '\\' {
                escaped = true;
                out.push(' ');
            } else if c == '"' {
                in_str = false;
                out.push(' ');
            } else if c == '\n' {
                in_str = false;
                out.push('\n');
            } else {
                out.push(' ');
            }
            continue;
        }
        if in_char {
            if escaped {
                escaped = false;
                out.push(' ');
            } else if c == '\\' {
                escaped = true;
                out.push(' ');
            } else if c == '\'' {
                in_char = false;
                out.push(' ');
            } else if c == '\n' {
                in_char = false;
                out.push('\n');
            } else {
                out.push(' ');
            }
            continue;
        }
        if c == '"' {
            in_str = true;
            out.push(' ');
        } else if c == '\'' {
            in_char = true;
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

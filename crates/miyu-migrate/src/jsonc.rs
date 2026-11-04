//! 旧版的配置是 JSONC（细则第四节）：读之前去掉注释和末尾多的逗号，再照 JSON 读。

use serde_json::Value;

/// 读一段 JSONC；读不懂的是 `None`。
pub fn parse(text: &str) -> Option<Value> {
    serde_json::from_str(&strip_comments(text)).ok()
}

/// JSONC 去掉 `//`、`/* */` 注释和对象、数组末尾多的逗号（字符串里的不动）。
pub fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(ch) = chars.next() {
        if in_string {
            out.push(ch);
            if ch == '\\' {
                out.extend(chars.next());
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match (ch, chars.peek()) {
            ('"', _) => {
                in_string = true;
                out.push(ch);
            }
            ('/', Some('/')) => while chars.next_if(|next| *next != '\n').is_some() {},
            ('/', Some('*')) => {
                chars.next();
                let mut last = ' ';
                for next in chars.by_ref() {
                    if last == '*' && next == '/' {
                        break;
                    }
                    last = next;
                }
            }
            _ => out.push(ch),
        }
    }
    // 末尾多的逗号：`,` 后面跟着（隔着空白）`}` 或 `]` 的去掉。
    let mut cleaned = String::with_capacity(out.len());
    let bytes: Vec<char> = out.chars().collect();
    let mut in_string = false;
    for (n, ch) in bytes.iter().enumerate() {
        if in_string {
            if *ch == '"' && (n == 0 || bytes[n - 1] != '\\') {
                in_string = false;
            }
            cleaned.push(*ch);
            continue;
        }
        if *ch == '"' {
            in_string = true;
        }
        if *ch == ',' {
            let next = bytes[n + 1..].iter().find(|c| !c.is_whitespace());
            if matches!(next, Some('}' | ']')) {
                continue;
            }
        }
        cleaned.push(*ch);
    }
    cleaned
}

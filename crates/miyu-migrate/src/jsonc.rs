//! 旧版的配置是 JSONC（细则第四节）：读之前去掉注释和末尾多的逗号，再照 JSON 读。

use serde_json::Value;

/// 读一段 JSONC；读不懂的是 `None`。
pub fn parse(text: &str) -> Option<Value> {
    serde_json::from_str(&strip_comments(text)).ok()
}

/// JSONC 去掉 `//`、`/* */` 注释和对象、数组末尾多的逗号（字符串里的不动）。一遍走完：字符串里遇 `\\` 连下一个字一起照抄，
/// 引号状态不会被转义了的反斜杠带反；串外的 `,` 记下位置，之后（隔着空白、注释）头一个别的字是 `}` 或 `]` 的把它去掉。
pub fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    let mut comma: Option<usize> = None;
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
            _ if ch.is_whitespace() => out.push(ch),
            _ => {
                if let Some(at) = comma.take()
                    && matches!(ch, '}' | ']')
                {
                    out.remove(at);
                }
                if ch == ',' {
                    comma = Some(out.len());
                } else if ch == '"' {
                    in_string = true;
                }
                out.push(ch);
            }
        }
    }
    out
}

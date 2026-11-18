//! 供应商编号、键里人起的一段：照新版的写法换。

use std::collections::BTreeMap;

/// 新版供应商编号的写法（小写字母开头，小写字母、数字、`-`、`_`，最长 32 个字）：照不上的换成能用的。
///
/// 带非 ASCII 字的（「智谱」这样）去掉那些字、后面接旧编号 BLAKE3 的前 8 位十六进制（同技能目录名的写法）：同一个旧编号
/// 每次得到同一个新编号（重跑不换名），等长的中文编号不会都成了一个。只有 ASCII 的照原来的写法。
pub fn new_id(old: &str) -> String {
    let old = old.trim();
    if !old.is_ascii() {
        let hash = blake3::hash(old.as_bytes()).to_hex();
        let kept: String = ascii_id(&old.chars().filter(char::is_ascii).collect::<String>())
            .chars()
            .take(32 - "p-".len() - 9)
            .collect();
        let joined = if kept.is_empty() {
            hash[..8].to_string()
        } else {
            format!("{kept}-{}", &hash[..8])
        };
        return lead(joined);
    }
    lead(ascii_id(old)).chars().take(32).collect()
}

/// 每个字照新版编号能用的写：大写换小写，别的换成 `-`。
fn ascii_id(old: &str) -> String {
    old.chars()
        .map(|ch| {
            let ch = ch.to_ascii_lowercase();
            if ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

/// 不是小写字母开头的前面加 `p-`。
fn lead(mut id: String) -> String {
    if !id.starts_with(|ch: char| ch.is_ascii_lowercase()) {
        id.insert_str(0, "p-");
    }
    id
}

/// 别的旧编号已经换成了 `id` 的（`ids` 是旧编号 → 新编号），后面加 `-2`、`-3`……直到没人用（照旧版里的先后，重跑得到同一个）。
pub(super) fn distinct(id: String, old: &str, ids: &BTreeMap<String, String>) -> String {
    let taken = |one: &str| ids.iter().any(|(other, new)| other != old && new == one);
    if !taken(&id) {
        return id;
    }
    (2..)
        .map(|n| {
            let suffix = format!("-{n}");
            let kept: String = id.chars().take(32 - suffix.len()).collect();
            format!("{kept}{suffix}")
        })
        .find(|one| !taken(one))
        .unwrap_or(id)
}

/// 键里人起的一段：只有字母、数字、`-`、`_` 的照写，别的带双引号（照 TOML 转义）。
pub fn segment(name: &str) -> String {
    if !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        name.to_string()
    } else {
        let mut quoted = String::with_capacity(name.len() + 2);
        quoted.push('"');
        for ch in name.chars() {
            match ch {
                '"' => quoted.push_str("\\\""),
                '\\' => quoted.push_str("\\\\"),
                '\n' => quoted.push_str("\\n"),
                '\r' => quoted.push_str("\\r"),
                '\t' => quoted.push_str("\\t"),
                ch if ch < ' ' || ch == '\u{7f}' => {
                    quoted.push_str(&format!("\\u{:04X}", u32::from(ch)));
                }
                ch => quoted.push(ch),
            }
        }
        quoted.push('"');
        quoted
    }
}

//! 旧版的示范对话（`dialogs/<scope>.md`）照旧版自己的读法拆成一对一对（旧版 `crates/miyu-core/src/persona_hint.rs` 的
//! `parse_dialogs`）：行首 `user`、`assistant` 不分大小写，冒号全角半角都行；没有标记的行接在当前那一句后面（留着换行）；
//! 没配上回复的尾巴、两头去空白后是空的不要。新版的写法由核心照 `pairs` 写。

/// 拆成 `(user, assistant)` 一对一对。
pub fn parse(raw: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut question: Option<String> = None;
    let mut answer: Option<String> = None;
    let mut flush = |question: &mut Option<String>, answer: &mut Option<String>| {
        if let (Some(q), Some(a)) = (question.take(), answer.take()) {
            let (q, a) = (q.trim().to_string(), a.trim().to_string());
            if !q.is_empty() && !a.is_empty() {
                pairs.push((q, a));
            }
        }
        *question = None;
        *answer = None;
    };
    for line in raw.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = marker(trimmed, "user") {
            flush(&mut question, &mut answer);
            question = Some(rest.to_string());
        } else if let Some(rest) = marker(trimmed, "assistant") {
            if question.is_some() && answer.is_none() {
                answer = Some(rest.to_string());
            }
        } else if let Some(current) = answer.as_mut().or(question.as_mut()) {
            current.push('\n');
            current.push_str(line);
        }
    }
    flush(&mut question, &mut answer);
    pairs
}

/// 行首是 `role` 加冒号（全角半角都行）的，交回冒号后面去掉开头空白的。
fn marker<'a>(line: &'a str, role: &str) -> Option<&'a str> {
    let head = line.get(..role.len())?;
    if !head.eq_ignore_ascii_case(role) {
        return None;
    }
    let rest = &line[role.len()..];
    rest.strip_prefix(':')
        .or_else(|| rest.strip_prefix('：'))
        .map(str::trim_start)
}

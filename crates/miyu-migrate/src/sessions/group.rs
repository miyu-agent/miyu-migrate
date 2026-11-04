//! 旧版群的一轮怎么拆（细则第一节第 11 条、第十一节第 6 条）：`user_content` 是几个群友的话拼成的，照旧版
//! `real_context/targeting.rs` 的写法分节，每节一行标题；消息行是 `[时:分] <名字>(QQ:<号>) [msg=<编号>]: <正文>`（不显示号的
//! 群没有 `(QQ:…)`），名字、编号、正文都照 JSON 字符串转义过（换行写成 `\n`）；接在下面、缩进两格的是这条的附带：
//! `quoted earlier message [msg=<编号>] by <谁>: “<原文>”`、`@mentions: <谁>、<谁>`（她自己写成 `[you]`）。
//!
//! 只拆后三节（这一轮新到的、同一个人这一轮早些的、随后补充的）；`[Prior group chat records]` 是她已经看过的上文，逐条由 im 的
//! 消息记录导，不拆。她自己的 `[you]` 行不拆（她的话已经是回复）。

/// 拆的几节的标题。
const SECTIONS: [&str; 3] = [
    "[New messages received this turn]",
    "[Earlier messages from the same sender this turn, in chronological order]",
    "[Follow-up messages sent later by the same sender, in chronological order]",
];

/// 不拆的那一节。
const PRIOR: &str = "[Prior group chat records]";

/// 拆出来的一条群消息。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Line {
    /// 发的人的号；拆不出的是 `None`（写 `qq:unknown`）。
    pub qq: Option<String>,
    /// 名字。
    pub name: String,
    /// 平台的消息编号。
    pub msg: String,
    /// 正文。
    pub text: String,
    /// 引用的那一条的编号。
    pub reply_to: Option<String>,
    /// 引用的原文。
    pub quoted: Option<String>,
    /// @ 了谁（号）。
    pub mentions: Vec<String>,
    /// @ 了她。
    pub mentions_me: bool,
}

/// 拆一轮：交回后三节里的消息行，照出现的先后（「这一轮新到的」那一条在前）。
pub fn split(content: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut taking = false;
    for raw in content.lines() {
        let trimmed = raw.trim_end();
        if SECTIONS.contains(&trimmed) {
            taking = true;
            continue;
        }
        if trimmed == PRIOR {
            taking = false;
            continue;
        }
        if !taking {
            continue;
        }
        if let Some(rest) = raw.strip_prefix("  ") {
            if let Some(last) = lines.last_mut() {
                extra(last, rest.trim());
            }
            continue;
        }
        if let Some(line) = message(trimmed) {
            lines.push(line);
        }
    }
    lines.retain(|line: &Line| line.name != "[you]");
    lines
}

/// 一行消息：`[时:分] <发的人> [msg=<编号>]: <正文>`。不是这个样子的是 `None`。
fn message(text: &str) -> Option<Line> {
    let rest = text.strip_prefix('[')?;
    let (time, rest) = rest.split_once("] ")?;
    if time.len() != 5 || time.as_bytes()[2] != b':' {
        return None;
    }
    let marker = rest.find(" [msg=")?;
    let (sender, rest) = rest.split_at(marker);
    let rest = rest.strip_prefix(" [msg=")?;
    let (msg, body) = rest
        .split_once("]: ")
        .or_else(|| rest.strip_suffix("]:").map(|msg| (msg, "")))?;
    let (name, qq) = match sender
        .strip_suffix(')')
        .and_then(|head| head.rsplit_once("(QQ:"))
    {
        Some((name, qq)) if !qq.is_empty() && qq.bytes().all(|b| b.is_ascii_digit()) => {
            (name, Some(qq.to_string()))
        }
        _ => (sender, None),
    };
    Some(Line {
        qq,
        name: unescape(name),
        msg: unescape(msg),
        text: unescape(body),
        ..Line::default()
    })
}

/// 消息下面缩进的一行附带。
fn extra(line: &mut Line, text: &str) {
    if let Some(rest) = text.strip_prefix("quoted earlier message [msg=") {
        let Some((msg, rest)) = rest.split_once(']') else {
            return;
        };
        line.reply_to = Some(unescape(msg));
        if let Some(start) = rest.find(": \u{201c}") {
            let quoted = &rest[start + ": \u{201c}".len()..];
            line.quoted = Some(unescape(quoted.strip_suffix('\u{201d}').unwrap_or(quoted)));
        }
    } else if let Some(rest) = text.strip_prefix("reply-to: msg=") {
        line.reply_to = Some(unescape(rest));
    } else if let Some(rest) = text.strip_prefix("@mentions: ") {
        for who in rest.split('、') {
            if who == "[you]" {
                line.mentions_me = true;
            } else if let Some(qq) = number(who) {
                line.mentions.push(qq);
            }
        }
    }
}

/// `名字(QQ:<号>)`、`QQ:<号>` 里的号。
fn number(who: &str) -> Option<String> {
    let qq = who
        .strip_suffix(')')
        .and_then(|head| head.rsplit_once("(QQ:"))
        .map(|(_, qq)| qq)
        .or_else(|| who.strip_prefix("QQ:"))?;
    (!qq.is_empty() && qq.bytes().all(|b| b.is_ascii_digit())).then(|| qq.to_string())
}

/// 旧版照 JSON 字符串转义过的一格（去了两头的引号）：照 JSON 读回来；读不懂的照原样。
fn unescape(text: &str) -> String {
    serde_json::from_str::<String>(&format!("\"{text}\"")).unwrap_or_else(|_| text.to_string())
}

/// 被挤出的轮里排着并进来的话（`followup:`、`before:`）：正文后面一段是
/// `Trusted metadata for this QQ follow-up message: sender QQ=<号>; message ID=<编号>[; replied-to message ID=…][; …]`。
/// 交回（正文，号，编号，引用的编号）；没有这一段的整段当正文。
pub fn followup(content: &str) -> (String, Option<String>, Option<String>, Option<String>) {
    const MARK: &str = "\n\nTrusted metadata for this QQ follow-up message: ";
    let Some(at) = content.rfind(MARK) else {
        return (content.to_string(), None, None, None);
    };
    let (text, meta) = (&content[..at], &content[at + MARK.len()..]);
    let field = |name: &str| {
        meta.split("; ")
            .find_map(|part| part.strip_prefix(name))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    };
    (
        text.to_string(),
        field("sender QQ="),
        field("message ID="),
        field("replied-to message ID="),
    )
}

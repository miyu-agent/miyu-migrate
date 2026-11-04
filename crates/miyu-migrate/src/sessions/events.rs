//! 一个旧会话拼成新版的事件（细则第一节第 7、10、11 条；外壳照核心 `kernel/events.md`）。这里只拼 JSON，读一遍、写成行交给
//! 核心的 crate（[`super::log`]），账本规矩由核心载入时再查一遍。
//!
//! 一轮：`message.user`（人说的字、附件）→ `turn.started` → 工具流每一轮一条 `message.assistant`，每个调用一条 `tool.result`
//! → 最后的回复 → `turn.ended`。

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::{Value, json};

use super::ids;
use super::old::{Session, Turn};

/// 旧版运行中那一轮的占位正文：不是她说的。
const PENDING: &str = "<system-reminder>上一轮prompt正在由另一轮回复处理中，你只需要回应用户当前的prompt，不要处理上一轮的prompt</system-reminder>";
/// 旧版给中断的轮末尾加的提示：给模型看的，不是她说的。
const INTERRUPTED: &str = "<system-reminder>上一轮prompt已被中断，除非用户重新要求否则不要处理上一轮的prompt</system-reminder>";

/// 会话头上的几样。
#[derive(Debug, Clone)]
pub struct Head<'a> {
    /// 场所，本机的是 `local`。
    pub venue: &'a str,
    /// 快照的哈希（`sha256:…`）。
    pub policy: &'a str,
    /// 新人格编号。
    pub persona: Option<&'a str>,
}

/// 一个附件放进 blob 库以后的样子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Media {
    /// `sha256:…`。
    pub blob: String,
    /// 文件名。
    pub name: String,
    /// 媒体类型。
    pub media_type: String,
    /// 图片的宽高；不是图片、认不出大小的是 `None`（写成文件块）。
    pub size: Option<(u64, u64)>,
}

/// 拼好的一个会话。
#[derive(Debug, Clone, Default)]
pub struct Built {
    /// 事件，序号从 1 起。
    pub events: Vec<Value>,
    /// 导了几轮。
    pub turns: u64,
    /// 跳过的摘要行。
    pub summaries: u64,
    /// 附件几个。
    pub attachments: u64,
    /// 没有工具流、只有展示转录的轮（工具细节留给后一步）。
    pub replay_only: u64,
}

#[derive(Deserialize)]
struct Round {
    #[serde(default)]
    assistant_content: String,
    #[serde(default)]
    assistant_reasoning: Option<String>,
    #[serde(default)]
    calls: Vec<Call>,
}

#[derive(Deserialize)]
struct Call {
    #[serde(default)]
    name: String,
    #[serde(default)]
    arguments: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    started_ms: Option<u64>,
    #[serde(default)]
    finished_ms: Option<u64>,
}

/// 拼事件用的：照先后编序号。
struct Log {
    events: Vec<Value>,
}

impl Log {
    /// 下一条的序号。
    fn next(&self) -> u64 {
        self.events.len() as u64 + 1
    }

    /// 加一条，交回它的序号。
    fn push(
        &mut self,
        (kind, at): (&str, &str),
        turn: Option<u64>,
        by: Value,
        cause: Option<&str>,
        body: Value,
    ) -> u64 {
        let seq = self.next();
        let mut event = json!({"seq": seq, "at": at, "kind": kind, "by": by, "body": body});
        if let Some(turn) = turn {
            event["turn"] = json!(turn);
        }
        if let Some(cause) = cause {
            event["cause"] = json!(cause);
        }
        self.events.push(event);
        seq
    }
}

/// 拼一个会话：`turns` 照 `seq` 排好，`media` 是每一轮的附件（照旧编号）。时刻读不懂的照前一条的。
pub fn build(
    head: &Head<'_>,
    session: &Session,
    turns: &[Turn],
    media: &HashMap<String, Vec<Media>>,
) -> Built {
    let mut log = Log { events: Vec::new() };
    let mut built = Built::default();
    let created = ids::parse(&session.created_at).unwrap_or(jiff::Timestamp::UNIX_EPOCH);
    let mut last = ids::at(created);
    let person = json!({"kind": "person"});
    let mut body = json!({
        "venue": head.venue,
        "policy": head.policy,
        "permission": {"level": "workspace", "read_only": false},
        "titles": false,
    });
    if let Some(persona) = head.persona {
        body["persona"] = json!(persona);
    }
    log.push(("session.created", &last), None, person.clone(), None, body);
    let title: String = session.name.trim().chars().take(200).collect();
    if !title.is_empty() {
        log.push(
            ("session.meta_changed", &last),
            None,
            person.clone(),
            None,
            json!({"title": title}),
        );
    }
    for turn in turns {
        if turn.summary {
            built.summaries += 1;
            continue;
        }
        let said = ids::parse(&turn.user_timestamp).map_or_else(|| last.clone(), ids::at);
        last.clone_from(&said);
        let cause = format!("legacy-{}", turn.id);
        let cause = (cause.len() <= 128).then_some(cause.as_str());
        let text = if turn.display_content.trim().is_empty() {
            &turn.user_content
        } else {
            &turn.display_content
        };
        let mut blocks = Vec::new();
        if !text.is_empty() {
            blocks.push(json!({"type": "text", "text": text}));
        }
        for one in media.get(&turn.id).into_iter().flatten() {
            blocks.push(block(one));
            built.attachments += 1;
        }
        if blocks.is_empty() {
            blocks.push(json!({"type": "text", "text": ""}));
        }
        let trigger = log.push(
            ("message.user", &said),
            None,
            person.clone(),
            cause,
            json!({"blocks": blocks}),
        );
        // `turn.started` 的 `turn` 是它自己的序号。
        let started = log.next();
        log.push(
            ("turn.started", &said),
            Some(started),
            json!({"kind": "kernel"}),
            cause,
            json!({"trigger": trigger}),
        );
        let started_at = said.clone();
        let model = json!({
            "kind": "model",
            "endpoint": nonempty(turn.provider.as_deref()),
            "model": nonempty(turn.model.as_deref()),
        });
        let rounds: Vec<Round> = turn
            .tool_flow
            .as_deref()
            .and_then(|flow| serde_json::from_str(flow).ok())
            .unwrap_or_default();
        if rounds.is_empty()
            && turn
                .replay_journal
                .as_deref()
                .is_some_and(|j| j.contains("tool_call"))
        {
            built.replay_only += 1;
        }
        let failed = failures(turn.replay_journal.as_deref());
        let mut seen_names: HashMap<String, usize> = HashMap::new();
        for round in &rounds {
            let seq = log.next();
            let mut blocks = Vec::new();
            if let Some(reasoning) = round
                .assistant_reasoning
                .as_deref()
                .filter(|r| !r.is_empty())
            {
                blocks.push(json!({"type": "reasoning", "text": reasoning}));
            }
            if !round.assistant_content.is_empty() {
                blocks.push(json!({"type": "text", "text": round.assistant_content}));
            }
            let calls: Vec<String> = (1..=round.calls.len())
                .map(|k| format!("call_{seq}_{k}"))
                .collect();
            for (call, id) in round.calls.iter().zip(&calls) {
                blocks.push(json!({"type": "tool_call", "call_id": id, "name": call.name, "args": call.arguments}));
            }
            if blocks.is_empty() {
                continue;
            }
            log.push(
                ("message.assistant", &started_at),
                Some(started),
                model.clone(),
                None,
                json!({"blocks": blocks, "seen": seq - 1}),
            );
            for (call, id) in round.calls.iter().zip(&calls) {
                let nth = seen_names.entry(call.name.clone()).or_default();
                let status = if failed
                    .get(&call.name)
                    .is_some_and(|list| list.get(*nth) == Some(&false))
                {
                    "error"
                } else {
                    "ok"
                };
                *nth += 1;
                let at = call
                    .finished_ms
                    .and_then(ids::at_millis)
                    .unwrap_or_else(|| started_at.clone());
                let mut body = json!({
                    "call_id": id,
                    "status": status,
                    "blocks": [{"type": "text", "text": call.output}],
                });
                if let (Some(start), Some(end)) = (call.started_ms, call.finished_ms) {
                    body["duration_ms"] = json!(end.saturating_sub(start));
                }
                log.push(
                    ("tool.result", &at),
                    Some(started),
                    json!({"kind": "tool", "call_id": id}),
                    None,
                    body,
                );
            }
        }
        let replied = turn
            .assistant_timestamp
            .as_deref()
            .and_then(ids::parse)
            .map_or_else(|| started_at.clone(), ids::at);
        let content = reply(&turn.assistant_content);
        let last_round = rounds.last().map(|round| round.assistant_content.as_str());
        let mut blocks = Vec::new();
        if let Some(reasoning) = turn
            .assistant_reasoning
            .as_deref()
            .filter(|r| !r.is_empty())
            && rounds
                .last()
                .and_then(|round| round.assistant_reasoning.as_deref())
                != Some(reasoning)
        {
            blocks.push(json!({"type": "reasoning", "text": reasoning}));
        }
        if !content.is_empty() && last_round != Some(content) {
            blocks.push(json!({"type": "text", "text": content}));
        }
        if !blocks.is_empty() {
            let seq = log.next();
            log.push(
                ("message.assistant", &replied),
                Some(started),
                model.clone(),
                None,
                json!({"blocks": blocks, "seen": seq - 1}),
            );
        }
        let reason = if turn.status == "completed" {
            "completed"
        } else {
            "interrupted"
        };
        log.push(
            ("turn.ended", &replied),
            Some(started),
            json!({"kind": "kernel"}),
            None,
            json!({"reason": reason}),
        );
        last = replied;
        built.turns += 1;
    }
    built.events = log.events;
    built
}

/// 一个附件的块：认出大小的图片写图片块，别的写文件块。
fn block(media: &Media) -> Value {
    match media.size {
        Some((width, height)) => json!({
            "type": "image",
            "blob": media.blob,
            "name": media.name,
            "media_type": media.media_type,
            "width": width,
            "height": height,
        }),
        None => json!({
            "type": "file",
            "blob": media.blob,
            "name": media.name,
            "media_type": media.media_type,
        }),
    }
}

/// 最后的回复：运行中的占位当没有，中断时加的提示去掉。
fn reply(content: &str) -> &str {
    if content == PENDING {
        return "";
    }
    content
        .strip_suffix(INTERRUPTED)
        .map_or(content, str::trim_end)
}

/// 供应商、模型：空的写 `legacy`。
fn nonempty(text: Option<&str>) -> &str {
    text.map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or("legacy")
}

/// 展示转录里每个工具名依次成没成（`tool_result` 的 `ok`）。读不懂的是空的。
fn failures(journal: Option<&str>) -> HashMap<String, Vec<bool>> {
    let mut found: HashMap<String, Vec<bool>> = HashMap::new();
    let entries: Vec<Value> = journal
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or_default();
    for entry in entries {
        if entry["kind"] == "tool_result"
            && let Some(name) = entry["name"].as_str()
        {
            found
                .entry(name.to_string())
                .or_default()
                .push(entry["ok"].as_bool().unwrap_or(true));
        }
    }
    found
}

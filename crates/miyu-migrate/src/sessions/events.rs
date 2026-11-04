//! 一个旧会话拼成新版的事件（细则第一节第 7、10、11 条；外壳照核心 `kernel/events.md`）。这里只拼 JSON，读一遍、写成行交给
//! 核心的 crate（[`super::log`]），账本规矩由核心载入时再查一遍。
//!
//! 一轮：`message.user`（人说的字、附件）→ `turn.started` → 工具流每一轮一条 `message.assistant`，每个调用一条 `tool.result`
//! → 最后的回复 → `turn.ended`。

use std::collections::{HashMap, VecDeque};

use serde_json::{Value, json};

use super::ids;
use super::old::{Session, Turn};
use super::rounds::{self, Round};
use super::said::{Who, said};

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
    /// 说话的是谁（本机的人、私聊的对方、群）。
    pub who: Who<'a>,
    /// 子会话的父会话（新编号）和深度。
    pub parent: Option<(&'a str, u64)>,
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

/// 一个会话里接在别处的块：人说的话后面的附件、工具结果后面的图和文件、提问。都照旧编号。
#[derive(Debug, Clone, Default)]
pub struct Attached {
    /// 人说的话的附件：turn_id → 几个。
    pub user: HashMap<String, Vec<Media>>,
    /// 工具结果后面接的块（图、文件、没迁过来的那一句）：(turn_id, 调用的旧编号) → 几块。
    pub tools: HashMap<(String, String), Vec<Value>>,
    /// 提问：turn_id → 每一次的 JSON。
    pub questions: HashMap<String, Vec<Value>>,
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
    /// 拼回来的被挤出的轮。
    pub evicted: u64,
    /// 群里拆不出号、写成 `qq:unknown` 的话。
    pub unknown: u64,
    /// 工具结果后面接的块（图、文件）。
    pub tool_blocks: u64,
    /// 提问。
    pub questions: u64,
    /// 工具的附带对不上调用、没接上的（拼的人数，[`super::import`] 填）。
    pub unplaced: u64,
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

/// 拼一个会话：`turns` 照先后排好，`attached` 是接在别处的块（照旧编号）。时刻读不懂的照前一条的。
pub fn build(head: &Head<'_>, session: &Session, turns: &[Turn], attached: &Attached) -> Built {
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
    if matches!(head.who, Who::Group(_)) {
        body["confined"] = json!(true);
    }
    if let Some((parent, depth)) = head.parent {
        body["parent"] = json!(parent);
        body["depth"] = json!(depth);
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
        let said_at = ids::parse(&turn.user_timestamp).map_or_else(|| last.clone(), ids::at);
        last.clone_from(&said_at);
        let cause = format!("legacy-{}", turn.id);
        let cause = (cause.len() <= 128).then_some(cause.as_str());
        let (said_all, attached_now) = said(
            head.venue,
            head.who,
            turn,
            attached.user.get(&turn.id).map_or(&[][..], Vec::as_slice),
        );
        built.attachments += attached_now;
        built.unknown += said_all.iter().filter(|one| one.unknown).count() as u64;
        built.evicted += u64::from(turn.evicted);
        let mut said_all = said_all.into_iter();
        let Some(first) = said_all.next() else {
            continue;
        };
        let trigger = log.push(
            ("message.user", &said_at),
            None,
            first.by,
            cause,
            first.body,
        );
        // `turn.started` 的 `turn` 是它自己的序号。
        let started = log.next();
        log.push(
            ("turn.started", &said_at),
            Some(started),
            json!({"kind": "kernel"}),
            cause,
            json!({"trigger": trigger}),
        );
        // 同一轮里别的话（群里别人说的、排着并进来的）：带这一轮的编号，排在开轮后面。
        for more in said_all {
            log.push(
                ("message.user", &said_at),
                Some(started),
                more.by,
                None,
                more.body,
            );
        }
        let started_at = said_at.clone();
        let model = json!({
            "kind": "model",
            "endpoint": nonempty(turn.provider.as_deref()),
            "model": nonempty(turn.model.as_deref()),
        });
        let mut rounds: Vec<Round> = rounds::from_flow(turn.tool_flow.as_deref());
        if rounds.is_empty()
            && turn
                .replay_journal
                .as_deref()
                .is_some_and(|j| j.contains("tool_call"))
        {
            rounds = rounds::from_replay(turn.replay_journal.as_deref());
            built.replay_only += 1;
        }
        // 最后一次来回说的、想的：最后的回复和它一样的不再写一遍。提问接在后面，不算。
        let last_said = rounds.last().map(|round| {
            (
                round.assistant_content.clone(),
                round.assistant_reasoning.clone(),
            )
        });
        for exchange in attached.questions.get(&turn.id).into_iter().flatten() {
            rounds.push(rounds::question(exchange));
            built.questions += 1;
        }
        let failed = failures(turn.replay_journal.as_deref());
        let mut seen_names: HashMap<String, usize> = HashMap::new();
        let mut bridged = bridged(attached, &turn.id);
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
                // 成没成照展示转录里同名的第几次（工具流里没有这一格；只有转录的轮也照它）。
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
                let mut blocks = vec![json!({"type": "text", "text": call.output})];
                let direct = attached.tools.get(&(turn.id.clone(), call.id.clone()));
                let more = direct.cloned().or_else(|| {
                    bridged
                        .get_mut(&call.name)
                        .and_then(|queue| queue.pop_front())
                });
                if let Some(more) = more {
                    built.tool_blocks += more.len() as u64;
                    blocks.extend(more);
                }
                let mut body = json!({"call_id": id, "status": status, "blocks": blocks});
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
        let (last_text, last_thought) = match &last_said {
            Some((text, thought)) => (Some(text.as_str()), thought.as_deref()),
            None => (None, None),
        };
        let mut blocks = Vec::new();
        if let Some(reasoning) = turn
            .assistant_reasoning
            .as_deref()
            .filter(|r| !r.is_empty())
            && last_thought != Some(reasoning)
        {
            blocks.push(json!({"type": "reasoning", "text": reasoning}));
        }
        if !content.is_empty() && last_text != Some(content) {
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

/// 旧版接入 QQ 的工具（表情包这些）出的图，`tool_id` 是 `bridge_<工具名>_<这一次调用里第几张>`，对不上调用的编号：照工具名排成
/// 队，一张一份，这一轮里同名的调用照先后一次领一份。一次调用出了好几张的（第 2 张起）拼不回是哪一次的，照先后接着领；主人那份
/// 15 张都是第 1 张。
fn bridged(attached: &Attached, turn: &str) -> HashMap<String, VecDeque<Vec<Value>>> {
    let mut found: Vec<(&str, &str, &Vec<Value>)> = attached
        .tools
        .iter()
        .filter(|((owner, _), _)| owner == turn)
        .filter_map(|((_, call), blocks)| {
            let rest = call.strip_prefix("bridge_")?;
            let (name, nth) = rest.rsplit_once('_')?;
            nth.bytes()
                .all(|b| b.is_ascii_digit())
                .then_some((name, nth, blocks))
        })
        .collect();
    found.sort_by(|left, right| (left.0, left.1).cmp(&(right.0, right.1)));
    let mut queues: HashMap<String, VecDeque<Vec<Value>>> = HashMap::new();
    for (name, _, blocks) in found {
        // 同一个编号下的几张（每次调用都是第 1 张）照出的先后。
        let queue = queues.entry(name.to_string()).or_default();
        queue.extend(blocks.iter().map(|block| vec![block.clone()]));
    }
    queues
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

//! 会话（细则第一节，施工 MG-3）：旧会话库里管理员的会话一个一个打成原始包、经 `session.import {archive: true}` 导成只读存档。
//! 本机会话（MG-3 上）、QQ 场所会话（MG-3 中：私聊照原样，群的一轮照每行开头的号拆）；被挤出上下文的轮拼回原来的会话
//! （[`evicted`]），原来的会话不在了的照人格归进一个兜底的本机存档。子代理的留给下一步，一次性的 `ask`、成员的不迁、别的平台的
//! 整会话留在压缩包里，都在报告里数出来。
//!
//! 新编号由旧编号算出来（[`ids::session_id`]），重跑时导入回 `present`，不重复。

mod events;
pub mod evicted;
pub mod group;
pub mod ids;
mod import;
mod old;
mod said;
mod snapshot;
mod stage;

use std::collections::HashMap;
use std::path::Path;

use miyu_client::places::DataRoot;

use crate::core::{Core, Said};
use crate::layout::Layout;
use crate::personas::Mapped;
use crate::report::{Item, Skip, Status};

pub use events::{Built, Head, Media, build};
pub use import::{Job, Owned};
pub use old::{Attachment, Binding, Db, NEWEST, Session, Turn};
pub use said::Who;
pub use snapshot::{Snapshot, borrow};
pub use stage::{Stage, StageError};

/// 旧格式一句话（写进给 agent 的报告）。
const FORMAT: &str = "SQLite（user_version 1-40）：sessions 一行一个会话（kind user/ask/voice/subagent，owner 非空是成员的），turns 一行一轮（user_content/display_content、assistant_content/assistant_reasoning、tool_flow JSON、replay_journal JSON、is_summary 摘要行），user_attachments 附件（data 列或 state/attachments/<编号>/<文件名>），platform_session_bindings 绑场所";

/// 迁了多少。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Counted {
    /// 导进去（新的、早就在的）几个会话。
    pub sessions: u64,
    /// 其中早就在的（重跑）。
    pub present: u64,
    /// 几轮。
    pub turns: u64,
    /// 跳过的摘要行。
    pub summaries: u64,
    /// 附件。
    pub attachments: u64,
    /// 只有展示转录、工具细节没导的轮。
    pub replay_only: u64,
    /// 拼回来的被挤出的轮。
    pub evicted: u64,
    /// 群里拆不出号的话。
    pub unknown: u64,
}

/// 迁会话：读 `copy`（解出来的副本）的会话库，人格照 `mapped`，暂存在 `stage`，导进 `core`（数据根 `root`：借快照时读它的
/// blob 库）。交回报告的一项。
///
/// # Errors
///
/// 连接断了（别的核心拒绝只挡那一个会话，写进报告）。
pub fn migrate(
    core: &mut dyn Core,
    root: &DataRoot,
    copy: &Layout,
    mapped: &[Mapped],
    stage: &Path,
) -> Result<Item, Said> {
    let path = copy.conversation_db();
    let source = path
        .strip_prefix(&copy.root)
        .unwrap_or(&path)
        .to_string_lossy()
        .replace('\\', "/");
    let mut item = Item {
        what: "sessions".to_string(),
        status: Status::Migrated,
        source: source.clone(),
        format: FORMAT.to_string(),
        migrated: 0,
        skipped: Vec::new(),
        target: Some("session.import (archive)".to_string()),
        note: None,
    };
    if !path.is_file() {
        item.status = Status::Skipped;
        item.note = Some("旧根里没有会话库。".to_string());
        return Ok(item);
    }
    let db = match Db::open(&path).and_then(|db| Ok((db.sessions()?, db.bindings()?, db))) {
        Ok((sessions, bindings, db)) if db.version <= NEWEST => (sessions, bindings, db),
        Ok((_, _, db)) => {
            return Ok(archived(
                item,
                format!(
                    "会话库的版本 {} 比认得的 {NEWEST} 新，整库留在压缩包里。",
                    db.version
                ),
            ));
        }
        Err(error) => return Ok(archived(item, format!("会话库读不了：{error}"))),
    };
    let (sessions, bindings, db) = db;
    let stage = match Stage::new(stage) {
        Ok(stage) => stage,
        Err(error) => return Ok(archived(item, format!("暂存目录建不了：{error}"))),
    };
    let (jobs, unreadable) = plan(
        copy,
        &source,
        &db,
        (&sessions, &bindings),
        &mut item.skipped,
    );
    let personas: HashMap<&str, Option<&str>> = mapped
        .iter()
        .map(|row| (row.old.as_str(), row.new.as_deref()))
        .collect();
    let mut snapshots: HashMap<Option<String>, Snapshot> = HashMap::new();
    let mut counted = Counted::default();
    let mut unmapped = 0u64;
    for job in &jobs {
        let scope = if job.session.persona.is_empty() {
            "default"
        } else {
            job.session.persona.as_str()
        };
        let persona = personas.get(scope).copied().flatten().map(str::to_string);
        if persona.is_none() {
            unmapped += 1;
        }
        let snapshot = match snapshots.get(&persona) {
            Some(snapshot) => snapshot.clone(),
            None => {
                let borrowed = borrow(core, root, persona.as_deref())?;
                snapshots.insert(persona.clone(), borrowed.clone());
                borrowed
            }
        };
        match import::one(core, copy, job, (&stage, &snapshot, persona.as_deref()))? {
            Ok((present, built)) => {
                counted.sessions += 1;
                counted.present += u64::from(present);
                counted.turns += built.turns;
                counted.summaries += built.summaries;
                counted.attachments += built.attachments;
                counted.replay_only += built.replay_only;
                counted.evicted += built.evicted;
                counted.unknown += built.unknown;
            }
            Err(why) => item.skipped.push(skip(&job.session.id, &why)),
        }
    }
    item.migrated = counted.sessions;
    if !item.skipped.is_empty() || !unreadable.is_empty() {
        item.status = Status::Partial;
    }
    let mut text = note(&counted, unmapped);
    if !unreadable.is_empty() {
        text.push_str(&format!(
            "被挤出上下文的库有 {} 个读不了（{}），留在压缩包里。",
            unreadable.len(),
            unreadable.join("、")
        ));
    }
    item.note = Some(text);
    Ok(item)
}

/// 排出要导的会话：会话库里该导的，被挤出的轮拼回去；原来的会话不在了的（或者是空的 `origin_session_id`），照人格归进一个兜底
/// 的本机存档。不导的记进 `skipped`。交回要导的，和读不了的被挤出的库（scope）。
fn plan(
    copy: &Layout,
    source: &str,
    db: &Db,
    (sessions, bindings): (&[Session], &HashMap<String, Binding>),
    skipped: &mut Vec<Skip>,
) -> (Vec<Job>, Vec<String>) {
    let personas = copy.state().join("personas");
    let (found, unreadable) = evicted::read_all(&personas);
    let mut evicted_of: HashMap<String, Vec<Turn>> = HashMap::new();
    let mut leftover: Vec<(String, Vec<Turn>)> = Vec::new();
    let known: HashMap<&str, &Session> =
        sessions.iter().map(|one| (one.id.as_str(), one)).collect();
    for (scope, by_origin) in found {
        let mut orphans = Vec::new();
        for (origin, turns) in by_origin {
            if known.contains_key(origin.as_str()) {
                evicted_of.entry(origin).or_default().extend(turns);
            } else {
                orphans.extend(turns);
            }
        }
        if !orphans.is_empty() {
            leftover.push((scope, orphans));
        }
    }
    let mut jobs = Vec::new();
    for session in sessions {
        let why = if !session.owner.is_empty() {
            Some("member".to_string())
        } else if session.kind == "ask" {
            Some("oneshot".to_string())
        } else if session.kind == "subagent" {
            Some("subagent-later".to_string())
        } else {
            match bindings.get(&session.id) {
                Some(binding) if binding.platform != "onebot" => {
                    Some(format!("platform-{}", binding.platform))
                }
                _ => None,
            }
        };
        if let Some(why) = why {
            skipped.push(skip(&session.id, &why));
            continue;
        }
        let (venue, who) = match bindings.get(&session.id) {
            Some(binding) => (
                format!(
                    "qq:{}:{}:{}",
                    binding.account, binding.kind, binding.conversation
                ),
                if binding.kind == "group" {
                    Owned::Group(binding.account.clone())
                } else {
                    Owned::Private(binding.conversation.clone())
                },
            ),
            None => ("local".to_string(), Owned::Person),
        };
        let (Ok(turns), Ok(attachments)) = (db.turns(&session.id), db.attachments(&session.id))
        else {
            skipped.push(skip(&session.id, "unreadable"));
            continue;
        };
        let turns = evicted::merge(turns, evicted_of.remove(&session.id).unwrap_or_default());
        jobs.push(Job {
            session: session.clone(),
            turns,
            attachments,
            venue,
            who,
            source: source.to_string(),
        });
    }
    for (scope, turns) in leftover {
        let first = turns
            .first()
            .map(|turn| turn.user_timestamp.clone())
            .unwrap_or_default();
        let last = turns
            .last()
            .map(|turn| turn.user_timestamp.clone())
            .unwrap_or_default();
        jobs.push(Job {
            session: Session {
                id: format!("evicted:{scope}"),
                persona: scope.clone(),
                name: "旧版被挤出的对话".to_string(),
                kind: "user".to_string(),
                owner: String::new(),
                parent: None,
                created_at: first,
                updated_at: last,
            },
            turns,
            attachments: Vec::new(),
            venue: "local".to_string(),
            who: Owned::Person,
            source: format!("state/personas/{scope}/memory/evicted_context.db"),
        });
    }
    (jobs, unreadable)
}

fn skip(key: &str, why: &str) -> Skip {
    Skip {
        key: key.to_string(),
        why: why.to_string(),
    }
}

fn archived(mut item: Item, note: String) -> Item {
    item.status = Status::Archived;
    item.note = Some(note);
    item
}

/// 给人看的一句：几轮、跳过的摘要行、附件；人格映射不上的；只有展示转录的轮。
fn note(counted: &Counted, unmapped: u64) -> String {
    let mut note = format!(
        "{} 轮，{} 个附件；摘要行 {} 条不导（是派生的）。",
        counted.turns, counted.attachments, counted.summaries
    );
    if counted.present > 0 {
        note.push_str(&format!("其中 {} 个会话以前就导过。", counted.present));
    }
    if unmapped > 0 {
        note.push_str(&format!(
            "{unmapped} 个会话的人格没映射上，导成了无人格的存档。"
        ));
    }
    if counted.replay_only > 0 {
        note.push_str(&format!(
            "{} 轮只有展示转录，工具细节这一步没导。",
            counted.replay_only
        ));
    }
    if counted.evicted > 0 {
        note.push_str(&format!(
            "被挤出上下文的 {} 轮拼回了原来的会话。",
            counted.evicted
        ));
    }
    if counted.unknown > 0 {
        note.push_str(&format!(
            "群里 {} 条话拆不出是谁说的，记成 qq:unknown。",
            counted.unknown
        ));
    }
    note.push_str("子代理的会话下一步再迁。");
    note
}

#[cfg(test)]
mod tests;

//! 会话（细则第一节，施工 MG-3）：旧会话库里管理员的会话一个一个打成原始包、经 `session.import {archive: true}` 导成只读存档。
//! 这一步（MG-3 上）只迁本机会话：绑了场所的（QQ）、子代理的留给后两步，一次性的 `ask`、成员的不迁，都在报告里数出来。
//!
//! 新编号由旧编号算出来（[`ids::session_id`]），重跑时导入回 `present`，不重复。

mod events;
pub mod ids;
mod old;
mod snapshot;
mod stage;

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde_json::json;

use miyu_client::places::DataRoot;

use crate::core::{Core, Said};
use crate::layout::Layout;
use crate::personas::Mapped;
use crate::report::{Item, Skip, Status};

pub use events::{Built, Head, Media, build};
pub use old::{Attachment, Db, NEWEST, Session, Turn};
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
    let db = match Db::open(&path).and_then(|db| Ok((db.sessions()?, db.bound()?, db))) {
        Ok((sessions, bound, db)) if db.version <= NEWEST => (sessions, bound, db),
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
    let (sessions, bound, db) = db;
    let stage = match Stage::new(stage) {
        Ok(stage) => stage,
        Err(error) => return Ok(archived(item, format!("暂存目录建不了：{error}"))),
    };
    let personas: HashMap<&str, Option<&str>> = mapped
        .iter()
        .map(|row| (row.old.as_str(), row.new.as_deref()))
        .collect();
    let mut snapshots: HashMap<Option<String>, Snapshot> = HashMap::new();
    let mut counted = Counted::default();
    let mut unmapped = 0u64;
    for session in &sessions {
        let why = if !session.owner.is_empty() {
            Some("member")
        } else if session.kind == "ask" {
            Some("oneshot")
        } else if session.kind == "subagent" {
            Some("subagent-later")
        } else if bound.contains(&session.id) {
            Some("venue-later")
        } else {
            None
        };
        if let Some(why) = why {
            item.skipped.push(skip(&session.id, why));
            continue;
        }
        let scope = if session.persona.is_empty() {
            "default"
        } else {
            session.persona.as_str()
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
        match one(
            core,
            &db,
            (copy, &source),
            session,
            (&stage, &snapshot, persona.as_deref()),
        )? {
            Ok((present, built)) => {
                counted.sessions += 1;
                counted.present += u64::from(present);
                counted.turns += built.turns;
                counted.summaries += built.summaries;
                counted.attachments += built.attachments;
                counted.replay_only += built.replay_only;
            }
            Err(why) => item.skipped.push(skip(&session.id, &why)),
        }
    }
    item.migrated = counted.sessions;
    if !item.skipped.is_empty() {
        item.status = Status::Partial;
    }
    item.note = Some(note(&counted, unmapped));
    Ok(item)
}

/// 一个会话：拼、打包、导入。交回（早就在的吗、拼了什么）；这一个迁不成的交回为什么（一个短词，不带内容），连接断了的交回
/// [`Said`]。
fn one(
    core: &mut dyn Core,
    db: &Db,
    (copy, source): (&Layout, &str),
    session: &Session,
    (stage, snapshot, persona): (&Stage, &Snapshot, Option<&str>),
) -> Result<Result<(bool, Built), String>, Said> {
    let Ok(turns) = db.turns(&session.id) else {
        return Ok(Err("unreadable".to_string()));
    };
    let Ok(attachments) = db.attachments(&session.id) else {
        return Ok(Err("unreadable".to_string()));
    };
    let mut media: HashMap<String, Vec<Media>> = HashMap::new();
    for attachment in &attachments {
        if let Some(one) = bytes(copy, attachment).and_then(|bytes| put(stage, attachment, &bytes))
        {
            media.entry(attachment.turn.clone()).or_default().push(one);
        }
    }
    if stage.put(&snapshot.bytes).is_err() {
        return Ok(Err("stage".to_string()));
    }
    let created = ids::parse(&session.created_at).map_or(0, |at| {
        u64::try_from(at.as_millisecond()).unwrap_or_default()
    });
    let id = ids::session_id(created, source, &session.id);
    let head = Head {
        venue: "local",
        policy: &snapshot.hash,
        persona,
    };
    let built = build(&head, session, &turns, &media);
    let exported = ids::parse(&session.updated_at)
        .map_or_else(|| ids::at(jiff::Timestamp::UNIX_EPOCH), ids::at);
    let bundle = match stage.bundle(&id, &built.events, &exported) {
        Ok(bundle) => bundle,
        Err(StageError::Event(seq, _)) => return Ok(Err(format!("bad-event-{seq}"))),
        Err(StageError::Ledger(seq, _)) => return Ok(Err(format!("ledger-{seq}"))),
        Err(_) => return Ok(Err("bundle".to_string())),
    };
    let path = std::path::absolute(&bundle).unwrap_or(bundle);
    let imported = match core.call(
        "session.import",
        json!({"path": path.display().to_string(), "archive": true}),
    ) {
        Ok(imported) => imported,
        Err(said) if matches!(said.code.as_str(), "io" | "disconnected" | "no_answer") => {
            return Err(said);
        }
        Err(said) => return Ok(Err(format!("import-{}", said.code))),
    };
    let present = imported["present"]
        .as_array()
        .is_some_and(|list| list.iter().any(|one| one == id.as_str()));
    Ok(Ok((present, built)))
}

/// 附件的字节：库里的，或者 `state/attachments/<编号>/<文件名>`。都没有的是 `None`。
fn bytes(copy: &Layout, attachment: &Attachment) -> Option<Vec<u8>> {
    if let Some(data) = &attachment.data {
        return Some(data.clone());
    }
    let name = Path::new(&attachment.file_name).file_name()?;
    fs::read(
        copy.state()
            .join("attachments")
            .join(&attachment.id)
            .join(name),
    )
    .ok()
}

/// 附件放进暂存的 blob 库：图片认得出大小的写成图片。
fn put(stage: &Stage, attachment: &Attachment, bytes: &[u8]) -> Option<Media> {
    let blob = stage.put(bytes).ok()?;
    let size = (attachment.kind == "image")
        .then(|| imagesize::blob_size(bytes).ok())
        .flatten()
        .map(|size| (size.width as u64, size.height as u64))
        .filter(|(width, height)| *width > 0 && *height > 0);
    let media_type = if attachment.mime.trim().is_empty() {
        "application/octet-stream".to_string()
    } else {
        attachment.mime.trim().to_string()
    };
    Some(Media {
        blob,
        name: attachment.file_name.clone(),
        media_type,
        size,
    })
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
    note.push_str("绑了场所的（QQ）、子代理的会话下一步再迁。");
    note
}

#[cfg(test)]
mod tests;

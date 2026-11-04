//! 一个会话：附件放进暂存、拼事件、打包、`session.import {archive: true}`（细则第一节第 14、15 条）。

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde_json::json;

use crate::core::{Core, Said};
use crate::layout::Layout;

use super::events::{Built, Head, Media, build};
use super::ids;
use super::old::{Attachment, Session, Turn};
use super::said::Who;
use super::snapshot::Snapshot;
use super::stage::{Stage, StageError};

/// 要导的一个会话。
#[derive(Debug, Clone)]
pub struct Job {
    /// 旧会话（被挤出的轮兜底的那个是拼出来的）。
    pub session: Session,
    /// 照先后排好的轮（会话库剩下的和拼回来的已经归并）。
    pub turns: Vec<Turn>,
    /// 附件。
    pub attachments: Vec<Attachment>,
    /// 场所：`local`、`qq:<机器人>:<种类>:<号>`。
    pub venue: String,
    /// 说话的是谁。
    pub who: Owned,
    /// 算新编号用的：会话库（或者被挤出的库）相对旧根的路径。
    pub source: String,
}

/// [`Who`] 的自有版本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Owned {
    /// 本机的人。
    Person,
    /// 私聊的对方。
    Private(String),
    /// 群（机器人的号）。
    Group(String),
}

impl Owned {
    fn who(&self) -> Who<'_> {
        match self {
            Owned::Person => Who::Person,
            Owned::Private(peer) => Who::Private(peer),
            Owned::Group(bot) => Who::Group(bot),
        }
    }
}

/// 导一个会话。交回（早就在的吗、拼了什么）；这一个迁不成的交回为什么（一个短词，不带内容），连接断了的交回 [`Said`]。
pub fn one(
    core: &mut dyn Core,
    copy: &Layout,
    job: &Job,
    (stage, snapshot, persona): (&Stage, &Snapshot, Option<&str>),
) -> Result<Result<(bool, Built), String>, Said> {
    let mut media: HashMap<String, Vec<Media>> = HashMap::new();
    for attachment in &job.attachments {
        if let Some(one) = bytes(copy, attachment).and_then(|bytes| put(stage, attachment, &bytes))
        {
            media.entry(attachment.turn.clone()).or_default().push(one);
        }
    }
    if stage.put(&snapshot.bytes).is_err() {
        return Ok(Err("stage".to_string()));
    }
    let created = ids::parse(&job.session.created_at).map_or(0, |at| {
        u64::try_from(at.as_millisecond()).unwrap_or_default()
    });
    let id = ids::session_id(created, &job.source, &job.session.id);
    let head = Head {
        venue: &job.venue,
        policy: &snapshot.hash,
        persona,
        who: job.who.who(),
    };
    let built = build(&head, &job.session, &job.turns, &media);
    let exported = ids::parse(&job.session.updated_at)
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

//! 一个会话（连同它的子代理会话）：附件、工具的附带放进暂存、拼事件、打成一个包、`session.import {archive: true}`（细则第一节第
//! 4、14、15 条）。

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use crate::core::{Core, Said};
use crate::layout::Layout;

use super::events::{Attached, Built, Head, Media, build};
use super::extras::{Extras, Raw};
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
    /// 照先后排好的轮（会话库剩下的和拼回来的已经归并，插话已经并进 `followups`）。
    pub turns: Vec<Turn>,
    /// 人说的话的附件。
    pub attachments: Vec<Attachment>,
    /// 工具的附带、提问。
    pub extras: Extras,
    /// 场所：`local`、`qq:<机器人>:<种类>:<号>`。
    pub venue: String,
    /// 说话的是谁。
    pub who: Owned,
    /// 算新编号用的：会话库（或者被挤出的库）相对旧根的路径。
    pub source: String,
    /// 子代理会话：父会话的旧编号、深度。主会话是 `None`。
    pub parent: Option<(String, u64)>,
    /// 子代理会话（连孙代理，都在这一个包里）。
    pub children: Vec<Job>,
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

impl Job {
    /// 新编号：旧会话的创建时刻、会话库的位置、旧编号算出来。
    pub fn id(&self) -> String {
        id_of(&self.session, &self.source)
    }
}

/// 会话 `session`（在 `source` 那个库里）的新编号。
pub fn id_of(session: &Session, source: &str) -> String {
    let created = ids::parse(&session.created_at).map_or(0, |at| {
        u64::try_from(at.as_millisecond()).unwrap_or_default()
    });
    ids::session_id(created, source, &session.id)
}

/// 导一个会话和它的子会话。交回（早就在的吗、拼了什么，子会话的条数加在一起）；迁不成的交回为什么（一个短词，不带内容），
/// 连接断了的交回 [`Said`]。
pub fn one(
    core: &mut dyn Core,
    copy: &Layout,
    job: &Job,
    (stage, snapshot, persona): (&Stage, &Snapshot, Option<&str>),
) -> Result<Result<(bool, Built), String>, Said> {
    if stage.put(&snapshot.bytes).is_err() {
        return Ok(Err("stage".to_string()));
    }
    let mut sessions = Vec::new();
    let mut total = Built::default();
    for one in std::iter::once(job).chain(job.children.iter()) {
        let attached = attach(copy, stage, one);
        let parent = one
            .parent
            .as_ref()
            .map(|(old, depth)| (id_of(&parent_of(job, old), &one.source), *depth));
        let head = Head {
            venue: &one.venue,
            policy: &snapshot.hash,
            persona,
            who: one.who.who(),
            parent: parent.as_ref().map(|(id, depth)| (id.as_str(), *depth)),
        };
        let mut built = build(&head, &one.session, &one.turns, &attached);
        let raws: usize = one.extras.tools.values().map(Vec::len).sum();
        built.unplaced = (raws as u64).saturating_sub(built.tool_blocks);
        add(&mut total, &built);
        sessions.push((one.id(), built.events));
    }
    let exported = ids::parse(&job.session.updated_at)
        .map_or_else(|| ids::at(jiff::Timestamp::UNIX_EPOCH), ids::at);
    let bundle = match stage.bundle_all(&sessions, &exported) {
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
    let id = job.id();
    let present = imported["present"]
        .as_array()
        .is_some_and(|list| list.iter().any(|one| one == id.as_str()));
    Ok(Ok((present, total)))
}

/// 子会话 `old`（旧编号）的父会话：主会话或者另一个子会话。找不到的当是主会话（排的时候已经核过）。
fn parent_of(job: &Job, old: &str) -> Session {
    std::iter::once(job)
        .chain(job.children.iter())
        .find(|one| one.session.id == old)
        .map_or_else(|| job.session.clone(), |one| one.session.clone())
}

fn add(total: &mut Built, built: &Built) {
    total.turns += built.turns;
    total.summaries += built.summaries;
    total.attachments += built.attachments;
    total.replay_only += built.replay_only;
    total.evicted += built.evicted;
    total.unknown += built.unknown;
    total.tool_blocks += built.tool_blocks;
    total.questions += built.questions;
    total.unplaced += built.unplaced;
}

/// 一个会话接在别处的块：附件、工具的附带放进暂存的 blob 库。
fn attach(copy: &Layout, stage: &Stage, job: &Job) -> Attached {
    let mut attached = Attached {
        questions: job.extras.questions.clone(),
        ..Attached::default()
    };
    for attachment in &job.attachments {
        if let Some(one) = bytes(copy, attachment).and_then(|bytes| put(stage, attachment, &bytes))
        {
            attached
                .user
                .entry(attachment.turn.clone())
                .or_default()
                .push(one);
        }
    }
    let mut tools: HashMap<(String, String), Vec<Value>> = HashMap::new();
    for (key, raws) in &job.extras.tools {
        let blocks = raws.iter().map(|raw| tool_block(stage, raw)).collect();
        tools.insert(key.clone(), blocks);
    }
    attached.tools = tools;
    attached
}

/// 工具的一样附带写成一块：图、文件；没有字节、放不进的写一块字说没迁过来。
fn tool_block(stage: &Stage, raw: &Raw) -> Value {
    let Some(blob) = raw.bytes.as_deref().and_then(|bytes| stage.put(bytes).ok()) else {
        return json!({"type": "text", "text": format!("[旧版图片：{}，没有迁过来]", raw.name)});
    };
    let size = raw
        .image
        .then(|| {
            raw.bytes
                .as_deref()
                .and_then(|bytes| imagesize::blob_size(bytes).ok())
        })
        .flatten()
        .map(|size| (size.width as u64, size.height as u64))
        .filter(|(width, height)| *width > 0 && *height > 0);
    super::said::block(&Media {
        blob,
        name: raw.name.clone(),
        media_type: if raw.media_type.trim().is_empty() {
            "application/octet-stream".to_string()
        } else {
            raw.media_type.clone()
        },
        size,
    })
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

//! 一轮的附带（细则第一节第 11 条「工具的附带」「溢出」「插话」、提问）：照旧编号读出来，拼事件时接到对应的地方。
//!
//! - 工具出的图 `image_assets`、内嵌媒体 `turn_inline_media`（有字节的；没字节、`source` 是本机路径的照旧根换到副本里读，读不到
//!   的写一块字说没迁过来）、产物 `artifact_assets`：照工具调用的旧编号接在那一条 `tool.result` 的字后面；
//! - 溢出 `state/spill/<会话>/<turn>-<call>-<工具>.txt`：对得上一个调用的，原文作文件块接在那一条 `tool.result` 后面；
//! - 提问 `question_exchanges`：一问一答写成一次 `ask_user` 调用和结果；
//! - 插话 `queued_prompts`（`consumed`、挂在这一轮的）：照 `seq` 各一条带这一轮的 `message.user`。

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde_json::Value;

use super::old::{Db, text};

/// 读出来的一样附带的字节和名字（还没放进 blob 库）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raw {
    /// 文件名。
    pub name: String,
    /// 媒体类型。
    pub media_type: String,
    /// 字节；读不到的是 `None`（写一块字说没迁过来）。
    pub bytes: Option<Vec<u8>>,
    /// 是图片（`image`），别的写文件块。
    pub image: bool,
}

/// 一个会话里每一轮的附带，照旧 turn_id。
#[derive(Debug, Clone, Default)]
pub struct Extras {
    /// 工具调用的附带：(turn_id, 调用的旧编号) → 几样，照出现的先后。
    pub tools: HashMap<(String, String), Vec<Raw>>,
    /// 提问：turn_id → 每一次（照 `exchange_index`）的 JSON。
    pub questions: HashMap<String, Vec<Value>>,
    /// 插话：turn_id → 每一条（给人看的字、交给模型的原文）。
    pub queued: HashMap<String, Vec<(String, String)>>,
}

impl Extras {
    /// 读会话 `session` 的附带。`copy` 是解出来的副本（读溢出、没字节的内嵌媒体），`original` 是旧根原来的位置。读不了的表当没有。
    pub fn read(db: &Db, session: &str, copy: &Path, original: &Path) -> Extras {
        let mut extras = Extras::default();
        images(db, session, &mut extras);
        inline(db, session, (copy, original), &mut extras);
        artifacts(db, session, &mut extras);
        questions(db, session, &mut extras);
        queued(db, session, &mut extras);
        spill(&copy.join("state").join("spill").join(session), &mut extras);
        extras
    }
}

/// 照 `sql` 一行一行读，交给 `each`；出错的整张表当没有。
fn rows(
    db: &Db,
    sql: Option<String>,
    session: &str,
    mut each: impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<()>,
) {
    let Some(sql) = sql else {
        return;
    };
    let Ok(mut statement) = db.conn.prepare(&sql) else {
        return;
    };
    let Ok(mut found) = statement.query([session]) else {
        return;
    };
    while let Ok(Some(row)) = found.next() {
        if each(row).is_err() {
            return;
        }
    }
}

fn images(db: &Db, session: &str, extras: &mut Extras) {
    let sql = db
        .select(
            "image_assets",
            &["turn_id", "tool_id", "asset_id", "mime", "data"],
            "WHERE turn_id IN (SELECT turn_id FROM turns WHERE session_id = ?1) AND tool_id IS NOT NULL ORDER BY created_at, asset_id",
        )
        .ok()
        .flatten();
    rows(db, sql, session, |row| {
        let mime = text(row, 3)?;
        let name = format!("{}.{}", text(row, 2)?, extension(&mime));
        extras
            .tools
            .entry((text(row, 0)?, text(row, 1)?))
            .or_default()
            .push(Raw {
                name,
                media_type: mime,
                bytes: row.get(4)?,
                image: true,
            });
        Ok(())
    });
}

fn inline(db: &Db, session: &str, (copy, original): (&Path, &Path), extras: &mut Extras) {
    let sql = db
        .select(
            "turn_inline_media",
            &["turn_id", "call_id", "kind", "mime", "source", "data", "media_id"],
            "WHERE turn_id IN (SELECT turn_id FROM turns WHERE session_id = ?1) ORDER BY turn_id, call_id, seq",
        )
        .ok()
        .flatten();
    rows(db, sql, session, |row| {
        let kind = text(row, 2)?;
        if kind == "text" {
            // 视觉旁路的文字描述：结果里已经有字，不另接。
            return Ok(());
        }
        let mime = text(row, 3)?;
        let source = text(row, 4)?;
        let mut bytes: Option<Vec<u8>> = row.get(5)?;
        if bytes.is_none() {
            bytes = Path::new(&source)
                .strip_prefix(original)
                .ok()
                .and_then(|inside| fs::read(copy.join(inside)).ok());
        }
        let name = Path::new(&source)
            .file_name()
            .and_then(|name| name.to_str())
            .map_or_else(
                || {
                    format!(
                        "media-{}.{}",
                        row.get::<_, i64>(6).unwrap_or_default(),
                        extension(&mime)
                    )
                },
                str::to_string,
            );
        extras
            .tools
            .entry((text(row, 0)?, text(row, 1)?))
            .or_default()
            .push(Raw {
                name,
                media_type: mime,
                bytes,
                image: kind == "image",
            });
        Ok(())
    });
}

fn artifacts(db: &Db, session: &str, extras: &mut Extras) {
    let sql = db
        .select(
            "artifact_assets",
            &["turn_id", "tool_id", "file_name", "mime", "data"],
            "WHERE turn_id IN (SELECT turn_id FROM turns WHERE session_id = ?1) AND tool_id IS NOT NULL ORDER BY updated_at, asset_id",
        )
        .ok()
        .flatten();
    rows(db, sql, session, |row| {
        extras
            .tools
            .entry((text(row, 0)?, text(row, 1)?))
            .or_default()
            .push(Raw {
                name: text(row, 2)?,
                media_type: text(row, 3)?,
                bytes: row.get(4)?,
                image: false,
            });
        Ok(())
    });
}

fn questions(db: &Db, session: &str, extras: &mut Extras) {
    let sql = db
        .select(
            "question_exchanges",
            &["turn_id", "payload"],
            "WHERE turn_id IN (SELECT turn_id FROM turns WHERE session_id = ?1) ORDER BY turn_id, exchange_index",
        )
        .ok()
        .flatten();
    rows(db, sql, session, |row| {
        if let Ok(payload) = serde_json::from_str::<Value>(&text(row, 1)?) {
            extras
                .questions
                .entry(text(row, 0)?)
                .or_default()
                .push(payload);
        }
        Ok(())
    });
}

fn queued(db: &Db, session: &str, extras: &mut Extras) {
    let sql = db
        .select(
            "queued_prompts",
            &["turn_id", "display_content", "content"],
            "WHERE status = 'consumed' AND turn_id IN (SELECT turn_id FROM turns WHERE session_id = ?1) ORDER BY seq",
        )
        .ok()
        .flatten();
    rows(db, sql, session, |row| {
        extras
            .queued
            .entry(text(row, 0)?)
            .or_default()
            .push((text(row, 1)?, text(row, 2)?));
        Ok(())
    });
}

/// 溢出的原文：`<turn>-<call>-<工具>.txt`。turn、call 的旧编号里本来就有 `_`，没有 `-`（`turn_<毫秒>_<n>`、`call_…`）。
fn spill(dir: &Path, extras: &mut Extras) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    files.sort();
    for file in files {
        let Some(name) = file
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        let Some(stem) = name.strip_suffix(".txt") else {
            continue;
        };
        let mut parts = stem.splitn(3, '-');
        let (Some(turn), Some(call)) = (parts.next(), parts.next()) else {
            continue;
        };
        extras
            .tools
            .entry((turn.to_string(), call.to_string()))
            .or_default()
            .push(Raw {
                name,
                media_type: "text/plain".to_string(),
                bytes: fs::read(&file).ok(),
                image: false,
            });
    }
}

/// 媒体类型对应的扩展名（起文件名用）。
fn extension(mime: &str) -> &str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "video/mp4" => "mp4",
        _ => "bin",
    }
}

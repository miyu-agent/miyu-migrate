//! 旧库一行一行转成 im 的写法（细则第二节「一行 `messages` 写成一行」）。

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{Connection, OpenFlags};
use serde_json::{Map, Value, json};

use super::VERSION;

/// 转好的：每一行（JSON），和数出来的几样。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Converted {
    /// 一行一条，`message` 在前、`happening` 在最后。
    pub lines: Vec<String>,
    /// 消息几条。
    pub messages: u64,
    /// 撤回了、消息不在了的几次（写成 `happening`）。
    pub happenings: u64,
    /// 不是 QQ（`platform` 不是 `onebot`）、没转的。
    pub foreign: u64,
}

/// 撤回的一次：(平台, 机器人, 种类, 号, 消息编号) → (谁撤的, 什么时候, 秒)。
type Recalls = HashMap<(String, String, String, String, String), (Option<String>, i64)>;

/// 转 `path` 这个库。版本不是 [`VERSION`] 的交回 `None`。
///
/// # Errors
///
/// 打不开、读不了。
pub fn convert(path: &Path) -> rusqlite::Result<Option<Converted>> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version != VERSION {
        return Ok(None);
    }
    let mut recalls = recalls(&conn)?;
    let mut converted = Converted::default();
    let mut statement = conn.prepare(
        "SELECT platform, account_id, conversation_kind, conversation_id, message_id, sender_id, sender_name, text,
                media_json, mentions_json, reply_to_message_id, is_bot, sent_at, recalled_at
         FROM messages ORDER BY id",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let platform: String = row.get(0)?;
        if platform != "onebot" {
            converted.foreign += 1;
            continue;
        }
        let key = (
            platform,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        );
        let mut line = Map::new();
        line.insert("type".to_string(), json!("message"));
        line.insert("venue".to_string(), json!(venue(&key.1, &key.2, &key.3)));
        line.insert("msg".to_string(), json!(key.4));
        line.insert(
            "sender".to_string(),
            json!(format!("qq:{}", row.get::<_, String>(5)?)),
        );
        line.insert(
            "at".to_string(),
            json!(row.get::<_, i64>(12)?.saturating_mul(1000)),
        );
        line.insert("text".to_string(), json!(row.get::<_, String>(7)?));
        let name: String = row.get(6)?;
        if !name.is_empty() {
            line.insert("name".to_string(), json!(name));
        }
        if let Some(reply) = row
            .get::<_, Option<String>>(10)?
            .filter(|reply| !reply.is_empty())
        {
            line.insert("reply_to".to_string(), json!(reply));
        }
        let mentions = mentions(&row.get::<_, String>(9)?);
        if !mentions.is_empty() {
            line.insert("mentions".to_string(), json!(mentions));
        }
        let media = media(&row.get::<_, String>(8)?);
        if !media.is_empty() {
            line.insert("media".to_string(), Value::Array(media));
        }
        if row.get::<_, i64>(11)? == 1 {
            line.insert("mine".to_string(), json!(true));
        }
        let recall = recalls.remove(&key);
        let recalled = row
            .get::<_, Option<i64>>(13)?
            .or(recall.as_ref().map(|(_, at)| *at));
        if let Some(at) = recalled {
            line.insert("recalled_at".to_string(), json!(at.saturating_mul(1000)));
            if let Some(by) = recall.and_then(|(by, _)| by).filter(|by| !by.is_empty()) {
                line.insert("recalled_by".to_string(), json!(format!("qq:{by}")));
            }
        }
        converted.lines.push(Value::Object(line).to_string());
        converted.messages += 1;
    }
    // 撤回了、消息不在了的：写成群里的事，排在最后（照键排，同样的输入同样的字节）。
    let mut left: Vec<_> = recalls
        .into_iter()
        .filter(|(key, _)| key.0 == "onebot")
        .collect();
    left.sort();
    for (key, (by, at)) in left {
        let mut line = json!({
            "type": "happening",
            "venue": venue(&key.1, &key.2, &key.3),
            "at": at.saturating_mul(1000),
            "kind": "recalled",
            "msg": key.4,
        });
        if let Some(by) = by.filter(|by| !by.is_empty()) {
            line["actor"] = json!(format!("qq:{by}"));
        }
        converted.lines.push(line.to_string());
        converted.happenings += 1;
    }
    Ok(Some(converted))
}

/// 场所的新写法：`qq:<机器人>:<种类>:<号>`。
fn venue(account: &str, kind: &str, conversation: &str) -> String {
    format!("qq:{account}:{kind}:{conversation}")
}

/// `recalls` 整张表。
fn recalls(conn: &Connection) -> rusqlite::Result<Recalls> {
    let mut statement = conn.prepare(
        "SELECT platform, account_id, conversation_kind, conversation_id, message_id, operator_id, recalled_at FROM recalls",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            (
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ),
            (row.get(5)?, row.get(6)?),
        ))
    })?;
    rows.collect()
}

/// `mentions_json`：字符串数组（号）和对象数组（`user_id`）两种都认，写成 `qq:<号>`。读不懂的当没有。
fn mentions(raw: &str) -> Vec<String> {
    let parsed: Vec<Value> = serde_json::from_str(raw).unwrap_or_default();
    parsed
        .iter()
        .filter_map(|one| match one {
            Value::String(qq) => Some(qq.clone()),
            Value::Number(qq) => Some(qq.to_string()),
            Value::Object(map) => map.get("user_id").and_then(|id| match id {
                Value::String(qq) => Some(qq.clone()),
                Value::Number(qq) => Some(qq.to_string()),
                _ => None,
            }),
            _ => None,
        })
        .filter(|qq| !qq.is_empty())
        .map(|qq| format!("qq:{qq}"))
        .collect()
}

/// `media_json`：`kind` 照写（`audio` 写 `voice`，`other` 写 `file`），`id` 取 `media_id`、没有的取 `label`，`name` 取 `label`。
/// 两样都没有的不写（im 认编号去取）。读不懂的当没有。
fn media(raw: &str) -> Vec<Value> {
    let parsed: Vec<Value> = serde_json::from_str(raw).unwrap_or_default();
    parsed
        .iter()
        .filter_map(|one| {
            let kind = match one["kind"].as_str()? {
                "audio" => "voice",
                "other" => "file",
                kind @ ("image" | "sticker" | "file" | "video") => kind,
                _ => "file",
            };
            let label = one["label"].as_str().filter(|label| !label.is_empty());
            let id = one["media_id"]
                .as_str()
                .filter(|id| !id.is_empty())
                .or(label)?;
            let mut out = json!({"kind": kind, "id": id});
            if let Some(label) = label {
                out["name"] = json!(label);
            }
            Some(out)
        })
        .collect()
}

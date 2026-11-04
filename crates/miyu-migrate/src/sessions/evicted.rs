//! 被挤出上下文的轮（细则〇第 1 条、第一节第 9 条）：旧版压缩时把可见的轮整行搬进
//! `state/personas/<scope>/memory/evicted_context.db` 的 `evicted_turns`，同一个事务里从会话库删掉。只迁会话库会丢掉它们，
//! 所以照 `source_id`（`<旧 turn_id>:<哪一句>`）拼回一轮，照 `origin_session_id` 插回原来的会话，和剩下的轮照时刻归并。
//!
//! 一行是一轮里的一句：`user`、`assistant`、`followup:<prompt_id>`（排着并进来的话）、`before:<prompt_id>`。这一类只有字。

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use rusqlite::{Connection, OpenFlags};

use super::ids;
use super::old::Turn;

/// 一个人格的库里读出来的：照原来的会话分好的轮；`origin_session_id` 是空的（更老的行）归在 `""` 下。
pub type ByOrigin = BTreeMap<String, Vec<Turn>>;

/// 读 `personas`（`state/personas/`）下每个人格的库，交回（scope，照会话分好的轮）。没有的是空的；读不了的那一个跳过，
/// 交回它的 scope。
pub fn read_all(personas: &Path) -> (Vec<(String, ByOrigin)>, Vec<String>) {
    let mut found = Vec::new();
    let mut unreadable = Vec::new();
    let Ok(entries) = fs::read_dir(personas) else {
        return (found, unreadable);
    };
    let mut scopes: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
        .collect();
    scopes.sort();
    for scope in scopes {
        let db = personas
            .join(&scope)
            .join("memory")
            .join("evicted_context.db");
        if !db.is_file() {
            continue;
        }
        match read(&db) {
            Ok(turns) if !turns.is_empty() => found.push((scope, turns)),
            Ok(_) => {}
            Err(_) => unreadable.push(scope),
        }
    }
    (found, unreadable)
}

/// 读一个库：一行一句，同一个旧 turn_id 的几行拼成一轮（照 `id` 的先后），照 `origin_session_id` 分开。
///
/// # Errors
///
/// 打不开、读不了。
pub fn read(path: &Path) -> rusqlite::Result<ByOrigin> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    let has_origin = conn
        .prepare(
            "SELECT 1 FROM pragma_table_info('evicted_turns') WHERE name = 'origin_session_id'",
        )?
        .exists([])?;
    let origin = if has_origin {
        "origin_session_id"
    } else {
        "''"
    };
    let mut statement = conn.prepare(&format!(
        "SELECT COALESCE(source_id, ''), timestamp, content, {origin} FROM evicted_turns ORDER BY id"
    ))?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?.unwrap_or_default(),
        ))
    })?;
    // 照 (会话, 旧 turn_id) 拼；先后照第一次出现。
    let mut order: Vec<(String, String)> = Vec::new();
    let mut turns: BTreeMap<(String, String), Turn> = BTreeMap::new();
    for row in rows {
        let (source, timestamp, content, origin) = row?;
        let (turn_id, part) = source.split_once(':').unwrap_or((source.as_str(), "user"));
        let key = (origin.clone(), turn_id.to_string());
        let turn = turns.entry(key.clone()).or_insert_with(|| {
            order.push(key.clone());
            Turn {
                id: turn_id.to_string(),
                user_timestamp: timestamp.clone(),
                status: "completed".to_string(),
                evicted: true,
                ..Turn::default()
            }
        });
        match part {
            "user" => {
                turn.user_content = content;
                turn.user_timestamp = timestamp;
            }
            "assistant" => {
                turn.assistant_content = content;
                turn.assistant_timestamp = Some(timestamp);
            }
            _ => turn.followups.push(content),
        }
    }
    let mut by: ByOrigin = BTreeMap::new();
    for key in order {
        if let Some(turn) = turns.remove(&key) {
            by.entry(key.0).or_default().push(turn);
        }
    }
    Ok(by)
}

/// 会话里剩下的轮（照 `seq`）和拼回来的轮（照搬走的先后）照人说话的时刻归并：两边各自的先后都不变，同一时刻的会话里剩下的
/// 在前；读不懂时刻的照前一轮的算。
pub fn merge(kept: Vec<Turn>, evicted: Vec<Turn>) -> Vec<Turn> {
    let at = |turn: &Turn| ids::parse(&turn.user_timestamp).map(|time| time.as_millisecond());
    let mut merged = Vec::with_capacity(kept.len() + evicted.len());
    let mut kept = kept.into_iter().peekable();
    let mut evicted = evicted.into_iter().peekable();
    loop {
        let take_kept = match (kept.peek(), evicted.peek()) {
            (None, None) => break,
            (Some(_), None) => true,
            (None, Some(_)) => false,
            (Some(left), Some(right)) => match (at(left), at(right)) {
                (Some(left), Some(right)) => left <= right,
                _ => true,
            },
        };
        let next = if take_kept {
            kept.next()
        } else {
            evicted.next()
        };
        merged.extend(next);
    }
    merged
}

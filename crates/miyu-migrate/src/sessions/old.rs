//! 读旧会话库（`conversation.db`，调研「第二部分」）：照列名读，老库缺的列当 NULL；表都没有的当空的。库是解出来的副本，
//! 照常打开（带着 WAL 的能读到），不往里写。

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use rusqlite::{Connection, OpenFlags, Row};

/// 认得的最新版本：比它新的整库不迁。
pub const NEWEST: i64 = 40;

/// 打开的旧库。
pub struct Db {
    conn: Connection,
    /// `PRAGMA user_version`。
    pub version: i64,
}

/// `sessions` 的一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// 旧编号（`sess_<毫秒>_<hex>`、`default`）。
    pub id: String,
    /// 人格 scope（`default` 的会话可能是空串）。
    pub persona: String,
    /// 名字，默认是空的。
    pub name: String,
    /// `user`、`ask`、`voice`、`subagent`。
    pub kind: String,
    /// 成员的会话写着账号，管理员的是空的。
    pub owner: String,
    /// 子代理的父会话。
    pub parent: Option<String>,
    /// 创建时刻（RFC 3339）。
    pub created_at: String,
    /// 最后改动（RFC 3339）。
    pub updated_at: String,
}

/// `turns` 的一行：一轮。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Turn {
    /// 旧编号。
    pub id: String,
    /// 会话内的序号（完成顺序）。
    pub seq: i64,
    /// 交给模型的原文。
    pub user_content: String,
    /// 给人看的（v5 起）。
    pub display_content: String,
    /// 人说话的时刻。
    pub user_timestamp: String,
    /// 最后的回复。
    pub assistant_content: String,
    /// 最后一段思考。
    pub assistant_reasoning: Option<String>,
    /// 回复的时刻。
    pub assistant_timestamp: Option<String>,
    /// `running`、`completed`、`interrupted`。
    pub status: String,
    /// 摘要行。
    pub summary: bool,
    /// 回复的供应商。
    pub provider: Option<String>,
    /// 回复的模型。
    pub model: Option<String>,
    /// 结构化的工具流（v20 起）。
    pub tool_flow: Option<String>,
    /// 展示用的转录（v17 起）。
    pub replay_journal: Option<String>,
    /// 从被挤出的库拼回来的（只有字）。
    pub evicted: bool,
    /// 被挤出的轮里排着并进来的话（原文，带旧版的可信元数据那一段）。
    pub followups: Vec<String>,
}

/// `platform_session_bindings` 的一行：会话绑在哪个场所。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// 平台：`onebot` 是 QQ。
    pub platform: String,
    /// 机器人的号。
    pub account: String,
    /// `group`、`private`。
    pub kind: String,
    /// 群号、对方的号。
    pub conversation: String,
}

/// `user_attachments` 里挂在某一轮的一个附件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    /// 旧编号（`att_…`）：字节不在库里的在 `state/attachments/<它>/<文件名>`。
    pub id: String,
    /// 哪一轮。
    pub turn: String,
    /// 文件名。
    pub file_name: String,
    /// 媒体类型。
    pub mime: String,
    /// `image`、`text`、`file`。
    pub kind: String,
    /// 库里的字节（v29 以前）。
    pub data: Option<Vec<u8>>,
}

impl Db {
    /// 打开 `path`。
    ///
    /// # Errors
    ///
    /// 打不开、不是 SQLite。
    pub fn open(path: &Path) -> rusqlite::Result<Db> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        let version = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        Ok(Db { conn, version })
    }

    /// 表 `table` 有哪些列；没有这张表的是空的。
    fn columns(&self, table: &str) -> rusqlite::Result<BTreeSet<String>> {
        let mut statement = self
            .conn
            .prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))?;
        let names = statement.query_map([], |row| row.get::<_, String>(0))?;
        names.collect()
    }

    /// `SELECT` 照列名：有的取它，没有的写 `NULL AS <列>`。表不在的是 `None`。
    fn select(&self, table: &str, wanted: &[&str], rest: &str) -> rusqlite::Result<Option<String>> {
        let have = self.columns(table)?;
        if have.is_empty() {
            return Ok(None);
        }
        let list: Vec<String> = wanted
            .iter()
            .map(|column| {
                if have.contains(*column) {
                    format!("\"{column}\"")
                } else {
                    format!("NULL AS \"{column}\"")
                }
            })
            .collect();
        Ok(Some(format!(
            "SELECT {} FROM \"{table}\" {rest}",
            list.join(", ")
        )))
    }

    /// 全部会话，照创建时刻、编号排。
    ///
    /// # Errors
    ///
    /// 读不了。
    pub fn sessions(&self) -> rusqlite::Result<Vec<Session>> {
        let Some(sql) = self.select(
            "sessions",
            &[
                "session_id",
                "persona",
                "name",
                "kind",
                "owner",
                "parent_session_id",
                "created_at",
                "updated_at",
            ],
            "ORDER BY created_at, session_id",
        )?
        else {
            return Ok(Vec::new());
        };
        let mut statement = self.conn.prepare(&sql)?;
        let rows = statement.query_map([], |row| {
            Ok(Session {
                id: row.get(0)?,
                persona: text(row, 1)?,
                name: text(row, 2)?,
                kind: row
                    .get::<_, Option<String>>(3)?
                    .unwrap_or_else(|| "user".to_string()),
                owner: text(row, 4)?,
                parent: row.get(5)?,
                created_at: text(row, 6)?,
                updated_at: text(row, 7)?,
            })
        })?;
        rows.collect()
    }

    /// 绑了场所的会话（`platform_session_bindings`，没有这张表的是空的）：会话的旧编号到绑在哪。
    ///
    /// # Errors
    ///
    /// 读不了。
    pub fn bindings(&self) -> rusqlite::Result<HashMap<String, Binding>> {
        let Some(sql) = self.select(
            "platform_session_bindings",
            &[
                "session_id",
                "platform",
                "account_id",
                "conversation_kind",
                "conversation_id",
            ],
            "",
        )?
        else {
            return Ok(HashMap::new());
        };
        let mut statement = self.conn.prepare(&sql)?;
        let rows = statement.query_map([], |row| {
            Ok((
                text(row, 0)?,
                Binding {
                    platform: text(row, 1)?,
                    account: text(row, 2)?,
                    kind: text(row, 3)?,
                    conversation: text(row, 4)?,
                },
            ))
        })?;
        rows.collect()
    }

    /// 会话 `session` 的轮，照 `seq` 排。
    ///
    /// # Errors
    ///
    /// 读不了。
    pub fn turns(&self, session: &str) -> rusqlite::Result<Vec<Turn>> {
        let Some(sql) = self.select(
            "turns",
            &[
                "turn_id",
                "seq",
                "user_content",
                "display_content",
                "user_timestamp",
                "assistant_content",
                "assistant_reasoning",
                "assistant_timestamp",
                "status",
                "is_summary",
                "assistant_provider_id",
                "assistant_model",
                "tool_flow",
                "replay_journal",
            ],
            "WHERE session_id = ?1 ORDER BY seq, turn_id",
        )?
        else {
            return Ok(Vec::new());
        };
        let mut statement = self.conn.prepare(&sql)?;
        let rows = statement.query_map([session], |row| {
            Ok(Turn {
                id: row.get(0)?,
                seq: row.get::<_, Option<i64>>(1)?.unwrap_or_default(),
                user_content: text(row, 2)?,
                display_content: text(row, 3)?,
                user_timestamp: text(row, 4)?,
                assistant_content: text(row, 5)?,
                assistant_reasoning: row.get(6)?,
                assistant_timestamp: row.get(7)?,
                status: text(row, 8)?,
                summary: row.get::<_, Option<i64>>(9)?.unwrap_or_default() != 0,
                provider: row.get(10)?,
                model: row.get(11)?,
                tool_flow: row.get(12)?,
                replay_journal: row.get(13)?,
                evicted: false,
                followups: Vec::new(),
            })
        })?;
        rows.collect()
    }

    /// 会话 `session` 里挂在轮上的附件，照创建时刻、编号排。
    ///
    /// # Errors
    ///
    /// 读不了。
    pub fn attachments(&self, session: &str) -> rusqlite::Result<Vec<Attachment>> {
        let Some(sql) = self.select(
            "user_attachments",
            &[
                "attachment_id",
                "turn_id",
                "file_name",
                "mime",
                "kind",
                "data",
            ],
            "WHERE session_id = ?1 AND turn_id IS NOT NULL ORDER BY created_at, attachment_id",
        )?
        else {
            return Ok(Vec::new());
        };
        let mut statement = self.conn.prepare(&sql)?;
        let rows = statement.query_map([session], |row| {
            Ok(Attachment {
                id: row.get(0)?,
                turn: row.get(1)?,
                file_name: text(row, 2)?,
                mime: text(row, 3)?,
                kind: text(row, 4)?,
                data: row.get(5)?,
            })
        })?;
        rows.collect()
    }
}

/// 一格字：NULL 当空的。
fn text(row: &Row<'_>, index: usize) -> rusqlite::Result<String> {
    Ok(row.get::<_, Option<String>>(index)?.unwrap_or_default())
}

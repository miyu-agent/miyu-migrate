//! 读旧记忆库、一条一条判成新版的样子（记忆类别对照第 2、3 节，细则第三节）。不调模型，机械规则判不准的照推荐的类导、报告里数。

use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};

/// 新版一条的正文上限（字）。
pub const TEXT_CHARS: usize = 120;
/// 关键词上限（字）。
pub const KEYWORD_CHARS: usize = 200;

/// 一条旧记忆判出来的结果。
#[derive(Debug, Clone, PartialEq)]
pub enum Judged {
    /// 导：`memory.import` 的一条。`guessed` 是没写是谁、照推荐的类导的（建议的类，导成的类）。
    Import {
        /// 那一条。
        item: Value,
        /// 没写是谁、照推荐导的。
        guessed: Option<(&'static str, &'static str)>,
        /// 号换不出、退到名字的。
        by_name: bool,
    },
    /// 不导：为什么（`self`、`too-long`、`inactive`、`empty`、`bad-time`），原来的类。
    Skip(&'static str, String),
}

/// 一条旧记忆：编号、判出来的。
#[derive(Debug, Clone, PartialEq)]
pub struct Fact {
    /// `facts.id`。
    pub id: i64,
    /// `miyu-legacy:<旧人格>:facts:<id>`。
    pub origin: String,
    /// 判的结果。
    pub judged: Judged,
}

/// 读 `path`（`<scope>/memory/memory.db`）的 `facts`，照 `owners`（主人的号，`qq:<号>`）判。没有 `facts` 表的是空的。
///
/// # Errors
///
/// 打不开、读不了。
pub fn read(path: &Path, scope: &str, owners: &HashSet<String>) -> rusqlite::Result<Vec<Fact>> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    let have: HashSet<String> = conn
        .prepare("SELECT name FROM pragma_table_info('facts')")?
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    if have.is_empty() {
        return Ok(Vec::new());
    }
    let column = |name: &str, default: &str| {
        if have.contains(name) {
            format!("\"{name}\"")
        } else {
            format!("{default} AS \"{name}\"")
        }
    };
    let principals = principals(&conn)?;
    let sql = format!(
        "SELECT id, {}, {}, {}, {}, {}, {}, {} FROM facts ORDER BY id",
        column("content", "''"),
        column("memory_type", "'fact'"),
        column("status", "'active'"),
        column("updated_at", "''"),
        column("tags", "'[]'"),
        column("subjects", "'[]'"),
        column("owner_principal", "''"),
    );
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            row.get::<_, Option<String>>(3)?.unwrap_or_default(),
            row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            row.get::<_, Option<String>>(5)?.unwrap_or_default(),
            row.get::<_, Option<String>>(6)?.unwrap_or_default(),
            row.get::<_, Option<String>>(7)?.unwrap_or_default(),
        ))
    })?;
    let mut facts = Vec::new();
    for row in rows {
        let (id, content, kind, status, updated, tags, subjects, owner) = row?;
        let origin = format!("miyu-legacy:{scope}:facts:{id}");
        let raw = Raw {
            origin: &origin,
            content: &content,
            kind: &kind,
            status: &status,
            updated: &updated,
            tags: &tags,
            subjects: &subjects,
            owner: &owner,
        };
        let judged = judge(&raw, &principals, owners);
        facts.push(Fact { id, origin, judged });
    }
    Ok(facts)
}

/// 一行 `facts` 里用得上的几格。
#[derive(Debug, Clone, Copy)]
pub struct Raw<'a> {
    /// `miyu-legacy:…`。
    pub origin: &'a str,
    /// 正文。
    pub content: &'a str,
    /// `memory_type`。
    pub kind: &'a str,
    /// `status`。
    pub status: &'a str,
    /// `updated_at`（RFC 3339）。
    pub updated: &'a str,
    /// `tags`（JSON 字符串列表）。
    pub tags: &'a str,
    /// `subjects`（JSON 对象列表，`name`、`principal`）。
    pub subjects: &'a str,
    /// `owner_principal`。
    pub owner: &'a str,
}

/// 同一个库的 `episodes`：旧版的身份哈希 → `qq:<号>`（`origin_platform` 是 `onebot`、`origin_sender_id` 非空的行）。一个哈希
/// 换出几个号的当换不出。
fn principals(conn: &Connection) -> rusqlite::Result<HashMap<String, String>> {
    let have: HashSet<String> = conn
        .prepare("SELECT name FROM pragma_table_info('episodes')")?
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    if !["owner_principal", "origin_platform", "origin_sender_id"]
        .iter()
        .all(|name| have.contains(*name))
    {
        return Ok(HashMap::new());
    }
    let mut statement = conn.prepare(
        "SELECT DISTINCT owner_principal, origin_sender_id FROM episodes
         WHERE owner_principal != '' AND origin_platform = 'onebot' AND origin_sender_id != ''",
    )?;
    let pairs = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut found: HashMap<String, HashSet<String>> = HashMap::new();
    for pair in pairs {
        let (principal, qq) = pair?;
        found.entry(principal).or_default().insert(qq);
    }
    Ok(found
        .into_iter()
        .filter(|(_, all)| all.len() == 1)
        .filter_map(|(principal, all)| {
            all.into_iter()
                .next()
                .map(|qq| (principal, format!("qq:{qq}")))
        })
        .collect())
}

/// 判一条（对照表第 2 节）。
pub fn judge(
    raw: &Raw<'_>,
    principals: &HashMap<String, String>,
    owners: &HashSet<String>,
) -> Judged {
    let kind = raw.kind.trim().to_string();
    if raw.status != "active" {
        return Judged::Skip("inactive", kind);
    }
    if kind == "self" {
        return Judged::Skip("self", kind);
    }
    let text = raw.content.trim();
    if text.is_empty() {
        return Judged::Skip("empty", kind);
    }
    if text.chars().count() > TEXT_CHARS {
        return Judged::Skip("too-long", kind);
    }
    let (who, by_name) = who(raw, principals);
    let mut guessed = None;
    let (class, who) = match who {
        Some(who) if owners.contains(&who) => ("user", None),
        Some(who) => ("person", Some(who)),
        None => {
            let class = match kind.as_str() {
                "preference" => "feedback",
                "task" => "episode",
                _ => "reference",
            };
            let suggested = match kind.as_str() {
                "preference" => "feedback",
                "task" => "episode",
                _ => "user/reference",
            };
            guessed = Some((suggested, class));
            (class, None)
        }
    };
    // 读不懂的时刻核心会拒、整批不写：这一条先挑出来。
    let Ok(at) = raw.updated.trim().parse::<jiff::Timestamp>() else {
        return Judged::Skip("bad-time", kind);
    };
    let at = at.to_string();
    let mut item = json!({"at": at, "class": class, "text": text, "origin": raw.origin});
    if let Some(who) = who {
        item["who"] = json!(who);
    }
    if let Some(keywords) = keywords(raw.tags) {
        item["keywords"] = json!(keywords);
    }
    Judged::Import {
        item,
        guessed,
        by_name,
    }
}

/// 说的是谁（对照表第 3 节）：`subjects` 里和这一条的 `owner_principal` 相同的那个，没有相同的取第一个；身份哈希换得出号的写
/// `qq:<号>`，换不出、只有名字的写名字（第二个交回 `true`）。
fn who(raw: &Raw<'_>, principals: &HashMap<String, String>) -> (Option<String>, bool) {
    let subjects: Vec<Value> = serde_json::from_str(raw.subjects).unwrap_or_default();
    let chosen = subjects
        .iter()
        .find(|one| !raw.owner.is_empty() && one["principal"] == raw.owner)
        .or_else(|| subjects.first());
    let Some(chosen) = chosen else {
        return (None, false);
    };
    if let Some(qq) = chosen["principal"]
        .as_str()
        .and_then(|principal| principals.get(principal))
    {
        return (Some(qq.clone()), false);
    }
    match chosen["name"]
        .as_str()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        Some(name) => (Some(name.to_string()), true),
        None => (None, false),
    }
}

/// `tags`（JSON 字符串列表）用「, 」连起来，超过上限的在逗号处截；没有的是 `None`。
fn keywords(tags: &str) -> Option<String> {
    let tags: Vec<String> = serde_json::from_str(tags).unwrap_or_default();
    let mut joined = String::new();
    for tag in tags
        .iter()
        .map(|tag| tag.trim())
        .filter(|tag| !tag.is_empty())
    {
        let next = if joined.is_empty() {
            tag.to_string()
        } else {
            format!("{joined}, {tag}")
        };
        if next.chars().count() > KEYWORD_CHARS {
            break;
        }
        joined = next;
    }
    (!joined.is_empty()).then_some(joined)
}

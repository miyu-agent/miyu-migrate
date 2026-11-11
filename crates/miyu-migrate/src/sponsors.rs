//! 赞助记录（细则第十一节已定第 4 条，im 施工 I-131、im 设计 03 第五节）：旧版会话库的 `sponsor_records` 一行转成赞助插件账本的一笔，
//! 经 `bus.call {"name": "sponsor", "method": "import", "params": {"entries": […]}}` 导：照总线上的名字调（施工 MG-17，核心设计 67），
//! 核心拼成插件清单里导出的 `sponsor.import`。插件照（平台、人、时刻、金额、币种）去重，重导是空操作。插件没装的跳过、提示装；
//! 装着但是旧的（总线上没有这个名字）、核心是旧的（不认 `bus.call`）提示升级，不退回旧名字再试。名字、号是别人的：报告只写条数。

use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};

use crate::core::{Core, Said};
use crate::layout::Layout;
use crate::report::{Item, Status};

/// 赞助插件：`package.list` 里照它看装没装。
pub const SPONSOR: &str = "miyu-im-sponsor";

/// 赞助插件在总线上提供的名字（im 施工 I-211）：导的时候照它调，不写插件的包编号。
pub const BUS_NAME: &str = "sponsor";

/// 旧格式一句话。
const FORMAT: &str = "会话库（SQLite v32 起）的 sponsor_records：platform（onebot）、account_id、sponsor_id（QQ 号或名字）、sponsor_name、amount_minor（分）、currency、cny_minor、fx_rate（同币种写 0）、note、recorded_by、sponsored_at（RFC 3339）";

/// 迁赞助记录：读 `copy` 的会话库，经 `core` 交赞助插件导。交回报告的一项。
///
/// # Errors
///
/// 连接断了。
pub fn migrate(core: &mut dyn Core, copy: &Layout) -> Result<Item, Said> {
    let path = copy.conversation_db();
    let mut item = Item {
        what: "sponsors".to_string(),
        status: Status::Migrated,
        source: format!(
            "{}（sponsor_records）",
            path.strip_prefix(&copy.root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/")
        ),
        format: FORMAT.to_string(),
        migrated: 0,
        skipped: Vec::new(),
        target: Some(format!("bus.call {BUS_NAME}.import")),
        note: None,
    };
    let entries = match read(&path) {
        Ok(entries) if !entries.is_empty() => entries,
        _ => {
            item.status = Status::Skipped;
            item.note = Some("旧版没有赞助记录。".to_string());
            return Ok(item);
        }
    };
    let installed = core
        .call("package.list", json!({}))?
        .get("packages")
        .and_then(Value::as_array)
        .is_some_and(|all| all.iter().any(|one| one["package"] == SPONSOR));
    if !installed {
        item.status = Status::Archived;
        item.note = Some(format!(
            "{} 笔赞助记录：赞助插件 `{SPONSOR}` 没装，装上以后重跑迁移（重跑不会多出来）。",
            entries.len()
        ));
        return Ok(item);
    }
    let params = json!({"name": BUS_NAME, "method": "import", "params": {"entries": entries}});
    match core.call("bus.call", params) {
        Ok(answer) => {
            let added = answer["added"].as_u64().unwrap_or_default();
            let skipped = answer["skipped"].as_u64().unwrap_or_default();
            item.migrated = added + skipped;
            item.note = Some(format!(
                "{} 笔赞助记录：新加 {added} 笔，{skipped} 笔账本里本来就有（以前导过、记过的）。",
                entries.len()
            ));
            if added + skipped != entries.len() as u64 {
                item.status = Status::Partial;
            }
        }
        Err(said)
            if matches!(
                said.code.as_str(),
                "io" | "disconnected" | "no_answer" | "unreachable"
            ) =>
        {
            return Err(said);
        }
        Err(said) => {
            item.status = Status::Archived;
            item.note = Some(match outdated(&said.code) {
                // 装着但是调不到（施工 MG-17）：写清楚升级哪一个，不退回旧名字再试（迁移程序和包一起发新版）。
                Some((why, upgrade)) => format!(
                    "{} 笔赞助记录：{why}（{}），留在压缩包里；{upgrade}以后重跑迁移（重跑不会多出来）。",
                    entries.len(),
                    said.code
                ),
                // 插件拒的话里写着第几笔哪一格不对，没有名字和金额，照原样写。
                None => format!(
                    "{} 笔赞助记录没导进去（{}：{}），留在压缩包里。",
                    entries.len(),
                    said.code,
                    said.message
                ),
            });
        }
    }
    Ok(item)
}

/// 核心拒的代码 `code` 说的是「对方太旧」的：交回（怎么回事，升级哪一个）。别的原因是空的。
///
/// - `no_provider`：总线上没有 `sponsor`（插件是 I-211 以前的，或者停用了）；`unregistered`、`not_exported`：没有 `sponsor.import`。
/// - `unknown_method`：核心不认 `bus.call`（总线以前的核心）。
fn outdated(code: &str) -> Option<(String, String)> {
    match code {
        "no_provider" | "unregistered" | "not_exported" => Some((
            format!("总线上调不到赞助插件的 `{BUS_NAME}.import`，装着的插件太旧或者停用了"),
            format!("把赞助插件 `{SPONSOR}` 升级（停用了的先启用）"),
        )),
        "unknown_method" => Some((
            "这一版 Miyu 不认 `bus.call`，太旧".to_string(),
            "把 Miyu 升级".to_string(),
        )),
        _ => None,
    }
}

/// 会话库的 `sponsor_records` 一行一笔，照 `record_id` 排。没有这张表的是空的。
///
/// # Errors
///
/// 打不开、读不了。
pub fn read(path: &std::path::Path) -> rusqlite::Result<Vec<Value>> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    let exists = conn
        .prepare("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'sponsor_records'")?
        .exists([])?;
    if !exists {
        return Ok(Vec::new());
    }
    let mut statement = conn.prepare(
        "SELECT platform, sponsor_id, sponsor_name, amount_minor, currency, cny_minor, fx_rate, note, recorded_by, sponsored_at
         FROM sponsor_records ORDER BY record_id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(Row {
            platform: row.get(0)?,
            sponsor: row.get(1)?,
            name: row.get(2)?,
            amount: row.get(3)?,
            currency: row.get(4)?,
            converted: row.get(5)?,
            rate: row.get(6)?,
            note: row.get(7)?,
            by: row.get(8)?,
            at: row.get(9)?,
        })
    })?;
    rows.map(|row| row.map(|row| entry(&row))).collect()
}

/// 旧库的一行。
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// `platform`。
    pub platform: String,
    /// `sponsor_id`。
    pub sponsor: String,
    /// `sponsor_name`。
    pub name: String,
    /// `amount_minor`（分）。
    pub amount: i64,
    /// `currency`。
    pub currency: String,
    /// `cny_minor`（分）。
    pub converted: i64,
    /// `fx_rate`（同币种写 0）。
    pub rate: f64,
    /// `note`。
    pub note: String,
    /// `recorded_by`。
    pub by: String,
    /// `sponsored_at`（RFC 3339）。
    pub at: String,
}

/// 一行换成账本的一笔（im 设计 03 第五节第 6 条）。
pub fn entry(row: &Row) -> Value {
    let platform = match row.platform.as_str() {
        "onebot" | "" => "qq".to_string(),
        other => other.to_string(),
    };
    let numeric = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    let sponsor = row.sponsor.trim();
    let user = numeric(sponsor).then(|| format!("{platform}:{sponsor}"));
    let name = row.name.trim();
    let currency = row.currency.trim().to_uppercase();
    let rate = if currency == "CNY" {
        json!(1)
    } else if row.rate > 0.0 {
        json!(row.rate)
    } else {
        Value::Null
    };
    let at = row
        .at
        .trim()
        .parse::<jiff::Timestamp>()
        .map_or(0, |at| at.as_millisecond());
    let by = row.by.trim();
    let by = if by.is_empty() {
        Value::Null
    } else if numeric(by) {
        json!(format!("{platform}:{by}"))
    } else {
        json!(by)
    };
    let note = row.note.trim();
    json!({
        "platform": platform,
        "user": user,
        "name": if name.is_empty() { Value::Null } else { json!(name) },
        "amount": minor(row.amount),
        "currency": currency,
        "base": "CNY",
        "converted": minor(row.converted),
        "rate": rate,
        "at": at,
        "note": if note.is_empty() { Value::Null } else { json!(note) },
        "by": by,
        "venue": Value::Null,
    })
}

/// 分换成元：整的写整数，有零头的写两位小数。
fn minor(cents: i64) -> Value {
    if cents % 100 == 0 {
        json!(cents / 100)
    } else {
        json!((cents as f64) / 100.0)
    }
}

#[cfg(test)]
mod tests;

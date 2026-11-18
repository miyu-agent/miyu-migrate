//! 赞助记录（细则第十一节已定第 4 条，im 施工 I-131、im 设计 03 第五节）：旧版会话库的 `sponsor_records` 一行转成赞助插件账本的一笔，
//! 经 `bus.call {"name": "sponsor", "method": "import", "params": {"entries": […]}}` 导：照总线上的名字调（施工 MG-17，核心设计 67），
//! 核心拼成插件清单里导出的 `sponsor.import`。插件照（平台、人、时刻、金额、币种）去重，重导是空操作。插件没装的跳过、提示装；
//! 装着但是旧的（总线上没有这个名字）、核心是旧的（不认 `bus.call`）提示升级，不退回旧名字再试。名字、号是别人的：报告只写条数。

use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};

use crate::core::{Core, Said};
use crate::layout::Layout;
use crate::report::{Item, Status};
use crate::words::Language;
use crate::words::setup::Sponsors;

/// 赞助插件：`package.list` 里照它看装没装。
pub const SPONSOR: &str = "im-sponsor";

/// 赞助插件在总线上提供的名字（im 施工 I-211）：导的时候照它调，不写插件的包编号。
pub const BUS_NAME: &str = "sponsor";

/// 迁赞助记录：读 `copy` 的会话库，经 `core` 交赞助插件导。交回报告的一项。
///
/// # Errors
///
/// 连接断了。
pub fn migrate(core: &mut dyn Core, copy: &Layout, language: Language) -> Result<Item, Said> {
    let words = crate::words::setup::sponsors(language);
    let path = copy.conversation_db();
    let mut item = Item {
        what: "sponsors".to_string(),
        status: Status::Migrated,
        source: (words.source)(
            &path
                .strip_prefix(&copy.root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/"),
        ),
        format: words.format.to_string(),
        migrated: 0,
        skipped: Vec::new(),
        target: Some(format!("bus.call {BUS_NAME}.import")),
        note: None,
    };
    // 没有会话库、没有这张表、表是空的才是「没有」；库在却读不了的写原因，记录留在压缩包里。
    let all = match path.is_file().then(|| read(&path)) {
        Some(Ok(all)) if !all.is_empty() => all,
        None | Some(Ok(_)) => {
            item.status = Status::Skipped;
            item.note = Some(words.none.to_string());
            return Ok(item);
        }
        Some(Err(error)) => {
            item.status = Status::Archived;
            item.note = Some((words.unreadable)(&error.to_string()));
            return Ok(item);
        }
    };
    // 时刻读不懂的（`at` 是 null）不送：赞助插件见一笔不对整份拒收（审计 #140），这几笔留在压缩包里、报告写几笔，别的照导。
    let (entries, unreadable): (Vec<Value>, Vec<Value>) =
        all.into_iter().partition(|one| one["at"].is_i64());
    if entries.is_empty() {
        item.status = Status::Archived;
    } else {
        send(core, &entries, words, &mut item)?;
    }
    if !unreadable.is_empty() {
        if item.status == Status::Migrated {
            item.status = Status::Partial;
        }
        let notes: Vec<String> = item
            .note
            .take()
            .into_iter()
            .chain([(words.unreadable_times)(unreadable.len())])
            .collect();
        item.note = Some(language.sentences(&notes));
    }
    Ok(item)
}

/// 把 `entries` 经 `core` 交赞助插件导，结果写进 `item`。
///
/// # Errors
///
/// 连接断了。
fn send(
    core: &mut dyn Core,
    entries: &[Value],
    words: &Sponsors,
    item: &mut Item,
) -> Result<(), Said> {
    let installed = core
        .call("package.list", json!({}))?
        .get("packages")
        .and_then(Value::as_array)
        .is_some_and(|all| all.iter().any(|one| one["package"] == SPONSOR));
    if !installed {
        item.status = Status::Archived;
        item.note = Some((words.not_installed)(entries.len(), SPONSOR));
        return Ok(());
    }
    let params = json!({"name": BUS_NAME, "method": "import", "params": {"entries": entries}});
    match core.call("bus.call", params) {
        Ok(answer) => {
            let added = answer["added"].as_u64().unwrap_or_default();
            let skipped = answer["skipped"].as_u64().unwrap_or_default();
            item.migrated = added + skipped;
            item.note = Some((words.imported)(entries.len(), added, skipped));
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
            item.note = Some(match outdated(&said.code, words) {
                // 装着但是调不到（施工 MG-17）：写清楚升级哪一个，不退回旧名字再试（迁移程序和包一起发新版）。
                Some((why, upgrade)) => (words.outdated)(entries.len(), &why, &said.code, &upgrade),
                // 插件拒的话里写着第几笔哪一格不对，没有名字和金额，照原样写。
                None => (words.refused)(entries.len(), &said.code, &said.message),
            });
        }
    }
    Ok(())
}

/// 核心拒的代码 `code` 说的是「对方太旧」的：交回（怎么回事，升级哪一个）。别的原因是空的。
///
/// - `no_provider`：总线上没有 `sponsor`（插件是 I-211 以前的，或者停用了）；`unregistered`、`not_exported`：没有 `sponsor.import`。
/// - `unknown_method`：核心不认 `bus.call`（总线以前的核心）。
fn outdated(code: &str, words: &Sponsors) -> Option<(String, String)> {
    match code {
        "no_provider" | "unregistered" | "not_exported" => Some((
            (words.no_bus_name)(BUS_NAME),
            (words.upgrade_plugin)(SPONSOR),
        )),
        "unknown_method" => Some((words.old_core.to_string(), words.upgrade_core.to_string())),
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
    /// `sponsored_at`（RFC 3339；没写时区的当 UTC，见 [`entry`]）。
    pub at: String,
}

/// 一行换成账本的一笔（im 设计 03 第五节第 6 条）。时刻读不懂的 `at` 写 null（[`millis`]），迁移不送这一笔。
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
    let at = millis(&row.at).map_or(Value::Null, |at| json!(at));
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
        // 外币没折算的（旧版写 `cny_minor = 0`、`fx_rate = 0`）是「未换算」，不是 0 元。
        "converted": if currency != "CNY" && row.converted == 0 && row.rate <= 0.0 {
            Value::Null
        } else {
            minor(row.converted)
        },
        "rate": rate,
        "at": at,
        "note": if note.is_empty() { Value::Null } else { json!(note) },
        "by": by,
        "venue": Value::Null,
    })
}

/// 旧库的时刻换成 1970 年起的毫秒：RFC 3339；没写时区的 `YYYY-MM-DD HH:MM:SS`（`T` 隔开也行）当 UTC。读不懂的、
/// 不在 1970 年以后的（赞助插件不收）交回空的。
pub fn millis(text: &str) -> Option<i64> {
    let text = text.trim();
    let at = text.parse::<jiff::Timestamp>().ok().or_else(|| {
        let civil = text.parse::<jiff::civil::DateTime>().ok()?;
        Some(civil.to_zoned(jiff::tz::TimeZone::UTC).ok()?.timestamp())
    })?;
    Some(at.as_millisecond()).filter(|at| *at > 0)
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

//! 赞助记录：一行怎么换成一笔（平台、号或名字、分换成元、汇率同币种写 1、时刻毫秒、记账的人、备注）；照会话库读；插件装着的导、
//! 没装的提示装、拒了的写原话；没有记录的不问核心。

use rusqlite::{Connection, params};
use serde_json::{Value, json};

use super::*;

fn row() -> Row {
    Row {
        platform: "onebot".to_string(),
        sponsor: "10001".to_string(),
        name: "小李".to_string(),
        amount: 1050,
        currency: "usd".to_string(),
        converted: 7600,
        rate: 7.238,
        note: " ".to_string(),
        by: "999".to_string(),
        at: "2026-09-25T07:00:00.123456789+00:00".to_string(),
    }
}

#[test]
fn a_row_becomes_a_ledger_entry() {
    assert_eq!(
        entry(&row()),
        json!({
            "platform": "qq", "user": "qq:10001", "name": "小李", "amount": 10.5, "currency": "USD",
            "base": "CNY", "converted": 76, "rate": 7.238, "at": 1_790_319_600_123i64, "note": null,
            "by": "qq:999", "venue": null,
        })
    );
    let mut cny = row();
    cny.currency = "CNY".to_string();
    cny.rate = 0.0;
    cny.sponsor = "某个老板".to_string();
    cny.by = String::new();
    cny.note = "感谢".to_string();
    let entry = entry(&cny);
    assert_eq!(entry["rate"], 1, "同币种写 1（旧库写 0）");
    assert_eq!(entry["user"], Value::Null, "不是号的只有名字");
    assert_eq!(entry["by"], Value::Null);
    assert_eq!(entry["note"], "感谢");
    let mut unknown = row();
    unknown.rate = 0.0;
    assert_eq!(
        super::entry(&unknown)["rate"],
        Value::Null,
        "不同币种、汇率是 0 的写 null"
    );
}

fn database(dir: &std::path::Path, rows: usize) -> std::path::PathBuf {
    let path = dir.join("conversation.db");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(
        "CREATE TABLE sponsor_records (record_id INTEGER PRIMARY KEY, platform TEXT NOT NULL DEFAULT '', account_id TEXT NOT NULL DEFAULT '',
             sponsor_id TEXT NOT NULL, sponsor_name TEXT NOT NULL DEFAULT '', amount_minor INTEGER NOT NULL, currency TEXT NOT NULL,
             cny_minor INTEGER NOT NULL, fx_rate REAL NOT NULL DEFAULT 0, fx_source TEXT NOT NULL DEFAULT '', note TEXT NOT NULL DEFAULT '',
             recorded_by TEXT NOT NULL DEFAULT '', sponsored_at TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);",
    )
    .unwrap();
    for n in 0..rows {
        conn.execute(
            "INSERT INTO sponsor_records (platform, sponsor_id, sponsor_name, amount_minor, currency, cny_minor, sponsored_at, created_at, updated_at)
             VALUES ('onebot', ?1, '某人', 500, 'CNY', 500, '2026-09-25T07:00:00Z', '', '')",
            params![format!("{}", 10000 + n)],
        )
        .unwrap();
    }
    path
}

#[test]
fn the_conversation_database_is_read_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let entries = read(&database(dir.path(), 2)).unwrap();
    let users: Vec<&Value> = entries.iter().map(|one| &one["user"]).collect();
    assert_eq!(users, vec![&json!("qq:10000"), &json!("qq:10001")]);
    let bare = dir.path().join("bare.db");
    Connection::open(&bare)
        .unwrap()
        .execute_batch("CREATE TABLE x (a);")
        .unwrap();
    assert!(read(&bare).unwrap().is_empty(), "没有这张表的是空的");
}

/// 假核心：`package.list` 里有没有赞助插件照 `installed`；`bus.call` 照 `answer` 答或者拒。
struct Fake {
    installed: bool,
    answer: Result<Value, &'static str>,
    calls: Vec<(String, Value)>,
}

impl Core for Fake {
    fn call(&mut self, method: &str, params: Value) -> Result<Value, Said> {
        self.calls.push((method.to_string(), params));
        match method {
            "package.list" => {
                let packages = if self.installed {
                    json!([{"package": SPONSOR}])
                } else {
                    json!([])
                };
                Ok(json!({"packages": packages}))
            }
            "bus.call" => self.answer.clone().map_err(|code| Said {
                code: code.to_string(),
                message: "entries[1]: amount must be positive".to_string(),
            }),
            other => panic!("没想到会调 {other}"),
        }
    }
}

fn old_root(rows: usize) -> (tempfile::TempDir, Layout) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".home-layout-v1"), "owner\n").unwrap();
    std::fs::create_dir_all(dir.path().join("home/owner")).unwrap();
    database(&dir.path().join("home/owner"), rows);
    let layout = crate::layout::read(dir.path()).unwrap();
    (dir, layout)
}

#[test]
fn the_plugin_takes_the_entries_or_the_report_says_why_not() {
    let (_dir, layout) = old_root(3);
    let mut core = Fake {
        installed: true,
        answer: Ok(json!({"added": 1, "skipped": 2})),
        calls: Vec::new(),
    };
    let item = migrate(&mut core, &layout, Language::Zh).unwrap();
    assert_eq!(item.migrated, 3);
    assert!(
        item.note
            .as_deref()
            .unwrap()
            .contains("新加 1 笔，2 笔账本里本来就有")
    );
    let (method, params) = &core.calls[1];
    // 施工 MG-17：照总线上的名字调，核心拼成插件清单里导出的 `sponsor.import`；不写插件的包编号。
    assert_eq!(method, "bus.call");
    assert_eq!(params["name"], "sponsor");
    assert_eq!(params["method"], "import");
    assert!(params.get("package").is_none(), "{params}");
    assert_eq!(item.target.as_deref(), Some("bus.call sponsor.import"));
    assert_eq!(params["params"]["entries"].as_array().unwrap().len(), 3);

    let mut missing = Fake {
        installed: false,
        answer: Ok(json!({})),
        calls: Vec::new(),
    };
    let item = migrate(&mut missing, &layout, Language::Zh).unwrap();
    assert_eq!(item.status, Status::Archived);
    assert_eq!(missing.calls.len(), 1, "没装的不调");
    assert!(item.note.unwrap().contains("没装"));

    let mut refused = Fake {
        installed: true,
        answer: Err("bad_params"),
        calls: Vec::new(),
    };
    let item = migrate(&mut refused, &layout, Language::Zh).unwrap();
    assert_eq!(item.status, Status::Archived);
    assert!(
        item.note
            .unwrap()
            .contains("bad_params：entries[1]: amount must be positive")
    );
}

#[test]
fn an_old_plugin_or_an_old_core_is_told_apart_from_a_refusal() {
    // 施工 MG-17：插件装着但是旧的（总线上没有 `sponsor`、没登记 `sponsor.import`）、核心是旧的（不认 `bus.call`），报告写清楚
    // 升级哪一个；不退回旧名字再试。
    let (_dir, layout) = old_root(2);
    for (code, upgrade) in [
        ("no_provider", "把赞助插件 `im-sponsor` 升级"),
        ("unregistered", "把赞助插件 `im-sponsor` 升级"),
        ("not_exported", "把赞助插件 `im-sponsor` 升级"),
        ("unknown_method", "把 Miyu 升级"),
    ] {
        let mut old = Fake {
            installed: true,
            answer: Err(code),
            calls: Vec::new(),
        };
        let item = migrate(&mut old, &layout, Language::Zh).unwrap();
        assert_eq!(item.status, Status::Archived, "{code}");
        assert_eq!(item.migrated, 0, "{code}");
        let note = item.note.unwrap();
        assert!(note.starts_with("2 笔赞助记录"), "{code}：{note}");
        assert!(note.contains(upgrade), "{code}：{note}");
        assert!(note.contains("重跑迁移"), "{code}：{note}");
        assert!(note.contains(code), "{code}：{note}");
        assert_eq!(old.calls.len(), 2, "{code}：问一次就停，不退回旧名字再试");
    }
}

#[test]
fn no_records_no_questions() {
    let (_dir, layout) = old_root(0);
    let mut core = Fake {
        installed: true,
        answer: Ok(json!({})),
        calls: Vec::new(),
    };
    let item = migrate(&mut core, &layout, Language::Zh).unwrap();
    assert_eq!(item.status, Status::Skipped);
    assert!(core.calls.is_empty());
}

/// 审计 #140：读会话库出错（坏了、读不了）也被外层 `_ =>` 吞成「旧版没有赞助记录」，主人察觉不到账本没迁。读不了要写原因，
/// 不能报成没有。
#[test]
fn audit_140_a_broken_conversation_database_is_not_reported_as_no_records() {
    let (dir, layout) = old_root(2);
    std::fs::write(
        dir.path().join("home/owner/conversation.db"),
        b"this is not a sqlite database at all, just garbage bytes ".repeat(64),
    )
    .unwrap();
    assert!(read(&layout.conversation_db()).is_err(), "坏库读不了");
    let mut core = Fake {
        installed: true,
        answer: Ok(json!({"added": 0, "skipped": 0})),
        calls: Vec::new(),
    };
    let item = migrate(&mut core, &layout, Language::Zh).unwrap();
    let none = crate::words::setup::sponsors(Language::Zh).none;
    assert_ne!(item.status, Status::Skipped, "读不了不是「没有」：{item:?}");
    assert_ne!(item.note.as_deref(), Some(none), "报告不能写「{none}」");
}

/// 审计 #140：旧版外币没给折算时写 `cny_minor = 0, fx_rate = 0`；迁过去写成 `converted: 0`，赞助插件当作 0 元计进合计，
/// 而不是「未换算」。应写 null。
#[test]
fn audit_140_an_unconverted_foreign_amount_is_not_zero_yuan() {
    let mut unconverted = row();
    unconverted.currency = "USD".to_string();
    unconverted.converted = 0;
    unconverted.rate = 0.0;
    let entry = entry(&unconverted);
    assert!(
        entry["converted"].is_null(),
        "没折算的外币应写 null，不是 0 元：{entry}"
    );
}

/// 往 `layout` 的会话库里加几笔，时刻照 `times` 写。
fn add_with_times(layout: &Layout, times: &[&str]) {
    let conn = Connection::open(layout.conversation_db()).unwrap();
    for at in times {
        conn.execute(
            "INSERT INTO sponsor_records (platform, sponsor_id, sponsor_name, amount_minor, currency, cny_minor, sponsored_at, created_at, updated_at)
             VALUES ('onebot', '20000', '某人', 500, 'CNY', 500, ?1, '', '')",
            params![at],
        )
        .unwrap();
    }
}

/// 审计 #140：`sponsored_at` 读不懂的一笔原来写成 `at: 0`，赞助插件见 `at` 不大于 0 整份拒收，一笔坏、整本账不导。
/// 无时区的 `YYYY-MM-DD HH:MM:SS` 当 UTC 读；还是读不懂的只跳过那一笔、报告里写几笔，别的照导。
#[test]
fn audit_140_an_unreadable_time_skips_only_that_record() {
    let (_dir, layout) = old_root(1);
    add_with_times(&layout, &["2026-09-25 07:00:00", "下午三点"]);
    for (language, said) in [
        (Language::Zh, "1 笔时间读不懂"),
        (Language::En, "unreadable time"),
    ] {
        let mut core = Fake {
            installed: true,
            answer: Ok(json!({"added": 2, "skipped": 0})),
            calls: Vec::new(),
        };
        let item = migrate(&mut core, &layout, language).unwrap();
        let (_, params) = &core.calls[1];
        let at: Vec<&Value> = params["params"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|one| &one["at"])
            .collect();
        assert_eq!(
            at,
            [&json!(1_790_319_600_000i64), &json!(1_790_319_600_000i64)],
            "好的、空格格式的照导，读不懂的不送"
        );
        assert_eq!(item.status, Status::Partial, "{item:?}");
        let note = item.note.unwrap_or_default();
        assert!(note.contains(said), "{language:?}：{note}");
    }

    let (_dir, only_bad) = old_root(0);
    add_with_times(&only_bad, &["not a time"]);
    let mut core = Fake {
        installed: true,
        answer: Ok(json!({"added": 0, "skipped": 0})),
        calls: Vec::new(),
    };
    let item = migrate(&mut core, &only_bad, Language::Zh).unwrap();
    assert_ne!(
        item.status,
        Status::Skipped,
        "有一笔，不是「没有」：{item:?}"
    );
    assert!(
        item.note
            .as_deref()
            .unwrap_or_default()
            .contains("1 笔时间读不懂"),
        "{item:?}"
    );
}

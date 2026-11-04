//! 会话迁进真核心（进程里起）：旧会话库里本机的会话导成存档，`session.list` 里是 `archived`，`view.page` 读得回字、工具、
//! 附件；一次性的、成员的、绑了场所的、子代理的不导、报告里数出来；再跑一遍全是 `present`，什么都不多。

#![allow(clippy::unwrap_used, reason = "测试：读坏了就该停")]

mod support;

use std::path::Path;

use rusqlite::{Connection, params};
use serde_json::{Value, json};

use miyu_migrate::core::Core;
use miyu_migrate::run::{self, Plan};
use miyu_migrate::sessions::ids;
use support::{connect, fresh};

/// 一张 2×3 的 PNG 的头（认大小只读到 IHDR）。
fn png() -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    bytes.extend_from_slice(&2u32.to_be_bytes());
    bytes.extend_from_slice(&3u32.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

/// 旧根（家目录布局）：会话库里六个会话，只有 `sess_local` 该导。
fn old_root() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".home-layout-v1"), "owner\n").unwrap();
    std::fs::create_dir_all(dir.path().join("home/owner")).unwrap();
    let db = Connection::open(dir.path().join("home/owner/conversation.db")).unwrap();
    db.execute_batch(
        "PRAGMA user_version = 40;
         CREATE TABLE sessions (session_id TEXT PRIMARY KEY, persona TEXT NOT NULL, name TEXT NOT NULL,
             kind TEXT NOT NULL DEFAULT 'user', parent_session_id TEXT, created_at TEXT NOT NULL,
             updated_at TEXT NOT NULL, owner TEXT NOT NULL DEFAULT '');
         CREATE TABLE turns (turn_id TEXT PRIMARY KEY, session_id TEXT NOT NULL, seq INTEGER NOT NULL,
             user_content TEXT NOT NULL, user_timestamp TEXT NOT NULL, assistant_content TEXT NOT NULL,
             assistant_reasoning TEXT, assistant_timestamp TEXT, status TEXT NOT NULL, hidden INTEGER NOT NULL DEFAULT 0,
             is_summary INTEGER NOT NULL DEFAULT 0, assistant_provider_id TEXT, assistant_model TEXT,
             display_content TEXT NOT NULL DEFAULT '', tool_flow TEXT, replay_journal TEXT);
         CREATE TABLE user_attachments (attachment_id TEXT PRIMARY KEY, session_id TEXT NOT NULL, turn_id TEXT,
             file_name TEXT NOT NULL, mime TEXT NOT NULL, kind TEXT NOT NULL, data BLOB, created_at TEXT NOT NULL);
         CREATE TABLE platform_session_bindings (session_id TEXT);",
    )
    .unwrap();
    let at = "2026-09-25T07:00:00.123456+00:00";
    for (id, kind, owner, name) in [
        ("sess_local", "user", "", "看猫"),
        ("sess_ask", "ask", "", ""),
        ("sess_member", "user", "acct_1", ""),
        ("sess_qq", "user", "", ""),
        ("sess_child", "subagent", "", ""),
    ] {
        db.execute(
            "INSERT INTO sessions VALUES (?1, '', ?2, ?3, NULL, ?4, ?4, ?5)",
            params![id, name, kind, at, owner],
        )
        .unwrap();
    }
    db.execute(
        "INSERT INTO platform_session_bindings VALUES ('sess_qq')",
        [],
    )
    .unwrap();
    let flow = json!([{"assistant_content": "我看看", "calls": [
        {"id": "c", "name": "read", "arguments": "{\"path\":\"/x\"}", "output": "一只猫", "started_ms": 1u64, "finished_ms": 5u64}
    ]}])
    .to_string();
    let turns: [(&str, i64, &str, i64, Option<&str>); 4] = [
        ("turn_1", 1, "completed", 0, Some(flow.as_str())),
        ("summary_1", 2, "completed", 0, None),
        ("turn_2", 3, "interrupted", 1, None),
        ("turn_3", 4, "running", 0, None),
    ];
    for (id, seq, status, hidden, flow) in turns {
        db.execute(
            "INSERT INTO turns VALUES (?1, 'sess_local', ?2, ?3, ?4, ?5, NULL, ?4, ?6, ?7, ?8, 'deepseek', 'chat', '', ?9, NULL)",
            params![
                id,
                seq,
                format!("{id} 说"),
                "2026-09-25T07:01:00+00:00",
                format!("{id} 答"),
                status,
                hidden,
                i64::from(id.starts_with("summary")),
                flow
            ],
        )
        .unwrap();
    }
    db.execute(
        "INSERT INTO user_attachments VALUES ('att_a', 'sess_local', 'turn_1', '猫.png', 'image/png', 'image', ?1, ?2)",
        params![png(), at],
    )
    .unwrap();
    db.execute(
        "INSERT INTO user_attachments VALUES ('att_b', 'sess_local', 'turn_1', 'a.txt', 'text/plain', 'text', NULL, ?1)",
        params![at],
    )
    .unwrap();
    let file = dir.path().join("state/attachments/att_b");
    std::fs::create_dir_all(&file).unwrap();
    std::fs::write(file.join("a.txt"), "附件的字").unwrap();
    dir
}

fn agent(done: &run::Done) -> Value {
    serde_json::from_str(&std::fs::read_to_string(&done.agent).unwrap()).unwrap()
}

fn plan(fresh: &support::Fresh, old: &Path, at: &str) -> Plan {
    plan_in(
        fresh,
        old,
        at,
        fresh.root.path().join("var/lib/miyu-migrate"),
    )
}

/// 同 [`plan`]，状态目录是 `state`。
fn plan_in(fresh: &support::Fresh, old: &Path, at: &str, state: std::path::PathBuf) -> Plan {
    Plan {
        old: old.to_path_buf(),
        state,
        at: at.parse().unwrap(),
        root: fresh.root.clone(),
        space: run::free_space,
    }
}

fn sessions_item(agent: &Value) -> Value {
    agent["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["what"] == "sessions")
        .unwrap()
        .clone()
}

#[test]
fn local_sessions_become_archives_once() {
    let fresh = fresh();
    let old = old_root();
    let mut core = connect(&fresh.root);
    let first =
        agent(&run::run(&plan(&fresh, old.path(), "2026-11-04T08:00:00Z"), &mut core).unwrap());
    let item = sessions_item(&first);
    assert_eq!(item["migrated"], 1, "{item}");
    let skipped: Vec<(String, String)> = item["skipped"]
        .as_array()
        .unwrap()
        .iter()
        .map(|skip| {
            (
                skip["key"].as_str().unwrap().to_string(),
                skip["why"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    // 创建时刻一样的照旧编号排。
    assert_eq!(
        skipped,
        vec![
            ("sess_ask".to_string(), "oneshot".to_string()),
            ("sess_child".to_string(), "subagent-later".to_string()),
            ("sess_member".to_string(), "member".to_string()),
            ("sess_qq".to_string(), "venue-later".to_string()),
        ]
    );
    let created = ids::parse("2026-09-25T07:00:00.123456+00:00")
        .unwrap()
        .as_millisecond() as u64;
    let id = ids::session_id(created, "home/owner/conversation.db", "sess_local");

    let listed = core.call("session.list", json!({})).unwrap();
    let entry = listed["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["session"] == id.as_str())
        .unwrap_or_else(|| panic!("{listed}"))
        .clone();
    assert_eq!(entry["archived"], true);
    assert_eq!(entry["title"], "看猫");
    assert_eq!(
        listed["sessions"].as_array().unwrap().len(),
        1,
        "临时会话删了"
    );

    let page = core
        .call("view.page", json!({"session": id, "turns": 50}))
        .unwrap();
    let events = page["events"].as_array().unwrap();
    let said: Vec<&str> = events
        .iter()
        .filter(|event| event["kind"] == "message.user")
        .map(|event| event["body"]["blocks"][0]["text"].as_str().unwrap())
        .collect();
    assert_eq!(
        said,
        vec!["turn_1 说", "turn_2 说", "turn_3 说"],
        "摘要行不导，隐藏的照导"
    );
    let first_said = events
        .iter()
        .find(|event| event["kind"] == "message.user")
        .unwrap();
    let blocks = first_said["body"]["blocks"].as_array().unwrap();
    assert_eq!(blocks[1]["type"], "image");
    assert_eq!(
        (blocks[1]["width"].clone(), blocks[1]["height"].clone()),
        (json!(2), json!(3))
    );
    assert_eq!(blocks[2]["type"], "file");
    let file = core
        .call("blob.get", json!({"blob": blocks[2]["blob"]}))
        .map(|got| got.to_string())
        .unwrap_or_default();
    assert!(!file.is_empty(), "附件的 blob 进了核心");
    let result = events
        .iter()
        .find(|event| event["kind"] == "tool.result")
        .unwrap();
    assert_eq!(result["body"]["blocks"][0]["text"], "一只猫");
    let ended: Vec<&str> = events
        .iter()
        .filter(|event| event["kind"] == "turn.ended")
        .map(|event| event["body"]["reason"].as_str().unwrap())
        .collect();
    assert_eq!(ended, vec!["completed", "interrupted", "interrupted"]);
    let refused = core.call("session.send", json!({"session": id, "text": "还在吗"}));
    assert_eq!(refused.unwrap_err().code, "archived");

    let second =
        agent(&run::run(&plan(&fresh, old.path(), "2026-11-04T08:00:01Z"), &mut core).unwrap());
    let item = sessions_item(&second);
    assert_eq!(item["migrated"], 1);
    assert!(
        item["note"]
            .as_str()
            .unwrap()
            .contains("其中 1 个会话以前就导过"),
        "{item}"
    );
    let again = core.call("session.list", json!({})).unwrap();
    assert_eq!(again["sessions"].as_array().unwrap().len(), 1, "什么都没多");
}

/// 核心建日志目录、blob 目录只认数据根里面的：状态目录在数据根外面的（人自己指了别处）照样导得进。
#[test]
fn a_state_folder_outside_the_data_root_still_imports() {
    let fresh = fresh();
    let old = old_root();
    let outside = tempfile::tempdir().unwrap();
    let mut core = connect(&fresh.root);
    let plan = plan_in(
        &fresh,
        old.path(),
        "2026-11-04T08:00:00Z",
        outside.path().join("state"),
    );
    let done = agent(&run::run(&plan, &mut core).unwrap());
    let item = sessions_item(&done);
    assert_eq!(item["migrated"], 1, "{item}");
    assert_eq!(item["skipped"].as_array().unwrap().len(), 4, "{item}");
}

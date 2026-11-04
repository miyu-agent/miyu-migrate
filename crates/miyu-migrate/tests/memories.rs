//! 记忆迁进真核心（进程里起）：旧 `default` 的知识点导进新建的 Miyu，别人的写成 `person`、没写是谁的照推荐的类，原来的时刻留着；
//! 超长的、自己的不导；再跑一遍照 `origin` 全跳过。

#![allow(clippy::unwrap_used, reason = "测试：读坏了就该停")]

mod support;

use rusqlite::{Connection, params};
use serde_json::{Value, json};

use miyu_migrate::core::Core;
use miyu_migrate::run::{self, Plan};
use support::{connect, fresh};

fn old_root() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".home-layout-v1"), "owner\n").unwrap();
    std::fs::create_dir_all(dir.path().join("home/owner")).unwrap();
    std::fs::write(dir.path().join("home/owner/conversation.db"), "").unwrap();
    let memory = dir.path().join("personas/default/memory");
    std::fs::create_dir_all(&memory).unwrap();
    let conn = Connection::open(memory.join("memory.db")).unwrap();
    conn.execute_batch(
        "CREATE TABLE facts (id INTEGER PRIMARY KEY, content TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'active',
             updated_at TEXT NOT NULL, owner_principal TEXT NOT NULL DEFAULT '', subjects TEXT NOT NULL DEFAULT '[]',
             memory_type TEXT NOT NULL DEFAULT 'fact', tags TEXT NOT NULL DEFAULT '[]');
         CREATE TABLE episodes (id INTEGER PRIMARY KEY, owner_principal TEXT NOT NULL DEFAULT '',
             origin_platform TEXT NOT NULL DEFAULT '', origin_sender_id TEXT NOT NULL DEFAULT '');
         INSERT INTO episodes (owner_principal, origin_platform, origin_sender_id) VALUES ('p:a', 'onebot', '111');",
    )
    .unwrap();
    for (content, kind, subjects, tags) in [
        (
            "小李喜欢猫",
            "preference",
            r#"[{"principal":"p:a","name":"小李"}]"#,
            r#"["猫"]"#,
        ),
        ("Arch 装 NVIDIA 驱动要选 dkms 版", "fact", "[]", "[]"),
        ("回答要短", "preference", "[]", "[]"),
        ("我是 Miyu", "self", "[]", "[]"),
    ] {
        conn.execute(
            "INSERT INTO facts (content, updated_at, memory_type, subjects, tags) VALUES (?1, '2026-09-20T03:00:00+00:00', ?2, ?3, ?4)",
            params![content, kind, subjects, tags],
        )
        .unwrap();
    }
    conn.execute(
        "INSERT INTO facts (content, updated_at) VALUES (?1, '2026-09-20T03:00:00+00:00')",
        params!["很长".repeat(80)],
    )
    .unwrap();
    dir
}

fn memories(agent: &Value) -> Value {
    agent["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["what"] == "memories")
        .unwrap()
        .clone()
}

#[test]
fn facts_land_in_the_mapped_persona_once() {
    let fresh = fresh();
    let old = old_root();
    let mut core = connect(&fresh.root);
    let plan = Plan {
        old: old.path().to_path_buf(),
        state: fresh.root.path().join("var/lib/miyu-migrate"),
        at: "2026-11-04T08:00:00Z".parse().unwrap(),
        root: fresh.root.clone(),
        records: miyu_migrate::records::Importer::new(|_| Err(std::io::Error::other("no im"))),
        home: None,
        space: run::free_space,
    };
    let done = run::run(&plan, &mut core).unwrap();
    let agent: Value = serde_json::from_str(&std::fs::read_to_string(done.agent).unwrap()).unwrap();
    let item = memories(&agent);
    assert_eq!(item["migrated"], 3, "{item}");
    let whys: Vec<&str> = item["skipped"]
        .as_array()
        .unwrap()
        .iter()
        .map(|skip| skip["why"].as_str().unwrap())
        .collect();
    assert_eq!(whys, vec!["self", "too-long"]);
    assert_eq!(agent["pending"][0]["what"], "memory-too-long");
    assert_eq!(agent["pending"][0]["key"], "5");
    let miyu = agent["personas"][0]["new"].as_str().unwrap().to_string();

    let listed = core
        .call("memory.list", json!({"persona": miyu, "limit": 50}))
        .unwrap();
    let mut got: Vec<(String, String, String)> = listed["memories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| {
            (
                one["text"].as_str().unwrap().to_string(),
                one["class"].as_str().unwrap().to_string(),
                one["at"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    got.sort();
    let texts: Vec<(&str, &str)> = got
        .iter()
        .map(|(text, class, _)| (text.as_str(), class.as_str()))
        .collect();
    assert_eq!(
        texts,
        vec![
            ("Arch 装 NVIDIA 驱动要选 dkms 版", "reference"),
            ("回答要短", "feedback"),
            ("小李喜欢猫", "person"),
        ]
    );
    assert!(
        got.iter().all(|(_, _, at)| at.starts_with("2026-09-20")),
        "原来的时刻：{got:?}"
    );

    let again = run::run(
        &Plan {
            at: "2026-11-04T08:00:01Z".parse().unwrap(),
            ..plan
        },
        &mut core,
    )
    .unwrap();
    let agent: Value =
        serde_json::from_str(&std::fs::read_to_string(again.agent).unwrap()).unwrap();
    let item = memories(&agent);
    assert_eq!(item["migrated"], 0);
    assert!(
        item["note"].as_str().unwrap().contains("以前导过 3 条"),
        "{item}"
    );
}

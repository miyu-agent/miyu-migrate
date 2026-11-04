//! 记忆：判类（主人的、别人的、没写是谁的、自己的、超长的、不在用的）、身份哈希换号、关键词；照批导、整理忙着就等、核心拒了停下；
//! 主人的号先看新版配置、再看旧配置。

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};

use rusqlite::{Connection, params};
use serde_json::{Value, json};

use super::*;

fn raw<'a>(kind: &'a str, content: &'a str, subjects: &'a str) -> Raw<'a> {
    Raw {
        origin: "miyu-legacy:default:facts:1",
        content,
        kind,
        status: "active",
        updated: "2026-09-25T07:00:00.123456789+00:00",
        tags: r#"["猫", "", "狗"]"#,
        subjects,
        owner: "p:a",
    }
}

fn item(judged: &Judged) -> &Value {
    match judged {
        Judged::Import { item, .. } => item,
        Judged::Skip(why, _) => panic!("没导：{why}"),
    }
}

#[test]
fn facts_are_judged_by_who_they_are_about() {
    let principals = HashMap::from([
        ("p:a".to_string(), "qq:111".to_string()),
        ("p:owner".to_string(), "qq:999".to_string()),
    ]);
    let owners = HashSet::from(["qq:999".to_string()]);
    let other = judge(
        &raw(
            "fact",
            " 喜欢猫 ",
            r#"[{"principal":"p:b","name":"乙"},{"principal":"p:a","name":"甲"}]"#,
        ),
        &principals,
        &owners,
    );
    assert_eq!(
        item(&other),
        &json!({"at": "2026-09-25T07:00:00.123456789Z", "class": "person", "who": "qq:111", "text": "喜欢猫", "origin": "miyu-legacy:default:facts:1", "keywords": "猫, 狗"}),
        "取和这一条归属相同的那个人"
    );
    let owner = judge(
        &raw("preference", "主人爱喝茶", r#"[{"principal":"p:owner"}]"#),
        &principals,
        &owners,
    );
    assert_eq!(item(&owner)["class"], "user");
    assert!(item(&owner).get("who").is_none());
    let named = judge(
        &raw(
            "fact",
            "丙会画画",
            r#"[{"principal":"p:gone","name":"丙"}]"#,
        ),
        &principals,
        &owners,
    );
    assert_eq!(item(&named)["who"], "丙");
    assert!(matches!(named, Judged::Import { by_name: true, .. }));
    for (kind, class) in [
        ("fact", "reference"),
        ("relationship", "reference"),
        ("preference", "feedback"),
        ("task", "episode"),
    ] {
        let alone = judge(&raw(kind, "没写是谁", "[]"), &principals, &owners);
        assert_eq!(item(&alone)["class"], class, "{kind}");
        assert!(
            matches!(
                alone,
                Judged::Import {
                    guessed: Some(_),
                    ..
                }
            ),
            "{kind}"
        );
    }
    let nameless = judge(
        &raw("fact", "谁也不知道", r#"[{"principal":"p:gone"}]"#),
        &principals,
        &owners,
    );
    assert_eq!(
        item(&nameless)["class"],
        "reference",
        "换不出、也没有名字的降成 reference"
    );
}

#[test]
fn some_facts_are_not_imported() {
    let none = HashMap::new();
    let owners = HashSet::new();
    assert_eq!(
        judge(&raw("self", "我是 Miyu", "[]"), &none, &owners),
        Judged::Skip("self", "self".to_string())
    );
    let long = "长".repeat(read::TEXT_CHARS + 1);
    assert_eq!(
        judge(&raw("fact", &long, "[]"), &none, &owners),
        Judged::Skip("too-long", "fact".to_string())
    );
    let edge = "长".repeat(read::TEXT_CHARS);
    assert!(matches!(
        judge(&raw("fact", &edge, "[]"), &none, &owners),
        Judged::Import { .. }
    ));
    let mut forgotten = raw("fact", "忘了", "[]");
    forgotten.status = "forgotten";
    assert_eq!(
        judge(&forgotten, &none, &owners),
        Judged::Skip("inactive", "fact".to_string())
    );
    let mut bad = raw("fact", "怪时刻", "[]");
    bad.updated = "昨天";
    assert_eq!(
        judge(&bad, &none, &owners),
        Judged::Skip("bad-time", "fact".to_string())
    );
}

#[test]
fn the_database_maps_principals_through_episodes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("memory.db");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(
        "CREATE TABLE facts (id INTEGER PRIMARY KEY, content TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'active',
             updated_at TEXT NOT NULL, owner_principal TEXT NOT NULL DEFAULT '', subjects TEXT NOT NULL DEFAULT '[]',
             memory_type TEXT NOT NULL DEFAULT 'fact', tags TEXT NOT NULL DEFAULT '[]');
         CREATE TABLE episodes (id INTEGER PRIMARY KEY, owner_principal TEXT NOT NULL DEFAULT '',
             origin_platform TEXT NOT NULL DEFAULT '', origin_sender_id TEXT NOT NULL DEFAULT '');
         INSERT INTO episodes (owner_principal, origin_platform, origin_sender_id) VALUES
             ('p:a', 'onebot', '111'), ('p:a', 'onebot', '111'), ('p:two', 'onebot', '1'), ('p:two', 'onebot', '2'),
             ('p:local', 'local', '5');",
    )
    .unwrap();
    for (content, subjects) in [
        ("甲的事", r#"[{"principal":"p:a","name":"甲"}]"#),
        ("两个号的", r#"[{"principal":"p:two","name":"双"}]"#),
    ] {
        conn.execute(
            "INSERT INTO facts (content, updated_at, subjects) VALUES (?1, '2026-09-25T07:00:00Z', ?2)",
            params![content, subjects],
        )
        .unwrap();
    }
    let facts = read(&path, "default", &HashSet::new()).unwrap();
    assert_eq!(facts[0].origin, "miyu-legacy:default:facts:1");
    assert_eq!(item(&facts[0].judged)["who"], "qq:111");
    assert_eq!(
        item(&facts[1].judged)["who"],
        "双",
        "一个哈希换出两个号的当换不出"
    );
    assert!(
        item(&facts[1].judged).get("keywords").is_none(),
        "没有标签的不写"
    );
}

/// 假核心：`memory.import` 照交来的几条都算导进；`memory.dream` 前 `busy` 次回忙着；`config.get` 照 `config` 答。
struct Fake {
    busy: u32,
    refuse: Option<&'static str>,
    config: Value,
    calls: Vec<(String, Value)>,
}

impl Core for Fake {
    fn call(&mut self, method: &str, params: Value) -> Result<Value, Said> {
        self.calls.push((method.to_string(), params.clone()));
        let said = |code: &str| Said {
            code: code.to_string(),
            message: String::new(),
        };
        match method {
            "config.get" => Ok(self.config.clone()),
            "memory.import" => match self.refuse {
                Some(code) => Err(said(code)),
                None => {
                    Ok(json!({"imported": params["items"].as_array().unwrap().len(), "skipped": 0}))
                }
            },
            "memory.dream" if self.busy > 0 => {
                self.busy -= 1;
                Err(said("memory_busy"))
            }
            "memory.dream" => Ok(json!({"given": 3, "retired": 1, "revised": 0, "summary": false})),
            other => panic!("没想到会调 {other}"),
        }
    }
}

static SLEPT: AtomicU32 = AtomicU32::new(0);

fn count_sleep(_: Duration) {
    SLEPT.fetch_add(1, Ordering::SeqCst);
}

/// 家目录布局的旧根，`default` 人格的库里 `n` 条短的、一条超长的、一条 `self`。
fn old_root(n: usize) -> (tempfile::TempDir, Layout) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".home-layout-v1"), "owner\n").unwrap();
    std::fs::create_dir_all(dir.path().join("home/owner")).unwrap();
    std::fs::write(dir.path().join("home/owner/conversation.db"), "").unwrap();
    let memory = dir.path().join("personas/default/memory");
    std::fs::create_dir_all(&memory).unwrap();
    let conn = Connection::open(memory.join("memory.db")).unwrap();
    conn.execute_batch(
        "CREATE TABLE facts (id INTEGER PRIMARY KEY, content TEXT NOT NULL, updated_at TEXT NOT NULL, memory_type TEXT NOT NULL DEFAULT 'fact');",
    )
    .unwrap();
    for k in 0..n {
        conn.execute(
            "INSERT INTO facts (content, updated_at) VALUES (?1, '2026-09-25T07:00:00Z')",
            params![format!("第 {k} 条")],
        )
        .unwrap();
    }
    conn.execute(
        "INSERT INTO facts (content, updated_at) VALUES (?1, '2026-09-25T07:00:00Z')",
        params!["长".repeat(200)],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO facts (content, updated_at, memory_type) VALUES ('我', '2026-09-25T07:00:00Z', 'self')",
        [],
    )
    .unwrap();
    let layout = crate::layout::read(dir.path()).unwrap();
    (dir, layout)
}

fn mapped() -> Vec<Mapped> {
    vec![Mapped {
        old: "default".to_string(),
        name: "Miyu".to_string(),
        new: Some("persona-1".to_string()),
        how: crate::personas::How::Created,
        note: None,
    }]
}

#[test]
fn big_personas_go_in_batches_and_a_busy_dream_is_waited_for() {
    let (_dir, layout) = old_root(BATCH + 1);
    let mut core = Fake {
        busy: 2,
        refuse: None,
        config: json!({"items": {"external.bindings": {"value": {"qq:999": "owner"}}}}),
        calls: Vec::new(),
    };
    SLEPT.store(0, Ordering::SeqCst);
    let (item, pending) = migrate(&mut core, &layout, &mapped(), count_sleep).unwrap();
    let imports: Vec<usize> = core
        .calls
        .iter()
        .filter(|(method, _)| method == "memory.import")
        .map(|(_, params)| params["items"].as_array().unwrap().len())
        .collect();
    assert_eq!(imports, vec![BATCH, 1]);
    assert_eq!(core.calls[1].1["persona"], "persona-1");
    assert_eq!(SLEPT.load(Ordering::SeqCst), 2, "忙了两次，等了两次");
    assert_eq!(item.migrated, (BATCH + 1) as u64);
    assert_eq!(item.status, Status::Partial, "超长的、自己的没导");
    let whys: Vec<&str> = item.skipped.iter().map(|skip| skip.why.as_str()).collect();
    assert_eq!(whys, vec!["too-long", "self"]);
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].what, "memory-too-long");
    assert_eq!(pending[0].key, (BATCH + 2).to_string());
    assert_eq!(pending[0].source, "personas/default/memory/memory.db");
    let note = item.note.unwrap();
    assert!(note.contains("整理了：交进 3、作废 1、改了 0"), "{note}");
    assert!(!note.contains("没找到主人的号"), "{note}");
}

#[test]
fn a_refused_batch_stops_that_persona_and_says_why() {
    let (_dir, layout) = old_root(2);
    let mut core = Fake {
        busy: 0,
        refuse: Some("memory_not_installed"),
        config: json!({}),
        calls: Vec::new(),
    };
    let (item, _) = migrate(&mut core, &layout, &mapped(), count_sleep).unwrap();
    assert_eq!(item.migrated, 0);
    assert_eq!(item.status, Status::Partial);
    assert_eq!(
        item.skipped
            .iter()
            .filter(|skip| skip.why == "refused")
            .count(),
        2
    );
    let note = item.note.unwrap();
    assert!(
        note.contains("核心拒了（memory_not_installed），没整理"),
        "{note}"
    );
    assert!(note.contains("没找到主人的号"), "{note}");
    assert!(
        !core
            .calls
            .iter()
            .any(|(method, _)| method == "memory.dream")
    );
}

#[test]
fn old_admins_come_from_the_jsonc_config() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.jsonc");
    std::fs::write(
        &path,
        "{\n  // 注释 \"不是字\"\n  \"platforms\": {\"qq\": {\"admin_users\": [\"10001\", 10002, \"\"], /* 块注释 */ \"note\": \"a//b\",}},\n}\n",
    )
    .unwrap();
    assert_eq!(
        old_admins(&path),
        vec!["qq:10001".to_string(), "qq:10002".to_string()]
    );
    assert!(old_admins(&dir.path().join("gone.jsonc")).is_empty());
}

#[test]
fn keywords_are_cut_at_a_comma_under_the_limit() {
    let tags: Vec<String> = (0..50).map(|n| format!("标签{n:02}")).collect();
    let tags = serde_json::to_string(&tags).unwrap();
    let mut one = raw("fact", "很多标签", "[]");
    one.tags = &tags;
    let judged = judge(&one, &HashMap::new(), &HashSet::new());
    let keywords = item(&judged)["keywords"].as_str().unwrap().to_string();
    assert!(
        keywords.chars().count() <= read::KEYWORD_CHARS,
        "{keywords}"
    );
    assert!(
        keywords.ends_with(char::is_numeric),
        "在逗号处截：{keywords}"
    );
    assert!(keywords.starts_with("标签00, 标签01"));
}

#[test]
fn without_new_bindings_the_old_admins_are_the_owner() {
    let (_dir, layout) = old_root(0);
    std::fs::create_dir_all(layout.config().parent().unwrap()).unwrap();
    std::fs::write(
        layout.config(),
        r#"{"platforms": {"qq": {"admin_users": ["10001"]}}}"#,
    )
    .unwrap();
    let mut empty = Fake {
        busy: 0,
        refuse: None,
        config: json!({"items": {}}),
        calls: Vec::new(),
    };
    assert_eq!(
        owners(&mut empty, &layout),
        HashSet::from(["qq:10001".to_string()])
    );
    let mut bound = Fake {
        busy: 0,
        refuse: None,
        config: json!({"items": {"external.bindings.qq:999": {"value": "owner"}}}),
        calls: Vec::new(),
    };
    assert_eq!(
        owners(&mut bound, &layout),
        HashSet::from(["qq:999".to_string()]),
        "新版写了的为准"
    );
}

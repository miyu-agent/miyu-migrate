//! 一轮的附带迁进真核心（进程里起）：工具出的图、内嵌媒体、溢出接在那一条工具结果后面；提问写成 `ask_user`；插话带着这一轮；
//! 只有展示转录的轮照转录拼出工具（失败的是 `error`）；子代理、孙代理作子会话和主会话一个包，`parent`、`depth` 对。

#![allow(clippy::unwrap_used, reason = "测试：读坏了就该停")]

mod support;

use rusqlite::{Connection, params};
use serde_json::{Value, json};

use miyu_migrate::run::{self, Plan};
use miyu_migrate::sessions::ids;
use support::{connect, fresh, logged, png};

const AT: &str = "2026-09-25T07:00:00+00:00";

/// 一轮：编号、会话、序号、回复、工具流、展示转录。
type Row<'a> = (
    &'a str,
    &'a str,
    i64,
    &'a str,
    Option<&'a str>,
    Option<&'a str>,
);

fn old_root() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".home-layout-v1"), "owner\n").unwrap();
    std::fs::create_dir_all(dir.path().join("home/owner")).unwrap();
    let db = Connection::open(dir.path().join("home/owner/conversation.db")).unwrap();
    db.execute_batch(
        "PRAGMA user_version = 40;
         CREATE TABLE sessions (session_id TEXT PRIMARY KEY, persona TEXT NOT NULL, name TEXT NOT NULL,
             kind TEXT NOT NULL DEFAULT 'user', parent_session_id TEXT, created_at TEXT NOT NULL,
             updated_at TEXT NOT NULL, owner TEXT NOT NULL DEFAULT '', depth INTEGER NOT NULL DEFAULT 0);
         CREATE TABLE turns (turn_id TEXT PRIMARY KEY, session_id TEXT NOT NULL, seq INTEGER NOT NULL,
             user_content TEXT NOT NULL, user_timestamp TEXT NOT NULL, assistant_content TEXT NOT NULL,
             status TEXT NOT NULL, tool_flow TEXT, replay_journal TEXT);
         CREATE TABLE image_assets (asset_id TEXT PRIMARY KEY, turn_id TEXT NOT NULL, tool_id TEXT, mime TEXT NOT NULL,
             data BLOB NOT NULL, created_at TEXT NOT NULL);
         CREATE TABLE turn_inline_media (media_id INTEGER PRIMARY KEY, turn_id TEXT NOT NULL, call_id TEXT NOT NULL,
             seq INTEGER NOT NULL, kind TEXT NOT NULL, mime TEXT NOT NULL, source TEXT NOT NULL, data BLOB);
         CREATE TABLE question_exchanges (turn_id TEXT NOT NULL, exchange_index INTEGER NOT NULL, payload TEXT NOT NULL);
         CREATE TABLE queued_prompts (seq INTEGER PRIMARY KEY AUTOINCREMENT, prompt_id TEXT NOT NULL, content TEXT NOT NULL,
             display_content TEXT NOT NULL, status TEXT NOT NULL, turn_id TEXT);",
    )
    .unwrap();
    for (id, kind, parent, depth) in [
        ("sess_main", "user", None, 0),
        ("sess_kid", "subagent", Some("sess_main"), 1),
        ("sess_grand", "subagent", Some("sess_kid"), 0),
    ] {
        db.execute(
            "INSERT INTO sessions VALUES (?1, '', '', ?2, ?3, ?4, ?4, '', ?5)",
            params![id, kind, parent, AT, depth],
        )
        .unwrap();
    }
    let flow = json!([{"assistant_content": "我截个图", "calls": [
        {"id": "call_x", "name": "shot", "arguments": "{}", "output": "截好了"}
    ]}])
    .to_string();
    let replay = json!([
        {"kind": "text", "text": "我查查"},
        {"kind": "tool_call", "name": "read", "arguments": "{\"path\":\"/a\"}"},
        {"kind": "tool_result", "name": "read", "ok": false, "output": "没有这个文件"},
        {"kind": "text", "text": "没找到"}
    ])
    .to_string();
    let turns: [Row<'_>; 4] = [
        ("turn_1", "sess_main", 1, "好了", Some(flow.as_str()), None),
        (
            "turn_2",
            "sess_main",
            2,
            "没找到",
            None,
            Some(replay.as_str()),
        ),
        ("turn_k", "sess_kid", 1, "子代理答", None, None),
        ("turn_g", "sess_grand", 1, "孙代理答", None, None),
    ];
    for (id, session, seq, reply, flow, replay) in turns {
        db.execute(
            "INSERT INTO turns VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'completed', ?7, ?8)",
            params![
                id,
                session,
                seq,
                format!("{id} 说"),
                AT,
                reply,
                flow,
                replay
            ],
        )
        .unwrap();
    }
    db.execute(
        "INSERT INTO image_assets VALUES ('img_1', 'turn_1', 'call_x', 'image/png', ?1, ?2)",
        params![png(), AT],
    )
    .unwrap();
    // 对不上调用的一张：那一轮的工具里没有这个编号。
    db.execute(
        "INSERT INTO image_assets VALUES ('img_2', 'turn_1', 'call_gone', 'image/png', ?1, ?2)",
        params![png(), AT],
    )
    .unwrap();
    db.execute(
        "INSERT INTO turn_inline_media VALUES (1, 'turn_1', 'call_x', 0, 'image', 'image/png', '/old/shot.png', ?1)",
        params![png()],
    )
    .unwrap();
    let asked = json!({"questions": [{"header": "选", "question": "选哪个", "options": []}], "answers": [["甲"]], "answered_at": AT});
    db.execute(
        "INSERT INTO question_exchanges VALUES ('turn_1', 0, ?1)",
        params![asked.to_string()],
    )
    .unwrap();
    db.execute(
        "INSERT INTO queued_prompts (prompt_id, content, display_content, status, turn_id) VALUES ('q1', '插一句（模型看的）', '插一句', 'consumed', 'turn_1')",
        [],
    )
    .unwrap();
    let spill = dir.path().join("state/spill/sess_main");
    std::fs::create_dir_all(&spill).unwrap();
    std::fs::write(spill.join("turn_1-call_x-shot.txt"), "很长的输出").unwrap();
    dir
}

fn id(old: &str) -> String {
    let created = ids::parse(AT).unwrap().as_millisecond() as u64;
    ids::session_id(created, "home/owner/conversation.db", old)
}

fn kinds(events: &[Value]) -> Vec<&str> {
    events
        .iter()
        .map(|event| event["kind"].as_str().unwrap())
        .collect()
}

#[test]
fn tool_extras_questions_queued_replays_and_children_come_along() {
    let fresh = fresh();
    let old = old_root();
    let mut core = connect(&fresh.root);
    let plan = Plan {
        old: old.path().to_path_buf(),
        state: fresh.root.path().join("var/lib/migrate"),
        at: "2026-11-04T08:00:00Z".parse().unwrap(),
        root: fresh.root.clone(),
        records: miyu_migrate::records::Importer::new(|_| Err(std::io::Error::other("no im"))),
        home: None,
        space: run::free_space,
    };
    let done = run::run(&plan, &mut core).unwrap();
    let agent: Value = serde_json::from_str(&std::fs::read_to_string(done.agent).unwrap()).unwrap();
    let item = agent["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["what"] == "sessions")
        .unwrap()
        .clone();
    assert_eq!(item["migrated"], 1, "子代理不另算：{item}");
    assert!(item.get("skipped").is_none(), "{item}");
    let note = item["note"].as_str().unwrap();
    assert!(note.contains("子代理会话 2 个作子会话"), "{note}");
    assert!(note.contains("工具结果后面接了 3 张图、文件"), "{note}");
    assert!(note.contains("有 1 样对不上调用"), "{note}");

    let main = logged(&fresh.root, &id("sess_main"));
    assert_eq!(
        kinds(&main),
        vec![
            "session.created",
            "message.user",
            "turn.started",
            "message.user",
            "message.assistant",
            "tool.result",
            "message.assistant",
            "tool.result",
            "message.assistant",
            "turn.ended",
            "message.user",
            "turn.started",
            "message.assistant",
            "tool.result",
            "message.assistant",
            "turn.ended",
        ]
    );
    assert_eq!(main[3]["turn"], 3, "插话带着这一轮");
    assert_eq!(main[3]["body"]["blocks"][0]["text"], "插一句");
    let shot: Vec<&str> = main[5]["body"]["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|block| block["type"].as_str().unwrap())
        .collect();
    assert_eq!(
        shot,
        vec!["text", "image", "image", "file"],
        "图、内嵌的图、溢出"
    );
    assert_eq!(
        main[5]["body"]["blocks"][3]["name"],
        "turn_1-call_x-shot.txt"
    );
    assert_eq!(main[6]["body"]["blocks"][0]["name"], "ask_user");
    assert_eq!(main[7]["body"]["blocks"][0]["text"], "问：选哪个\n答：甲");
    assert_eq!(main[8]["body"]["blocks"][0]["text"], "好了");
    assert_eq!(
        main[12]["body"]["blocks"],
        json!([
            {"type": "text", "text": "我查查"},
            {"type": "tool_call", "call_id": "call_13_1", "name": "read", "args": "{\"path\":\"/a\"}"}
        ]),
        "只有展示转录的照转录拼"
    );
    assert_eq!(main[13]["body"]["status"], "error");
    assert_eq!(
        main[14]["body"]["blocks"][0]["text"], "没找到",
        "最后一句不写两遍"
    );

    let kid = logged(&fresh.root, &id("sess_kid"));
    assert_eq!(kid[0]["body"]["parent"], id("sess_main").as_str());
    assert_eq!(kid[0]["body"]["depth"], 1);
    let grand = logged(&fresh.root, &id("sess_grand"));
    assert_eq!(grand[0]["body"]["parent"], id("sess_kid").as_str());
    assert_eq!(grand[0]["body"]["depth"], 2, "旧库写 0 的照往上几层算");

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
    let note = agent["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["what"] == "sessions")
        .unwrap()["note"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(note.contains("其中 1 个会话以前就导过"), "{note}");
}

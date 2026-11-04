//! QQ 消息记录：一行消息怎么写（场所新写法、毫秒、@ 两种写法、媒体几种、她自己发的、撤回带谁撤的）；不是 QQ 的跳过；撤回了、
//! 消息不在了的写成群里的事排在最后；版本不对的不转；im 说的条数照数读出来。

use std::fs;
use std::sync::Arc;

use rusqlite::{Connection, params};
use serde_json::{Value, json};

use super::*;

/// 一行消息：平台、发的人、编号、名字、正文、媒体、@、引用、她自己发的、撤回时刻。
type Row<'a> = (
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    Option<&'a str>,
    i64,
    Option<i64>,
);

fn db(version: i64) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.sqlite3");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(&format!(
        "PRAGMA user_version = {version};
         CREATE TABLE messages (id INTEGER PRIMARY KEY, platform TEXT NOT NULL, account_id TEXT NOT NULL,
             conversation_kind TEXT NOT NULL, conversation_id TEXT NOT NULL, message_id TEXT NOT NULL,
             sender_id TEXT NOT NULL, sender_name TEXT NOT NULL, text TEXT NOT NULL, media_json TEXT NOT NULL,
             mentions_json TEXT NOT NULL, reply_to_message_id TEXT, is_bot INTEGER NOT NULL, sent_at INTEGER NOT NULL,
             recalled_at INTEGER);
         CREATE TABLE recalls (id INTEGER PRIMARY KEY, platform TEXT NOT NULL, account_id TEXT NOT NULL,
             conversation_kind TEXT NOT NULL, conversation_id TEXT NOT NULL, message_id TEXT NOT NULL,
             operator_id TEXT, recalled_at INTEGER NOT NULL);"
    ))
    .unwrap();
    let rows: [Row<'_>; 4] = [
        (
            "onebot",
            "1",
            "-5",
            "小李",
            "在吗",
            "[]",
            r#"["222", 333]"#,
            None,
            0,
            None,
        ),
        (
            "onebot",
            "9",
            "7",
            "Miyu",
            "回你",
            r#"[{"kind":"image","label":"a.png","mime":null},{"kind":"audio","label":null,"media_id":"v1"},{"kind":"other","label":"x"},{"kind":"sticker","label":null}]"#,
            r#"[{"user_id":"1","display_name":"小李"}]"#,
            Some("-5"),
            1,
            Some(1_700_000_100),
        ),
        (
            "telegram",
            "1",
            "8",
            "外人",
            "不是 QQ",
            "[]",
            "[]",
            None,
            0,
            None,
        ),
        (
            "onebot",
            "1",
            "ingress-3",
            "小李",
            "",
            "[]",
            "坏的",
            Some(""),
            0,
            None,
        ),
    ];
    for (platform, sender, msg, name, text, media, mentions, reply, bot, recalled) in rows {
        conn.execute(
            "INSERT INTO messages (platform, account_id, conversation_kind, conversation_id, message_id, sender_id,
                 sender_name, text, media_json, mentions_json, reply_to_message_id, is_bot, sent_at, recalled_at)
             VALUES (?1, '9', 'group', '555', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1700000000, ?10)",
            params![platform, msg, sender, name, text, media, mentions, reply, bot, recalled],
        )
        .unwrap();
    }
    for (msg, operator) in [("7", Some("1")), ("gone", None), ("-5", None)] {
        conn.execute(
            "INSERT INTO recalls (platform, account_id, conversation_kind, conversation_id, message_id, operator_id, recalled_at)
             VALUES ('onebot', '9', 'group', '555', ?1, ?2, 1700000200)",
            params![msg, operator],
        )
        .unwrap();
    }
    (dir, path)
}

#[test]
fn messages_become_im_lines_and_lost_recalls_come_last() {
    let (_dir, path) = db(4);
    let converted = convert(&path).unwrap().unwrap();
    assert_eq!(
        (converted.messages, converted.happenings, converted.foreign),
        (3, 1, 1)
    );
    let lines: Vec<Value> = converted
        .lines
        .iter()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(
        lines[0],
        json!({
            "type": "message", "venue": "qq:9:group:555", "msg": "-5", "sender": "qq:1",
            "at": 1_700_000_000_000i64, "text": "在吗", "name": "小李",
            "mentions": ["qq:222", "qq:333"],
            "recalled_at": 1_700_000_200_000i64,
        }),
        "撤回表里有、没写谁撤的"
    );
    assert_eq!(lines[1]["mine"], true);
    assert_eq!(lines[1]["reply_to"], "-5");
    assert_eq!(lines[1]["mentions"], json!(["qq:1"]));
    assert_eq!(
        lines[1]["media"],
        json!([
            {"kind": "image", "id": "a.png", "name": "a.png"},
            {"kind": "voice", "id": "v1"},
            {"kind": "file", "id": "x", "name": "x"},
        ]),
        "没编号没名字的表情不写"
    );
    assert_eq!(
        lines[1]["recalled_at"], 1_700_000_100_000i64,
        "消息行上写的为准"
    );
    assert_eq!(lines[1]["recalled_by"], "qq:1");
    assert!(lines[2].get("mentions").is_none(), "读不懂的 @ 当没有");
    assert!(lines[2].get("reply_to").is_none(), "空的引用不写");
    assert_eq!(
        lines[3],
        json!({"type": "happening", "venue": "qq:9:group:555", "at": 1_700_000_200_000i64, "kind": "recalled", "msg": "gone"})
    );
}

#[test]
fn another_version_is_not_converted() {
    let (_dir, path) = db(3);
    assert_eq!(convert(&path).unwrap(), None);
}

#[test]
fn what_im_says_is_counted() {
    let said = "imported from /a/b 2026/qq-records.jsonl: 12 messages, 3 events, 0 judgments\nskipped 40 already in the records and 2 whose number is taken by a different message\n";
    assert_eq!(numbers(said), vec![12, 3, 0, 40, 2]);
}

/// 命令退出时的样子（成功是 0）。
fn exit(code: i32) -> std::process::ExitStatus {
    #[cfg(unix)]
    {
        std::os::unix::process::ExitStatusExt::from_raw(code << 8)
    }
    #[cfg(windows)]
    {
        std::os::windows::process::ExitStatusExt::from_raw(code as u32)
    }
}

/// 假的 im：照 `code` 退出，标准输出、错误照给的说，记下交来的文件里有几行。
fn fake(
    code: i32,
    out: &'static str,
    err: &'static str,
    seen: Arc<std::sync::Mutex<usize>>,
) -> Importer {
    Importer::new(move |file| {
        *seen.lock().unwrap() = fs::read_to_string(file)?.lines().count();
        Ok(Output {
            status: exit(code),
            stdout: out.as_bytes().to_vec(),
            stderr: err.as_bytes().to_vec(),
        })
    })
}

/// 家目录布局的旧根，消息记录库放在该在的地方。
fn copy() -> (tempfile::TempDir, crate::layout::Layout) {
    let (dir, path) = db(4);
    fs::write(dir.path().join(".home-layout-v1"), "owner\n").unwrap();
    fs::create_dir_all(dir.path().join("home/owner")).unwrap();
    fs::write(dir.path().join("home/owner/conversation.db"), "").unwrap();
    let target = dir.path().join("data/platforms/onebot/message_history");
    fs::create_dir_all(&target).unwrap();
    fs::rename(&path, target.join("history.sqlite3")).unwrap();
    let layout = crate::layout::read(dir.path()).unwrap();
    (dir, layout)
}

#[test]
fn im_imports_the_file_and_its_counts_land_in_the_report() {
    let (_dir, layout) = copy();
    let out = tempfile::tempdir().unwrap();
    let seen = Arc::new(std::sync::Mutex::new(0));
    let said = "imported from /x: 2 messages, 1 events, 0 judgments\nskipped 1 already in the records and 1 whose number is taken by a different message\n";
    let item = migrate(&layout, out.path(), &fake(0, said, "", Arc::clone(&seen)));
    assert_eq!(*seen.lock().unwrap(), 4, "三条消息、一件撤回");
    assert!(out.path().join("qq-records.jsonl").is_file());
    assert_eq!(item.migrated, 3);
    assert_eq!(item.status, Status::Partial, "有编号被占的");
    let note = item.note.unwrap();
    assert!(note.contains("im 新收了 2 条消息、1 件群事件"), "{note}");
    assert!(note.contains("1 条 im 里本来就有"), "{note}");
    assert!(note.contains("1 条不是 QQ 的没转"), "{note}");
}

#[test]
fn without_im_the_file_stays_with_the_line_to_run() {
    let (_dir, layout) = copy();
    let out = tempfile::tempdir().unwrap();
    let seen = Arc::new(std::sync::Mutex::new(0));
    let refused = migrate(
        &layout,
        out.path(),
        &fake(
            1,
            "",
            "nothing imported: 1 wrong lines in /x\nline 1: bad\n",
            Arc::clone(&seen),
        ),
    );
    assert_eq!(refused.status, Status::Archived);
    let note = refused.note.unwrap();
    assert!(
        note.contains("im 没导（nothing imported: 1 wrong lines in /x）"),
        "{note}"
    );
    assert!(note.contains("miyu im records import "), "{note}");
    let missing = migrate(
        &layout,
        out.path(),
        &Importer::new(|_| Err(io::Error::other("no such program"))),
    );
    assert_eq!(missing.status, Status::Archived);
    assert!(
        missing
            .note
            .unwrap()
            .contains("跑不了 im（no such program）")
    );
}

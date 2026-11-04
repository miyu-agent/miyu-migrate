//! 零碎：在的才进报告（大小、条数、提醒还没到点的）；成员只数个数；平台的表照插件数；记忆的经历和旧向量照人格数；数据根外面的照实说，
//! 新版的包装脚本不叫人删。

use std::fs;

use rusqlite::Connection;
use serde_json::json;

use super::*;

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn old_root() -> (tempfile::TempDir, Layout) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(&root.join(".home-layout-v1"), b"owner\n");
    let db = root.join("home/owner/conversation.db");
    fs::create_dir_all(db.parent().unwrap()).unwrap();
    Connection::open(&db)
        .unwrap()
        .execute_batch(
            "CREATE TABLE repl_history (id INTEGER PRIMARY KEY, entry TEXT);
             INSERT INTO repl_history (entry) VALUES ('a'), ('b');
             CREATE TABLE platform_plugin_kv (plugin_id TEXT, key TEXT, value_json TEXT);
             INSERT INTO platform_plugin_kv VALUES ('affection', 'k1', '1'), ('affection', 'k2', '2'), ('mood', 'k', '3');
             CREATE TABLE platform_meme_refs (x);
             INSERT INTO platform_meme_refs VALUES (1);",
        )
        .unwrap();
    write(&root.join("home/kanaki/conversation.db"), b"");
    write(&root.join("state/repl-history/history"), b"ls\n");
    write(&root.join("config/config.jsonc.bak-1"), b"{}");
    write(&root.join("config/config.jsonc.bak-2"), b"{}");
    let alarms = json!([
        {"id": "a", "label": "喝水", "time": "10m", "due_at": 1, "status": "scheduled"},
        {"id": "b", "label": "开会", "time": "15:30", "due_at": 9_999_999_999i64, "status": "scheduled"},
    ]);
    write(
        &root.join("state/alarms.json"),
        alarms.to_string().as_bytes(),
    );
    write(&root.join("logs/core.log"), b"x");
    write(&root.join("cache/models_cache.json"), b"{}");
    let memory = root.join("personas/default/memory/memory.db");
    fs::create_dir_all(memory.parent().unwrap()).unwrap();
    Connection::open(&memory)
        .unwrap()
        .execute_batch(
            "CREATE TABLE episodes (retention TEXT); INSERT INTO episodes VALUES ('short_term'), ('long_term');
             CREATE TABLE memory_revisions (x); CREATE TABLE memory_embeddings (x); INSERT INTO memory_embeddings VALUES (1);",
        )
        .unwrap();
    let layout = crate::layout::read(root).unwrap();
    (dir, layout)
}

fn find<'a>(items: &'a [Item], what: &str) -> &'a Item {
    items
        .iter()
        .find(|item| item.what == what)
        .unwrap_or_else(|| panic!("没有 {what}"))
}

#[test]
fn leftovers_are_listed_with_where_and_how_many() {
    let (_old, layout) = old_root();
    let items = items(&layout, None);
    let whats: Vec<&str> = items.iter().map(|item| item.what.as_str()).collect();
    assert!(!whats.contains(&"usage"), "没有的不进：{whats:?}");
    let history = find(&items, "repl-history");
    assert_eq!(history.status, Status::Archived);
    assert!(
        history
            .note
            .as_deref()
            .unwrap()
            .contains("会话库里另有 repl_history 2 条")
    );
    let backups = find(&items, "config-backups");
    assert_eq!(
        backups.source,
        "config/config.jsonc.bak-1、config/config.jsonc.bak-2"
    );
    let alarms = find(&items, "alarms");
    assert!(
        alarms
            .note
            .as_deref()
            .unwrap()
            .contains("提醒 2 条，还没到点的 1 条")
    );
    assert!(alarms.format.contains("schedule.add") && alarms.format.contains("未核实"));
    assert_eq!(find(&items, "derived/logs").status, Status::Skipped);
    assert_eq!(
        find(&items, "derived/cache").source,
        "cache/models_cache.json"
    );
    assert_eq!(
        find(&items, "members").note.as_deref(),
        Some("1 个成员的家目录，没迁（新版只给一个人用）。")
    );
    let platform = find(&items, "platform-tables").note.clone().unwrap();
    assert!(platform.contains("affection 2 条、mood 1 条"), "{platform}");
    assert!(platform.contains("表情引用 1 条"), "{platform}");
    let memory = find(&items, "memory-rest").note.clone().unwrap();
    assert_eq!(memory, "`default`：逐轮经历 1、日记 1、旧向量 1。");
    assert!(
        !whats.iter().any(|what| what.starts_with("outside/")),
        "没给家目录的不看"
    );
}

#[test]
fn outside_things_are_reported_and_the_new_wrapper_is_kept() {
    let (_old, layout) = old_root();
    let home = tempfile::tempdir().unwrap();
    write(
        &home.path().join(".local/bin/miyu"),
        b"#!/bin/sh\n# miyu-remake wrapper\n",
    );
    write(&home.path().join(".config/fish/conf.d/miyu.fish"), b"");
    let items = items(&layout, Some(home.path()));
    let wrapper = find(&items, "outside/.local/bin/miyu");
    assert_eq!(
        wrapper.note.as_deref(),
        Some("这是新版的包装脚本，不是旧版的，别删。")
    );
    let fish = find(&items, "outside/.config/fish/conf.d/miyu.fish");
    assert!(fish.note.as_deref().unwrap().contains("确认是旧版的"));
    assert!(
        !items
            .iter()
            .any(|item| item.what == "outside/.local/state/miyu"),
        "不在的不报"
    );
}

#[test]
fn sizes_read_like_people_write_them() {
    assert_eq!(human_size(512), "512 字节");
    assert_eq!(human_size(2048), "2.0 KB");
    assert_eq!(human_size(3 * 1_048_576), "3.0 MB");
}

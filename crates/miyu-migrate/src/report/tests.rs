//! 两份报告：给 agent 的读回来一样；给人的有每一类、没迁的那张表只列没迁的；写进目录的是那两个文件名。

use super::*;

fn sample() -> Report {
    Report {
        format: FORMAT,
        migrate: "0.1.0".to_string(),
        at: "2026-11-04T08:00:00Z".to_string(),
        archive: Some(Archive {
            path: "/data/var/lib/migrate/run/miyu-0.7.tar.zst".to_string(),
            sha256: "ab".repeat(32),
            files: 9351,
            bytes: 1_600_000_000,
            manifest: "/data/var/lib/migrate/run/miyu-0.7.manifest.json".to_string(),
        }),
        old_root: OldRoot {
            path: "/home/a/.miyu-0.7".to_string(),
            layout: "home-layout-v1".to_string(),
            admin: Some("a".to_string()),
            rolled_back: false,
        },
        personas: vec![crate::personas::Mapped {
            old: "default".to_string(),
            name: "Miyu".to_string(),
            new: Some("persona-1".to_string()),
            how: crate::personas::How::Existing,
            note: None,
        }],
        items: vec![
            Item {
                what: "sessions".to_string(),
                status: Status::Partial,
                source: "home/a/conversation.db".to_string(),
                format: "SQLite user_version 40".to_string(),
                migrated: 128,
                skipped: vec![Skip {
                    key: "summary_1_2".to_string(),
                    why: "summary".to_string(),
                }],
                target: Some("root/sessions".to_string()),
                note: None,
            },
            Item {
                what: "usage".to_string(),
                status: Status::Archived,
                source: "state/usage.db".to_string(),
                format: "SQLite usage_records".to_string(),
                migrated: 0,
                skipped: Vec::new(),
                target: None,
                note: Some("新版账本没有导入入口。".to_string()),
            },
            Item {
                what: "caches".to_string(),
                status: Status::Skipped,
                source: "cache".to_string(),
                format: "派生的".to_string(),
                migrated: 0,
                skipped: Vec::new(),
                target: None,
                note: None,
            },
        ],
        pending: vec![Pending {
            what: "memory-too-long".to_string(),
            source: "personas/dev/memory/memory.db".to_string(),
            table: Some("facts".to_string()),
            key: "12".to_string(),
            note: None,
        }],
    }
}

#[test]
fn the_agent_report_reads_back_the_same() {
    let report = sample();
    let text = report.agent().unwrap();
    assert_eq!(serde_json::from_str::<Report>(&text).unwrap(), report);
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["items"][0]["status"], "partial");
    assert!(value["items"][1].get("skipped").is_none(), "空的不写");
}

#[test]
fn the_human_report_lists_only_what_was_left_behind() {
    let human = sample().human();
    assert!(
        human.contains("/data/var/lib/migrate/run/miyu-0.7.tar.zst"),
        "{human}"
    );
    assert!(
        human.contains("### sessions\n\n迁了一部分，128 条，跳过 1 条。"),
        "{human}"
    );
    assert!(
        human.contains("| `default` | Miyu | `persona-1` | 已经有同名的 |"),
        "{human}"
    );
    let table = &human[human.find("## 没迁的").expect("有没迁的那一节")..];
    assert!(
        table.contains("| sessions | `home/a/conversation.db` |"),
        "{table}"
    );
    assert!(table.contains("| usage | `state/usage.db` |"), "{table}");
    assert!(!table.contains("caches"), "不用迁的不列：{table}");
}

#[test]
fn both_reports_land_in_the_folder() {
    let dir = tempfile::tempdir().unwrap();
    let (human, agent) = sample().write(&dir.path().join("run")).unwrap();
    assert_eq!(human.file_name().unwrap(), "报告.md");
    assert_eq!(agent.file_name().unwrap(), "给agent.json");
    assert!(std::fs::read_to_string(agent).unwrap().ends_with("}\n"));
}

#[test]
fn derived_things_fold_into_one_line_and_the_old_root_comes_last() {
    let mut report = sample();
    for what in ["derived/logs", "derived/cache"] {
        report.items.push(Item {
            what: what.to_string(),
            status: Status::Skipped,
            source: "logs".to_string(),
            format: "日志".to_string(),
            migrated: 0,
            skipped: Vec::new(),
            target: None,
            note: None,
        });
    }
    report.items.push(Item {
        what: "outside/.local/bin/miyu".to_string(),
        status: Status::Skipped,
        source: "~/.local/bin/miyu".to_string(),
        format: "数据根外面".to_string(),
        migrated: 0,
        skipped: Vec::new(),
        target: None,
        note: Some("这是新版的包装脚本，不是旧版的，别删。".to_string()),
    });
    let human = report.human();
    assert!(!human.contains("### derived/"), "派生的不一条条列：{human}");
    assert!(human.contains("### 派生的缓存\n\n2 处"), "{human}");
    assert!(human.contains("## 数据根外面\n\n旧版在数据根外面装过这些，不在压缩包里：\n\n- `~/.local/bin/miyu`：这是新版的包装脚本"), "{human}");
    let last = &human[human.find("## 旧数据根").expect("有旧数据根那一节")..];
    assert!(
        last.contains("`/home/a/.miyu-0.7` 迁移一个字节都没动"),
        "{last}"
    );
    assert!(
        last.contains(&"ab".repeat(32)),
        "带压缩包的 SHA-256：{last}"
    );
    assert!(last.contains("删由你自己来"), "{last}");
    assert!(!last[2..].contains("\n## "), "是最后一节");
    let agent = report.agent().unwrap();
    assert!(
        agent.contains("derived/logs") && agent.contains("derived/cache"),
        "给 agent 的那份照样逐条"
    );
}

#[test]
fn paths_to_paste_are_quoted_when_a_shell_would_split_them() {
    assert_eq!(
        shell_path(Path::new("/home/a/.miyu/x-1_2.jsonl")),
        "/home/a/.miyu/x-1_2.jsonl"
    );
    assert_eq!(
        shell_path(Path::new(
            "/home/a/root/documents/旧版 Miyu 迁移/t/qq-records.jsonl"
        )),
        "'/home/a/root/documents/旧版 Miyu 迁移/t/qq-records.jsonl'"
    );
    assert_eq!(shell_path(Path::new("/a/b c")), "'/a/b c'");
    assert_eq!(shell_path(Path::new("")), "''", "空的也要占一个参数");
    assert_eq!(shell_path(Path::new("/a/it's $x")), r#""/a/it's \$x""#);
    assert_eq!(shell_path(Path::new(r#"/a/"q"`\"#)), r#"'/a/"q"`\'"#);
    assert_eq!(shell_path(Path::new("/a/'\"`\\")), r#""/a/'\"\`\\""#);
}

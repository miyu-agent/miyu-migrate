//! 文件：几类目录拷到新版的同类目录（两处图片合并、产物照会话名、工作目录进「旧版」）；同名一样的算迁过、不一样的改名；分享不迁；
//! 链接不拷；会话标题当目录名的写法。

use std::fs;
use std::path::Path;

use rusqlite::{Connection, params};

use super::*;

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

#[test]
fn titles_become_folder_names() {
    assert_eq!(clean("看猫/看狗"), "看猫_看狗");
    assert_eq!(clean("  ..  "), "_");
    assert_eq!(clean(&"长".repeat(100)).chars().count(), 80);
}

/// 家目录布局的旧根：两处图片、文档、两个会话的产物（一个有名字）、两处工作目录、分享。
fn old_root() -> (tempfile::TempDir, Layout) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(&root.join(".home-layout-v1"), b"owner\n");
    fs::create_dir_all(root.join("home/owner")).unwrap();
    let db = Connection::open(root.join("home/owner/conversation.db")).unwrap();
    db.execute_batch(
        "CREATE TABLE sessions (session_id TEXT PRIMARY KEY, persona TEXT NOT NULL, name TEXT NOT NULL, kind TEXT NOT NULL,
             created_at TEXT NOT NULL, updated_at TEXT NOT NULL);",
    )
    .unwrap();
    for (id, name) in [("sess_a", "画猫"), ("sess_b", "")] {
        db.execute(
            "INSERT INTO sessions VALUES (?1, '', ?2, 'user', '2026-09-25T07:00:00+00:00', '2026-09-25T07:00:00+00:00')",
            params![id, name],
        )
        .unwrap();
    }
    write(&root.join("home/owner/pictures/cat.png"), b"cat");
    write(&root.join("data/pictures/old.png"), b"old");
    write(&root.join("home/owner/documents/a.md"), b"a");
    write(&root.join("home/owner/artifacts/sess_a/out.svg"), b"svg");
    write(&root.join("home/owner/artifacts/sess_b/out.txt"), b"txt");
    write(
        &root.join("home/owner/workspace/proj/main.rs"),
        b"fn main() {}",
    );
    write(&root.join("workspace/top.txt"), b"top");
    write(&root.join("home/owner/shares/s.txt"), b"share");
    #[cfg(unix)]
    std::os::unix::fs::symlink("/etc/hostname", root.join("home/owner/documents/link")).unwrap();
    let layout = crate::layout::read(root).unwrap();
    (dir, layout)
}

#[test]
fn files_land_in_the_new_folders() {
    let (_old, layout) = old_root();
    let new = tempfile::tempdir().unwrap();
    write(&new.path().join("root/documents/a.md"), b"new a");
    let item = migrate(&layout, new.path());
    let at = |path: &str| fs::read(new.path().join("root").join(path)).unwrap();
    assert_eq!(at("pictures/cat.png"), b"cat");
    assert_eq!(at("pictures/old.png"), b"old", "两处图片合并");
    assert_eq!(at("documents/a.md"), b"new a", "新版的不盖");
    assert_eq!(at("documents/a (旧版 1).md"), b"a");
    assert_eq!(at("documents/旧版产物/画猫/out.svg"), b"svg");
    let unnamed = ids::session_id(
        ids::parse("2026-09-25T07:00:00+00:00")
            .unwrap()
            .as_millisecond() as u64,
        "home/owner/conversation.db",
        "sess_b",
    );
    assert_eq!(
        at(&format!("documents/旧版产物/{unnamed}/out.txt")),
        b"txt",
        "没名字的照新编号"
    );
    assert_eq!(at("workspace/旧版/proj/main.rs"), b"fn main() {}");
    assert_eq!(at("workspace/旧版/top.txt"), b"top");
    assert!(!new.path().join("root/documents/s.txt").exists());
    assert_eq!(item.migrated, 7);
    let whys: Vec<(&str, &str)> = item
        .skipped
        .iter()
        .map(|one| (one.key.as_str(), one.why.as_str()))
        .collect();
    #[cfg(unix)]
    assert_eq!(
        whys,
        vec![
            ("home/owner/documents/link", "link"),
            ("home/owner/shares", "shares")
        ]
    );
    #[cfg(not(unix))]
    assert_eq!(whys, vec![("home/owner/shares", "shares")]);
    let note = item.note.unwrap();
    assert!(note.contains("拷了 7 个文件，0 个以前迁过"), "{note}");
    assert!(
        note.contains("home/owner/documents/a.md → a (旧版 1).md"),
        "{note}"
    );

    let again = migrate(&layout, new.path());
    let note = again.note.unwrap();
    assert!(note.contains("拷了 0 个文件，7 个以前迁过"), "{note}");
    assert!(!new.path().join("root/documents/a (旧版 2).md").exists());
}

#[test]
fn an_old_root_without_files_says_so() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join(".home-layout-v1"), b"owner\n");
    write(&dir.path().join("home/owner/conversation.db"), b"");
    let layout = crate::layout::read(dir.path()).unwrap();
    let item = migrate(&layout, dir.path());
    assert_eq!(item.status, Status::Skipped);
}

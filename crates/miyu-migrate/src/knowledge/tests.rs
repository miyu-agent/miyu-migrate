//! 知识库：自己放的文档拷进 `root/kb/legacy/`、建库；默认库不迁；同名一样的算迁过、不一样的改名；不认的类型照拷、报一句；旧清单
//! 的哈希对不上的数出来；没有库的不问核心。

use std::fs;
use std::path::Path;

use rusqlite::{Connection, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::*;

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn sha(bytes: &[u8]) -> String {
    crate::archive::hex(&Sha256::digest(bytes))
}

/// 家目录布局的旧根：知识库里三篇自己的（一篇 docx、一篇哈希对不上）、一篇默认库的。
fn old_root() -> (tempfile::TempDir, Layout) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(&root.join(".home-layout-v1"), b"owner\n");
    write(&root.join("home/owner/conversation.db"), b"");
    let files = [
        ("notes/日语.md", "# 日语".as_bytes(), true),
        ("a.docx", b"docx".as_slice(), true),
        ("b.txt", b"changed".as_slice(), false),
        ("default-kb/wiki.md", b"wiki".as_slice(), true),
    ];
    write(&root.join("data/kb/kb_meta.db"), b"");
    fs::remove_file(root.join("data/kb/kb_meta.db")).unwrap();
    let conn = Connection::open(root.join("data/kb/kb_meta.db")).unwrap();
    conn.execute_batch(
        "CREATE TABLE files (name TEXT PRIMARY KEY, path TEXT NOT NULL, size_bytes INTEGER NOT NULL, mtime REAL NOT NULL, content_sha256 TEXT NOT NULL, updated_at REAL NOT NULL);",
    )
    .unwrap();
    for (name, bytes, honest) in files {
        write(&root.join("data/kb/files").join(name), bytes);
        let recorded = if honest { sha(bytes) } else { sha(b"before") };
        conn.execute(
            "INSERT INTO files VALUES (?1, '/old/abs/path', 1, 0, ?2, 0)",
            params![name, recorded],
        )
        .unwrap();
    }
    // 清单里一条跑出 `files/` 的名字：不认。
    conn.execute(
        "INSERT INTO files VALUES ('../escape.md', '/old', 1, 0, '', 0)",
        [],
    )
    .unwrap();
    let layout = crate::layout::read(root).unwrap();
    (dir, layout)
}

/// 假核心：`kb.create` 照 `exists` 回已有或收下；`kb.list` 答这个库认得几篇。
struct Fake {
    exists: bool,
    calls: Vec<String>,
}

impl Core for Fake {
    fn call(&mut self, method: &str, params: Value) -> Result<Value, Said> {
        self.calls.push(method.to_string());
        match method {
            "kb.create" if self.exists => Err(Said {
                code: "kb_exists".to_string(),
                message: String::new(),
            }),
            "kb.create" => {
                assert_eq!(params["kb"], BASE);
                Ok(json!({}))
            }
            "kb.list" => Ok(json!({"bases": [{"id": BASE, "documents": 2}]})),
            other => panic!("没想到会调 {other}"),
        }
    }
}

#[test]
fn own_documents_land_in_the_legacy_base() {
    let (_old, layout) = old_root();
    let new = tempfile::tempdir().unwrap();
    // 新版里已经有一篇同名不一样的 b.txt。
    write(&new.path().join("root/kb/legacy/b.txt"), b"new");
    let mut core = Fake {
        exists: false,
        calls: Vec::new(),
    };
    let item = migrate(&mut core, &layout, new.path()).unwrap();
    let base = new.path().join("root/kb/legacy");
    assert_eq!(
        fs::read(base.join("notes/日语.md")).unwrap(),
        "# 日语".as_bytes()
    );
    assert_eq!(fs::read(base.join("a.docx")).unwrap(), b"docx");
    assert_eq!(fs::read(base.join("b-legacy.txt")).unwrap(), b"changed");
    assert_eq!(fs::read(base.join("b.txt")).unwrap(), b"new", "新版的不盖");
    assert!(!base.join("default-kb").exists(), "默认库不迁");
    assert_eq!(item.migrated, 3);
    let skipped: Vec<(&str, &str)> = item
        .skipped
        .iter()
        .map(|one| (one.key.as_str(), one.why.as_str()))
        .collect();
    assert_eq!(skipped, vec![("../escape.md", "bad-name")]);
    let note = item.note.unwrap();
    assert!(note.contains("新版这个库现在认得 2 篇"), "{note}");
    assert!(
        note.contains("第一次列出、搜这个库之前新版会先建索引"),
        "{note}"
    );
    assert!(note.contains("1 篇不是新版认的类型"), "{note}");
    assert!(note.contains("旧版内置的默认库 1 篇不迁"), "{note}");
    assert!(note.contains("b.txt → b-legacy.txt"), "{note}");
    assert!(note.contains("1 篇和旧清单记的 SHA-256 对不上"), "{note}");

    let mut again = Fake {
        exists: true,
        calls: Vec::new(),
    };
    let item = migrate(&mut again, &layout, new.path()).unwrap();
    assert_eq!(item.migrated, 3, "已经有一样的算迁过");
    let note = item.note.unwrap();
    assert!(
        note.contains("0 篇拷进了 `root/kb/legacy/`，3 篇以前迁过"),
        "{note}"
    );
    assert!(!note.contains("先建索引"), "这一次没拷新的，不说：{note}");
    assert!(!base.join("b-legacy-3.txt").exists(), "不再多拷一份");
}

#[test]
fn without_a_knowledge_base_the_core_is_not_asked() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join(".home-layout-v1"), b"owner\n");
    write(&dir.path().join("home/owner/conversation.db"), b"");
    let layout = crate::layout::read(dir.path()).unwrap();
    let mut core = Fake {
        exists: false,
        calls: Vec::new(),
    };
    let item = migrate(&mut core, &layout, dir.path()).unwrap();
    assert_eq!(item.status, Status::Skipped);
    assert!(core.calls.is_empty());
}

#[test]
fn only_the_builtin_wiki_makes_no_base() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join(".home-layout-v1"), b"owner\n");
    write(&dir.path().join("home/owner/conversation.db"), b"");
    let meta = dir.path().join("data/kb/kb_meta.db");
    fs::create_dir_all(meta.parent().unwrap()).unwrap();
    let conn = Connection::open(&meta).unwrap();
    conn.execute_batch(
        "CREATE TABLE files (name TEXT PRIMARY KEY, path TEXT NOT NULL, size_bytes INTEGER NOT NULL, mtime REAL NOT NULL, content_sha256 TEXT NOT NULL, updated_at REAL NOT NULL);
         INSERT INTO files VALUES ('default-kb/wiki.md', '/old', 1, 0, '', 0);",
    )
    .unwrap();
    let layout = crate::layout::read(dir.path()).unwrap();
    let mut core = Fake {
        exists: false,
        calls: Vec::new(),
    };
    let item = migrate(&mut core, &layout, dir.path()).unwrap();
    assert_eq!(item.status, Status::Skipped);
    assert!(item.note.unwrap().contains("只有内置的默认库（1 篇"));
    assert!(core.calls.is_empty(), "不建空库");
}

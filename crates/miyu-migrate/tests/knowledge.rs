//! 知识库迁进真核心（进程里起）：建了 `legacy` 这个库，拷进去的文档核心认得（`kb.list` 的篇数）；再跑一遍不多。

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
    let files = dir.path().join("data/kb/files");
    std::fs::create_dir_all(files.join("notes")).unwrap();
    std::fs::write(
        files.join("notes/arch.md"),
        "# Arch\n装 NVIDIA 驱动选 dkms 版。\n",
    )
    .unwrap();
    std::fs::write(files.join("todo.txt"), "买猫粮\n").unwrap();
    let conn = Connection::open(dir.path().join("data/kb/kb_meta.db")).unwrap();
    conn.execute_batch(
        "CREATE TABLE files (name TEXT PRIMARY KEY, path TEXT NOT NULL, size_bytes INTEGER NOT NULL, mtime REAL NOT NULL, content_sha256 TEXT NOT NULL, updated_at REAL NOT NULL);",
    )
    .unwrap();
    for name in ["notes/arch.md", "todo.txt"] {
        conn.execute(
            "INSERT INTO files VALUES (?1, '/old', 1, 0, '', 0)",
            params![name],
        )
        .unwrap();
    }
    dir
}

fn documents(core: &mut dyn Core) -> Option<u64> {
    let listed = core.call("kb.list", json!({})).unwrap();
    listed["bases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|base| base["id"] == "legacy")
        .and_then(|base| base["documents"].as_u64())
}

#[test]
fn documents_land_in_a_base_the_core_reads() {
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
        .find(|item| item["what"] == "knowledge")
        .unwrap()
        .clone();
    assert_eq!(item["migrated"], 2, "{item}");
    assert_eq!(documents(&mut core), Some(2));

    run::run(
        &Plan {
            at: "2026-11-04T08:00:01Z".parse().unwrap(),
            ..plan
        },
        &mut core,
    )
    .unwrap();
    assert_eq!(documents(&mut core), Some(2), "再跑一遍不多");
}

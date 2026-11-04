//! 表情包对真核心（进程里起）：miyu-memes 没装，迁移把包做好、留着那一行；做好的包核心认得出（`package.info {path}`），装得上，
//! 装上以后是给 miyu-memes 的内容包。

#![allow(clippy::unwrap_used, reason = "测试：读坏了就该停")]

mod support;

use serde_json::{Value, json};

use miyu_migrate::core::Core;
use miyu_migrate::run::{self, Plan};
use support::{connect, fresh};

fn old_root() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".home-layout-v1"), "owner\n").unwrap();
    std::fs::create_dir_all(dir.path().join("home/owner")).unwrap();
    std::fs::write(dir.path().join("home/owner/conversation.db"), "").unwrap();
    let library = dir.path().join("data/memes/cats");
    std::fs::create_dir_all(library.join("images")).unwrap();
    std::fs::write(library.join("images/a.png"), b"\x89PNG\r\n\x1a\n").unwrap();
    std::fs::write(
        library.join("index.json"),
        json!({"memes": [{"file": "images/a.png", "description": "一只猫", "usage": "开心的时候"}]}).to_string(),
    )
    .unwrap();
    dir
}

#[test]
fn the_package_is_made_and_the_core_takes_it() {
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
    let agent: Value =
        serde_json::from_str(&std::fs::read_to_string(&done.agent).unwrap()).unwrap();
    let item = agent["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["what"] == "memes")
        .unwrap()
        .clone();
    assert_eq!(item["migrated"], 0, "miyu-memes 没装：{item}");
    assert!(
        item["note"].as_str().unwrap().contains("miyu pkg install "),
        "{item}"
    );

    let pack = done.human.parent().unwrap().join("legacy-memes-cats");
    let path = pack.display().to_string();
    let info = core.call("package.info", json!({"path": path})).unwrap();
    assert_eq!(info["package"], "legacy-memes-cats", "{info}");
    let installed = core.call("package.install", json!({"path": path})).unwrap();
    assert_eq!(installed["package"], "legacy-memes-cats", "{installed}");
    let listed = core.call("package.list", json!({})).unwrap();
    assert!(
        listed["packages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|one| one["package"] == "legacy-memes-cats"),
        "{listed}"
    );
}

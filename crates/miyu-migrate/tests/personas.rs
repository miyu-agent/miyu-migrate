//! 人格迁进真核心（进程里起，照 miyu-im 测试的起法）：旧 Miyu 新建成 Miyu、提示词和示范对话照旧版的，`data/prompts/` 的
//! 一个新建一个，`dev` 映射到软件工程师；再跑一次什么都不建，映射照旧。

#![allow(clippy::unwrap_used, reason = "测试：读坏了就该停")]

mod support;

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use miyu_migrate::core::{Core, Live};
use miyu_migrate::personas::{LEGACY_PERSONA, parse_dialogs};
use miyu_migrate::run::{self, Plan};
use miyu_store::root::DataRoot;
use support::{connect, fresh};

/// 旧根（家目录布局）：一个自己建的人格「小猫」（带改过的示范对话），旧 Miyu 的角色扮演提示停用了。
fn old_root() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, text) in [
        (".home-layout-v1", "owner\n"),
        ("home/owner/conversation.db", ""),
        ("data/prompts/小猫.md", "你是一只小猫。\n"),
        (
            "data/prompts/dialogs/persona-x.md",
            "user: 不会用到\nassistant: 不会\n",
        ),
        ("data/prompts/hints/default.md", ""),
    ] {
        let path = dir.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    dir
}

fn plan(root: &DataRoot, old: &Path, state: PathBuf, at: &str) -> Plan {
    Plan {
        old: old.to_path_buf(),
        state,
        at: at.parse().unwrap(),
        root: root.clone(),
        records: miyu_migrate::records::Importer::new(|_| Err(std::io::Error::other("no im"))),
        home: None,
        space: run::free_space,
    }
}

fn read(core: &mut Live, persona: &str, prompt: &str) -> Value {
    core.call(
        "persona.read",
        json!({"persona": persona, "prompt": prompt}),
    )
    .unwrap()
}

#[test]
fn personas_land_in_the_core_once() {
    let fresh = fresh();
    let old = old_root();
    let state = fresh.root.path().join("var/lib/miyu-migrate");
    let mut core = connect(&fresh.root);
    let done = run::run(
        &plan(
            &fresh.root,
            old.path(),
            state.clone(),
            "2026-11-04T08:00:00Z",
        ),
        &mut core,
    )
    .unwrap();
    let agent: Value =
        serde_json::from_str(&std::fs::read_to_string(&done.agent).unwrap()).unwrap();
    let rows = agent["personas"].as_array().unwrap().clone();
    let by_old = |old: &str| rows.iter().find(|row| row["old"] == old).unwrap().clone();
    let miyu = by_old("default");
    assert_eq!(miyu["how"], "created", "{rows:?}");
    assert_eq!(by_old("dev")["new"], "software-engineer");
    let cat = rows
        .iter()
        .find(|row| row["name"] == "小猫")
        .unwrap()
        .clone();
    assert_eq!(cat["how"], "created");

    let miyu_id = miyu["new"].as_str().unwrap();
    let got = core
        .call("persona.get", json!({"persona": miyu_id}))
        .unwrap();
    assert_eq!(got["name"], "Miyu");
    assert_eq!(got["prompts"]["reminders"], false, "停用的没写");
    assert_eq!(
        read(&mut core, miyu_id, "persona")["text"],
        LEGACY_PERSONA.trim()
    );
    let pairs = read(&mut core, miyu_id, "examples")["pairs"].clone();
    let legacy = parse_dialogs(miyu_migrate::personas::LEGACY_DIALOGS);
    assert_eq!(pairs.as_array().unwrap().len(), legacy.len());
    assert_eq!(pairs[0]["user"], legacy[0].0.as_str());

    let cat_id = cat["new"].as_str().unwrap();
    assert_eq!(read(&mut core, cat_id, "persona")["text"], "你是一只小猫。");

    let before = core.call("persona.list", json!({})).unwrap()["personas"].clone();
    let again = run::run(
        &plan(&fresh.root, old.path(), state, "2026-11-04T08:00:01Z"),
        &mut core,
    )
    .unwrap();
    let agent: Value =
        serde_json::from_str(&std::fs::read_to_string(&again.agent).unwrap()).unwrap();
    for row in agent["personas"].as_array().unwrap() {
        if row["old"] != "dev" {
            assert_eq!(row["how"], "existing", "{row}");
        }
    }
    assert_eq!(agent["personas"][0]["new"], miyu_id);
    let after = core.call("persona.list", json!({})).unwrap()["personas"].clone();
    assert_eq!(before, after, "第二遍什么都没建");
}

//! 技能迁进真核心（进程里起）：拷进新数据根的技能核心认得（`skill.list` 里有，名字对）。

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
    let skill = dir.path().join("extensions/skills/Take_Notes");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("SKILL.md"),
        "---\nname: Take_Notes\ndescription: \"Keep meeting notes. Use when asked to take notes.\"\n---\n# Notes\n",
    )
    .unwrap();
    dir
}

#[test]
fn copied_skills_are_seen_by_the_core() {
    let fresh = fresh();
    let old = old_root();
    let mut core = connect(&fresh.root);
    let plan = Plan {
        old: old.path().to_path_buf(),
        state: fresh.root.path().join("var/lib/miyu-migrate"),
        at: "2026-11-04T08:00:00Z".parse().unwrap(),
        root: fresh.root.clone(),
        records: miyu_migrate::records::Importer::new(|_| Err(std::io::Error::other("no im"))),
        space: run::free_space,
    };
    run::run(&plan, &mut core).unwrap();
    let listed = core.call("skill.list", json!({})).unwrap();
    let names: Vec<Value> = listed["skills"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["name"].clone())
        .collect();
    assert!(names.contains(&json!("take-notes")), "{listed}");
}

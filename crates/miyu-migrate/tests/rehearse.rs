//! 演练（细则第十三节第 4 条）：拿旧目录的一份副本，迁进一个全新的数据根（核心在进程里起，不连真模型），跑两遍：第一遍印每一类
//! 迁了几条、人格怎么映射的（只有条数，不印内容）；第二遍什么都不建。平时不跑（`#[ignore]`），演练时照下面那样起：
//!
//! ```text
//! MIYU_MIGRATE_OLD=<旧目录的副本> MIYU_MIGRATE_SCRATCH=<放新数据根的目录> \
//!     cargo test --test rehearse -- --ignored --nocapture
//! ```
//!
//! 副本要先去掉 `config/config.jsonc` 里的 key、删掉 `daemon.lock`；演完新数据根自己删，副本由演练的人删。

#![allow(clippy::unwrap_used, reason = "演练：出错就该停")]

mod support;

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Value, json};

use miyu_migrate::core::Core;
use miyu_migrate::run::{self, Plan};

/// 报告里每一类、每种映射的条数（不带内容）。
fn summary(agent: &Value) -> String {
    let mut out = String::new();
    for item in agent["items"].as_array().unwrap() {
        let mut whys: BTreeMap<String, usize> = BTreeMap::new();
        for skip in item["skipped"].as_array().into_iter().flatten() {
            *whys
                .entry(skip["why"].as_str().unwrap().to_string())
                .or_default() += 1;
        }
        out.push_str(&format!(
            "  {} {}：迁了 {}，跳过 {whys:?}\n",
            item["what"].as_str().unwrap(),
            item["status"].as_str().unwrap(),
            item["migrated"],
        ));
        // 补充只有条数（拼的时候就不带内容）。
        if let Some(note) = item["note"].as_str() {
            out.push_str(&format!("    {note}\n"));
        }
    }
    let mut hows: BTreeMap<String, usize> = BTreeMap::new();
    for row in agent["personas"].as_array().unwrap() {
        *hows
            .entry(row["how"].as_str().unwrap().to_string())
            .or_default() += 1;
    }
    out.push_str(&format!("  人格映射：{hows:?}\n"));
    out
}

fn agent(done: &run::Done) -> Value {
    serde_json::from_str(&std::fs::read_to_string(&done.agent).unwrap()).unwrap()
}

#[test]
#[ignore = "演练：要 MIYU_MIGRATE_OLD、MIYU_MIGRATE_SCRATCH"]
fn rehearse() {
    let old = PathBuf::from(std::env::var_os("MIYU_MIGRATE_OLD").expect("MIYU_MIGRATE_OLD"));
    let scratch =
        PathBuf::from(std::env::var_os("MIYU_MIGRATE_SCRATCH").expect("MIYU_MIGRATE_SCRATCH"));
    let fresh = support::fresh_in(&scratch);
    let state = fresh.root.path().join("var/lib/miyu-migrate");
    let mut core = support::connect(&fresh.root);
    let plan = |at: &str| Plan {
        old: old.clone(),
        state: state.clone(),
        at: at.parse().unwrap(),
        root: fresh.root.clone(),
        space: run::free_space,
    };
    let first = run::run(&plan("2026-11-04T08:00:00Z"), &mut core).unwrap();
    let first = agent(&first);
    println!(
        "第一遍：压缩包 {} 个文件\n{}",
        first["archive"]["files"],
        summary(&first)
    );
    let before = core.call("persona.list", json!({})).unwrap()["personas"].clone();
    let second = run::run(&plan("2026-11-04T08:00:01Z"), &mut core).unwrap();
    let second = agent(&second);
    println!("第二遍：\n{}", summary(&second));
    let after = core.call("persona.list", json!({})).unwrap()["personas"].clone();
    assert_eq!(before, after, "第二遍什么都没建");
    assert!(
        second["personas"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["how"] != "created"),
        "第二遍全是映射"
    );
}

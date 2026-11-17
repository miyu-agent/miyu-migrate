//! 配置和密钥（细则第四节）：旧版 `config/config.jsonc` 里新版用得上的几样补进新版的系统配置，只补新版没有的。
//!
//! - 密钥先 `secret.list`，名字已经设了的跳过，没有的 `secret.set`；值只交给核心，报告、日志里只写名字。
//! - 设置一项一发 `config.set`（一次几项是全成或全不写），每项带 `expect: {}`：新版这一层已经写了的回 `config_conflict`，算新版
//!   配过、跳过、列进报告；新版不收的（写法、范围）也跳过、写原因码。
//! - 写完 `config.get` 读回比一遍，对不上的数出来。
//! - 新版已经有同编号的供应商整家跳过；旧版的 CLI 中转、新版没有对应的整段只报。

mod plan;

use std::collections::BTreeSet;

use serde_json::{Value, json};

use crate::core::{Core, Said};
use crate::layout::Layout;
use crate::report::{Item, Skip, Status};
use crate::words::Language;

pub use plan::{Change, Planned, Secret, new_id, plan, segment};

/// 迁配置：读 `copy` 的旧配置和属主档案，写进 `core`。交回报告的一项（只有名字和条数）。
///
/// # Errors
///
/// 连接断了。
pub fn migrate(core: &mut dyn Core, copy: &Layout, language: Language) -> Result<Item, Said> {
    let words = crate::words::setup::settings(language);
    let path = copy.config();
    let source = path
        .strip_prefix(&copy.root)
        .unwrap_or(&path)
        .to_string_lossy()
        .replace('\\', "/");
    let mut item = Item {
        what: "config".to_string(),
        status: Status::Migrated,
        source,
        format: words.format.to_string(),
        migrated: 0,
        skipped: Vec::new(),
        target: Some(words.target.to_string()),
        note: None,
    };
    let Some(old) = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| crate::jsonc::parse(&text))
    else {
        item.status = Status::Skipped;
        item.note = Some(words.none.to_string());
        return Ok(item);
    };
    let profile = std::fs::read_to_string(copy.profile()).ok();
    let current = core.call("config.get", json!({}))?;
    let existing: BTreeSet<String> = current["items"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(key, _)| {
            key.strip_prefix("providers.")?
                .split('.')
                .next()
                .map(|id| id.trim_matches('"').to_string())
        })
        .collect();
    let planned = plan(&old, profile.as_deref(), &existing);
    let mut notes = Vec::new();
    // 密钥：名字已经设了的跳过。
    let listed = core.call("secret.list", json!({}))?;
    let set: BTreeSet<String> = listed["secrets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|one| one["set"] == true)
        .filter_map(|one| one["name"].as_str().map(str::to_string))
        .collect();
    let mut secrets = 0;
    for secret in &planned.secrets {
        if set.contains(&secret.name) {
            item.skipped
                .push(skip(&format!("secret:{}", secret.name), "already-set"));
            continue;
        }
        match core.call(
            "secret.set",
            json!({"name": secret.name, "value": secret.value}),
        ) {
            Ok(_) => secrets += 1,
            Err(said) if fatal(&said) => return Err(said),
            Err(said) => item
                .skipped
                .push(skip(&format!("secret:{}", secret.name), &said.code)),
        }
    }
    // 设置：一项一发，带 expect。
    let mut written = Vec::new();
    for change in &planned.changes {
        let params = json!({"changes": [{"key": change.key, "value": change.value, "expect": {}}]});
        match core.call("config.set", params) {
            Ok(_) => written.push(change),
            Err(said) if fatal(&said) => return Err(said),
            Err(said) if said.code == "config_conflict" => {
                item.skipped.push(skip(&change.key, "already-set"))
            }
            Err(said) => item.skipped.push(skip(&change.key, &said.code)),
        }
    }
    // 读回比一遍。
    let after = core.call("config.get", json!({}))?;
    let differ = written
        .iter()
        .filter(|change| !same(&after["items"][&change.key]["value"], &change.value))
        .count();
    item.migrated = (written.len() + secrets) as u64;
    for (key, why) in &planned.left {
        item.skipped.push(skip(key, why));
    }
    notes.push((words.written)(written.len(), secrets));
    if !planned.renamed.is_empty() {
        let renamed: Vec<String> = planned
            .renamed
            .iter()
            .map(|(old, new)| format!("{old} → {new}"))
            .collect();
        notes.push((words.renamed)(&language.list(&renamed)));
    }
    if differ > 0 {
        notes.push((words.differ)(differ));
    }
    if !item.skipped.is_empty() {
        item.status = Status::Partial;
    }
    item.note = Some(language.sentences(&notes));
    Ok(item)
}

/// 读回的和写的一样吗：数照数值比（文件里的 `1.0` 和 `1` 算一样）。
fn same(got: &Value, wrote: &Value) -> bool {
    match (got.as_f64(), wrote.as_f64()) {
        (Some(left), Some(right)) => (left - right).abs() < 1e-9,
        _ => got == wrote,
    }
}

fn fatal(said: &Said) -> bool {
    matches!(
        said.code.as_str(),
        "io" | "disconnected" | "no_answer" | "unreachable"
    )
}

fn skip(key: &str, why: &str) -> Skip {
    Skip {
        key: key.to_string(),
        why: why.to_string(),
    }
}

#[cfg(test)]
mod tests;

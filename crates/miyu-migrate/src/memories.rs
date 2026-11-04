//! 记忆（细则第三节、记忆类别对照）：每个旧人格的 `memory.db` 里的知识点 `facts` 判成新版的一条，经 `memory.import` 导进映射到
//! 的那个新人格（一批不超过 500 条），再 `memory.dream` 合掉和新版已经记下的重复。`origin` 是 `miyu-legacy:<旧人格>:facts:<id>`：
//! 重跑照它跳过，删掉的也不再导回来。
//!
//! 不导的：`self`、不是 `active` 的、超过 120 字的（不拆、不调模型：给 agent 读的那份报告里写在压缩包的哪个库哪一行，以后改短了再
//! 照同一个 `origin` 导）。日记 `episodes`、改动记录、向量这些只在压缩包里。

mod read;

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

use crate::core::{Core, Said};
use crate::layout::Layout;
use crate::personas::Mapped;
use crate::report::{Item, Pending, Skip, Status};

pub use read::{Fact, Judged, Raw, judge, read};

/// 一批最多几条（`memory.import` 的上限）。
const BATCH: usize = 500;
/// 整理忙着的，隔多久再问。
const BUSY_WAIT: Duration = Duration::from_secs(10);
/// 最多问几次（五分钟）。
const BUSY_TRIES: u32 = 30;

/// 旧格式一句话。
const FORMAT: &str = "SQLite（没有 user_version）：facts 一行一条（content、memory_type fact/preference/relationship/task/self/other、status、updated_at、tags JSON 列表、subjects JSON [{name, principal}]、owner_principal），episodes 日记（owner_principal、origin_platform、origin_sender_id 换得出 QQ 号）、memory_revisions、memory_embeddings 只在压缩包里";

/// 迁记忆：读 `copy` 的每个旧人格的库，人格照 `mapped`，导进 `core`。`pause` 是等整理时睡的（测试换成不睡的）。交回报告的一项、
/// 留给以后接着迁的（超长的）。
///
/// # Errors
///
/// 连接断了。
pub fn migrate(
    core: &mut dyn Core,
    copy: &Layout,
    mapped: &[Mapped],
    pause: fn(Duration),
) -> Result<(Item, Vec<Pending>), Said> {
    let dir = copy.personas();
    let source = dir
        .strip_prefix(&copy.root)
        .unwrap_or(&dir)
        .to_string_lossy()
        .replace('\\', "/");
    let mut item = Item {
        what: "memories".to_string(),
        status: Status::Migrated,
        source: format!("{source}/<人格>/memory/memory.db"),
        format: FORMAT.to_string(),
        migrated: 0,
        skipped: Vec::new(),
        target: Some("memory.import".to_string()),
        note: None,
    };
    let mut pending = Vec::new();
    let mut notes = Vec::new();
    let mut scopes: Vec<String> = fs::read_dir(&dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| entry.path().join("memory").join("memory.db").is_file())
                .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    scopes.sort();
    // 没有记忆库的不问核心。
    let owners = if scopes.is_empty() {
        HashSet::new()
    } else {
        owners(core, copy)
    };
    for scope in &scopes {
        let db = dir.join(scope).join("memory").join("memory.db");
        let at = format!("{source}/{scope}/memory/memory.db");
        let facts = match read(&db, scope, &owners) {
            Ok(facts) => facts,
            Err(error) => {
                notes.push(format!("`{scope}` 的库读不了（{error}），留在压缩包里。"));
                item.status = Status::Partial;
                continue;
            }
        };
        if facts.is_empty() {
            continue;
        }
        let Some(persona) = mapped
            .iter()
            .find(|row| row.old == *scope)
            .and_then(|row| row.new.clone())
        else {
            notes.push(format!(
                "`{scope}` 没映射上新人格，{} 条没导。",
                facts.len()
            ));
            item.skipped
                .extend(facts.iter().map(|fact| skip(&fact.origin, "unmapped")));
            continue;
        };
        let outcome = one(core, &persona, &facts, pause)?;
        for fact in &facts {
            if let Judged::Skip(why, kind) = &fact.judged {
                item.skipped.push(skip(&fact.origin, why));
                if *why == "too-long" {
                    pending.push(Pending {
                        what: "memory-too-long".to_string(),
                        source: at.clone(),
                        table: Some("facts".to_string()),
                        key: fact.id.to_string(),
                        note: Some(format!("origin {}，原来的类 {kind}", fact.origin)),
                    });
                }
            }
        }
        item.migrated += outcome.imported;
        item.skipped
            .extend(outcome.refused.iter().map(|origin| skip(origin, "refused")));
        notes.push(outcome.note(scope, &persona, &facts));
        if outcome.stopped {
            item.status = Status::Partial;
        }
    }
    if scopes.is_empty() {
        item.status = Status::Skipped;
        notes.push("旧根里没有记忆库。".to_string());
    }
    if owners.is_empty() && !scopes.is_empty() {
        notes.push("没找到主人的号（新版 `[external.bindings]`、旧版 `platforms.qq.admin_users` 都没有）：说的是谁的一律当别人。".to_string());
    }
    if item.status == Status::Migrated && item.skipped.iter().any(|skip| skip.why != "duplicate") {
        item.status = Status::Partial;
    }
    item.note = Some(notes.join(""));
    Ok((item, pending))
}

/// 一个人格导完的样子。
#[derive(Debug, Default)]
struct Outcome {
    imported: u64,
    duplicate: u64,
    /// 核心整批拒了的（`bad_params` 之类）那一批的 origin。
    refused: Vec<String>,
    /// 整理的回执，或者为什么没整理。
    dream: String,
    /// 记忆软件包没装、整批被拒：没导完。
    stopped: bool,
}

impl Outcome {
    /// 给人看的一句（只有条数）。
    fn note(&self, scope: &str, persona: &str, facts: &[Fact]) -> String {
        let mut guessed: std::collections::BTreeMap<String, u64> =
            std::collections::BTreeMap::new();
        let mut by_name = 0;
        let mut skipped: std::collections::BTreeMap<&str, u64> = std::collections::BTreeMap::new();
        for fact in facts {
            match &fact.judged {
                Judged::Import {
                    guessed: Some((suggested, class)),
                    ..
                } => *guessed.entry(format!("{suggested}→{class}")).or_default() += 1,
                Judged::Import { by_name: true, .. } => by_name += 1,
                Judged::Skip(why, _) => *skipped.entry(why).or_default() += 1,
                Judged::Import { .. } => {}
            }
        }
        let mut note = format!(
            "`{scope}` → `{persona}`：导进 {} 条，以前导过 {} 条",
            self.imported, self.duplicate
        );
        if !skipped.is_empty() {
            note.push_str(&format!("，没导 {skipped:?}"));
        }
        if !guessed.is_empty() {
            note.push_str(&format!("；没写是谁、照推荐导的（建议→导成）{guessed:?}"));
        }
        if by_name > 0 {
            note.push_str(&format!("；{by_name} 条号换不出、记成名字"));
        }
        note.push_str(&format!("；{}。", self.dream));
        note
    }
}

/// 一个人格：照批导，再整理。
fn one(
    core: &mut dyn Core,
    persona: &str,
    facts: &[Fact],
    pause: fn(Duration),
) -> Result<Outcome, Said> {
    let items: Vec<(&str, &Value)> = facts
        .iter()
        .filter_map(|fact| match &fact.judged {
            Judged::Import { item, .. } => Some((fact.origin.as_str(), item)),
            Judged::Skip(..) => None,
        })
        .collect();
    let mut outcome = Outcome::default();
    for batch in items.chunks(BATCH) {
        let sent: Vec<&Value> = batch.iter().map(|(_, item)| *item).collect();
        match core.call("memory.import", json!({"persona": persona, "items": sent})) {
            Ok(answer) => {
                outcome.imported += answer["imported"].as_u64().unwrap_or_default();
                outcome.duplicate += answer["skipped"].as_u64().unwrap_or_default();
            }
            Err(said) if fatal(&said) => return Err(said),
            Err(said) => {
                outcome
                    .refused
                    .extend(batch.iter().map(|(origin, _)| (*origin).to_string()));
                outcome.stopped = true;
                outcome.dream = format!("核心拒了（{}），没整理", said.code);
                return Ok(outcome);
            }
        }
    }
    outcome.dream = dream(core, persona, pause)?;
    Ok(outcome)
}

/// 整理一次：忙着的隔十秒再问，最多五分钟；等不到的不算失败。交回给人看的回执。
fn dream(core: &mut dyn Core, persona: &str, pause: fn(Duration)) -> Result<String, Said> {
    for _ in 0..BUSY_TRIES {
        match core.call("memory.dream", json!({"persona": persona})) {
            Ok(answer) => {
                return Ok(format!(
                    "整理了：交进 {}、作废 {}、改了 {}",
                    answer["given"].as_u64().unwrap_or_default(),
                    answer["retired"].as_u64().unwrap_or_default(),
                    answer["revised"].as_u64().unwrap_or_default()
                ));
            }
            Err(said) if said.code == "memory_busy" => pause(BUSY_WAIT),
            Err(said) if fatal(&said) => return Err(said),
            Err(said) => {
                return Ok(format!(
                    "整理没成（{}），以后照整理的间隔会再整理",
                    said.code
                ));
            }
        }
    }
    Ok("整理还在跑（等了五分钟），以后照整理的间隔会再整理".to_string())
}

/// 连接断了、没回：整个迁移停下。
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

/// 主人的号（`qq:<号>`）：新版系统配置 `[external.bindings]` 里写了的；没有的看旧配置 `platforms.qq.admin_users`。
fn owners(core: &mut dyn Core, copy: &Layout) -> HashSet<String> {
    let mut found = HashSet::new();
    if let Ok(answer) = core.call("config.get", json!({})) {
        for (key, entry) in answer["items"].as_object().into_iter().flatten() {
            if key == "external.bindings" {
                for (who, value) in entry["value"].as_object().into_iter().flatten() {
                    if value.as_str().is_some_and(|value| !value.is_empty()) {
                        found.insert(who.clone());
                    }
                }
            } else if let Some(who) = key.strip_prefix("external.bindings.")
                && entry["value"]
                    .as_str()
                    .is_some_and(|value| !value.is_empty())
            {
                found.insert(who.to_string());
            }
        }
    }
    if found.is_empty() {
        found.extend(old_admins(&copy.config()));
    }
    found
}

/// 旧配置（JSONC）里 `platforms.qq.admin_users` 的号。读不了、没写的是空的。
fn old_admins(path: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(config) = serde_json::from_str::<Value>(&strip_comments(&text)) else {
        return Vec::new();
    };
    config["platforms"]["qq"]["admin_users"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|one| match one {
            Value::String(qq) => Some(qq.clone()),
            Value::Number(qq) => Some(qq.to_string()),
            _ => None,
        })
        .filter(|qq| !qq.is_empty())
        .map(|qq| format!("qq:{qq}"))
        .collect()
}

/// JSONC 去掉 `//`、`/* */` 注释和对象、数组末尾多的逗号（字符串里的不动）。
fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(ch) = chars.next() {
        if in_string {
            out.push(ch);
            if ch == '\\' {
                out.extend(chars.next());
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match (ch, chars.peek()) {
            ('"', _) => {
                in_string = true;
                out.push(ch);
            }
            ('/', Some('/')) => while chars.next_if(|next| *next != '\n').is_some() {},
            ('/', Some('*')) => {
                chars.next();
                let mut last = ' ';
                for next in chars.by_ref() {
                    if last == '*' && next == '/' {
                        break;
                    }
                    last = next;
                }
            }
            _ => out.push(ch),
        }
    }
    // 末尾多的逗号：`,` 后面跟着（隔着空白）`}` 或 `]` 的去掉。
    let mut cleaned = String::with_capacity(out.len());
    let bytes: Vec<char> = out.chars().collect();
    let mut in_string = false;
    for (n, ch) in bytes.iter().enumerate() {
        if in_string {
            if *ch == '"' && (n == 0 || bytes[n - 1] != '\\') {
                in_string = false;
            }
            cleaned.push(*ch);
            continue;
        }
        if *ch == '"' {
            in_string = true;
        }
        if *ch == ',' {
            let next = bytes[n + 1..].iter().find(|c| !c.is_whitespace());
            if matches!(next, Some('}' | ']')) {
                continue;
            }
        }
        cleaned.push(*ch);
    }
    cleaned
}

#[cfg(test)]
mod tests;

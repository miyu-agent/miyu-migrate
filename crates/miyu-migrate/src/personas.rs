//! 人格（细则第六节、第十一节已定第 2 条）：旧人格映射成新人格，会话、记忆都照这张表写新编号。
//!
//! - 旧 `default`（旧版的 Miyu，提示词编进二进制、从不落盘）新建成名字叫 Miyu 的人格，提示词取迁移包带的旧版原文
//!   （`legacy/`，旧版仓库 `5df19e2f` 的 `src/prompts/`），人改过的（`hints/default.md`、`dialogs/default.md`）盖在上面；
//! - 旧 `dev` 映射到出厂的 `software-engineer`，不改它；
//! - `data/prompts/<名字>.md`（`system-prompt.md` 除外）每一个新建一个；
//! - 新版已经有同名人格的不建、映射到它、不改它：重跑也就是这样，不重复建。

mod dialogs;
mod read;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::core::{Core, Said};

pub use dialogs::parse as parse_dialogs;
pub use read::{Extra, Found, read, scope_name};

/// 出厂的软件工程师：旧 `dev` 映射到它。
pub const ENGINEER: &str = "software-engineer";

/// 旧版 Miyu 的人设、角色扮演提示、示范对话（旧版仓库 `src/prompts/` 原样）。
pub const LEGACY_PERSONA: &str = include_str!("../legacy/miyu.md");
/// 同上，角色扮演提示。
pub const LEGACY_HINT: &str = include_str!("../legacy/miyu.hint.md");
/// 同上，示范对话。
pub const LEGACY_DIALOGS: &str = include_str!("../legacy/miyu-dialogs.md");

/// 一个旧人格：要建成什么样。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Old {
    /// 旧 scope：会话库、记忆目录里写的那个。
    pub scope: String,
    /// 新人格的名字（同名的已经有了就映射到它）。
    pub name: String,
    /// 哪一种。
    pub kind: Kind,
    /// 人设。
    pub persona: Option<String>,
    /// 角色扮演提示（旧版的手写提醒）。空的不写。
    pub reminders: Option<String>,
    /// 示范对话，一对一对。
    pub examples: Vec<(String, String)>,
    /// 头像在副本里的位置。
    pub avatar: Option<std::path::PathBuf>,
}

/// 旧人格的哪一种。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// 旧版的 Miyu（`default`）。
    Miyu,
    /// 开发模式（`dev`）：映射到出厂的软件工程师。
    Dev,
    /// `data/prompts/<名字>.md`。
    Prompt,
}

/// 映射表的一行（写进给 agent 的报告）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mapped {
    /// 旧 scope。
    pub old: String,
    /// 名字。
    pub name: String,
    /// 新编号；没映射上的是空的。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new: Option<String>,
    /// 怎么来的。
    pub how: How,
    /// 没建成、头像没传上这样的一句。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// 映射是怎么来的。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum How {
    /// 这一次新建的。
    Created,
    /// 新版已经有同名的。
    Existing,
    /// 出厂的软件工程师。
    Engineer,
    /// 没映射上（出厂的被卸了、建的时候核心拒了）。
    Missing,
}

/// 照 `olds` 建、映射，交回映射表（顺序同 `olds`）。一个建不成的不挡别的，写进那一行的 `note`。
///
/// # Errors
///
/// 列不出新版的人格（`persona.list` 被拒、连接断了）：[`Said`]。
pub fn apply(core: &mut dyn Core, olds: &[Old]) -> Result<Vec<Mapped>, Said> {
    let listed = core.call("persona.list", json!({}))?;
    let existing: Vec<(String, Option<String>)> = listed["personas"]
        .as_array()
        .map(|all| {
            all.iter()
                .filter_map(|one| {
                    let id = one["persona"].as_str()?.to_string();
                    Some((id, one["name"].as_str().map(str::to_string)))
                })
                .collect()
        })
        .unwrap_or_default();
    let mut mapped = Vec::with_capacity(olds.len());
    for old in olds {
        mapped.push(one(core, old, &existing));
    }
    Ok(mapped)
}

/// 一个旧人格：映射到已有的，或者新建。
fn one(core: &mut dyn Core, old: &Old, existing: &[(String, Option<String>)]) -> Mapped {
    let row = |new: Option<String>, how: How, note: Option<String>| Mapped {
        old: old.scope.clone(),
        name: old.name.clone(),
        new,
        how,
        note,
    };
    if old.kind == Kind::Dev {
        return if existing.iter().any(|(id, _)| id == ENGINEER) {
            row(Some(ENGINEER.to_string()), How::Engineer, None)
        } else {
            row(
                None,
                How::Missing,
                Some(format!("新版没有 {ENGINEER}（出厂的包被卸了）")),
            )
        };
    }
    if let Some((id, _)) = existing
        .iter()
        .find(|(_, name)| name.as_deref() == Some(old.name.as_str()))
    {
        return row(Some(id.clone()), How::Existing, None);
    }
    let created = match core.call("persona.set", create_params(old)) {
        Ok(created) => created,
        Err(said) => return row(None, How::Missing, Some(said.to_string())),
    };
    let Some(id) = created["persona"].as_str().map(str::to_string) else {
        return row(
            None,
            How::Missing,
            Some("persona.set 没交回编号".to_string()),
        );
    };
    let note = old
        .avatar
        .as_ref()
        .and_then(|avatar| set_avatar(core, &id, avatar).err())
        .map(|said| format!("头像没传上：{said}"));
    row(Some(id), How::Created, note)
}

/// 新建的参数：名字，有的提示词。
fn create_params(old: &Old) -> Value {
    let mut prompts = Map::new();
    if let Some(text) = old.persona.as_deref().filter(|text| !text.is_empty()) {
        prompts.insert("persona".to_string(), json!({"text": text}));
    }
    if let Some(text) = old.reminders.as_deref().filter(|text| !text.is_empty()) {
        prompts.insert("reminders".to_string(), json!({"text": text}));
    }
    if !old.examples.is_empty() {
        let pairs: Vec<Value> = old
            .examples
            .iter()
            .map(|(user, assistant)| json!({"user": user, "assistant": assistant}))
            .collect();
        prompts.insert("examples".to_string(), json!({"pairs": pairs}));
    }
    let mut params = json!({"changes": [{"key": "persona.name", "value": old.name}]});
    if !prompts.is_empty() {
        params["prompts"] = Value::Object(prompts);
    }
    params
}

/// 传头像：先放进 blob 库，再写给人格。
fn set_avatar(core: &mut dyn Core, persona: &str, path: &std::path::Path) -> Result<(), Said> {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let put = core.call("blob.put", json!({"path": absolute.display().to_string()}))?;
    let blob = put["blob"].clone();
    core.call(
        "persona.set",
        json!({"persona": persona, "avatar": {"blob": blob}}),
    )
    .map(|_| ())
}

#[cfg(test)]
mod tests;

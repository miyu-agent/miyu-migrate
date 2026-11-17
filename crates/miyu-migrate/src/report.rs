//! 两份报告（细则第十二节，DESIGN 第六节第 9 条）：给人看的 `报告.md`，给 agent 读的 `给agent.json`（英文的叫 `report.md`、
//! `for-agent.json`，MG-21）。都只写位置、条数和
//! 为什么，不带旧数据的内容。每一类迁完往里加一项；迁不了的不硬迁，写明它在压缩包里的哪个路径、旧格式是什么样。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::words::Language;

/// 给 agent 读的那份的格式版本。
pub const FORMAT: u32 = 1;

/// 一次迁移的报告。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    /// 格式版本。
    pub format: u32,
    /// 迁移程序的版本。
    pub migrate: String,
    /// 迁移的时刻，RFC 3339。
    pub at: String,
    /// 压缩包。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive: Option<Archive>,
    /// 旧根是什么样。
    pub old_root: OldRoot,
    /// 旧人格映射成了哪个新人格（会话、记忆照它写）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub personas: Vec<crate::personas::Mapped>,
    /// 每一类，照迁的先后。一类只一项（`what` 不重复，给人的那份照它印节标题）：同一类有几份文件的，`source` 用「、」并起来。
    pub items: Vec<Item>,
    /// 留给以后接着迁的（例如超长没导的记忆），只有位置。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending: Vec<Pending>,
}

/// 压缩包在哪、多大、整份的哈希。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Archive {
    /// 绝对路径。
    pub path: String,
    /// 整个包的 SHA-256。
    pub sha256: String,
    /// 打进去几个文件。
    pub files: u64,
    /// 文件一共多少字节（打包前）。
    pub bytes: u64,
    /// 逐文件清单在哪（绝对路径）。
    pub manifest: String,
}

/// 旧根：在哪、哪一代布局、管理员家目录名。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OldRoot {
    /// 绝对路径。
    pub path: String,
    /// 布局：`home-layout-v1` 这样。
    pub layout: String,
    /// 管理员家目录名。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin: Option<String>,
    /// 回滚过家目录布局。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub rolled_back: bool,
}

/// 一类东西迁成了什么样。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// 哪一类：`sessions`、`qq-records`、`memories` 这样。
    pub what: String,
    /// 怎么样了。
    pub status: Status,
    /// 在压缩包里的相对路径；几份的用「、」并起来。
    pub source: String,
    /// 旧格式一句话：库的表名和版本、JSON 的结构。
    pub format: String,
    /// 迁了几条。
    pub migrated: u64,
    /// 没迁的，每条为什么。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<Skip>,
    /// 迁到哪（新数据根里的相对路径，或者协议方法）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// 给人看的一句补充。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// 一类东西怎么样了。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// 迁了。
    Migrated,
    /// 迁了一部分，有跳过的。
    Partial,
    /// 没迁，在压缩包里。
    Archived,
    /// 没迁，不需要（派生的、旧版自己的）。
    Skipped,
}

impl Status {
    /// 给人看的状态（字在 [`crate::words::report`]）。
    fn shown(self, language: Language) -> &'static str {
        let status = &crate::words::report::words(language).status;
        match self {
            Status::Migrated => status[0],
            Status::Partial => status[1],
            Status::Archived => status[2],
            Status::Skipped => status[3],
        }
    }
}

/// 没迁的一条：哪一条（旧编号、路径）、为什么。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skip {
    /// 旧编号或者相对路径。
    pub key: String,
    /// 为什么：一个短词，例如 `summary`、`owner`、`too_long`。
    pub why: String,
}

/// 留给以后接着迁的一条：只有位置和类别。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending {
    /// 哪一种：例如 `memory-too-long`。
    pub what: String,
    /// 在压缩包里的哪个文件。
    pub source: String,
    /// 哪张表（库里的）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    /// 哪一行、哪个编号。
    pub key: String,
    /// 补充（例如原来的类、导入时会用的 `origin`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Report {
    /// 给人看的那份：一段话、每一类一节、没迁的一张表；字照 `language`（[`crate::words::report`]），各类的那一句迁的时候已经照
    /// 同一个语言写好了。
    pub fn human(&self, language: Language) -> String {
        let words = crate::words::report::words(language);
        let mut out = (words.intro)(&self.at, &self.old_root.path, &self.old_root.layout);
        if let Some(archive) = &self.archive {
            out.push_str(&(words.archive_intro)(&archive.path));
        }
        if !self.personas.is_empty() {
            out.push_str(words.personas);
            for row in &self.personas {
                let how = match row.how {
                    crate::personas::How::Created => words.how[0],
                    crate::personas::How::Existing => words.how[1],
                    crate::personas::How::Engineer => words.how[2],
                    crate::personas::How::Missing => words.how[3],
                };
                out.push_str(&format!(
                    "| `{}` | {} | {} | {how}{} |\n",
                    row.old,
                    row.name,
                    row.new
                        .as_deref()
                        .map_or("—".to_string(), |new| format!("`{new}`")),
                    row.note.as_deref().map_or(String::new(), words.how_note),
                ));
            }
            out.push('\n');
        }
        out.push_str(words.items);
        // 派生的、数据根外面的另外写：派生的合成一行，数据根外面的单独一节。给 agent 的那份照样逐条。
        let derived = self
            .items
            .iter()
            .filter(|item| item.what.starts_with("derived/"))
            .count();
        let outside: Vec<&Item> = self
            .items
            .iter()
            .filter(|item| item.what.starts_with("outside/"))
            .collect();
        for item in self
            .items
            .iter()
            .filter(|item| !item.what.starts_with("derived/") && !item.what.starts_with("outside/"))
        {
            out.push_str(&(words.item)(
                &item.what,
                item.status.shown(language),
                item.migrated,
            ));
            if !item.skipped.is_empty() {
                out.push_str(&(words.skipped)(item.skipped.len()));
            }
            out.push_str(words.end);
            if let Some(target) = &item.target {
                out.push_str(&(words.target)(target));
            }
            if let Some(note) = &item.note {
                out.push_str(&format!("{note}\n"));
            }
            out.push('\n');
        }
        if derived > 0 {
            out.push_str(&(words.derived)(derived));
        }
        if !outside.is_empty() {
            out.push_str(words.outside);
            for item in outside {
                out.push_str(&format!(
                    "- `{}`{}{}\n",
                    item.source,
                    words.outside_colon,
                    item.note.as_deref().unwrap_or_default()
                ));
            }
            out.push('\n');
        }
        let left: Vec<&Item> = self
            .items
            .iter()
            .filter(|item| matches!(item.status, Status::Archived | Status::Partial))
            .collect();
        if !left.is_empty() {
            out.push_str(&(words.left)(words.agent_file));
            for item in left {
                out.push_str(&format!(
                    "| {} | `{}` | {} |\n",
                    item.what,
                    shown_source(&item.source, language),
                    item.format
                ));
            }
            out.push('\n');
        }
        out.push_str(&(words.old_root)(&self.old_root.path));
        if let Some(archive) = &self.archive {
            out.push_str(&(words.archive)(
                &archive.path,
                archive.files,
                &language.human_size(archive.bytes),
                &archive.sha256,
                &archive.manifest,
            ));
        }
        out.push_str(words.delete_later);
        out
    }

    /// 给 agent 读的那份：整个报告的 JSON。
    ///
    /// # Errors
    ///
    /// 写不成 JSON（实际不会）。
    pub fn agent(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    /// 两份都写进 `dir`，交回两个文件的位置；文件名照 `language`（`报告.md`、`给agent.json`，英文 `report.md`、`for-agent.json`）。
    ///
    /// # Errors
    ///
    /// 写不进去。
    pub fn write(&self, dir: &Path, language: Language) -> io::Result<(PathBuf, PathBuf)> {
        fs::create_dir_all(dir)?;
        let words = crate::words::report::words(language);
        let human = dir.join(words.human_file);
        let agent = dir.join(words.agent_file);
        fs::write(&human, self.human(language))?;
        fs::write(&agent, self.agent().map_err(io::Error::other)? + "\n")?;
        Ok((human, agent))
    }
}

/// 给人看的来源：几份的在给 agent 的那份里照旧是「、」并起来的，给人看的照语言重新列（英文用逗号）。
fn shown_source(source: &str, language: Language) -> String {
    language.list(&source.split('、').collect::<Vec<_>>())
}

/// 报告里给人照抄进终端的路径：只有字母、数字和 `/._-+:,@%~` 的照原样，别的（空格、中文目录名带的空格这些）加引号。没有单引号的用
/// 单引号（fish、bash、zsh、PowerShell 都照字面）；带单引号的用双引号，里面的 `"`、`$`、`\`、`` ` `` 前面加反斜杠（fish、bash、zsh）。
#[must_use]
pub fn shell_path(path: &Path) -> String {
    let text = path.display().to_string();
    let plain = !text.is_empty()
        && text
            .chars()
            .all(|one| one.is_ascii_alphanumeric() || "/._-+:,@%~".contains(one));
    if plain {
        return text;
    }
    if !text.contains('\'') {
        return format!("'{text}'");
    }
    let mut quoted = String::from("\"");
    for one in text.chars() {
        if matches!(one, '"' | '$' | '\\' | '`') {
            quoted.push('\\');
        }
        quoted.push(one);
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests;

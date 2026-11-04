//! 两份报告（细则第十二节，DESIGN 第六节第 9 条）：给人看的 `报告.md`，给 agent 读的 `给agent.json`。都只写位置、条数和
//! 为什么，不带旧数据的内容。每一类迁完往里加一项；迁不了的不硬迁，写明它在压缩包里的哪个路径、旧格式是什么样。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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
    /// 每一类，照迁的先后。
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
    /// 在压缩包里的相对路径。
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
    fn zh(self) -> &'static str {
        match self {
            Status::Migrated => "迁了",
            Status::Partial => "迁了一部分",
            Status::Archived => "没迁，在压缩包里",
            Status::Skipped => "不用迁",
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
    /// 给人看的那份：一段话、每一类一节、没迁的一张表。
    pub fn human(&self) -> String {
        let mut out = format!(
            "# 旧版 Miyu 迁移报告\n\n迁移时刻：{}。旧数据目录：`{}`（{}）。\n\n",
            self.at, self.old_root.path, self.old_root.layout
        );
        if let Some(archive) = &self.archive {
            out.push_str(&format!(
                "旧目录整个打成了压缩包：`{}`（{} 个文件，SHA-256 `{}`）。确认新版用着没问题以后，可以删掉旧目录，只留这个包。\n\n",
                archive.path, archive.files, archive.sha256
            ));
        }
        if !self.personas.is_empty() {
            out.push_str("## 人格\n\n| 旧版 | 名字 | 新版 | 怎么来的 |\n|---|---|---|---|\n");
            for row in &self.personas {
                let how = match row.how {
                    crate::personas::How::Created => "新建",
                    crate::personas::How::Existing => "已经有同名的",
                    crate::personas::How::Engineer => "出厂的软件工程师",
                    crate::personas::How::Missing => "没映射上",
                };
                out.push_str(&format!(
                    "| `{}` | {} | {} | {how}{} |\n",
                    row.old,
                    row.name,
                    row.new
                        .as_deref()
                        .map_or("—".to_string(), |new| format!("`{new}`")),
                    row.note
                        .as_deref()
                        .map_or(String::new(), |note| format!("（{note}）")),
                ));
            }
            out.push('\n');
        }
        out.push_str("## 每一类\n\n");
        for item in &self.items {
            out.push_str(&format!(
                "### {}\n\n{}，{} 条",
                item.what,
                item.status.zh(),
                item.migrated
            ));
            if !item.skipped.is_empty() {
                out.push_str(&format!("，跳过 {} 条", item.skipped.len()));
            }
            out.push_str("。\n");
            if let Some(target) = &item.target {
                out.push_str(&format!("迁到：`{target}`。\n"));
            }
            if let Some(note) = &item.note {
                out.push_str(&format!("{note}\n"));
            }
            out.push('\n');
        }
        let left: Vec<&Item> = self
            .items
            .iter()
            .filter(|item| matches!(item.status, Status::Archived | Status::Partial))
            .collect();
        if !left.is_empty() {
            out.push_str("## 没迁的\n\n要的时候，让 agent 照同一目录里的 `给agent.json` 接着迁。\n\n| 东西 | 在压缩包里 | 旧格式 |\n|---|---|---|\n");
            for item in left {
                out.push_str(&format!(
                    "| {} | `{}` | {} |\n",
                    item.what, item.source, item.format
                ));
            }
        }
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

    /// 两份都写进 `dir`，交回两个文件的位置。
    ///
    /// # Errors
    ///
    /// 写不进去。
    pub fn write(&self, dir: &Path) -> io::Result<(PathBuf, PathBuf)> {
        fs::create_dir_all(dir)?;
        let human = dir.join("报告.md");
        let agent = dir.join("给agent.json");
        fs::write(&human, self.human())?;
        fs::write(&agent, self.agent().map_err(io::Error::other)? + "\n")?;
        Ok((human, agent))
    }
}

#[cfg(test)]
mod tests;

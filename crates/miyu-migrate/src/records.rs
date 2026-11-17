//! QQ 消息记录（细则第二节）：旧版 `data/platforms/onebot/message_history/history.sqlite3`（第 4 版）转成 im 的 JSON Lines，交
//! `miyu im records import` 导进 im 的库。im 照 (场所, 平台编号) 去重：重跑、和新 im 并行接过的那一段重叠，都不会多出来。
//!
//! 一行 `messages` 写成一行 `message`；撤回写在消息行上（`recalled_by` 照 `recalls` 同键那一行），`recalls` 里有、`messages`
//! 里没有的写一行 `happening`（`recalled`），排在最后。不是 `onebot` 的跳过、数出来；`context_boundaries` 不迁。

mod convert;

use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::Arc;

use crate::layout::Layout;
use crate::report::{Item, Status};
use crate::words::Language;

pub use convert::{Converted, convert};

/// 旧库要是这一版。
pub const VERSION: i64 = 4;

/// 导一份文件、交回命令的输出。
pub type Run = dyn Fn(&Path) -> io::Result<Output> + Send + Sync;

/// 交 im 导一份文件：真跑的是 `miyu im records import <文件>`，测试换成假的。
#[derive(Clone)]
pub struct Importer(Arc<Run>);

impl fmt::Debug for Importer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Importer")
    }
}

impl Importer {
    /// 照 `run` 导。
    pub fn new(run: impl Fn(&Path) -> io::Result<Output> + Send + Sync + 'static) -> Importer {
        Importer(Arc::new(run))
    }

    /// 照 `miyu`（`program`）的命令行导：英文说，好照它的话数条数。
    pub fn program(program: PathBuf) -> Importer {
        Importer(Arc::new(move |file: &Path| {
            std::process::Command::new(&program)
                .args(["im", "records", "import"])
                .arg(file)
                .env("LC_ALL", "C.UTF-8")
                .env("LANG", "C.UTF-8")
                .output()
        }))
    }
}

/// 迁 QQ 消息记录：读 `copy`（解出来的副本）的旧库，写成 `<dir>/qq-records.jsonl`，交 `importer` 导。交回报告的一项。
pub fn migrate(copy: &Layout, dir: &Path, importer: &Importer, language: Language) -> Item {
    let words = crate::words::sessions::records(language);
    let path = copy.qq_history();
    let source = path
        .strip_prefix(&copy.root)
        .unwrap_or(&path)
        .to_string_lossy()
        .replace('\\', "/");
    let mut item = Item {
        what: "qq-records".to_string(),
        status: Status::Migrated,
        source,
        format: words.format.to_string(),
        migrated: 0,
        skipped: Vec::new(),
        target: Some("miyu im records import".to_string()),
        note: None,
    };
    if !path.is_file() {
        item.status = Status::Skipped;
        item.note = Some(words.none.to_string());
        return item;
    }
    let converted = match convert(&path) {
        Ok(Some(converted)) => converted,
        Ok(None) => {
            item.status = Status::Archived;
            item.note = Some((words.wrong_version)(VERSION));
            return item;
        }
        Err(error) => {
            item.status = Status::Archived;
            item.note = Some((words.unreadable)(&error.to_string()));
            return item;
        }
    };
    let file = dir.join("qq-records.jsonl");
    if let Err(error) = write(&file, &converted.lines) {
        item.status = Status::Archived;
        item.note = Some((words.cannot_write)(&file, &error.to_string()));
        return item;
    }
    let absolute = std::path::absolute(&file).unwrap_or(file);
    let command = format!(
        "miyu im records import {}",
        crate::report::shell_path(&absolute)
    );
    let mut note = (words.converted)(converted.messages, converted.happenings, &absolute);
    if converted.foreign > 0 {
        note.push_str(&(words.foreign)(converted.foreign));
    }
    match (importer.0)(&absolute) {
        Ok(output) if output.status.success() => {
            let said = String::from_utf8_lossy(&output.stdout);
            let counts = numbers(&said);
            let added = counts.first().copied().unwrap_or_default();
            item.migrated = added + counts.get(1).copied().unwrap_or_default();
            note.push_str(&(words.added)(
                added,
                counts.get(1).copied().unwrap_or_default(),
            ));
            if let (Some(same), Some(taken)) = (counts.get(3), counts.get(4)) {
                note.push_str(&(words.same)(*same, *taken));
                if *taken > 0 {
                    item.status = Status::Partial;
                }
            }
            note.push_str(words.full_stop);
        }
        Ok(output) => {
            item.status = Status::Archived;
            let said = String::from_utf8_lossy(&output.stderr);
            let first = said.lines().next().unwrap_or_default().trim().to_string();
            note.push_str(&(words.refused)(&first, &command));
        }
        Err(error) => {
            item.status = Status::Archived;
            note.push_str(&(words.cannot_run)(&error.to_string(), &command));
        }
    }
    item.note = Some(note);
    item
}

/// 一行一条写进 `file`（先写临时文件再挂上）。
fn write(file: &Path, lines: &[String]) -> io::Result<()> {
    let temp = file.with_extension("jsonl.partial");
    let mut out = BufWriter::new(File::create(&temp)?);
    for line in lines {
        out.write_all(line.as_bytes())?;
        out.write_all(b"\n")?;
    }
    out.into_inner()
        .map_err(io::IntoInnerError::into_error)?
        .sync_all()?;
    fs::rename(&temp, file)
}

/// im 说的话里的数，照出现的先后：`imported from <路径>: N messages, N events, N judgments`，有跳过的另一行
/// `skipped X already in the records and Y whose number is taken by a different message`。路径里的数不算（冒号以前的不看）。
fn numbers(said: &str) -> Vec<u64> {
    said.lines()
        .map(|line| line.rsplit_once(": ").map_or(line, |(_, rest)| rest))
        .flat_map(|line| {
            line.split(|ch: char| !ch.is_ascii_digit())
                .filter(|part| !part.is_empty())
                .filter_map(|part| part.parse().ok())
                .collect::<Vec<u64>>()
        })
        .collect()
}

#[cfg(test)]
mod tests;

//! 知识库（细则第五节）：旧版 `data/kb/files/` 里自己放的文档拷进新版的一个库 `root/kb/legacy/`，经 `kb.create` 建这个库（已经有的
//! 当迁过）。不逐个 `kb.add`：核心列出、搜之前照文件补索引。
//!
//! - 清单照 `kb_meta.db` 的 `files`（`name` 是相对 `files/` 的路径，`path` 是旧机器上的绝对路径，不用）；`default-kb/` 开头的是旧版
//!   从远端拉的默认库，不迁、数出来。
//! - 同名的：字节一样的算以前迁过，不一样的改名 `<名>-legacy.<扩展名>` 并列进报告。拷完逐个照 `content_sha256` 核对。
//! - 新版认的只有 `.md`、`.markdown`、`.txt`、`.pdf`、`.html`、`.htm`：别的照拷，报告写「新版不建索引」。

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::core::{Core, Said};
use crate::layout::Layout;
use crate::report::{Item, Skip, Status};
use crate::words::Language;

/// 新版的库编号。
pub const BASE: &str = "legacy";
/// 新版认的文档。
const KINDS: &[&str] = &["md", "markdown", "txt", "pdf", "html", "htm"];

/// 迁知识库：`copy` 的文档拷进新数据根 `root`（`<它>/root/kb/legacy/`），经 `core` 建库、读回。交回报告的一项。
///
/// # Errors
///
/// 连接断了。
pub fn migrate(
    core: &mut dyn Core,
    copy: &Layout,
    root: &Path,
    language: Language,
) -> Result<Item, Said> {
    let words = crate::words::files::knowledge(language);
    let dir = copy.knowledge();
    let mut item = Item {
        what: "knowledge".to_string(),
        status: Status::Migrated,
        source: dir
            .strip_prefix(&copy.root)
            .unwrap_or(&dir)
            .to_string_lossy()
            .replace('\\', "/"),
        format: words.format.to_string(),
        migrated: 0,
        skipped: Vec::new(),
        target: Some((words.target)(BASE)),
        note: None,
    };
    // 没有清单、清单里没有 `files` 表才是「没有」；清单在却读不了的写原因，文档留在压缩包里。
    let files = match listed(&dir.join("kb_meta.db")) {
        Ok(Some(files)) => files,
        Ok(None) => {
            item.status = Status::Skipped;
            item.note = Some(words.none.to_string());
            return Ok(item);
        }
        Err(error) => {
            item.status = Status::Archived;
            item.note = Some((words.unreadable)(&error.to_string()));
            return Ok(item);
        }
    };
    let (builtin, mine): (Vec<_>, Vec<_>) = files
        .into_iter()
        .partition(|(name, _)| name.starts_with("default-kb/"));
    if mine.is_empty() {
        item.status = Status::Skipped;
        item.note = Some((words.builtin_only)(builtin.len()));
        return Ok(item);
    }
    match core.call(
        "kb.create",
        json!({"kb": BASE, "name": words.name, "description": words.description}),
    ) {
        Ok(_) => {}
        Err(said) if said.code == "kb_exists" => {}
        Err(said) if fatal(&said) => return Err(said),
        Err(said) => {
            item.status = Status::Archived;
            item.note = Some((words.create_failed)(&said.code));
            return Ok(item);
        }
    }
    let target = root.join("root").join("kb").join(BASE);
    let mut renamed = Vec::new();
    let mut other = 0;
    let mut mismatch = 0;
    let mut present = 0;
    for (name, sha256) in &mine {
        let Some(relative) = inside(name) else {
            item.skipped.push(skip(name, "bad-name"));
            continue;
        };
        let from = dir.join("files").join(&relative);
        let bytes = match fs::read(&from) {
            Ok(bytes) => bytes,
            Err(_) => {
                item.skipped.push(skip(name, "missing"));
                continue;
            }
        };
        if !sha256.is_empty() && crate::archive::hex(&Sha256::digest(&bytes)) != *sha256 {
            mismatch += 1;
        }
        match place(&target, &relative, &bytes) {
            Ok(Placed::Present) => {
                item.migrated += 1;
                present += 1;
            }
            Ok(Placed::At(at)) => {
                item.migrated += 1;
                if at != relative {
                    renamed.push(format!(
                        "{name} → {}",
                        at.to_string_lossy().replace('\\', "/")
                    ));
                }
            }
            Err(error) => item
                .skipped
                .push(skip(name, &format!("write: {}", error.kind()))),
        }
        let indexed = relative
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| KINDS.contains(&ext.to_ascii_lowercase().as_str()));
        if !indexed {
            other += 1;
        }
    }
    let documents = core
        .call("kb.list", json!({}))?
        .get("bases")
        .and_then(|bases| bases.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .find(|base| base["id"] == BASE)
        .and_then(|base| base["documents"].as_u64());
    let mut notes = vec![(words.copied)(item.migrated - present, BASE, present)];
    if let Some(documents) = documents {
        notes.push((words.documents)(documents));
    }
    if item.migrated > present {
        notes.push(words.indexing.to_string());
    }
    if other > 0 {
        notes.push((words.other)(other));
    }
    if !builtin.is_empty() {
        notes.push((words.builtin)(builtin.len()));
    }
    if !renamed.is_empty() {
        notes.push((words.renamed)(&language.list(&renamed)));
    }
    if mismatch > 0 {
        notes.push((words.mismatch)(mismatch));
    }
    if !item.skipped.is_empty() {
        item.status = Status::Partial;
    }
    item.note = Some(language.sentences(&notes));
    Ok(item)
}

/// `kb_meta.db` 的 `files`：(`name`, `content_sha256`)，照 `name` 排。没有这个库、库里没有这张表的是 `None`。
///
/// # Errors
///
/// 库在，打不开、读不了。
fn listed(path: &Path) -> rusqlite::Result<Option<Vec<(String, String)>>> {
    if !path.is_file() {
        return Ok(None);
    }
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    let exists = conn
        .prepare("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'files'")?
        .exists([])?;
    if !exists {
        return Ok(None);
    }
    let mut statement = conn.prepare("SELECT name, content_sha256 FROM files ORDER BY name")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get(0)?,
            row.get::<_, Option<String>>(1)?.unwrap_or_default(),
        ))
    })?;
    rows.collect::<rusqlite::Result<_>>().map(Some)
}

/// 拷完是什么样。
enum Placed {
    /// 已经有一样的。
    Present,
    /// 拷到了这个相对路径。
    At(PathBuf),
}

/// 放进 `target/<relative>`：已经有一样的不动，不一样的改名 `<名>-legacy[-n].<扩展名>`。
fn place(target: &Path, relative: &Path, bytes: &[u8]) -> io::Result<Placed> {
    let stem = relative
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default();
    let ext = relative
        .extension()
        .map(|ext| format!(".{}", ext.to_string_lossy()))
        .unwrap_or_default();
    let mut at = relative.to_path_buf();
    let mut n = 1;
    loop {
        let path = target.join(&at);
        match fs::read(&path) {
            Ok(there) if there == bytes => return Ok(Placed::Present),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&path, bytes)?;
                return Ok(Placed::At(at));
            }
            Err(error) => return Err(error),
        }
        n += 1;
        let suffix = if n == 2 {
            "-legacy".to_string()
        } else {
            format!("-legacy-{n}")
        };
        at = relative.with_file_name(format!("{stem}{suffix}{ext}"));
    }
}

/// 清单里的名字是 `files/` 里面的相对路径（不带 `..`、不是绝对路径）的，交回它。
fn inside(name: &str) -> Option<PathBuf> {
    let path = PathBuf::from(name);
    let ok = !name.is_empty()
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_)));
    ok.then_some(path)
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

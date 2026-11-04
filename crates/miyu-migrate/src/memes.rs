//! 表情包（细则第七节）：旧版的用户库 `data/memes/<库>/`（`index.json` 加 `images/…`）每个做成一个 miyu-memes 的内容包
//! `legacy-memes-<库>/`，miyu-memes 装着的经 `package.install {path}` 装上；没装的包目录留在这一次的目录里，报告写装上 miyu-memes
//! 以后跑哪一行。同一个编号再装是换成新的，重跑不会多（miyu-memes 照图去重）。
//!
//! - 出厂库不在数据根，不迁；用户库里只有 `disabled_ids`、只覆盖出厂条目（图不在这个库里）的没有自己的图，跳过、列进报告。
//! - 一条写 `file`、`description`、`usage`、`reason`（「旧版表情包 <库>」，从聊天里收来的接上在哪、谁发的、什么时候收的）；
//!   `description`、`usage` 有空的那条 miyu-memes 会跳过，这里先跳过、数出来。

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::core::{Core, Said};
use crate::layout::Layout;
use crate::report::{Item, Skip, Status};

/// 内容包给的那个软件。
pub const MEMES: &str = "miyu-memes";

/// 旧格式一句话。
const FORMAT: &str = "data/memes/<库>/index.json（{library, version 2, memes:[{id sha256:…, name {zh,en}, file images/…, mime_type, animated, description, usage, tags, origin {platform, conversation_kind, conversation_id, sender_id, sender_name, message_id, sent_at, collected_at}}], disabled_ids}）加 images/ 下的图";

#[derive(Debug, Deserialize)]
struct Index {
    #[serde(default)]
    memes: Vec<Meme>,
}

#[derive(Debug, Deserialize)]
struct Meme {
    #[serde(default)]
    file: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    usage: String,
    #[serde(default)]
    origin: Option<Origin>,
}

#[derive(Debug, Default, Deserialize)]
struct Origin {
    #[serde(default)]
    platform: String,
    #[serde(default)]
    conversation_kind: String,
    #[serde(default)]
    conversation_id: String,
    #[serde(default)]
    sender_id: String,
    #[serde(default)]
    sender_name: String,
    #[serde(default)]
    collected_at: String,
}

/// 一个库做好的包：编号、在哪、几条，没做进去的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pack {
    /// 包的编号。
    pub id: String,
    /// 包目录。
    pub dir: PathBuf,
    /// 写进去几条。
    pub count: u64,
    /// 没写进去的：哪一条（库里的文件名），为什么（`no-text`、`no-image`）。
    pub left: Vec<(String, String)>,
}

/// 迁表情包：`copy` 的用户库做成包放在 `dir` 下，miyu-memes 装着的装上。交回报告的一项。
///
/// # Errors
///
/// 连接断了。
pub fn migrate(core: &mut dyn Core, copy: &Layout, dir: &Path) -> Result<Item, Said> {
    let memes = copy.memes();
    let source = memes
        .strip_prefix(&copy.root)
        .unwrap_or(&memes)
        .to_string_lossy()
        .replace('\\', "/");
    let mut item = Item {
        what: "memes".to_string(),
        status: Status::Migrated,
        source: format!("{source}/<库>"),
        format: FORMAT.to_string(),
        migrated: 0,
        skipped: Vec::new(),
        target: Some("package.install（miyu-memes 的内容包）".to_string()),
        note: None,
    };
    let mut libraries: Vec<PathBuf> = fs::read_dir(&memes)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.join("index.json").is_file())
                .collect()
        })
        .unwrap_or_default();
    libraries.sort();
    if libraries.is_empty() {
        item.status = Status::Skipped;
        item.note = Some("旧根里没有自己的表情包库。".to_string());
        return Ok(item);
    }
    let mut packs = Vec::new();
    for library in &libraries {
        let name = library
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        match pack(library, &name, dir) {
            Ok(Some(pack)) => {
                item.skipped.extend(
                    pack.left
                        .iter()
                        .map(|(file, why)| skip(&format!("{name}/{file}"), why)),
                );
                packs.push(pack);
            }
            Ok(None) => item.skipped.push(skip(&name, "no-own-images")),
            Err(error) => item
                .skipped
                .push(skip(&name, &format!("unreadable: {}", error.kind()))),
        }
    }
    let installed = core
        .call("package.list", json!({}))?
        .get("packages")
        .and_then(Value::as_array)
        .is_some_and(|all| all.iter().any(|one| one["package"] == MEMES));
    let mut notes = Vec::new();
    for pack in &packs {
        let path = std::path::absolute(&pack.dir).unwrap_or_else(|_| pack.dir.clone());
        let line = format!("miyu pkg install {}", crate::report::shell_path(&path));
        if !installed {
            notes.push(format!(
                "`{}` {} 条：{MEMES} 没装，装上以后跑 `{line}`。",
                pack.id, pack.count
            ));
            item.status = Status::Partial;
            continue;
        }
        match core.call(
            "package.install",
            json!({"path": path.display().to_string()}),
        ) {
            Ok(_) => {
                item.migrated += pack.count;
                notes.push(format!("`{}` 装上了，{} 条。", pack.id, pack.count));
            }
            Err(said) if fatal(&said) => return Err(said),
            Err(said) => {
                item.status = Status::Partial;
                notes.push(format!(
                    "`{}` 没装上（{}），改好以后跑 `{line}`。",
                    pack.id, said.code
                ));
            }
        }
    }
    if !item.skipped.is_empty() && item.status == Status::Migrated {
        item.status = Status::Partial;
    }
    item.note = Some(notes.join(""));
    Ok(item)
}

/// 一个库做成包，放在 `out/<编号>/`（已经有的先删掉重做）。没有自己的图的交回 `None`。
///
/// # Errors
///
/// 读不了 `index.json`、写不了包。
pub fn pack(library: &Path, name: &str, out: &Path) -> io::Result<Option<Pack>> {
    let text = fs::read_to_string(library.join("index.json"))?;
    let index: Index = serde_json::from_str(&text).map_err(io::Error::other)?;
    let id = package_id(name);
    let dir = out.join(&id);
    let mut entries = Vec::new();
    let mut left = Vec::new();
    let mut files = Vec::new();
    for meme in &index.memes {
        let Some(relative) = inside(&meme.file) else {
            left.push((meme.file.clone(), "no-image".to_string()));
            continue;
        };
        if !library.join(&relative).is_file() {
            // 覆盖出厂条目的：图在出厂库里，不在这个库。
            left.push((meme.file.clone(), "builtin".to_string()));
            continue;
        }
        if meme.description.trim().is_empty() || meme.usage.trim().is_empty() {
            left.push((meme.file.clone(), "no-text".to_string()));
            continue;
        }
        entries.push(json!({
            "file": meme.file,
            "description": meme.description.trim(),
            "usage": meme.usage.trim(),
            "reason": reason(name, meme.origin.as_ref()),
        }));
        files.push(relative);
    }
    left.retain(|(_, why)| why != "builtin");
    if entries.is_empty() {
        return Ok(None);
    }
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    fs::create_dir_all(dir.join("memes"))?;
    for relative in &files {
        let to = dir.join("memes").join(relative);
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(library.join(relative), to)?;
    }
    let manifest = format!(
        "[package]\nversion = \"1.0.0\"\nname = {{ zh = \"旧版表情包：{lib}\", en = \"Old Miyu stickers: {lib}\" }}\n\n[content.{MEMES}]\ndir = \"memes\"\n",
        lib = name.replace('\\', "\\\\").replace('"', "\\\"")
    );
    fs::write(dir.join("package.toml"), manifest)?;
    let index =
        serde_json::to_string_pretty(&json!({"memes": entries})).map_err(io::Error::other)?;
    fs::write(dir.join("memes").join("index.json"), index + "\n")?;
    Ok(Some(Pack {
        id,
        dir,
        count: entries.len() as u64,
        left,
    }))
}

/// 包的编号：`legacy-memes-<库>`，照包编号的写法（小写字母开头，小写字母、数字、`-`、`_`，最多 64 个）；库名一个能用的字都
/// 没有的（中文名）用库名的 blake3 前 8 位。
pub fn package_id(library: &str) -> String {
    let cleaned: String = library
        .trim()
        .chars()
        .map(|ch| {
            let ch = ch.to_ascii_lowercase();
            if ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches('-');
    let tail = if cleaned.is_empty() {
        blake3::hash(library.as_bytes()).to_hex()[..8].to_string()
    } else {
        cleaned.to_string()
    };
    format!("legacy-memes-{tail}").chars().take(64).collect()
}

/// `file` 是库里面的相对路径（不带 `..`、不是绝对路径）的，交回它。
fn inside(file: &str) -> Option<PathBuf> {
    let path = Path::new(file);
    let ok = !file.is_empty()
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_) | Component::CurDir));
    ok.then(|| path.to_path_buf())
}

/// 来源：「旧版表情包 <库>」，从聊天里收来的接上在哪、谁发的、什么时候收的。
fn reason(library: &str, origin: Option<&Origin>) -> String {
    let mut reason = format!("旧版表情包 {library}");
    if let Some(origin) = origin.filter(|origin| !origin.platform.is_empty()) {
        let platform = if origin.platform == "onebot" {
            "QQ"
        } else {
            origin.platform.as_str()
        };
        let place = match origin.conversation_kind.as_str() {
            "group" => format!("群 {}", origin.conversation_id),
            "private" => "私聊".to_string(),
            _ => origin.conversation_id.clone(),
        };
        let who = match (origin.sender_name.trim(), origin.sender_id.trim()) {
            ("", "") => "有人".to_string(),
            (name, "") => name.to_string(),
            ("", id) => id.to_string(),
            (name, id) => format!("{name}（{id}）"),
        };
        reason.push_str(&format!("；收集自 {platform} {place} 的 {who}"));
        if !origin.collected_at.is_empty() {
            reason.push_str(&format!("，{}", origin.collected_at));
        }
    }
    reason
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

//! 图片、文档、产物、工作目录（细则第九节）：旧版给你存的文件拷进新版数据根的同类目录。
//!
//! | 旧的 | 新的 |
//! |---|---|
//! | `home/<管>/pictures/`、`data/pictures/`（两处都有的合并） | `root/pictures/` |
//! | `home/<管>/documents/` | `root/documents/` |
//! | `home/<管>/artifacts/<旧会话>/` | `root/documents/旧版产物/<会话标题或新编号>/` |
//! | `home/<管>/workspace/`、根目录 `workspace/` | `root/workspace/旧版/` |
//! | `home/<管>/shares/` | 不迁，列进报告 |
//!
//! 同名的：字节一样的算以前迁过，不一样的改成 `<名> (旧版 N).<后缀>` 并列进报告。拷完逐个照 SHA-256 核对。链接不跟、不拷。

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::layout::Layout;
use crate::report::{Item, Skip, Status};
use crate::sessions::{Db, ids};

/// 迁文件：`copy` 里的拷进新数据根 `root`。交回报告的一项。
pub fn migrate(copy: &Layout, root: &Path) -> Item {
    let new = root.join("root");
    let titles = titles(copy);
    let mut item = Item {
        what: "files".to_string(),
        status: Status::Migrated,
        source: "home/<管>/{pictures,documents,artifacts,workspace}、data/pictures、workspace"
            .to_string(),
        format: "普通文件：图片、文档、工具产出的产物（artifacts/<旧会话编号>/）、工作目录"
            .to_string(),
        migrated: 0,
        skipped: Vec::new(),
        target: Some("root/{pictures,documents,workspace}".to_string()),
        note: None,
    };
    let mut pairs: Vec<(PathBuf, PathBuf)> = vec![
        (copy.user_dir("pictures"), new.join("pictures")),
        (
            copy.root.join("data").join("pictures"),
            new.join("pictures"),
        ),
        (copy.user_dir("documents"), new.join("documents")),
        (copy.workspace(), new.join("workspace").join("旧版")),
        (
            copy.root.join("workspace"),
            new.join("workspace").join("旧版"),
        ),
    ];
    // 产物：照会话放，目录名换成会话标题或新编号。
    let artifacts = copy.user_dir("artifacts");
    for dir in children(&artifacts) {
        let old = dir
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let shown = titles.get(&old).cloned().unwrap_or(old);
        pairs.push((
            dir,
            new.join("documents").join("旧版产物").join(clean(&shown)),
        ));
    }
    pairs.dedup_by(|left, right| left.0 == right.0);
    let mut tally = Tally::default();
    let mut seen = std::collections::BTreeSet::new();
    for (from, to) in &pairs {
        if !from.is_dir() || !seen.insert(from.clone()) {
            continue;
        }
        copy_tree(from, to, &copy.root, &mut tally, &mut item.skipped);
    }
    let shares = copy.user_dir("shares");
    if shares.is_dir() {
        item.skipped
            .push(skip(&relative(&shares, &copy.root), "shares"));
    }
    item.migrated = tally.copied + tally.present;
    let mut notes = vec![format!(
        "拷了 {} 个文件，{} 个以前迁过。",
        tally.copied, tally.present
    )];
    if !tally.renamed.is_empty() {
        notes.push(format!(
            "新版里已经有同名不一样的，改了名：{}。",
            tally.renamed.join("、")
        ));
    }
    if tally.mismatch > 0 {
        notes.push(format!(
            "{} 个拷完核对 SHA-256 对不上，照实写在这里。",
            tally.mismatch
        ));
    }
    if shares.is_dir() {
        notes.push("`shares/`（以前给别人的分享）新版没有对应的，留在压缩包里。".to_string());
    }
    if !item.skipped.is_empty() || tally.mismatch > 0 {
        item.status = Status::Partial;
    }
    if tally.copied + tally.present == 0 && item.skipped.is_empty() {
        item.status = Status::Skipped;
        notes = vec!["旧根里没有这几样文件。".to_string()];
    }
    item.note = Some(notes.join(""));
    item
}

/// 数出来的。
#[derive(Debug, Default)]
struct Tally {
    copied: u64,
    present: u64,
    mismatch: u64,
    renamed: Vec<String>,
}

/// 旧会话编号 → 产物目录名：会话的名字，没有名字的用新编号（和导进去的存档对得上）。会话库读不了的是空的。
fn titles(copy: &Layout) -> HashMap<String, String> {
    let path = copy.conversation_db();
    let source = path
        .strip_prefix(&copy.root)
        .unwrap_or(&path)
        .to_string_lossy()
        .replace('\\', "/");
    let Ok(sessions) = Db::open(&path).and_then(|db| db.sessions()) else {
        return HashMap::new();
    };
    sessions
        .into_iter()
        .map(|session| {
            let title = session.name.trim().to_string();
            let shown = if title.is_empty() {
                let created = ids::parse(&session.created_at).map_or(0, |at| {
                    u64::try_from(at.as_millisecond()).unwrap_or_default()
                });
                ids::session_id(created, &source, &session.id)
            } else {
                title
            };
            (session.id, shown)
        })
        .collect()
}

/// 会话标题当目录名：去掉路径分隔、控制字符，两头空白，最长 80 个字；空了的写 `_`。
fn clean(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|ch| {
            if ch == '/' || ch == '\\' || ch.is_control() || ch == ':' {
                '_'
            } else {
                ch
            }
        })
        .take(80)
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    if cleaned.is_empty() {
        "_".to_string()
    } else {
        cleaned
    }
}

/// `dir` 下的子目录，照名字排。
fn children(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

/// 整个 `from` 拷进 `to`（链接不跟、不拷）。
fn copy_tree(from: &Path, to: &Path, old_root: &Path, tally: &mut Tally, skipped: &mut Vec<Skip>) {
    for entry in walkdir::WalkDir::new(from)
        .follow_links(false)
        .sort_by_file_name()
    {
        let Ok(entry) = entry else {
            continue;
        };
        if !entry.file_type().is_file() {
            if entry.file_type().is_symlink() {
                skipped.push(skip(&relative(entry.path(), old_root), "link"));
            }
            continue;
        }
        let inside = entry.path().strip_prefix(from).unwrap_or(entry.path());
        match place(entry.path(), &to.join(inside)) {
            Ok(Placed::Present) => tally.present += 1,
            Ok(Placed::At(at, same)) => {
                tally.copied += 1;
                if !same {
                    tally.mismatch += 1;
                }
                if at != to.join(inside) {
                    tally.renamed.push(format!(
                        "{} → {}",
                        relative(entry.path(), old_root),
                        at.file_name()
                            .map(|name| name.to_string_lossy().to_string())
                            .unwrap_or_default()
                    ));
                }
            }
            Err(error) => skipped.push(skip(
                &relative(entry.path(), old_root),
                &format!("write: {}", error.kind()),
            )),
        }
    }
}

/// 放完是什么样。
enum Placed {
    /// 已经有一样的。
    Present,
    /// 拷到了这里；拷完核对 SHA-256 对不对得上。
    At(PathBuf, bool),
}

/// 一个文件拷到 `to`：已经有一样的不动，不一样的改成 `<名> (旧版 N).<后缀>`。
fn place(from: &Path, to: &Path) -> io::Result<Placed> {
    let bytes = fs::read(from)?;
    let stem = to
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default();
    let ext = to
        .extension()
        .map(|ext| format!(".{}", ext.to_string_lossy()))
        .unwrap_or_default();
    let mut at = to.to_path_buf();
    let mut n = 1;
    loop {
        match fs::read(&at) {
            Ok(there) if there == bytes => return Ok(Placed::Present),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if let Some(parent) = at.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&at, &bytes)?;
                let same = Sha256::digest(fs::read(&at)?) == Sha256::digest(&bytes);
                return Ok(Placed::At(at, same));
            }
            Err(error) => return Err(error),
        }
        at = to.with_file_name(format!("{stem} (旧版 {n}){ext}"));
        n += 1;
    }
}

fn relative(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn skip(key: &str, why: &str) -> Skip {
    Skip {
        key: key.to_string(),
        why: why.to_string(),
    }
}

#[cfg(test)]
mod tests;

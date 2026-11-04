//! 技能、脚本（细则第八节）：
//!
//! - 旧版包管理器装的（`extensions/pm/lock.json` 的 `files` 里列着的）不拷：报告里列包名，照官方源的名字表（[`OFFICIAL`]，照软件源
//!   仓库的 `names.toml` 抄）提示装同名的新包。
//! - 自己的技能 `<技能目录>/<名字>/SKILL.md` 和人格层的 `<技能目录>/personas/<scope>/<名字>/` 拷进新版数据根的 `root/skills/<名字>/`
//!   （新版没有人格层，报告写原来属于哪个人格）。名字照新版的写法（小写字母、数字、`-`）；新版已经有同名的、内容一样的算以前迁过，
//!   不一样的改名 `<名字>-legacy` 并列进报告，`SKILL.md` 头里的 `name` 跟着改。原来停用的（有 `.disabled`）照拷，新版里是开着的，
//!   报告里写一句。
//! - 自己的脚本：新版的格式不一样（旧版是头部注释加 `index.json` 覆盖层），原样留在压缩包里、列进报告。

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::layout::Layout;
use crate::report::{Item, Skip, Status};

/// 官方源里的包（软件源仓库 `names.toml` 的 `[names]`，2026-11-05 抄的）：旧版包管理器装的包照名字找新包。
pub const OFFICIAL: &[&str] = &[
    "miyu-embed",
    "miyu-im",
    "miyu-im-sponsor",
    "miyu-memes",
    "miyu-render",
    "miyu-scripts-ai-draw",
    "miyu-scripts-archwiki-query",
    "miyu-scripts-bangumi",
    "miyu-scripts-calculator",
    "miyu-scripts-codec",
    "miyu-scripts-crack-search",
    "miyu-scripts-deepseek-status",
    "miyu-scripts-dice",
    "miyu-scripts-douyin-dl",
    "miyu-scripts-exchange-rate",
    "miyu-scripts-fortune",
    "miyu-scripts-game-compat",
    "miyu-scripts-goofish",
    "miyu-scripts-juya-daily",
    "miyu-scripts-moegirl-query",
    "miyu-scripts-online-man",
    "miyu-scripts-reddit-search",
    "miyu-scripts-tarot",
    "miyu-scripts-weather",
    "miyu-scripts-web-image",
    "miyu-scripts-zhouyi",
    "miyu-skills-aur-pkgbuild-review",
    "miyu-skills-bilibili-live-stream",
    "miyu-skills-office",
    "miyu-skills-travel-planner",
];

/// 旧版包管理器装的一个包、照名字找到的新包。
pub fn successor(old: &str) -> Option<&'static str> {
    let old = old.trim().to_ascii_lowercase();
    let bare = old.strip_prefix("miyu-").unwrap_or(&old);
    [
        format!("miyu-scripts-{bare}"),
        format!("miyu-skills-{bare}"),
        format!("miyu-{bare}"),
    ]
    .into_iter()
    .find_map(|name| OFFICIAL.iter().copied().find(|one| *one == name))
}

/// 迁完的样子：报告的几项。
#[derive(Debug, Clone)]
pub struct Moved {
    /// 技能。
    pub skills: Item,
    /// 自己的脚本（只报）。
    pub scripts: Item,
    /// 包管理器装的包（只报、提示装新的）。
    pub packages: Item,
}

/// 迁技能、报脚本和包：读 `copy`，技能拷进新数据根 `root`（`<它>/root/skills/`）。
pub fn migrate(copy: &Layout, root: &Path) -> Moved {
    let locked = lock(copy);
    let files: BTreeSet<PathBuf> = locked
        .iter()
        .flat_map(|(_, files)| files.iter().cloned())
        .collect();
    let relative = |path: &Path| -> String {
        path.strip_prefix(&copy.root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    };
    let skills_dir = copy.skills();
    let target = root.join("root").join("skills");
    let mut skills = Item {
        what: "skills".to_string(),
        status: Status::Migrated,
        source: relative(&skills_dir),
        format: "一个技能一个目录：SKILL.md（YAML 头 name、description）加资源；personas/<scope>/<名字>/ 是人格层；.disabled 是停用".to_string(),
        migrated: 0,
        skipped: Vec::new(),
        target: Some("root/skills/<名字>/".to_string()),
        note: None,
    };
    let mut notes = Vec::new();
    for (owner, dir) in found(&skills_dir) {
        let shown = relative(&dir);
        let installed = walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(Result::ok)
            .any(|entry| files.contains(&PathBuf::from(relative(entry.path()))));
        if installed {
            skills.skipped.push(skip(&shown, "package"));
            continue;
        }
        match copy_skill(&dir, &target) {
            Ok(Copied::Present) => {}
            Ok(Copied::As(name)) => {
                skills.migrated += 1;
                let original = dir
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default();
                if name != original {
                    notes.push(format!("`{shown}` 新版里叫 `{name}`。"));
                }
                if let Some(scope) = &owner {
                    notes.push(format!(
                        "`{name}` 原来是人格 `{scope}` 的（新版的技能不分人格）。"
                    ));
                }
                if dir.join(".disabled").exists() {
                    notes.push(format!(
                        "`{name}` 原来停用了，新版里是开着的，不要的在设置里关掉。"
                    ));
                }
            }
            Err(error) => skills
                .skipped
                .push(skip(&shown, &format!("unreadable: {}", error.kind()))),
        }
    }
    if !skills.skipped.iter().all(|one| one.why == "package") {
        skills.status = Status::Partial;
    }
    skills.note = (!notes.is_empty()).then(|| notes.join(""));
    let scripts = scripts(copy, &files, relative(&copy.scripts()));
    let packages = packages(&locked);
    Moved {
        skills,
        scripts,
        packages,
    }
}

/// 包管理器的锁：包名和它装的文件（相对旧根）。没有锁的是空的。
fn lock(copy: &Layout) -> Vec<(String, Vec<PathBuf>)> {
    let candidates = [
        copy.root.join("extensions").join("pm").join("lock.json"),
        copy.root.join("data").join("pm").join("lock.json"),
    ];
    let Some(text) = candidates
        .iter()
        .find_map(|path| fs::read_to_string(path).ok())
    else {
        return Vec::new();
    };
    let lock: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    lock["packages"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(name, entry)| {
            let files = entry["files"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(|file| PathBuf::from(file.replace('\\', "/")))
                .collect();
            (name.clone(), files)
        })
        .collect()
}

/// 技能目录：顶层的（人格是 `None`）和人格层的（`Some(scope)`），照路径排。以 `.` 开头的、没有 `SKILL.md` 的不算。
fn found(dir: &Path) -> Vec<(Option<String>, PathBuf)> {
    let mut all = Vec::new();
    for (owner, parent) in std::iter::once((None, dir.to_path_buf())).chain(
        children(&dir.join("personas")).into_iter().map(|scope| {
            let name = scope
                .file_name()
                .map(|name| name.to_string_lossy().to_string());
            (name, scope)
        }),
    ) {
        // 顶层的 `personas/` 是人格层，没有 `SKILL.md`，不当技能。
        for skill in children(&parent) {
            if skill.join("SKILL.md").is_file() {
                all.push((owner.clone(), skill));
            }
        }
    }
    all
}

/// `dir` 下不以 `.` 开头的子目录，照名字排。
fn children(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .filter(|path| {
                    !path
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().starts_with('.'))
                })
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

/// 拷完是什么样。
enum Copied {
    /// 新版已经有一样的（以前迁过）。
    Present,
    /// 拷成了这个名字。
    As(String),
}

/// 一个技能拷进 `target/<名字>/`：同名的内容一样的算迁过，不一样的改名。
fn copy_skill(dir: &Path, target: &Path) -> io::Result<Copied> {
    let original = dir
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let base = skill_name(&original);
    let text = fs::read_to_string(dir.join("SKILL.md"))?;
    let mut name = base.clone();
    let mut n = 1;
    loop {
        let wanted = rename(&text, &name);
        let at = target.join(&name);
        match fs::read_to_string(at.join("SKILL.md")) {
            Ok(there) if there == wanted => return Ok(Copied::Present),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound && !at.exists() => {
                fs::create_dir_all(&at)?;
                for entry in walkdir::WalkDir::new(dir).min_depth(1) {
                    let entry = entry.map_err(io::Error::other)?;
                    let inside = entry.path().strip_prefix(dir).map_err(io::Error::other)?;
                    if inside == Path::new(".disabled") {
                        continue;
                    }
                    let to = at.join(inside);
                    if entry.file_type().is_dir() {
                        fs::create_dir_all(&to)?;
                    } else if entry.file_type().is_file() {
                        fs::copy(entry.path(), &to)?;
                    }
                }
                fs::write(at.join("SKILL.md"), wanted)?;
                return Ok(Copied::As(name));
            }
            Err(_) => {}
        }
        n += 1;
        name = if n == 2 {
            format!("{base}-legacy")
        } else {
            format!("{base}-legacy-{n}")
        };
    }
}

/// 新版技能名的写法：小写字母、数字、`-`。
pub fn skill_name(original: &str) -> String {
    let cleaned: String = original
        .chars()
        .map(|ch| {
            let ch = ch.to_ascii_lowercase();
            if ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' {
                ch
            } else {
                '-'
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches('-').to_string();
    if cleaned.is_empty() {
        format!("skill-{}", &blake3::hash(original.as_bytes()).to_hex()[..8])
    } else {
        cleaned
    }
}

/// `SKILL.md` 的 YAML 头里 `name:` 那一行换成 `name`（没有头、没有这一行的原样）。
pub fn rename(text: &str, name: &str) -> String {
    let Some(rest) = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
    else {
        return text.to_string();
    };
    let Some(end) = rest.find("\n---") else {
        return text.to_string();
    };
    let (head, body) = rest.split_at(end);
    let head: Vec<String> = head
        .lines()
        .map(|line| {
            if line.starts_with("name:") {
                format!("name: {name}")
            } else {
                line.to_string()
            }
        })
        .collect();
    format!("---\n{}{body}", head.join("\n"))
}

/// 自己的脚本：不是包装的、不是 `index.json`、备份、`__pycache__` 的普通文件，原样留着、列名字；名字对得上官方源里一个新包的
/// （照文件名去掉扩展名、`_` 换 `-` 找），提示装那个包。
fn scripts(copy: &Layout, files: &BTreeSet<PathBuf>, source: String) -> Item {
    let dir = copy.scripts();
    let mut mine: Vec<String> = walkdir::WalkDir::new(&dir)
        .into_iter()
        .filter_entry(|entry| entry.file_name() != "__pycache__")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy();
            name != "index.json" && !name.starts_with('.') && !name.contains(".bak")
        })
        .filter_map(|entry| {
            let relative = entry
                .path()
                .strip_prefix(&copy.root)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");
            (!files.contains(&PathBuf::from(&relative))).then_some(relative)
        })
        .collect();
    mine.sort();
    let mut skipped = Vec::new();
    let mut ported = Vec::new();
    for key in mine {
        let stem = Path::new(&key)
            .file_stem()
            .map(|stem| stem.to_string_lossy().replace('_', "-"))
            .unwrap_or_default();
        match successor(&stem) {
            Some(new) => {
                ported.push(format!("`{key}` → `miyu pkg install {new}`"));
                skipped.push(skip(&key, &format!("install:{new}")));
            }
            None => skipped.push(skip(&key, "format")),
        }
    }
    let rest = skipped.iter().filter(|one| one.why == "format").count();
    let mut note = Vec::new();
    if !ported.is_empty() {
        note.push(format!("有新包的到官方源装：{}。", ported.join("；")));
    }
    if rest > 0 {
        note.push(format!("{rest} 个自己写的脚本新版的格式不一样，原样留在压缩包里；要的时候照新版的写法（`docs/authors/scripts-skills.md`）改，或者让 agent 照 `给agent.json` 改。"));
    }
    Item {
        what: "scripts".to_string(),
        status: if skipped.is_empty() { Status::Skipped } else { Status::Archived },
        source,
        format: "一个脚本一个文件，头部注释写说明和参数，index.json 是覆盖层（id、path、parameters、load_policy……）；personas/<scope>/ 是人格层".to_string(),
        migrated: 0,
        skipped,
        target: None,
        note: (!note.is_empty()).then(|| note.join("")),
    }
}

/// 包管理器装的包：列名字，有新包的提示装。
fn packages(locked: &[(String, Vec<PathBuf>)]) -> Item {
    let mut lines = Vec::new();
    let mut skipped = Vec::new();
    for (name, _) in locked {
        match successor(name) {
            Some(new) => {
                lines.push(format!("`{name}` → `miyu pkg install {new}`"));
                skipped.push(skip(name, &format!("install:{new}")));
            }
            None => {
                lines.push(format!("`{name}` 新版没有对应的包"));
                skipped.push(skip(name, "no-package"));
            }
        }
    }
    Item {
        what: "packages".to_string(),
        status: if locked.is_empty() { Status::Skipped } else { Status::Archived },
        source: "extensions/pm/lock.json".to_string(),
        format: "JSON：{packages: {<包名>: {source, version, kind extension/persona, files [相对旧根的路径]}}}".to_string(),
        migrated: 0,
        skipped,
        target: Some("官方源".to_string()),
        note: (!lines.is_empty()).then(|| format!("旧版包管理器装的包不拷，到官方源装新的：{}。", lines.join("；"))),
    }
}

fn skip(key: &str, why: &str) -> Skip {
    Skip {
        key: key.to_string(),
        why: why.to_string(),
    }
}

#[cfg(test)]
mod tests;

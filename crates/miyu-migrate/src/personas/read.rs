//! 从解出来的副本里读旧人格（旧版格式见细则第六节、调研「第五部分 §2」）：
//!
//! - `data/prompts/<名字>.md`：一个人格的人设，文件名去掉 `.md` 是名字；`system-prompt.md` 是保留名，不是人格；
//! - `data/prompts/hints/<scope>.md`：角色扮演提示，`dialogs/<scope>.md`：示范对话；**有文件但空的是停用**；
//! - `data/prompts/<名字>.json`：网页的附带，`avatar_path` 指头像：`persona-avatars/…` 在资源目录下，绝对路径是旧机器上的
//!   （在旧根里的换到副本里，不在的够不着），别的相对配置目录；
//! - `personas/<scope>/persona.toml`：子系统、插件白名单，新版没有对应，只报；
//! - `config/dev-prompt.md`：开发模式的提示词，空的、等于那句旧默认的当没写；不盖出厂的软件工程师，只报。

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::layout::Layout;

use super::{Kind, LEGACY_DIALOGS, LEGACY_HINT, LEGACY_PERSONA, Old, dialogs};

/// 旧版 09-24 以前自动写进 `dev-prompt.md` 的那句，等于它的当没写。
const LEGACY_DEV_PROMPT: &str = "You are a helpful software engineer assistant.";

/// 读出来的：要建、要映射的旧人格，和只报不迁的。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Found {
    /// 旧人格，`default`、`dev` 在前，`data/prompts/` 的照文件名排。
    pub olds: Vec<Old>,
    /// 只报不迁的（相对旧根的路径和为什么）。
    pub extras: Vec<Extra>,
}

/// 只报不迁的一样。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extra {
    /// 相对旧根的路径。
    pub source: String,
    /// 哪一种：`persona-settings`、`dev-prompt`、`system-prompt`、`avatar-missing`。
    pub why: &'static str,
}

/// 旧版的 scope 写法（旧版 `crates/miyu-base/src/config/paths.rs` 的 `persona_scope_name`）：去两头空白，ASCII 字母数字、`-`、
/// `_` 留着（字母小写），别的换成 `-`，去掉两头的 `-`；空名是 `default`；剩下空的或者只剩 `md`（纯中文名带 `.md`）的是
/// `persona-<名字的 blake3 前 12 位十六进制>`。
pub fn scope_name(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        return "default".to_string();
    }
    let normalized: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let normalized = normalized.trim_matches('-');
    if normalized.is_empty() || normalized == "md" {
        format!("persona-{}", &blake3::hash(name.as_bytes()).to_hex()[..12])
    } else {
        normalized.to_string()
    }
}

/// 读 `layout`（解出来的副本）里的旧人格。`original` 是旧根原来的位置：附带里写的绝对路径照它换到副本里。
///
/// # Errors
///
/// 目录、文件读不了（没有的不算错）。
pub fn read(layout: &Layout, original: &Path) -> io::Result<Found> {
    let prompts = layout.prompts();
    let mut found = Found::default();
    let relative = |path: &Path| {
        path.strip_prefix(&layout.root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    };
    let hint = optional(&prompts.join("hints").join("default.md"))?;
    let dialogs = optional(&prompts.join("dialogs").join("default.md"))?;
    found.olds.push(Old {
        scope: "default".to_string(),
        name: "Miyu".to_string(),
        kind: Kind::Miyu,
        persona: Some(LEGACY_PERSONA.trim().to_string()),
        reminders: Some(hint.unwrap_or_else(|| LEGACY_HINT.trim().to_string())),
        examples: dialogs::parse(dialogs.as_deref().unwrap_or(LEGACY_DIALOGS)),
        avatar: None,
    });
    found.olds.push(Old {
        scope: "dev".to_string(),
        name: "dev".to_string(),
        kind: Kind::Dev,
        persona: None,
        reminders: None,
        examples: Vec::new(),
        avatar: None,
    });
    let dev = layout.root.join("config").join("dev-prompt.md");
    if optional(&dev)?.is_some_and(|text| !text.is_empty() && text != LEGACY_DEV_PROMPT) {
        found.extras.push(Extra {
            source: relative(&dev),
            why: "dev-prompt",
        });
    }
    for file in markdown(&prompts)? {
        let Some(file_name) = file.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file_name == "system-prompt.md" {
            found.extras.push(Extra {
                source: relative(&file),
                why: "system-prompt",
            });
            continue;
        }
        let scope = scope_name(file_name);
        let name = file_name.strip_suffix(".md").unwrap_or(file_name);
        let hint = optional(&prompts.join("hints").join(format!("{scope}.md")))?;
        let dialogs = optional(&prompts.join("dialogs").join(format!("{scope}.md")))?;
        let avatar = sidecar_avatar(&prompts.join(format!("{name}.json")))?
            .map(|value| locate(layout, original, &value));
        let avatar = match avatar {
            Some(Some(path)) if path.is_file() => Some(path),
            Some(_) => {
                found.extras.push(Extra {
                    source: relative(&prompts.join(format!("{name}.json"))),
                    why: "avatar-missing",
                });
                None
            }
            None => None,
        };
        found.olds.push(Old {
            scope,
            name: name.to_string(),
            kind: Kind::Prompt,
            persona: Some(fs::read_to_string(&file)?.trim().to_string()),
            reminders: hint,
            examples: dialogs.as_deref().map(dialogs::parse).unwrap_or_default(),
            avatar,
        });
    }
    let personas = layout.personas();
    if personas.is_dir() {
        let mut settings: Vec<PathBuf> = fs::read_dir(&personas)?
            .filter_map(Result::ok)
            .map(|entry| entry.path().join("persona.toml"))
            .filter(|path| path.is_file())
            .collect();
        settings.sort();
        found.extras.extend(settings.iter().map(|path| Extra {
            source: relative(path),
            why: "persona-settings",
        }));
    }
    Ok(found)
}

/// `dir` 下的 `*.md`（不进子目录），照文件名排。没有这个目录的是空的。
fn markdown(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut files = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// 有就读出来、去两头空白；没有这个文件的是 `None`（有但空的是空的字：停用）。
fn optional(path: &Path) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text.trim().to_string())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// 网页附带里的 `avatar_path`：没有附带、没写、空的是 `None`；附带读不懂的当没写。
fn sidecar_avatar(path: &Path) -> io::Result<Option<String>> {
    let Some(text) = optional(path)? else {
        return Ok(None);
    };
    Ok(serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|value| {
            value["avatar_path"]
                .as_str()
                .map(str::trim)
                .map(str::to_string)
        })
        .filter(|value| !value.is_empty()))
}

/// `avatar_path` 换成副本里的位置（旧版 `resolve_persona_asset_path` 的读法）。够不着的（旧根以外的绝对路径、`..`）是 `None`。
fn locate(layout: &Layout, original: &Path, value: &str) -> Option<PathBuf> {
    let path = Path::new(value);
    let normal = |path: &Path| {
        path.components()
            .filter(|part| !matches!(part, Component::CurDir))
            .map(|part| match part {
                Component::Normal(name) => Some(name.to_os_string()),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
    };
    if path.is_absolute() {
        let inside = path.strip_prefix(original).ok()?;
        return normal(inside).map(|parts| {
            parts
                .iter()
                .fold(layout.root.clone(), |at, part| at.join(part))
        });
    }
    let parts = normal(path)?;
    let base = if parts
        .first()
        .is_some_and(|first| first == "persona-avatars")
    {
        layout.prompts().parent()?.to_path_buf()
    } else {
        layout.root.join("config")
    };
    Some(parts.iter().fold(base, |at, part| at.join(part)))
}

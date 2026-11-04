//! 认旧版数据根是哪一代布局（细则第一节第 1 条、DESIGN 第四节）：旧版换过三代，各在根下留一个标记文件。先读标记，
//! 再照这一代找东西；同一样东西三代的位置照旧版 `MiyuPaths` 的访问器（`crates/miyu-base/src/paths/mod.rs:331-561`）。

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// 旧版的哪一代布局。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Generation {
    /// 没有标记、根下有 `config/config.jsonc`：设了 `MIYU_HOME` 的老布局（旧版这时不写 `.layout-v1`），东西在 `config/`、`data/`。
    Unmarked,
    /// `.layout-v1`：从 XDG 散布局搬进了数据根，资源还在 `config/` 下。
    Layout,
    /// `.resource-layout-v1`：技能、脚本、提示词、身份搬到了 `data/` 下。
    Resource,
    /// `.home-layout-v1`：`personas/`、`extensions/`、`home/<管理员>/` 的布局（旧版最后一代）。
    Home,
}

impl Generation {
    /// 写进报告的名字。
    pub fn name(self) -> &'static str {
        match self {
            Generation::Unmarked => "unmarked",
            Generation::Layout => "layout-v1",
            Generation::Resource => "resource-layout-v1",
            Generation::Home => "home-layout-v1",
        }
    }
}

/// 认出来的旧数据根：在哪、哪一代、管理员家目录叫什么（只有家目录布局有）、回滚过没有。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// 数据根。
    pub root: PathBuf,
    /// 哪一代。
    pub generation: Generation,
    /// 管理员家目录名：`.home-layout-v1` 的内容。
    pub admin: Option<String>,
    /// 根下有 `.home-layout-off`：人回滚过家目录布局，东西照上一代找。
    pub rolled_back: bool,
}

/// 认不出来。
#[derive(Debug)]
pub enum LayoutError {
    /// 不是一个目录。
    NotADirectory(PathBuf),
    /// 没有任何一代的标记，也没有旧版的配置文件：不是旧版的数据根。
    NotOld(PathBuf),
    /// 有旧版搬家没收尾的日志（`*.journal.json`）：要先让旧版自己恢复。
    Unfinished(PathBuf),
    /// `.home-layout-v1` 里的管理员名不合旧版的写法。
    BadAdmin(String),
    /// 读不了。
    Io(PathBuf, io::Error),
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayoutError::NotADirectory(path) => write!(f, "{} is not a directory", path.display()),
            LayoutError::NotOld(path) => {
                write!(f, "{} is not an old Miyu data folder", path.display())
            }
            LayoutError::Unfinished(path) => write!(
                f,
                "old Miyu did not finish moving its files ({}); start the old version once to let it recover",
                path.display()
            ),
            LayoutError::BadAdmin(name) => {
                write!(f, "the home layout marker names an invalid user: {name:?}")
            }
            LayoutError::Io(path, error) => write!(f, "cannot read {}: {error}", path.display()),
        }
    }
}

impl std::error::Error for LayoutError {}

/// 旧版搬家中途的日志：存在就是上次没收尾（`paths/resource_migration.rs:16-28`）。
const JOURNALS: [&str; 2] = [
    ".resource-layout-v1.journal.json",
    ".home-layout-v1.journal.json",
];

/// 认 `root` 是哪一代。
///
/// # Errors
///
/// 不是目录、不是旧版的数据根、搬家没收尾、管理员名不对、读不了，见 [`LayoutError`]。
pub fn read(root: &Path) -> Result<Layout, LayoutError> {
    let meta = fs::metadata(root).map_err(|error| LayoutError::Io(root.to_path_buf(), error))?;
    if !meta.is_dir() {
        return Err(LayoutError::NotADirectory(root.to_path_buf()));
    }
    for journal in JOURNALS {
        let path = root.join(journal);
        if path.exists() {
            return Err(LayoutError::Unfinished(path));
        }
    }
    let rolled_back = root.join(".home-layout-off").is_file();
    let home = root.join(".home-layout-v1");
    let (generation, admin) = if home.is_file() && !rolled_back {
        let text =
            fs::read_to_string(&home).map_err(|error| LayoutError::Io(home.clone(), error))?;
        let name = text.trim().to_string();
        if !valid_home_name(&name) {
            return Err(LayoutError::BadAdmin(name));
        }
        (Generation::Home, Some(name))
    } else if root.join(".resource-layout-v1").is_file() {
        (Generation::Resource, None)
    } else if root.join(".layout-v1").is_file() {
        (Generation::Layout, None)
    } else if root.join("config").join("config.jsonc").is_file() {
        (Generation::Unmarked, None)
    } else {
        return Err(LayoutError::NotOld(root.to_path_buf()));
    };
    Ok(Layout {
        root: root.to_path_buf(),
        generation,
        admin,
        rolled_back,
    })
}

/// 旧版的家目录名：3 到 32 个字，`[A-Za-z0-9_.-]`，第一个是字母或数字（`paths/home_layout.rs:145-157`）。
fn valid_home_name(name: &str) -> bool {
    let len = name.chars().count();
    (3..=32).contains(&len)
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

impl Layout {
    /// 管理员的家目录（只有家目录布局有）。
    fn home(&self) -> Option<PathBuf> {
        self.admin
            .as_ref()
            .map(|admin| self.root.join("home").join(admin))
    }

    /// `config/` 下还是 `data/` 下：资源搬过没有。
    fn resources(&self) -> PathBuf {
        match self.generation {
            Generation::Unmarked | Generation::Layout => self.root.join("config"),
            Generation::Resource | Generation::Home => self.root.join("data"),
        }
    }

    /// 配置文件，各代一样。
    pub fn config(&self) -> PathBuf {
        self.root.join("config").join("config.jsonc")
    }

    /// `state/`，各代一样（家目录布局也只搬走了会话库）。
    pub fn state(&self) -> PathBuf {
        self.root.join("state")
    }

    /// 管理员的会话库。
    pub fn conversation_db(&self) -> PathBuf {
        match self.home() {
            Some(home) => home.join("conversation.db"),
            None => self.state().join("conversation.db"),
        }
    }

    /// 共享人格的数据目录（记忆库在 `<它>/<scope>/memory/memory.db`）。
    pub fn personas(&self) -> PathBuf {
        match self.generation {
            Generation::Home => self.root.join("personas"),
            _ => self.root.join("data").join("personas"),
        }
    }

    /// 人格提示词（`<名字>.md`）。
    pub fn prompts(&self) -> PathBuf {
        self.resources().join("prompts")
    }

    /// 用户技能。
    pub fn skills(&self) -> PathBuf {
        match self.generation {
            Generation::Home => self.root.join("extensions").join("skills"),
            _ => self.resources().join("skills"),
        }
    }

    /// 用户脚本。
    pub fn scripts(&self) -> PathBuf {
        match self.generation {
            Generation::Home => self.root.join("extensions").join("scripts"),
            _ => self.resources().join("scripts"),
        }
    }

    /// 属主档案。
    pub fn profile(&self) -> PathBuf {
        match (self.home(), self.generation) {
            (Some(home), _) => home.join("profile.md"),
            (None, Generation::Resource) => self
                .root
                .join("data")
                .join("identities")
                .join("user-identity.md"),
            (None, _) => self.root.join("config").join("user-identity.md"),
        }
    }

    /// 图片、文档、产物（`what` 是 `pictures`、`documents`、`artifacts`）。
    pub fn user_dir(&self, what: &str) -> PathBuf {
        match self.home() {
            Some(home) => home.join(what),
            None => self.root.join("data").join(what),
        }
    }

    /// 默认沙盒的根。
    pub fn workspace(&self) -> PathBuf {
        match self.home() {
            Some(home) => home.join("workspace"),
            None => self.root.join("workspace"),
        }
    }

    /// QQ 消息记录库。
    pub fn qq_history(&self) -> PathBuf {
        self.root
            .join("data")
            .join("platforms")
            .join("onebot")
            .join("message_history")
            .join("history.sqlite3")
    }

    /// 知识库（`files/` 和 `kb_meta.db`）。
    pub fn knowledge(&self) -> PathBuf {
        self.root.join("data").join("kb")
    }

    /// 用户表情库（`<库>/index.json`）。
    pub fn memes(&self) -> PathBuf {
        self.root.join("data").join("memes")
    }

    /// 用量库。
    pub fn usage(&self) -> PathBuf {
        self.state().join("usage.db")
    }

    /// 旧核心的家目录单例锁。
    pub fn daemon_lock(&self) -> PathBuf {
        self.root.join("daemon.lock")
    }
}

#[cfg(test)]
mod tests;

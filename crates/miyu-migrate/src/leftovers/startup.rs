//! 旧版终端集成写进 bash、zsh 启动文件的那段（SH-OLD）：只报不动。认法同新版终端集成 `install` 清旧版那一份（终端蓝图
//! 「旧版那一份」）：候选文件照旧版 `startup.rs`，记号独占一行（两头空白、行尾 `\r` 不算），一个结尾记号和它前面最近的
//! 开头记号之间算一段（同终端 install 实际删的），只有开头的、孤立的结尾不算，一个文件几段都报；照字节读，不是 UTF-8 的也一样。启动文件只读，不写、不建、不改权限；
//! 读不了的报一项说读不了，不让零碎失败。

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::report::{Item, Status};
use crate::words::leftovers::Leftovers;

/// 这台机器的家目录、`ZDOTDIR`、`XDG_CONFIG_HOME`：看数据根外面旧版装过的东西（测试换成临时目录）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Home {
    /// 家目录。
    pub dir: PathBuf,
    /// zsh 的启动文件跟着的 `ZDOTDIR`；没设、空的、不是绝对路径的是 `None`。
    pub zdotdir: Option<PathBuf>,
    /// fish 的配置跟着的 `XDG_CONFIG_HOME`；没设、空的、不是绝对路径的是 `None`（看 `~/.config`）。
    pub config: Option<PathBuf>,
}

impl Home {
    /// `zdotdir`、`config` 是环境里 `ZDOTDIR`、`XDG_CONFIG_HOME` 的原样：空的、不是绝对路径的不算（相对路径照 shell
    /// 起来时的目录找，迁移时猜不到；旧版 `paths::fish_conf_dir` 也只认绝对路径）。
    #[must_use]
    pub fn new(dir: PathBuf, zdotdir: Option<OsString>, config: Option<OsString>) -> Home {
        let absolute =
            |value: Option<OsString>| value.map(PathBuf::from).filter(|path| path.is_absolute());
        Home {
            dir,
            zdotdir: absolute(zdotdir),
            config: absolute(config),
        }
    }

    /// 旧版 fish 终端集成的那份：`$XDG_CONFIG_HOME/fish/conf.d/miyu.fish`，没设的在 `~/.config` 下（同旧版
    /// `paths::fish_conf_dir`、新版终端 install）。
    pub(super) fn old_fish(&self) -> PathBuf {
        self.config
            .clone()
            .unwrap_or_else(|| self.dir.join(".config"))
            .join("fish/conf.d/miyu.fish")
    }
}

/// 每个有旧版那一段的启动文件一项（几段的行号都写在 `note` 里）；读不了的一项说读不了；没有的、没那一段的不报。
pub(super) fn items(home: &Home, words: &Leftovers) -> Vec<Item> {
    candidates(home)
        .into_iter()
        .filter_map(|(shell, file)| {
            let note = match fs::read(&file) {
                Ok(bytes) => {
                    let found = blocks(&bytes, shell);
                    if found.is_empty() {
                        return None;
                    }
                    (words.hook_note)(shell, &found)
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
                Err(error) => (words.hook_unreadable)(&error.to_string()),
            };
            let (what, source) = named(
                &home.dir,
                &file,
                "ZDOTDIR",
                home.zdotdir.as_deref().unwrap_or(&home.dir),
            );
            Some(Item {
                what,
                status: Status::Skipped,
                source,
                format: (words.hook_format)(shell),
                migrated: 0,
                skipped: Vec::new(),
                target: None,
                note: Some(note),
            })
        })
        .collect()
}

/// 旧版看过的启动文件（旧版 `startup.rs` 的 `candidates`）：bash 四个（旧版在 macOS 上写登录文件），zsh 的 `~/.zshrc` 和
/// `$ZDOTDIR/.zshrc`，同一个文件只看一次。
fn candidates(home: &Home) -> Vec<(&'static str, PathBuf)> {
    let mut files: Vec<(&'static str, PathBuf)> =
        [".bashrc", ".bash_profile", ".bash_login", ".profile"]
            .into_iter()
            .map(|name| ("bash", home.dir.join(name)))
            .collect();
    let zshrc = home.dir.join(".zshrc");
    let dotted = home
        .zdotdir
        .as_ref()
        .map(|zdotdir| zdotdir.join(".zshrc"))
        .filter(|file| !same(file, &zshrc));
    files.push(("zsh", zshrc));
    files.extend(dotted.map(|file| ("zsh", file)));
    files
}

/// 同一个文件：路径一样，或者解开链接以后一样。
fn same(one: &Path, other: &Path) -> bool {
    one == other
        || fs::canonicalize(one)
            .ok()
            .zip(fs::canonicalize(other).ok())
            .is_some_and(|(one, other)| one == other)
}

/// 报告里叫什么、写哪个文件：家目录里的是 `outside/<相对路径>` 和 `~/<相对路径>`；环境变量 `var`（`ZDOTDIR`、
/// `XDG_CONFIG_HOME`）指到家目录外面的 `base` 里的，是 `outside/$<var>/<相对 base 的路径>` 和实际路径。
pub(super) fn named(home: &Path, file: &Path, var: &str, base: &Path) -> (String, String) {
    let slashed = |path: &Path| path.to_string_lossy().replace('\\', "/");
    if let Ok(relative) = file.strip_prefix(home) {
        let relative = slashed(relative);
        return (format!("outside/{relative}"), format!("~/{relative}"));
    }
    let relative = file.strip_prefix(base).map_or_else(
        |_| {
            file.file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default()
        },
        slashed,
    );
    (
        format!("outside/${var}/{relative}"),
        file.display().to_string(),
    )
}

/// 文件里旧版写的那几段：(开头记号那一行, 结尾记号那一行)，从 1 数；开头取结尾前面最近的那个。
fn blocks(bytes: &[u8], shell: &str) -> Vec<(usize, usize)> {
    let begin = format!("# >>> miyu {shell} hook >>>");
    let end = format!("# <<< miyu {shell} hook <<<");
    let mut found = Vec::new();
    let mut open = None;
    for (index, line) in bytes.split(|byte| *byte == b'\n').enumerate() {
        let line = line.trim_ascii();
        if line == begin.as_bytes() {
            // 开着又遇到开头的：前一个只有开头、不算一段，从这个重新算。
            open = Some(index + 1);
        } else if line == end.as_bytes()
            && let Some(start) = open.take()
        {
            found.push((start, index + 1));
        }
    }
    found
}

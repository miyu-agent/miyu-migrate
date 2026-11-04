//! 搜索脚本的登录态（细则第十节，第十一节已定第 5 条）：旧版 `cache/<工具>/` 里浏览器登录、cookie 这些。
//!
//! - `goofish-search/`：`session.json`（Playwright 的 storage_state）和监控票据 `<md5 前 12 位>.json` 拷进新版
//!   `miyu-scripts-goofish` 的状态目录 `root/state/scripts/miyu-scripts-goofish/goofish/legacy/`；新包认不认这两种格式没核实，报告写清。
//!   包没装的不拷，提示装。
//! - `zhihu-search/`、`xhs-search/`、`bilibili/`：新版没有对应的包，只列进报告，登录态留在压缩包里。
//! - `bilibili-live-stream/`：新版有 `miyu-skills-bilibili-live-stream`，旧版的登录态格式没核实，不拷：报告提示装上新包、在里面重新登录。
//! - `*.lock`、`login-qr.png` 不拷。登录态是凭据：报告只写文件名和条数；拷过去的 Unix 上是 0600（目录 0700）。

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::core::{Core, Said};
use crate::layout::Layout;
use crate::report::{Item, Skip, Status};

/// 闲鱼的新包。
pub const GOOFISH: &str = "miyu-scripts-goofish";
/// B 站开播的新包。
pub const LIVE: &str = "miyu-skills-bilibili-live-stream";

/// 迁登录态：读 `copy` 的 `cache/`，闲鱼的拷进新数据根 `root`（包装着的话）。交回报告的一项。
///
/// # Errors
///
/// 连接断了。
pub fn migrate(core: &mut dyn Core, copy: &Layout, root: &Path) -> Result<Item, Said> {
    let cache = copy.root.join("cache");
    let mut item = Item {
        what: "logins".to_string(),
        status: Status::Migrated,
        source: "cache/<工具>/".to_string(),
        format: "goofish-search：session.json（Playwright storage_state）、<md5 前 12 位>.json（监控票据）；zhihu-search、xhs-search：Chromium 的 profile；bilibili：cookie".to_string(),
        migrated: 0,
        skipped: Vec::new(),
        target: Some(format!("root/state/scripts/{GOOFISH}/goofish/legacy/")),
        note: None,
    };
    let mut notes = Vec::new();
    let goofish = cache.join("goofish-search");
    let files = wanted(&goofish);
    if !files.is_empty() {
        let installed = core
            .call("package.list", json!({}))?
            .get("packages")
            .and_then(Value::as_array)
            .is_some_and(|all| all.iter().any(|one| one["package"] == GOOFISH));
        if installed {
            let target = root
                .join("root")
                .join("state")
                .join("scripts")
                .join(GOOFISH)
                .join("goofish")
                .join("legacy");
            for file in &files {
                let name = file
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default();
                let copied =
                    fs::create_dir_all(&target).and_then(|()| private(file, &target, &name));
                match copied {
                    Ok(_) => item.migrated += 1,
                    Err(error) => item.skipped.push(skip(
                        &format!("cache/goofish-search/{name}"),
                        &format!("write: {}", error.kind()),
                    )),
                }
            }
            notes.push(format!(
                "闲鱼的登录态 {} 个文件拷进了 `root/state/scripts/{GOOFISH}/goofish/legacy/`；新包认不认旧版的格式没核实，要是还得重新登录，照新包的说明登一次。",
                item.migrated
            ));
        } else {
            item.skipped
                .push(skip("cache/goofish-search", &format!("install:{GOOFISH}")));
            notes.push(format!("闲鱼的登录态 {} 个文件：`{GOOFISH}` 没装，装上以后重跑迁移，或者在新包里重新登录。", files.len()));
        }
    }
    let mut others = Vec::new();
    for entry in children(&cache) {
        let name = entry
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        if matches!(name.as_str(), "zhihu-search" | "xhs-search" | "bilibili") {
            item.skipped
                .push(skip(&format!("cache/{name}"), "no-package"));
            others.push(name);
        } else if name == "bilibili-live-stream" {
            item.skipped
                .push(skip(&format!("cache/{name}"), &format!("install:{LIVE}")));
            notes.push(format!("B 站开播的登录态有新包 `{LIVE}`，旧版的格式没核实、没拷：装上新包以后在里面重新登录一次。"));
        }
    }
    if !others.is_empty() {
        notes.push(format!(
            "{} 新版还没有对应的包，登录态留在压缩包里（`cache/` 下），以后有了包再迁。",
            others.join("、")
        ));
    }
    if files.is_empty() && item.skipped.is_empty() {
        item.status = Status::Skipped;
        notes.push("旧根里没有搜索脚本的登录态。".to_string());
    } else if !item.skipped.is_empty() {
        item.status = Status::Partial;
    }
    item.note = Some(notes.join(""));
    Ok(item)
}

/// 闲鱼目录里要拷的：`session.json` 和别的 `.json`（监控票据），`*.lock`、`login-qr.png` 不要。照名字排。
fn wanted(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_file())
                .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

/// 凭据拷成只有自己读得到的（Unix 上目录 0700、文件 0600），不跟着源文件、umask 放宽：数据根以后有人放宽了权限，凭据也不跟着露。
fn private(file: &Path, target: &Path, name: &str) -> std::io::Result<()> {
    let to = target.join(name);
    fs::copy(file, &to)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(target, fs::Permissions::from_mode(0o700))?;
        fs::set_permissions(&to, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
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

fn skip(key: &str, why: &str) -> Skip {
    Skip {
        key: key.to_string(),
        why: why.to_string(),
    }
}

#[cfg(test)]
mod tests;

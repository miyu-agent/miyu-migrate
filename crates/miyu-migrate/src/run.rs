//! 一次迁移从头到尾（细则第十三节）：认布局、查旧核心停了没有、打包核对、逐类迁（骨架这一步还没有类）、出报告。

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::archive::{self, ArchiveError};
use crate::layout::{self, LayoutError};
use crate::lock;
use crate::report::{self, Archive, Item, OldRoot, Report, Status};

/// 迁到哪：这一次的东西都放在 `<状态目录>/<时刻>/` 下（包的状态目录，转交时由 `MIYU_STATE_DIR` 给）。
#[derive(Debug, Clone)]
pub struct Plan {
    /// 旧数据根。
    pub old: PathBuf,
    /// 包的状态目录。
    pub state: PathBuf,
    /// 这一次的时刻：目录名、报告里写的。
    pub at: jiff::Timestamp,
}

/// 迁移失败。
#[derive(Debug)]
pub enum RunError {
    /// 认不出旧根。
    Layout(LayoutError),
    /// 旧核心还在跑。
    StillRunning(Option<u32>),
    /// 打包、核对。
    Archive(ArchiveError),
    /// 别的读写。
    Io(PathBuf, io::Error),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::Layout(error) => write!(f, "{error}"),
            RunError::StillRunning(Some(pid)) => {
                write!(f, "old Miyu is still running (pid {pid}); stop it first")
            }
            RunError::StillRunning(None) => write!(f, "old Miyu is still running; stop it first"),
            RunError::Archive(error) => write!(f, "{error}"),
            RunError::Io(path, error) => write!(f, "{}: {error}", path.display()),
        }
    }
}

impl std::error::Error for RunError {}

/// 迁完交回的：两份报告在哪。
#[derive(Debug)]
pub struct Done {
    /// 给人看的。
    pub human: PathBuf,
    /// 给 agent 读的。
    pub agent: PathBuf,
}

/// 照 `plan` 迁一次。
///
/// # Errors
///
/// 认不出旧根、旧核心还在跑、打包核对出错、写不了，见 [`RunError`]。
pub fn run(plan: &Plan) -> Result<Done, RunError> {
    let old = layout::read(&plan.old).map_err(RunError::Layout)?;
    let lock = old.daemon_lock();
    if let Some(holder) = lock::holder(&lock).map_err(|error| RunError::Io(lock.clone(), error))? {
        return Err(RunError::StillRunning(holder.pid));
    }
    let dir = plan.state.join(stamp(plan.at));
    fs::create_dir_all(&dir).map_err(|error| RunError::Io(dir.clone(), error))?;
    let packed = dir.join("miyu-0.7.tar.zst");
    let manifest = archive::pack(&old.root, &packed).map_err(RunError::Archive)?;
    let listed = dir.join("miyu-0.7.manifest.json");
    let text = serde_json::to_string_pretty(&manifest)
        .map_err(|error| RunError::Io(listed.clone(), io::Error::other(error)))?;
    fs::write(&listed, text + "\n").map_err(|error| RunError::Io(listed.clone(), error))?;
    let work = dir.join("work");
    archive::unpack_and_check(&packed, &manifest, &work).map_err(RunError::Archive)?;
    let (files, bytes) = manifest.totals();
    let (_, sha256) =
        archive::hash_file(&packed).map_err(|error| RunError::Io(packed.clone(), error))?;
    let report = Report {
        format: report::FORMAT,
        migrate: env!("CARGO_PKG_VERSION").to_string(),
        at: plan.at.to_string(),
        archive: Some(Archive {
            path: absolute(&packed),
            sha256,
            files,
            bytes,
            manifest: absolute(&listed),
        }),
        old_root: OldRoot {
            path: absolute(&old.root),
            layout: old.generation.name().to_string(),
            admin: old.admin.clone(),
            rolled_back: old.rolled_back,
        },
        items: vec![Item {
            what: "archive".to_string(),
            status: Status::Migrated,
            source: ".".to_string(),
            format: "旧数据根整个打成 tar.zst，逐文件 SHA-256 记在清单里".to_string(),
            migrated: files,
            skipped: Vec::new(),
            target: Some(absolute(&packed)),
            note: (!manifest.skipped.is_empty()).then(|| {
                format!(
                    "{} 样不是普通文件、目录、链接，没打进去。",
                    manifest.skipped.len()
                )
            }),
        }],
        pending: Vec::new(),
    };
    let (human, agent) = report
        .write(&dir)
        .map_err(|error| RunError::Io(dir.clone(), error))?;
    // 解出来的副本用完就删：留着的是压缩包、清单、报告。
    fs::remove_dir_all(&work).map_err(|error| RunError::Io(work.clone(), error))?;
    Ok(Done { human, agent })
}

/// 目录名用的时刻：秒以下不要，冒号换掉（Windows 的文件名里不能有）。
fn stamp(at: jiff::Timestamp) -> String {
    at.strftime("%Y%m%dT%H%M%SZ").to_string()
}

fn absolute(path: &Path) -> String {
    std::path::absolute(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}

#[cfg(test)]
mod tests;

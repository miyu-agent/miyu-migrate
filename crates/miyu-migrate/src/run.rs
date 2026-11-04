//! 一次迁移从头到尾（细则第十三节）：认布局、查旧核心停了没有、看盘够不够、打包核对、逐类迁（人格、会话、QQ 消息记录、记忆、配置）、
//! 出报告。

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::archive::{self, ArchiveError};
use crate::core::{Core, Said};
use crate::layout::{self, LayoutError};
use crate::lock;
use crate::personas::{self, How};
use crate::report::{self, Archive, Item, OldRoot, Report, Skip, Status};

/// 迁到哪：这一次的东西都放在 `<状态目录>/<时刻>/` 下（包的状态目录，转交时由 `MIYU_STATE_DIR` 给）。
#[derive(Debug, Clone)]
pub struct Plan {
    /// 旧数据根。
    pub old: PathBuf,
    /// 包的状态目录。
    pub state: PathBuf,
    /// 这一次的时刻：目录名、报告里写的。
    pub at: jiff::Timestamp,
    /// 新版的数据根：借快照时读它的 blob 库。
    pub root: miyu_client::places::DataRoot,
    /// QQ 消息记录交 im 导（真跑的是 `miyu im records import`）。
    pub records: crate::records::Importer,
    /// 一块盘还剩多少字节（真跑的是 [`free_space`]，测试换成假的）。
    pub space: fn(&Path) -> io::Result<u64>,
}

/// `path` 所在那块盘还剩多少字节（给这个用户的）。
///
/// # Errors
///
/// 问不到。
pub fn free_space(path: &Path) -> io::Result<u64> {
    fs4::available_space(path)
}

/// 迁移失败。
#[derive(Debug)]
pub enum RunError {
    /// 认不出旧根。
    Layout(LayoutError),
    /// 旧核心还在跑。
    StillRunning(Option<u32>),
    /// 这一次的目录已经有了（同一秒跑了两次）：不盖。
    Exists(PathBuf),
    /// 状态目录那块盘不够：要多少、有多少（字节）。
    NoSpace {
        /// 要：旧根的两倍（压缩包按不压缩算，加上解出来核对的那一份）。
        need: u64,
        /// 有。
        have: u64,
    },
    /// 核心拒了、断了。
    Core(Said),
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
            RunError::Exists(dir) => write!(
                f,
                "{} already exists; wait a second and run again",
                dir.display()
            ),
            RunError::NoSpace { need, have } => write!(
                f,
                "not enough disk space for the archive and its check: need {} MiB, have {} MiB",
                need.div_ceil(1 << 20),
                have >> 20
            ),
            RunError::Core(said) => write!(f, "the core said {said}"),
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

/// 照 `plan` 迁一次，写新版经 `core`。
///
/// # Errors
///
/// 认不出旧根、旧核心还在跑、这一次的目录已经有了、盘不够、打包核对出错、核心拒了、写不了，见 [`RunError`]。
pub fn run(plan: &Plan, core: &mut dyn Core) -> Result<Done, RunError> {
    let old = layout::read(&plan.old).map_err(RunError::Layout)?;
    let lock = old.daemon_lock();
    if let Some(holder) = lock::holder(&lock).map_err(|error| RunError::Io(lock.clone(), error))? {
        return Err(RunError::StillRunning(holder.pid));
    }
    fs::create_dir_all(&plan.state).map_err(|error| RunError::Io(plan.state.clone(), error))?;
    let need = size(&old.root)
        .map_err(|error| RunError::Io(old.root.clone(), error))?
        .saturating_mul(2);
    let have =
        (plan.space)(&plan.state).map_err(|error| RunError::Io(plan.state.clone(), error))?;
    if have < need {
        return Err(RunError::NoSpace { need, have });
    }
    let dir = plan.state.join(stamp(plan.at));
    match fs::create_dir(&dir) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return Err(RunError::Exists(dir));
        }
        Err(error) => return Err(RunError::Io(dir, error)),
    }
    let packed = dir.join("miyu-0.7.tar.zst");
    let manifest = archive::pack(&old.root, &packed).map_err(RunError::Archive)?;
    let listed = dir.join("miyu-0.7.manifest.json");
    let text = serde_json::to_string_pretty(&manifest)
        .map_err(|error| RunError::Io(listed.clone(), io::Error::other(error)))?;
    fs::write(&listed, text + "\n").map_err(|error| RunError::Io(listed.clone(), error))?;
    let work = dir.join("work");
    archive::unpack_and_check(&packed, &manifest, &work).map_err(RunError::Archive)?;
    let copy = layout::read(&work).map_err(RunError::Layout)?;
    let found =
        personas::read(&copy, &old.root).map_err(|error| RunError::Io(work.clone(), error))?;
    let mapped = personas::apply(core, &found.olds).map_err(RunError::Core)?;
    let sessions = crate::sessions::migrate(
        core,
        &plan.root,
        (&copy, &old.root),
        &mapped,
        &dir.join("stage"),
    )
    .map_err(RunError::Core)?;
    let (files, bytes) = manifest.totals();
    let (_, sha256) =
        archive::hash_file(&packed).map_err(|error| RunError::Io(packed.clone(), error))?;
    let mut report = Report {
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
        items: Vec::new(),
        personas: Vec::new(),
        pending: Vec::new(),
    };
    report.items.push(Item {
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
    });
    report.items.extend(persona_items(&mapped, &found.extras));
    report.items.push(sessions);
    report
        .items
        .push(crate::records::migrate(&copy, &dir, &plan.records));
    let (memories, pending) = crate::memories::migrate(core, &copy, &mapped, std::thread::sleep)
        .map_err(RunError::Core)?;
    report.items.push(memories);
    report.pending.extend(pending);
    report
        .items
        .push(crate::settings::migrate(core, &copy).map_err(RunError::Core)?);
    report.personas = mapped;
    let (human, agent) = report
        .write(&dir)
        .map_err(|error| RunError::Io(dir.clone(), error))?;
    // 解出来的副本、暂存的包用完就删：留着的是压缩包、清单、报告。
    fs::remove_dir_all(&work).map_err(|error| RunError::Io(work.clone(), error))?;
    let stage = dir.join("stage");
    if stage.exists() {
        fs::remove_dir_all(&stage).map_err(|error| RunError::Io(stage.clone(), error))?;
    }
    Ok(Done { human, agent })
}

/// 人格这一类、只报不迁的几样（细则第六节）。
fn persona_items(mapped: &[personas::Mapped], extras: &[personas::Extra]) -> Vec<Item> {
    let skipped: Vec<Skip> = mapped
        .iter()
        .filter(|row| row.how == How::Missing)
        .map(|row| Skip {
            key: row.old.clone(),
            why: "unmapped".to_string(),
        })
        .collect();
    let mut items = vec![Item {
        what: "personas".to_string(),
        status: if skipped.is_empty() {
            Status::Migrated
        } else {
            Status::Partial
        },
        source: "data/prompts".to_string(),
        format: "<名字>.md 是人设；hints/<scope>.md 角色扮演提示、dialogs/<scope>.md 示范对话（行首 user:/assistant:），空文件是停用；<名字>.json 的 avatar_path 是头像".to_string(),
        migrated: mapped.iter().filter(|row| row.new.is_some()).count() as u64,
        skipped,
        target: Some("persona.set".to_string()),
        note: None,
    }];
    for extra in extras {
        let (what, format) = match extra.why {
            "persona-settings" => (
                "persona-settings",
                "TOML：[subsystems] memory/persona_reminder/voice/emotion，[plugins] enabled/scripts/skills/mcp 白名单；新版没有对应",
            ),
            "dev-prompt" => (
                "dev-prompt",
                "开发模式的提示词（Markdown）；新版的软件工程师是出厂的，不改",
            ),
            "system-prompt" => (
                "system-prompt",
                "旧版没选人格时的自定义系统提示词（Markdown）；新版没有对应",
            ),
            _ => (
                "persona-avatar",
                "网页附带的 avatar_path 指的头像在旧根以外或者不在了",
            ),
        };
        items.push(Item {
            what: what.to_string(),
            status: Status::Archived,
            source: extra.source.clone(),
            format: format.to_string(),
            migrated: 0,
            skipped: Vec::new(),
            target: None,
            note: None,
        });
    }
    items
}

/// 旧根一共多大：普通文件的字节数（链接不跟）。
fn size(root: &Path) -> io::Result<u64> {
    let mut total = 0u64;
    for entry in walkdir::WalkDir::new(root) {
        let entry = entry.map_err(io::Error::other)?;
        if entry.file_type().is_file() {
            total = total.saturating_add(entry.metadata().map_err(io::Error::other)?.len());
        }
    }
    Ok(total)
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

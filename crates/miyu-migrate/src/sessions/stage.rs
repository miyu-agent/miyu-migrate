//! 一个会话打成原始包（细则第一节第 14 条）：拼好的事件交核心的 crate 读一遍（`Event::from_line`）、照它的规矩写进暂存目录的
//! 会话日志（`SessionLog`，段照它的上限切；写之前照核心的账本查一遍），附件、快照放进暂存的 blob 库，再交 `bundle::write` 打成 ZIP（引用了、库里没有的
//! 进清单的 `absent`）。行都由核心的 crate 写，不手拼。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

use miyu_kernel::event::Event;
use miyu_kernel::id::{ContentHash, SessionId};
use miyu_kernel::ledger::Ledger;
use miyu_store::blob::Blobs;
use miyu_store::bundle::{self, About, Part};
use miyu_store::log::{SEGMENT_LIMIT, SessionLog};

/// 暂存：`<它>/sessions/<编号>/` 放日志，`<它>/blobs/` 放 blob，`<它>/bundles/` 放打好的包。
pub struct Stage {
    dir: PathBuf,
    blobs: Blobs,
}

/// 打包出错。
#[derive(Debug)]
pub enum StageError {
    /// 拼出来的一条核心读不懂：第几条、原话（拼法的 bug，或者旧数据里有核心不收的东西）。
    Event(u64, String),
    /// 过不了核心的账本：第几条、原话（导进去也载入不了）。
    Ledger(u64, String),
    /// 编号写法不对。
    Id(String),
    /// 打包。
    Bundle(String),
    /// 读写。
    Io(io::Error),
}

impl std::fmt::Display for StageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StageError::Event(seq, why) => write!(f, "event {seq}: {why}"),
            StageError::Ledger(seq, why) => write!(f, "ledger at {seq}: {why}"),
            StageError::Id(why) => write!(f, "{why}"),
            StageError::Bundle(why) => write!(f, "bundle: {why}"),
            StageError::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for StageError {}

impl From<io::Error> for StageError {
    fn from(error: io::Error) -> StageError {
        StageError::Io(error)
    }
}

impl Stage {
    /// 在 `dir` 下暂存（没有的建）。
    ///
    /// # Errors
    ///
    /// 建不了目录。
    pub fn new(dir: &Path) -> io::Result<Stage> {
        fs::create_dir_all(dir.join("bundles"))?;
        Ok(Stage {
            dir: dir.to_path_buf(),
            blobs: Blobs::new(dir.join("blobs")),
        })
    }

    /// 放一份 blob，交回 `sha256:…`。
    ///
    /// # Errors
    ///
    /// 写不进。
    pub fn put(&self, bytes: &[u8]) -> io::Result<String> {
        // 不用 `Blobs::put`：它建目录只认数据根里面的，暂存可能不在数据根里。照它的布局（`<前两位>/<64 位>`）自己写，读还交给它。
        let hash = ContentHash::of(bytes);
        let path = self.blobs.path(&hash);
        if !path.is_file() {
            if let Some(fan) = path.parent() {
                fs::create_dir_all(fan)?;
            }
            fs::write(&path, bytes)?;
        }
        Ok(hash.to_string())
    }

    /// 会话 `id` 的事件写成日志、打成包，交回包的位置。已经打过的（重跑）先删掉暂存里的旧的。
    ///
    /// # Errors
    ///
    /// 见 [`StageError`]。
    pub fn bundle(
        &self,
        id: &str,
        events: &[Value],
        exported: &str,
    ) -> Result<PathBuf, StageError> {
        let session = SessionId::parse(id).map_err(|error| StageError::Id(error.to_string()))?;
        let dir = self.dir.join("sessions").join(id);
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
        let parsed = events
            .iter()
            .map(|event| {
                Event::from_line(&event.to_string()).map_err(|error| {
                    StageError::Event(event["seq"].as_u64().unwrap_or_default(), error.to_string())
                })
            })
            .collect::<Result<Vec<Event>, StageError>>()?;
        // 载入时核心照账本一条条查，过不了的整个会话打不开：这里先照同一套规矩查一遍，过不了的不打包。
        let mut ledger = Ledger::default();
        for event in &parsed {
            ledger
                .append(event)
                .map_err(|error| StageError::Ledger(event.seq.get(), error.to_string()))?;
        }
        // 核心建日志目录只认数据根里面的（防的是往删掉了的数据根里写）；暂存是迁移程序自己的目录，先建好，它就照常写。
        fs::create_dir_all(&dir)?;
        let mut log = SessionLog::create(&dir, SEGMENT_LIMIT)?;
        log.append(&parsed)?;
        drop(log);
        let out = self.dir.join("bundles").join(format!("{id}.miyu"));
        if out.exists() {
            fs::remove_file(&out)?;
        }
        let about = About {
            main: session.clone(),
            miyu: format!("miyu-migrate {}", env!("CARGO_PKG_VERSION")),
            exported: exported.to_string(),
        };
        let part = Part {
            session,
            dir,
            upto: None,
        };
        bundle::write(&[part], &self.blobs, &about, &out)
            .map_err(|error| StageError::Bundle(format!("{error:?}")))?;
        Ok(out)
    }
}

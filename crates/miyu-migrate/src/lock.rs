//! 旧核心停了没有（细则第十三节第 1 条）：照旧版自己的判法，先对 `<旧根>/daemon.lock` 试排他锁，拿得到就是没有旧核心在跑；
//! 拿不到再读里面的 `{"pid","runtime_dir"}` 说给人听（`crates/miyu-core/src/ipc/lifecycle.rs:123-172`）。不能只读文件里的
//! pid：那多半是上一任留下的。Windows 上的锁是强制的，锁着的文件别的句柄读不了，那里只说得出「占着」、说不出是谁。

use std::fs::{OpenOptions, TryLockError};
use std::io;
use std::path::Path;

use serde::Deserialize;

/// 占着锁的旧核心：锁文件里写的。
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct Holder {
    /// 进程号。
    #[serde(default)]
    pub pid: Option<u32>,
    /// 它的运行时目录。
    #[serde(default)]
    pub runtime_dir: Option<String>,
}

/// 有没有旧核心占着 `lock`（旧根下的 `daemon.lock`）：没有锁文件、拿得到锁的是没有；拿不到的交回锁里写的。
///
/// # Errors
///
/// 打不开锁文件，或者试锁出了「被占着」以外的错。
pub fn holder(lock: &Path) -> io::Result<Option<Holder>> {
    let file = match OpenOptions::new().read(true).write(true).open(lock) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    match file.try_lock() {
        Ok(()) => {
            file.unlock()?;
            Ok(None)
        }
        Err(TryLockError::WouldBlock) => Ok(Some(written(lock))),
        Err(TryLockError::Error(error)) => Err(error),
    }
}

/// 锁文件里写的：读不出、写坏了的当什么都没写。
fn written(lock: &Path) -> Holder {
    std::fs::read_to_string(lock)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;

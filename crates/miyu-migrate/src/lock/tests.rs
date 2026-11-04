//! 旧核心停了没有：没有锁文件的、拿得到锁的是停了；占着的交回锁里写的进程号；锁里写坏了的照样认出占着。

use std::fs::{File, OpenOptions};

use super::*;

/// 替旧核心占着锁：交回拿着锁的文件，放掉它就松开。
fn hold(lock: &Path, text: &str) -> File {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(lock)
        .unwrap();
    file.lock().unwrap();
    std::fs::write(lock, text).unwrap();
    file
}

#[test]
fn no_lock_file_or_a_free_lock_means_stopped() {
    let dir = tempfile::tempdir().unwrap();
    let lock = dir.path().join("daemon.lock");
    assert_eq!(holder(&lock).unwrap(), None);
    std::fs::write(&lock, r#"{"pid":42,"runtime_dir":"/run/user/1000/miyu"}"#).unwrap();
    assert_eq!(holder(&lock).unwrap(), None, "上一任留下的 pid 不算");
}

#[test]
fn a_held_lock_names_the_old_core() {
    let dir = tempfile::tempdir().unwrap();
    let lock = dir.path().join("daemon.lock");
    let held = hold(&lock, r#"{"pid":4242,"runtime_dir":"/run/user/1000/miyu"}"#);
    let found = holder(&lock).unwrap().expect("占着");
    assert_eq!(found.pid, Some(4242));
    assert_eq!(found.runtime_dir.as_deref(), Some("/run/user/1000/miyu"));
    drop(held);
    assert_eq!(holder(&lock).unwrap(), None, "放掉了");
}

#[test]
fn a_held_lock_with_garbage_is_still_held() {
    let dir = tempfile::tempdir().unwrap();
    let lock = dir.path().join("daemon.lock");
    let _held = hold(&lock, "not json");
    assert_eq!(holder(&lock).unwrap(), Some(Holder::default()));
}

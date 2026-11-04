//! 一次迁移：打包、核对、两份报告落在 `<状态目录>/<时刻>/`，解出来的副本删掉；旧核心还在跑的拒、什么都不写。

use super::*;

fn old_root() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("home/shorin")).unwrap();
    fs::write(dir.path().join(".home-layout-v1"), "shorin\n").unwrap();
    fs::write(dir.path().join("home/shorin/conversation.db"), b"db").unwrap();
    dir
}

fn plan(old: &Path, state: &Path) -> Plan {
    Plan {
        old: old.to_path_buf(),
        state: state.to_path_buf(),
        at: "2026-11-04T08:00:00Z".parse().unwrap(),
    }
}

#[test]
fn a_run_leaves_the_archive_its_list_and_two_reports() {
    let old = old_root();
    let state = tempfile::tempdir().unwrap();
    let done = run(&plan(old.path(), state.path())).unwrap();
    let dir = state.path().join("20261104T080000Z");
    assert_eq!(done.human, dir.join("报告.md"));
    assert!(dir.join("miyu-0.7.tar.zst").is_file());
    assert!(dir.join("miyu-0.7.manifest.json").is_file());
    assert!(!dir.join("work").exists(), "副本删了");
    let agent: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&done.agent).unwrap()).unwrap();
    assert_eq!(agent["old_root"]["layout"], "home-layout-v1");
    assert_eq!(agent["old_root"]["admin"], "shorin");
    assert_eq!(agent["archive"]["files"], 2);
    assert!(
        old.path().join("home/shorin/conversation.db").is_file(),
        "旧目录不动"
    );
}

#[test]
fn a_running_old_core_stops_the_run_before_anything_is_written() {
    let old = old_root();
    let lock = old.path().join("daemon.lock");
    let held = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock)
        .unwrap();
    held.lock().unwrap();
    fs::write(&lock, r#"{"pid":777}"#).unwrap();
    let state = tempfile::tempdir().unwrap();
    match run(&plan(old.path(), state.path())) {
        Err(RunError::StillRunning(Some(777))) => {}
        other => panic!("{other:?}"),
    }
    assert_eq!(fs::read_dir(state.path()).unwrap().count(), 0, "什么都没写");
}

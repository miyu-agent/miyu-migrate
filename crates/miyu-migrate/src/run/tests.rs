//! 一次迁移：打包、核对、人格映射、两份报告落在新数据根的 `root/documents/旧版 Miyu 迁移/<时刻>/`，状态目录里解出来的副本
//! 删掉；旧核心还在跑的、哪块盘不够的拒、什么都不写；同一秒再跑一次的拒、不盖上一次的。

use serde_json::{Value, json};

use super::*;

/// 假核心：只有出厂的软件工程师，新建的都叫 `persona-1`。
struct Fake;

impl Core for Fake {
    fn call(&mut self, method: &str, _params: Value) -> Result<Value, Said> {
        match method {
            "persona.list" => Ok(json!({"personas": [{"persona": "software-engineer"}]})),
            "persona.set" => Ok(json!({"persona": "persona-1"})),
            other => panic!("没想到会调 {other}"),
        }
    }
}

/// 盘大得很。同真的一样，问的目录要已经有了。
fn plenty(path: &Path) -> io::Result<u64> {
    match path.is_dir() {
        true => Ok(u64::MAX),
        false => Err(io::Error::from(io::ErrorKind::NotFound)),
    }
}

/// 正好剩旧根两倍（18 个字节）。
fn exact(_: &Path) -> io::Result<u64> {
    Ok(18)
}

/// 只剩 3 个字节。
fn tight(_: &Path) -> io::Result<u64> {
    Ok(3)
}

/// 放数据根的那块盘（目录名带 `keepdisk`）大得很，别的只剩 3 个字节。
fn state_tight(path: &Path) -> io::Result<u64> {
    let roomy = path.to_string_lossy().contains("keepdisk");
    Ok(if roomy { u64::MAX } else { 3 })
}

/// 这一次留给人的目录。
fn kept(state: &Path) -> PathBuf {
    state.join("new-root/root/documents/旧版 Miyu 迁移/20261104T080000Z")
}

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
        root: miyu_client::places::DataRoot::at(state.join("new-root")),
        records: crate::records::Importer::new(|_| Err(std::io::Error::other("no im"))),
        home: None,
        space: plenty,
    }
}

#[test]
fn a_run_leaves_the_archive_its_list_and_two_reports() {
    let old = old_root();
    let state = tempfile::tempdir().unwrap();
    let done = run(&plan(old.path(), state.path()), &mut Fake).unwrap();
    let dir = kept(state.path());
    assert_eq!(done.human, dir.join("报告.md"));
    assert_eq!(done.agent, dir.join("给agent.json"));
    assert!(dir.join("miyu-0.7.tar.zst").is_file());
    assert!(dir.join("miyu-0.7.manifest.json").is_file());
    assert!(
        !state.path().join("20261104T080000Z").exists(),
        "状态目录里的中间产物删了"
    );
    let agent: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&done.agent).unwrap()).unwrap();
    assert_eq!(agent["old_root"]["layout"], "home-layout-v1");
    assert_eq!(agent["old_root"]["admin"], "shorin");
    assert_eq!(agent["archive"]["files"], 2);
    assert_eq!(
        agent["personas"],
        json!([
            {"old": "default", "name": "Miyu", "new": "persona-1", "how": "created"},
            {"old": "dev", "name": "dev", "new": "software-engineer", "how": "engineer"},
        ])
    );
    assert_eq!(agent["items"][1]["what"], "personas");
    assert_eq!(agent["items"][1]["migrated"], 2);
    assert!(
        old.path().join("home/shorin/conversation.db").is_file(),
        "旧目录不动"
    );
}

#[test]
fn a_running_old_core_stops_the_run_before_anything_is_written() {
    let old = old_root();
    let lock = old.path().join("daemon.lock");
    fs::write(&lock, r#"{"pid":777}"#).unwrap();
    let held = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&lock)
        .unwrap();
    held.lock().unwrap();
    let state = tempfile::tempdir().unwrap();
    match run(&plan(old.path(), state.path()), &mut Fake) {
        // Windows 上锁着的文件读不出写了谁，只知道占着。
        Err(RunError::StillRunning(pid)) if pid == (!cfg!(windows)).then_some(777) => {}
        other => panic!("{other:?}"),
    }
    assert_eq!(fs::read_dir(state.path()).unwrap().count(), 0, "什么都没写");
}

#[test]
fn the_same_second_twice_is_refused_and_keeps_the_first() {
    let old = old_root();
    let state = tempfile::tempdir().unwrap();
    let first = run(&plan(old.path(), state.path()), &mut Fake).unwrap();
    let report = fs::read(&first.human).unwrap();
    match run(&plan(old.path(), state.path()), &mut Fake) {
        Err(RunError::Exists(dir)) => assert_eq!(dir, kept(state.path())),
        other => panic!("{other:?}"),
    }
    assert_eq!(fs::read(&first.human).unwrap(), report, "上一次的没盖");
}

#[test]
fn too_little_room_stops_before_packing() {
    let old = old_root();
    let state = tempfile::tempdir().unwrap();
    let tight_plan = Plan {
        home: None,
        space: tight,
        ..plan(old.path(), state.path())
    };
    match run(&tight_plan, &mut Fake) {
        // 旧根里 `shorin\n` 和 `db` 一共 9 个字节，要两倍。
        Err(RunError::NoSpace {
            need: 18,
            have: 3,
            at,
        }) => assert_eq!(
            at,
            state.path().join("new-root/root/documents/旧版 Miyu 迁移")
        ),
        other => panic!("{other:?}"),
    }
    assert_eq!(fs::read_dir(state.path()).unwrap().count(), 0, "什么都没写");
}

#[test]
fn the_state_disk_needs_room_for_the_unpacked_copy_too() {
    let old = old_root();
    let state = tempfile::tempdir().unwrap();
    let disk = tempfile::Builder::new()
        .prefix("keepdisk")
        .tempdir()
        .unwrap();
    let split = Plan {
        root: miyu_client::places::DataRoot::at(disk.path().to_path_buf()),
        space: state_tight,
        ..plan(old.path(), &state.path().join("var/lib/miyu-migrate"))
    };
    match run(&split, &mut Fake) {
        // 解出来的那一份 9 个字节，状态目录还没建，问的是它已有的上一层。
        Err(RunError::NoSpace {
            need: 9,
            have: 3,
            at,
        }) => {
            assert_eq!(at, state.path().join("var/lib/miyu-migrate"));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(fs::read_dir(state.path()).unwrap().count(), 0, "什么都没写");
    assert_eq!(
        fs::read_dir(disk.path()).unwrap().count(),
        0,
        "数据根里也没写"
    );
}

#[test]
fn exactly_enough_room_is_enough() {
    let old = old_root();
    let state = tempfile::tempdir().unwrap();
    let just = Plan {
        space: exact,
        ..plan(old.path(), state.path())
    };
    run(&just, &mut Fake).unwrap();
    assert!(kept(state.path()).join("报告.md").is_file());
}

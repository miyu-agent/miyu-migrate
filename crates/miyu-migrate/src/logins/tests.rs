//! 登录态：闲鱼的包装着的拷进它的状态目录（锁、二维码不拷），没装的提示装；知乎、小红书、B 站的只报；没有的不问核心。

use std::fs;

use serde_json::{Value, json};

use super::*;

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

/// 假核心：`package.list` 里有没有闲鱼照 `installed`。
struct Fake {
    installed: bool,
    calls: u32,
}

impl Core for Fake {
    fn call(&mut self, method: &str, _params: Value) -> Result<Value, Said> {
        assert_eq!(method, "package.list");
        self.calls += 1;
        let packages = if self.installed {
            json!([{"package": GOOFISH}])
        } else {
            json!([])
        };
        Ok(json!({"packages": packages}))
    }
}

fn old_root() -> (tempfile::TempDir, Layout) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(&root.join(".home-layout-v1"), b"owner\n");
    write(&root.join("home/owner/conversation.db"), b"");
    write(&root.join("cache/goofish-search/session.json"), b"{}");
    write(&root.join("cache/goofish-search/abcdef012345.json"), b"{}");
    write(&root.join("cache/goofish-search/session.lock"), b"");
    write(&root.join("cache/goofish-search/login-qr.png"), b"png");
    write(&root.join("cache/zhihu-search/profile/Cookies"), b"secret");
    write(&root.join("cache/bilibili/cookie.json"), b"secret");
    write(
        &root.join("cache/bilibili-live-stream/credentials.json"),
        b"secret",
    );
    write(&root.join("cache/meme-embeddings/x.db"), b"");
    let layout = crate::layout::read(root).unwrap();
    (dir, layout)
}

#[test]
fn goofish_goes_to_its_package_and_the_rest_is_reported() {
    let (_old, layout) = old_root();
    let new = tempfile::tempdir().unwrap();
    let mut core = Fake {
        installed: true,
        calls: 0,
    };
    let item = migrate(&mut core, &layout, new.path()).unwrap();
    let target = new
        .path()
        .join("root/state/scripts")
        .join(GOOFISH)
        .join("goofish/legacy");
    let mut copied: Vec<String> = fs::read_dir(&target)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    copied.sort();
    assert_eq!(
        copied,
        vec!["abcdef012345.json", "session.json"],
        "锁、二维码不拷"
    );
    assert_eq!(item.migrated, 2);
    let whys: Vec<(&str, &str)> = item
        .skipped
        .iter()
        .map(|one| (one.key.as_str(), one.why.as_str()))
        .collect();
    assert_eq!(
        whys,
        vec![
            ("cache/bilibili", "no-package"),
            (
                "cache/bilibili-live-stream",
                "install:miyu-skills-bilibili-live-stream"
            ),
            ("cache/zhihu-search", "no-package"),
        ]
    );
    let note = item.note.unwrap();
    assert!(!note.contains("secret"), "{note}");
    assert!(note.contains("B 站开播的登录态有新包"), "{note}");
}

#[test]
fn without_the_goofish_package_nothing_is_copied() {
    let (_old, layout) = old_root();
    let new = tempfile::tempdir().unwrap();
    let mut core = Fake {
        installed: false,
        calls: 0,
    };
    let item = migrate(&mut core, &layout, new.path()).unwrap();
    assert!(!new.path().join("root").exists());
    assert_eq!(item.migrated, 0);
    assert!(
        item.skipped
            .iter()
            .any(|one| one.why == format!("install:{GOOFISH}"))
    );
}

#[test]
fn no_logins_no_questions() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join(".home-layout-v1"), b"owner\n");
    write(&dir.path().join("home/owner/conversation.db"), b"");
    let layout = crate::layout::read(dir.path()).unwrap();
    let mut core = Fake {
        installed: true,
        calls: 0,
    };
    let item = migrate(&mut core, &layout, dir.path()).unwrap();
    assert_eq!(item.status, Status::Skipped);
    assert_eq!(core.calls, 0);
}

/// 凭据拷过去只有自己读得到：源文件是 0644 的也一样。
#[cfg(unix)]
#[test]
fn copied_logins_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let (old, layout) = old_root();
    let session = old.path().join("cache/goofish-search/session.json");
    fs::set_permissions(&session, fs::Permissions::from_mode(0o644)).unwrap();
    let new = tempfile::tempdir().unwrap();
    let mut core = Fake {
        installed: true,
        calls: 0,
    };
    migrate(&mut core, &layout, new.path()).unwrap();
    let target = new
        .path()
        .join("root/state/scripts")
        .join(GOOFISH)
        .join("goofish/legacy");
    let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&target.join("session.json")), 0o600);
    assert_eq!(mode(&target.join("abcdef012345.json")), 0o600);
    assert_eq!(mode(&target), 0o700);
}

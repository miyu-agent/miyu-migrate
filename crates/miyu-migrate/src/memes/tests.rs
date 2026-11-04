//! 表情包：包编号照包的写法；来源写成一句；一个库做成包（只要自己的图，缺字的跳过，覆盖出厂的不算）；miyu-memes 装着的装上、
//! 没装的留着包和那一行、核心拒了的写原因码。

use serde_json::{Value, json};

use super::*;

#[test]
fn package_ids_follow_the_package_rules() {
    assert_eq!(package_id("miyu"), "legacy-memes-miyu");
    assert_eq!(package_id("My Cats!"), "legacy-memes-my-cats");
    let chinese = package_id("猫猫");
    assert_eq!(chinese.len(), "legacy-memes-".len() + 8);
    assert_eq!(package_id(&"x".repeat(80)).len(), 64);
}

#[test]
fn reasons_say_where_a_meme_came_from() {
    assert_eq!(reason("miyu", None), "旧版表情包 miyu");
    let origin = Origin {
        platform: "onebot".to_string(),
        conversation_kind: "group".to_string(),
        conversation_id: "555".to_string(),
        sender_id: "111".to_string(),
        sender_name: "小李".to_string(),
        collected_at: "2026-09-25T07:00:00Z".to_string(),
    };
    assert_eq!(
        reason("miyu", Some(&origin)),
        "旧版表情包 miyu；收集自 QQ 群 555 的 小李（111），2026-09-25T07:00:00Z"
    );
}

/// 一个旧库：两张自己的图（一张缺用法），一条覆盖出厂的（图不在这里），一条跑出库外的。
fn library(root: &Path, name: &str) -> PathBuf {
    let dir = root.join(name);
    fs::create_dir_all(dir.join("images")).unwrap();
    fs::write(dir.join("images/a.png"), "png-a").unwrap();
    fs::write(dir.join("images/b.png"), "png-b").unwrap();
    let index = json!({
        "library": name, "version": 2, "disabled_ids": ["sha256:x"],
        "memes": [
            {"id": "sha256:a", "file": "images/a.png", "description": "猫", "usage": "开心", "name": {"zh": "猫"}, "tags": ["猫"],
             "origin": {"platform": "onebot", "conversation_kind": "private", "sender_name": "小李", "collected_at": ""}},
            {"id": "sha256:b", "file": "images/b.png", "description": "狗", "usage": " "},
            {"id": "sha256:c", "file": "images/expr_builtin.jpg", "description": "改过的出厂", "usage": "改过"},
            {"id": "sha256:d", "file": "../escape.png", "description": "x", "usage": "y"}
        ]
    });
    fs::write(dir.join("index.json"), index.to_string()).unwrap();
    dir
}

#[test]
fn a_library_becomes_a_content_package() {
    let old = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    let dir = library(old.path(), "miyu");
    let pack = pack(&dir, "miyu", out.path()).unwrap().unwrap();
    assert_eq!(pack.id, "legacy-memes-miyu");
    assert_eq!(pack.count, 1);
    assert_eq!(
        pack.left,
        vec![
            ("images/b.png".to_string(), "no-text".to_string()),
            ("../escape.png".to_string(), "no-image".to_string()),
        ],
        "覆盖出厂的不算没迁"
    );
    let manifest = fs::read_to_string(pack.dir.join("package.toml")).unwrap();
    assert!(
        manifest.contains("name = { zh = \"旧版表情包：miyu\", en = \"Old Miyu stickers: miyu\" }"),
        "名字照「语言到一句话」写：{manifest}"
    );
    assert!(
        manifest.contains("[content.miyu-memes]\ndir = \"memes\""),
        "{manifest}"
    );
    let index: Value =
        serde_json::from_str(&fs::read_to_string(pack.dir.join("memes/index.json")).unwrap())
            .unwrap();
    assert_eq!(
        index,
        json!({"memes": [{"file": "images/a.png", "description": "猫", "usage": "开心", "reason": "旧版表情包 miyu；收集自 QQ 私聊 的 小李"}]})
    );
    assert_eq!(
        fs::read(pack.dir.join("memes/images/a.png")).unwrap(),
        b"png-a"
    );
    assert!(
        !pack.dir.join("memes/images/b.png").exists(),
        "跳过的图不拷"
    );
}

#[test]
fn a_library_that_only_hides_or_overrides_shipped_memes_has_nothing_to_pack() {
    let old = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    let dir = old.path().join("miyu");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("index.json"),
        json!({"disabled_ids": ["sha256:x"], "memes": [{"file": "images/expr_1.jpg", "description": "a", "usage": "b"}]}).to_string(),
    )
    .unwrap();
    assert_eq!(pack(&dir, "miyu", out.path()).unwrap(), None);
}

/// 假核心：`package.list` 里有没有 miyu-memes 照 `installed`；`package.install` 照 `refuse` 拒或者收。
struct Fake {
    installed: bool,
    refuse: Option<&'static str>,
    installs: Vec<Value>,
}

impl Core for Fake {
    fn call(&mut self, method: &str, params: Value) -> Result<Value, Said> {
        match method {
            "package.list" => {
                let packages = if self.installed {
                    json!([{"package": MEMES}])
                } else {
                    json!([])
                };
                Ok(json!({"packages": packages}))
            }
            "package.install" => {
                self.installs.push(params);
                match self.refuse {
                    Some(code) => Err(Said {
                        code: code.to_string(),
                        message: String::new(),
                    }),
                    None => Ok(json!({})),
                }
            }
            other => panic!("没想到会调 {other}"),
        }
    }
}

/// 家目录布局的旧根，带一个库。
fn old_root() -> (tempfile::TempDir, Layout) {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".home-layout-v1"), "owner\n").unwrap();
    fs::create_dir_all(dir.path().join("home/owner")).unwrap();
    fs::write(dir.path().join("home/owner/conversation.db"), "").unwrap();
    library(&dir.path().join("data/memes"), "miyu");
    let layout = crate::layout::read(dir.path()).unwrap();
    (dir, layout)
}

#[test]
fn installed_memes_get_the_package_and_missing_ones_get_the_line() {
    let (_old, layout) = old_root();
    // 留给人的目录（「旧版 Miyu 迁移」）带空格：要照抄的那一行加引号。
    let out = tempfile::Builder::new()
        .prefix("旧版 迁移")
        .tempdir()
        .unwrap();
    let mut with = Fake {
        installed: true,
        refuse: None,
        installs: Vec::new(),
    };
    let item = migrate(&mut with, &layout, out.path()).unwrap();
    assert_eq!(item.migrated, 1);
    let path = with.installs[0]["path"].as_str().unwrap().to_string();
    assert!(path.ends_with("legacy-memes-miyu"), "{path}");
    assert_eq!(item.status, Status::Partial, "有跳过的条目");

    let mut without = Fake {
        installed: false,
        refuse: None,
        installs: Vec::new(),
    };
    let item = migrate(&mut without, &layout, out.path()).unwrap();
    assert!(without.installs.is_empty());
    assert_eq!(item.migrated, 0);
    assert!(
        item.note
            .unwrap()
            .contains("miyu-memes 没装，装上以后跑 `miyu pkg install '")
    );

    let mut refused = Fake {
        installed: true,
        refuse: Some("package_invalid"),
        installs: Vec::new(),
    };
    let item = migrate(&mut refused, &layout, out.path()).unwrap();
    assert_eq!(item.migrated, 0);
    assert!(item.note.unwrap().contains("没装上（package_invalid）"));
}

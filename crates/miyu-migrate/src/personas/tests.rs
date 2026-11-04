//! 人格：scope 照旧版规整；示范对话照旧版拆；读旧根（改过的、停用的、没写的提示词，头像，只报不迁的）；建、映射（同名的不建、
//! `dev` 映射到软件工程师、核心拒了不挡别的、头像先放 blob 再写）。

use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use super::*;
use crate::layout;

/// 照「家目录布局」拼一个旧根：`files` 是相对旧根的路径和内容。
fn old_root(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let mut all = vec![
        (".home-layout-v1", "owner\n"),
        ("home/owner/conversation.db", ""),
    ];
    all.extend_from_slice(files);
    for (path, text) in all {
        let path = dir.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    dir
}

fn found(dir: &Path) -> Found {
    found_from(dir, Path::new("/home/a/.miyu"))
}

/// 同 [`found`]，旧根原来在 `original`。
fn found_from(dir: &Path, original: &Path) -> Found {
    read(&layout::read(dir).unwrap(), original).unwrap()
}

#[test]
fn scopes_are_normalized_like_the_old_version() {
    assert_eq!(scope_name("Miyu.md"), "miyu-md");
    assert_eq!(scope_name("  "), "default");
    assert_eq!(scope_name("my_cat Bot.md"), "my_cat-bot-md");
    let chinese = scope_name("小猫.md");
    assert_eq!(chinese.len(), "persona-".len() + 12);
    assert_eq!(
        chinese,
        format!(
            "persona-{}",
            &blake3::hash("小猫.md".as_bytes()).to_hex()[..12]
        )
    );
}

#[test]
fn dialogs_are_split_like_the_old_version() {
    let raw = "User： 你好\n多一行\nASSISTANT: 嗨\n\nuser: 没回的\nassistant:\nuser: 第二\nassistant: 答\n  接着\nuser: 尾巴";
    assert_eq!(
        parse_dialogs(raw),
        vec![
            ("你好\n多一行".to_string(), "嗨".to_string()),
            ("第二".to_string(), "答\n  接着".to_string()),
        ]
    );
}

#[test]
fn the_old_miyu_takes_the_shipped_prompts_unless_the_person_changed_them() {
    let untouched = old_root(&[]);
    let olds = found(untouched.path()).olds;
    assert_eq!(olds[0].scope, "default");
    assert_eq!(olds[0].name, "Miyu");
    assert_eq!(olds[0].persona.as_deref(), Some(LEGACY_PERSONA.trim()));
    assert_eq!(olds[0].reminders.as_deref(), Some(LEGACY_HINT.trim()));
    assert_eq!(olds[0].examples, parse_dialogs(LEGACY_DIALOGS));
    assert!(!olds[0].examples.is_empty());
    assert_eq!((olds[1].scope.as_str(), olds[1].kind), ("dev", Kind::Dev));

    let changed = old_root(&[
        ("data/prompts/hints/default.md", "  记得撒娇\n"),
        ("data/prompts/dialogs/default.md", ""),
    ]);
    let olds = found(changed.path()).olds;
    assert_eq!(olds[0].reminders.as_deref(), Some("记得撒娇"));
    assert!(olds[0].examples.is_empty(), "空的是停用");
}

#[test]
fn every_prompt_file_is_a_persona_with_its_own_hints_dialogs_and_avatar() {
    let dir = old_root(&[
        ("data/prompts/小猫.md", "你是小猫\n"),
        ("data/prompts/Bot.md", "bot"),
        ("data/prompts/system-prompt.md", "旧的自定义"),
        ("data/prompts/hints/bot-md.md", ""),
        ("data/prompts/dialogs/bot-md.md", "user: a\nassistant: b"),
        (
            "data/prompts/Bot.json",
            r#"{"avatar_path":"persona-avatars/Bot/face.png"}"#,
        ),
        ("data/persona-avatars/Bot/face.png", "png"),
        (
            "data/prompts/小猫.json",
            r#"{"avatar_path":"/somewhere/else.png"}"#,
        ),
        (
            "personas/dev/persona.toml",
            "[subsystems]\nmemory = false\n",
        ),
        (
            "config/dev-prompt.md",
            "You are a helpful software engineer assistant.\n",
        ),
    ]);
    let found = found(dir.path());
    let prompts: Vec<&Old> = found
        .olds
        .iter()
        .filter(|old| old.kind == Kind::Prompt)
        .collect();
    assert_eq!(prompts.len(), 2, "system-prompt.md 不是人格");
    let bot = prompts[0];
    assert_eq!((bot.scope.as_str(), bot.name.as_str()), ("bot-md", "Bot"));
    assert_eq!(bot.persona.as_deref(), Some("bot"));
    assert_eq!(bot.reminders.as_deref(), Some(""), "空的是停用");
    assert_eq!(bot.examples, vec![("a".to_string(), "b".to_string())]);
    assert_eq!(
        bot.avatar.as_deref(),
        Some(
            dir.path()
                .join("data/persona-avatars/Bot/face.png")
                .as_path()
        )
    );
    let cat = prompts[1];
    assert_eq!(cat.name, "小猫");
    assert!(cat.scope.starts_with("persona-"));
    assert_eq!(
        (cat.reminders.as_deref(), cat.avatar.as_deref()),
        (None, None)
    );
    let extras: Vec<(&str, &str)> = found
        .extras
        .iter()
        .map(|extra| (extra.source.as_str(), extra.why))
        .collect();
    assert_eq!(
        extras,
        vec![
            ("data/prompts/system-prompt.md", "system-prompt"),
            ("data/prompts/小猫.json", "avatar-missing"),
            ("personas/dev/persona.toml", "persona-settings"),
        ],
        "旧默认的 dev-prompt 当没写"
    );
}

#[test]
fn an_avatar_with_the_old_absolute_path_is_found_in_the_copy() {
    // 旧机器上的绝对路径：哪个平台都是绝对的。
    let original = std::env::temp_dir().join("old-miyu");
    let sidecar =
        json!({"avatar_path": original.join("data").join("persona-avatars").join("b.png")});
    let sidecar = sidecar.to_string();
    let dir = old_root(&[
        ("data/prompts/Bot.md", "bot"),
        ("data/prompts/Bot.json", &sidecar),
        ("data/persona-avatars/b.png", "png"),
        ("config/dev-prompt.md", "自己写的开发提示"),
    ]);
    let found = found_from(dir.path(), &original);
    assert_eq!(
        found.olds[2].avatar.as_deref(),
        Some(dir.path().join("data/persona-avatars/b.png").as_path())
    );
    assert_eq!(found.extras[0].why, "dev-prompt");
}

/// 假核心：`persona.list` 照 `listed` 答，`persona.set` 新建的起 `persona-<n>`、名字是 `refuse` 的拒，`blob.put` 交一个哈希。
struct Fake {
    listed: Value,
    refuse: &'static str,
    calls: Vec<(String, Value)>,
}

impl Core for Fake {
    fn call(&mut self, method: &str, params: Value) -> Result<Value, Said> {
        self.calls.push((method.to_string(), params.clone()));
        match method {
            "persona.list" => Ok(self.listed.clone()),
            "persona.set" if params["changes"][0]["value"] == self.refuse => Err(Said {
                code: "persona_invalid".to_string(),
                message: "bad".to_string(),
            }),
            "persona.set" if params.get("persona").is_some() => Ok(params),
            "persona.set" => Ok(json!({"persona": format!("persona-{}", self.calls.len())})),
            "blob.put" => Ok(json!({"blob": format!("sha256:{}", "a".repeat(64))})),
            other => panic!("没想到会调 {other}"),
        }
    }
}

fn old(scope: &str, name: &str, kind: Kind) -> Old {
    Old {
        scope: scope.to_string(),
        name: name.to_string(),
        kind,
        persona: Some(format!("{name} 的人设")),
        reminders: Some(String::new()),
        examples: Vec::new(),
        avatar: None,
    }
}

#[test]
fn same_names_are_mapped_dev_goes_to_the_engineer_and_the_rest_are_created() {
    let mut core = Fake {
        listed: json!({"personas": [
            {"persona": "software-engineer", "name": "软件工程师"},
            {"persona": "persona-1", "name": "Miyu"},
        ]}),
        refuse: "坏的",
        calls: Vec::new(),
    };
    let mut cat = old("persona-x", "小猫", Kind::Prompt);
    cat.examples = vec![("a".to_string(), "b".to_string())];
    cat.avatar = Some(std::path::PathBuf::from("/tmp/face.png"));
    let olds = [
        old("default", "Miyu", Kind::Miyu),
        old("dev", "dev", Kind::Dev),
        cat,
        old("bad-md", "坏的", Kind::Prompt),
    ];
    let mapped = apply(&mut core, &olds).unwrap();
    let rows: Vec<(&str, Option<&str>, How)> = mapped
        .iter()
        .map(|row| (row.old.as_str(), row.new.as_deref(), row.how))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("default", Some("persona-1"), How::Existing),
            ("dev", Some("software-engineer"), How::Engineer),
            ("persona-x", Some("persona-2"), How::Created),
            ("bad-md", None, How::Missing),
        ]
    );
    assert!(
        mapped[3]
            .note
            .as_deref()
            .unwrap()
            .contains("persona_invalid")
    );
    let created = &core.calls[1].1;
    assert_eq!(
        created,
        &json!({
            "changes": [{"key": "persona.name", "value": "小猫"}],
            "prompts": {
                "persona": {"text": "小猫 的人设"},
                "examples": {"pairs": [{"user": "a", "assistant": "b"}]},
            },
        }),
        "空的角色扮演提示不写"
    );
    assert_eq!(core.calls[2].0, "blob.put");
    assert_eq!(
        core.calls[3].1,
        json!({"persona": "persona-2", "avatar": {"blob": format!("sha256:{}", "a".repeat(64))}})
    );
}

#[test]
fn without_the_engineer_dev_is_left_unmapped() {
    let mut core = Fake {
        listed: json!({"personas": []}),
        refuse: "",
        calls: Vec::new(),
    };
    let mapped = apply(&mut core, &[old("dev", "dev", Kind::Dev)]).unwrap();
    assert_eq!(
        (mapped[0].new.as_deref(), mapped[0].how),
        (None, How::Missing)
    );
    assert_eq!(core.calls.len(), 1, "只列了一次");
}

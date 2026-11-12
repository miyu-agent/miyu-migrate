//! 技能、脚本：新包照名字找；技能名的写法、头里的名字跟着改；拷进新数据根（顶层的、人格层的、包装的不拷、同名一样的算迁过、
//! 不一样的改名、停用的照拷）；自己的脚本只报；包管理器的包提示装新的。

use std::fs;
use std::path::Path;

use serde_json::json;

use super::*;

#[test]
fn old_packages_find_their_successors() {
    assert_eq!(successor("weather"), Some("weather"));
    assert_eq!(successor("miyu-scripts-tarot"), Some("tarot"));
    assert_eq!(successor("office"), Some("office"));
    assert_eq!(successor("memes"), Some("memes"));
    assert_eq!(successor("zhihu-search"), None);
}

#[test]
fn skill_names_and_their_header_follow_the_new_rules() {
    assert_eq!(skill_name("Take_Notes"), "take-notes");
    assert!(skill_name("笔记").starts_with("skill-"));
    let text = "---\nname: notes\ndescription: \"x\"\n---\n# Notes\nname: not the header\n";
    assert_eq!(
        rename(text, "notes-legacy"),
        "---\nname: notes-legacy\ndescription: \"x\"\n---\n# Notes\nname: not the header\n"
    );
    assert_eq!(rename("# 没有头\n", "x"), "# 没有头\n");
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn skill(name: &str) -> String {
    format!("---\nname: {name}\ndescription: \"{name} skill\"\n---\n# {name}\n")
}

/// 家目录布局的旧根：顶层技能两个（一个停用）、人格层一个、包装的一个；脚本两个（一个包装的）；锁里两个包。
fn old_root() -> (tempfile::TempDir, Layout) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(&root.join(".home-layout-v1"), "owner\n");
    write(&root.join("home/owner/conversation.db"), "");
    write(
        &root.join("extensions/skills/notes/SKILL.md"),
        &skill("notes"),
    );
    write(&root.join("extensions/skills/notes/ref/a.md"), "参考");
    write(
        &root.join("extensions/skills/Old_One/SKILL.md"),
        &skill("Old_One"),
    );
    write(&root.join("extensions/skills/Old_One/.disabled"), "");
    write(
        &root.join("extensions/skills/personas/persona-x/cat/SKILL.md"),
        &skill("cat"),
    );
    write(
        &root.join("extensions/skills/office/SKILL.md"),
        &skill("office"),
    );
    write(
        &root.join("extensions/skills/.hidden/SKILL.md"),
        &skill("hidden"),
    );
    write(&root.join("extensions/scripts/mine.py"), "# 我的\n");
    write(&root.join("extensions/scripts/weather.py"), "# 天气\n");
    write(&root.join("extensions/scripts/index.json"), "{}");
    write(&root.join("extensions/scripts/index.json.bak-1"), "{}");
    write(&root.join("extensions/scripts/__pycache__/mine.pyc"), "");
    write(
        &root.join("extensions/scripts/personas/default/juya_daily"),
        "# 日报\n",
    );
    let lock = json!({"packages": {
        "weather": {"version": "1", "files": ["extensions/scripts/weather.py"]},
        "office": {"version": "1", "files": ["extensions/skills/office/SKILL.md"]},
    }});
    write(&root.join("extensions/pm/lock.json"), &lock.to_string());
    let layout = crate::layout::read(root).unwrap();
    (dir, layout)
}

#[test]
fn own_skills_are_copied_and_the_rest_is_reported() {
    let (_old, layout) = old_root();
    let new = tempfile::tempdir().unwrap();
    // 新版已经有一个不一样的 notes：拷成 notes-legacy。
    write(
        &new.path().join("root/skills/notes/SKILL.md"),
        &skill("notes").replace("skill", "新版的"),
    );
    let moved = migrate(&layout, new.path());
    let skills = new.path().join("root/skills");
    assert_eq!(moved.skills.migrated, 3);
    assert_eq!(
        fs::read_to_string(skills.join("notes-legacy/SKILL.md")).unwrap(),
        skill("notes").replace("name: notes", "name: notes-legacy")
    );
    assert_eq!(
        fs::read_to_string(skills.join("notes-legacy/ref/a.md")).unwrap(),
        "参考"
    );
    assert!(skills.join("old-one/SKILL.md").is_file());
    assert!(!skills.join("old-one/.disabled").exists(), "停用的标记不拷");
    assert!(skills.join("cat/SKILL.md").is_file());
    assert!(!skills.join("office").exists(), "包装的不拷");
    assert!(!skills.join("hidden").exists());
    let whys: Vec<(&str, &str)> = moved
        .skills
        .skipped
        .iter()
        .map(|one| (one.key.as_str(), one.why.as_str()))
        .collect();
    assert_eq!(whys, vec![("extensions/skills/office", "package")]);
    let note = moved.skills.note.unwrap();
    assert!(
        note.contains("`extensions/skills/notes` 新版里叫 `notes-legacy`"),
        "{note}"
    );
    assert!(note.contains("`old-one` 原来停用了"), "{note}");
    assert!(note.contains("`cat` 原来是人格 `persona-x` 的"), "{note}");

    let scripts: Vec<(&str, &str)> = moved
        .scripts
        .skipped
        .iter()
        .map(|one| (one.key.as_str(), one.why.as_str()))
        .collect();
    assert_eq!(
        scripts,
        vec![
            ("extensions/scripts/mine.py", "format"),
            (
                "extensions/scripts/personas/default/juya_daily",
                "install:juya-daily"
            ),
        ],
        "包装的、覆盖层、备份、缓存不算；有新包的提示装"
    );
    assert_eq!(moved.scripts.status, Status::Archived);
    let packages: Vec<(&str, &str)> = moved
        .packages
        .skipped
        .iter()
        .map(|one| (one.key.as_str(), one.why.as_str()))
        .collect();
    assert_eq!(
        packages,
        vec![("office", "install:office"), ("weather", "install:weather")]
    );

    let again = migrate(&layout, new.path());
    assert_eq!(again.skills.migrated, 0, "以前迁过的一样的不再拷");
    assert!(!skills.join("notes-legacy-3").exists());
}

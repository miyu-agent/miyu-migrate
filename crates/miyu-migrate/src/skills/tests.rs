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
    let moved = migrate(&layout, new.path(), Language::Zh);
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

    let again = migrate(&layout, new.path(), Language::Zh);
    assert_eq!(again.skills.migrated, 0, "以前迁过的一样的不再拷");
    assert!(!skills.join("notes-legacy-3").exists());
}

/// 审计 #143：找 YAML 头结尾用的是 `rest.find("\n---")`，不要求后面跟换行或到结尾；头里有一行以 `---` 开头（比如块字符串里的
/// `---foo`）就被当成头的结尾，后面的 `name:` 不改名，技能目录名和头里的 name 对不上。
#[test]
fn audit_143_a_line_starting_with_dashes_inside_the_header_is_not_its_end() {
    let text = "---\ndescription: |\n---foo\nname: old\n---\nbody line\n";
    let renamed = rename(text, "new");
    assert!(renamed.contains("\nname: new\n"), "name 没改：{renamed:?}");
    assert!(!renamed.contains("name: old"), "{renamed:?}");
    assert!(
        renamed.ends_with("---\nbody line\n"),
        "正文原样：{renamed:?}"
    );
}

/// 审计 #143：技能里的链接（指到文件的、悬空的）拷的时候不跟、也不拷，原来报告里一个字都没有，主人以为整个技能都过去了。
/// 照旧不跟（可能指到技能目录外面），报告写哪个技能里有几个链接没拷。
#[cfg(unix)]
#[test]
fn audit_143_links_inside_a_skill_are_reported_not_dropped_silently() {
    let (old, layout) = old_root();
    let linked = old.path().join("extensions/skills/linked");
    write(&linked.join("SKILL.md"), &skill("linked"));
    write(&linked.join("a.md"), "正文");
    std::os::unix::fs::symlink(linked.join("a.md"), linked.join("alias.md")).unwrap();
    std::os::unix::fs::symlink("/nowhere/at/all", linked.join("gone.md")).unwrap();
    for (language, said) in [
        (Language::Zh, "`linked` 里有 2 个链接没拷"),
        (
            Language::En,
            "2 link(s) in the skill `linked` were not copied",
        ),
    ] {
        let new = tempfile::tempdir().unwrap();
        let moved = migrate(&layout, new.path(), language);
        let copied = new.path().join("root/skills/linked");
        assert_eq!(fs::read_to_string(copied.join("a.md")).unwrap(), "正文");
        for name in ["alias.md", "gone.md"] {
            assert!(
                fs::symlink_metadata(copied.join(name)).is_err(),
                "链接不跟、不拷：{name}"
            );
        }
        assert_eq!(moved.skills.status, Status::Partial, "{:?}", moved.skills);
        let note = moved.skills.note.unwrap_or_default();
        assert!(note.contains(said), "{language:?}：{note}");
    }
}

/// 新数据根里迁出来的文件有没有一个含 `marker`。
#[cfg(unix)]
fn any_file_contains(root: &Path, marker: &str) -> bool {
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .any(|entry| fs::read_to_string(entry.path()).is_ok_and(|text| text.contains(marker)))
}

/// `SKILL.md` 本身是链接的技能：迁一遍，交回结果和新数据根。
#[cfg(unix)]
fn migrate_with_linked_head(
    make: impl Fn(&Path, &Path),
    language: Language,
) -> (Moved, tempfile::TempDir) {
    let (old, layout) = old_root();
    let outside = tempfile::tempdir().unwrap();
    let dir = old.path().join("extensions/skills/linked");
    fs::create_dir_all(&dir).unwrap();
    make(&dir, outside.path());
    let new = tempfile::tempdir().unwrap();
    let moved = migrate(&layout, new.path(), language);
    (moved, new)
}

/// 技能这一项里 why = link 的那几条的键。
#[cfg(unix)]
fn link_skips(moved: &Moved) -> Vec<&str> {
    moved
        .skills
        .skipped
        .iter()
        .filter(|one| one.why == "link")
        .map(|one| one.key.as_str())
        .collect()
}

/// 审计收尾（同核心 AU-008）：`SKILL.md` 是指到技能目录外面一个文件的链接，原来照读，外面的内容写进了新技能。
/// 不跟、整个技能不拷，记一条 link、这一项是 Partial、报告写清楚。
#[cfg(unix)]
#[test]
fn a_skill_file_linked_outside_the_skill_is_not_followed() {
    const MARKER: &str = "OUTSIDE-MARKER-5c1e";
    let make = |dir: &Path, outside: &Path| {
        write(
            &outside.join("secret.md"),
            &format!("---\nname: linked\n---\n{MARKER}\n"),
        );
        std::os::unix::fs::symlink(outside.join("secret.md"), dir.join("SKILL.md")).unwrap();
    };
    for (language, said) in [
        (
            Language::Zh,
            "`extensions/skills/linked` 的 SKILL.md 是链接",
        ),
        (
            Language::En,
            "The SKILL.md of `extensions/skills/linked` is a link",
        ),
    ] {
        let (moved, new) = migrate_with_linked_head(make, language);
        assert!(
            !any_file_contains(new.path(), MARKER),
            "外面的内容进了新技能"
        );
        assert!(
            !new.path().join("root/skills/linked").exists(),
            "整个技能不拷"
        );
        assert_eq!(moved.skills.status, Status::Partial, "{:?}", moved.skills);
        assert_eq!(link_skips(&moved), ["extensions/skills/linked/SKILL.md"]);
        let note = moved.skills.note.unwrap_or_default();
        assert!(note.contains(said), "{language:?}：{note}");
    }
}

/// `SKILL.md` 链到技能目录里面的 `docs/real.md`、技能目录本身也是链接过去的：照常迁，内容对。
#[cfg(unix)]
#[test]
fn a_skill_file_linked_inside_the_skill_is_read_as_before() {
    let (old, layout) = old_root();
    let elsewhere = tempfile::tempdir().unwrap();
    let real_dir = elsewhere.path().join("inlink");
    write(&real_dir.join("docs/real.md"), &skill("inlink"));
    std::os::unix::fs::symlink("docs/real.md", real_dir.join("SKILL.md")).unwrap();
    std::os::unix::fs::symlink(&real_dir, old.path().join("extensions/skills/inlink")).unwrap();
    let new = tempfile::tempdir().unwrap();
    let moved = migrate(&layout, new.path(), Language::Zh);
    let copied = new.path().join("root/skills/inlink");
    assert_eq!(
        fs::read_to_string(copied.join("SKILL.md")).unwrap(),
        skill("inlink")
    );
    assert_eq!(
        fs::read_to_string(copied.join("docs/real.md")).unwrap(),
        skill("inlink")
    );
    assert_eq!(moved.skills.status, Status::Migrated, "{:?}", moved.skills);
    assert!(link_skips(&moved).is_empty(), "{:?}", moved.skills);
}

/// `SKILL.md` 是悬空链接：不 panic，不报成都迁了。
#[cfg(unix)]
#[test]
fn a_dangling_skill_file_is_reported() {
    let (moved, new) = migrate_with_linked_head(
        |dir, _| std::os::unix::fs::symlink("/nowhere/at/all", dir.join("SKILL.md")).unwrap(),
        Language::Zh,
    );
    assert!(!new.path().join("root/skills/linked").exists());
    assert_eq!(moved.skills.status, Status::Partial, "{:?}", moved.skills);
    assert_eq!(link_skips(&moved), ["extensions/skills/linked/SKILL.md"]);
}

/// 经中间链接绕出去的：`SKILL.md` → `inner/link.md`，`inner` 是指到技能外面目录的链接。不跟。
#[cfg(unix)]
#[test]
fn a_skill_file_that_leaves_through_an_inner_link_is_not_followed() {
    const MARKER: &str = "DETOUR-MARKER-91b2";
    let (moved, new) = migrate_with_linked_head(
        |dir, outside| {
            write(
                &outside.join("link.md"),
                &format!("---\nname: linked\n---\n{MARKER}\n"),
            );
            std::os::unix::fs::symlink(outside, dir.join("inner")).unwrap();
            std::os::unix::fs::symlink("inner/link.md", dir.join("SKILL.md")).unwrap();
        },
        Language::Zh,
    );
    assert!(
        !any_file_contains(new.path(), MARKER),
        "绕出去的内容进了新技能"
    );
    assert!(!new.path().join("root/skills/linked").exists());
    assert_eq!(moved.skills.status, Status::Partial, "{:?}", moved.skills);
    assert_eq!(link_skips(&moved), ["extensions/skills/linked/SKILL.md"]);
}

//! 旧版 fish 终端集成的那份照 `XDG_CONFIG_HOME` 找（同旧版 `paths::fish_conf_dir`、同终端 install）：设了绝对路径的只看
//! `$XDG_CONFIG_HOME/fish/conf.d/miyu.fish`，没设、空的、相对路径的看 `~/.config/fish/conf.d/miyu.fish`。

use std::ffi::OsStr;

use super::*;

/// 零碎里数据根外面的那几项：`home` 当家目录，`xdg` 当 `XDG_CONFIG_HOME`。
fn outside_items(home: &Path, xdg: Option<&OsStr>) -> Vec<Item> {
    let (_old, layout) = old_root();
    let _ = xdg;
    let home = Home::new(home.to_path_buf(), None);
    items(&layout, Some(&home), Language::Zh)
        .into_iter()
        .filter(|item| item.what.starts_with("outside/"))
        .collect()
}

const IN_HOME: &str = "outside/.config/fish/conf.d/miyu.fish";

#[test]
fn xdg_outside_home_is_where_the_old_fish_file_is() {
    let home = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let file = config.path().join("fish/conf.d/miyu.fish");
    write(&file, b"# miyu shell hook \xc2\xb7 v0.7.4\n");
    let listed = outside_items(home.path(), Some(config.path().as_os_str()));
    let fish = find(&listed, "outside/$XDG_CONFIG_HOME/fish/conf.d/miyu.fish");
    assert_eq!(fish.source, file.display().to_string());
    assert!(
        fish.note
            .as_deref()
            .unwrap()
            .contains("miyu shellhook install --shell fish"),
        "{fish:?}"
    );
    assert_eq!(listed.len(), 1, "{listed:?}");
}

#[test]
fn xdg_inside_home_is_written_from_home() {
    let home = tempfile::tempdir().unwrap();
    let config = home.path().join("cfg");
    write(&config.join("fish/conf.d/miyu.fish"), b"");
    let listed = outside_items(home.path(), Some(config.as_os_str()));
    let fish = find(&listed, "outside/cfg/fish/conf.d/miyu.fish");
    assert_eq!(fish.source, "~/cfg/fish/conf.d/miyu.fish");
    assert_eq!(listed.len(), 1, "{listed:?}");
}

/// 设了 XDG 的只看它指的那一处：`~/.config` 里的不报。
#[test]
fn with_xdg_set_the_home_config_copy_is_not_looked_at() {
    let home = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    write(&home.path().join(".config/fish/conf.d/miyu.fish"), b"");
    let listed = outside_items(home.path(), Some(config.path().as_os_str()));
    assert!(listed.is_empty(), "{listed:?}");
}

/// 空的、相对路径的当没设，看 `~/.config`。
#[test]
fn empty_or_relative_xdg_falls_back_to_dot_config() {
    let home = tempfile::tempdir().unwrap();
    write(&home.path().join(".config/fish/conf.d/miyu.fish"), b"");
    for xdg in ["", "relative/cfg"] {
        let listed = outside_items(home.path(), Some(OsStr::new(xdg)));
        let fish = find(&listed, IN_HOME);
        assert_eq!(fish.source, "~/.config/fish/conf.d/miyu.fish", "{xdg:?}");
        assert_eq!(listed.len(), 1, "{xdg:?}：{listed:?}");
    }
}

/// 没设的：和原来逐字一样。
#[test]
fn without_xdg_the_item_is_unchanged() {
    let home = tempfile::tempdir().unwrap();
    write(&home.path().join(".config/fish/conf.d/miyu.fish"), b"");
    let listed = outside_items(home.path(), None);
    assert_eq!(
        listed,
        vec![Item {
            what: IN_HOME.to_string(),
            status: Status::Skipped,
            source: "~/.config/fish/conf.d/miyu.fish".to_string(),
            format: "数据根外面的文件，不在压缩包里".to_string(),
            migrated: 0,
            skipped: Vec::new(),
            target: None,
            note: Some(
                "旧版的 fish 终端集成，迁移不动它；在新版里跑 `miyu shellhook install --shell fish` 时，认得出是旧版的会先备份再清掉，不用终端集成的可以自己删。"
                    .to_string()
            ),
        }]
    );
}

//! 旧版终端集成写进 bash、zsh 启动文件的那段（SH-OLD）：每个有旧记号一段的启动文件报一项，写清哪个文件、第几行到第几行；
//! 认法同新版终端集成（记号独占一行，两头空白、行尾 `\r` 不算；开头到它后面头一个结尾算一段；只有开头的不算；照字节读）。
//! 只报不动：跑完启动文件逐字节不变、修改时间不变、家目录里不多文件。

use std::collections::BTreeMap;
use std::time::SystemTime;

use super::*;

const BASH_BLOCK: &str = "# >>> miyu bash hook >>>\n[ -r '/home/u/.miyu/shell/hook.bash' ] && source '/home/u/.miyu/shell/hook.bash'\n# <<< miyu bash hook <<<\n";
const ZSH_BLOCK: &str = "# >>> miyu zsh hook >>>\n[ -r '/home/u/.miyu/shell/hook.zsh' ] && source '/home/u/.miyu/shell/hook.zsh'\n# <<< miyu zsh hook <<<\n";

/// 零碎里数据根外面的那几项：`home` 当家目录，`zdotdir` 当 `ZDOTDIR`。
fn outside_items(home: &Path, zdotdir: Option<&Path>, language: Language) -> Vec<Item> {
    let (_old, layout) = old_root();
    let home = Home::new(
        home.to_path_buf(),
        zdotdir.map(|dir| dir.as_os_str().to_owned()),
    );
    items(&layout, Some(&home), language)
        .into_iter()
        .filter(|item| item.what.starts_with("outside/"))
        .collect()
}

fn note(item: &Item) -> &str {
    item.note.as_deref().unwrap_or_default()
}

#[test]
fn a_block_in_bashrc_is_reported_with_its_lines() {
    let home = tempfile::tempdir().unwrap();
    let text = format!("# 我自己的\nexport A=1\n\nalias ll='ls -l'\n{BASH_BLOCK}export B=2\n");
    write(&home.path().join(".bashrc"), text.as_bytes());
    let listed = outside_items(home.path(), None, Language::Zh);
    let bashrc = find(&listed, "outside/.bashrc");
    assert_eq!(bashrc.status, Status::Skipped);
    assert_eq!(bashrc.source, "~/.bashrc");
    assert!(
        bashrc.format.contains("# >>> miyu bash hook >>>"),
        "{bashrc:?}"
    );
    let said = note(bashrc);
    assert!(said.contains("第 5～7 行"), "{said}");
    assert!(
        said.contains("miyu shellhook install --shell bash"),
        "{said}"
    );
    assert!(said.contains("备份"), "{said}");
    assert_eq!(listed.len(), 1, "{listed:?}");
}

#[test]
fn every_candidate_file_with_a_block_is_one_item() {
    let home = tempfile::tempdir().unwrap();
    let zdot = tempfile::tempdir().unwrap();
    write(
        &home.path().join(".bash_profile"),
        format!("export PATH=$PATH:~/bin\n{BASH_BLOCK}").as_bytes(),
    );
    write(&home.path().join(".bash_login"), b"echo hi\n");
    write(&home.path().join(".profile"), BASH_BLOCK.as_bytes());
    write(
        &home.path().join(".zshrc"),
        format!("setopt autocd\n\n{ZSH_BLOCK}").as_bytes(),
    );
    write(&zdot.path().join(".zshrc"), ZSH_BLOCK.as_bytes());
    let listed = outside_items(home.path(), Some(zdot.path()), Language::Zh);

    let profile = find(&listed, "outside/.bash_profile");
    assert!(note(profile).contains("第 2～4 行"), "{profile:?}");
    assert!(note(find(&listed, "outside/.profile")).contains("第 1～3 行"));
    let zshrc = find(&listed, "outside/.zshrc");
    assert_eq!(zshrc.source, "~/.zshrc");
    assert!(note(zshrc).contains("第 3～5 行"), "{zshrc:?}");
    assert!(note(zshrc).contains("miyu shellhook install --shell zsh"));
    assert!(
        zshrc.format.contains("# <<< miyu zsh hook <<<"),
        "{zshrc:?}"
    );
    // ZDOTDIR 指到家目录外面的，写实际路径。
    let real = zdot.path().join(".zshrc").display().to_string();
    let dotted = listed
        .iter()
        .find(|item| item.source == real)
        .unwrap_or_else(|| panic!("没报 ZDOTDIR 的：{listed:?}"));
    assert!(note(dotted).contains("第 1～3 行"), "{dotted:?}");
    assert!(!listed.iter().any(|item| item.what == "outside/.bash_login"));
    assert_eq!(listed.len(), 4, "{listed:?}");
    let whats: Vec<&str> = listed.iter().map(|item| item.what.as_str()).collect();
    let mut unique = whats.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), whats.len(), "what 不重：{whats:?}");
}

#[test]
fn zdotdir_at_home_is_reported_once() {
    let home = tempfile::tempdir().unwrap();
    write(&home.path().join(".zshrc"), ZSH_BLOCK.as_bytes());
    let listed = outside_items(home.path(), Some(home.path()), Language::Zh);
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].source, "~/.zshrc");
}

#[test]
fn two_blocks_in_one_file_give_both_line_ranges() {
    let home = tempfile::tempdir().unwrap();
    let text = format!("a=1\n{BASH_BLOCK}b=2\n\n{BASH_BLOCK}");
    write(&home.path().join(".bashrc"), text.as_bytes());
    let listed = outside_items(home.path(), None, Language::Zh);
    assert_eq!(listed.len(), 1, "一个文件一项：{listed:?}");
    let said = note(&listed[0]);
    assert!(
        said.contains("第 2～4 行") && said.contains("第 7～9 行"),
        "{said}"
    );
}

/// CRLF、记号两头有空白、末尾没换行、不是 UTF-8：照样认，行号对，不 panic。
#[test]
fn crlf_spaces_no_final_newline_and_bad_bytes_are_fine() {
    let home = tempfile::tempdir().unwrap();
    write(
        &home.path().join(".bashrc"),
        b"\xff\xfe latin \xe9\r\n  # >>> miyu bash hook >>>\t \r\nsource '/x/\xff'\r\n\t# <<< miyu bash hook <<<  ",
    );
    let listed = outside_items(home.path(), None, Language::Zh);
    let said = note(find(&listed, "outside/.bashrc"));
    assert!(said.contains("第 2～4 行"), "{said}");
}

/// 只有开头记号的、记号不独占一行的、bash 的记号写在 zsh 文件里的：不报。
#[test]
fn half_blocks_and_markers_inside_a_line_are_not_blocks() {
    let home = tempfile::tempdir().unwrap();
    write(
        &home.path().join(".bashrc"),
        b"# >>> miyu bash hook >>>\nsource /x\n",
    );
    write(
        &home.path().join(".profile"),
        b"echo \"# >>> miyu bash hook >>>\"\nsource /x\necho \"# <<< miyu bash hook <<<\"\n",
    );
    write(&home.path().join(".zshrc"), BASH_BLOCK.as_bytes());
    let listed = outside_items(home.path(), None, Language::Zh);
    assert!(listed.is_empty(), "{listed:?}");
}

#[test]
fn startup_files_without_blocks_add_nothing() {
    let home = tempfile::tempdir().unwrap();
    write(&home.path().join(".bashrc"), b"export A=1\n");
    write(&home.path().join(".zshrc"), b"setopt autocd\n");
    write(&home.path().join(".profile"), b"");
    assert!(outside_items(home.path(), None, Language::Zh).is_empty());
}

/// 家目录里每个文件的内容和修改时间、权限。
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime, fs::Permissions)> {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        .map(|entry| {
            let meta = fs::symlink_metadata(entry.path()).unwrap();
            let bytes = if meta.is_file() {
                fs::read(entry.path()).unwrap()
            } else {
                Vec::new()
            };
            (
                entry.path().to_path_buf(),
                (bytes, meta.modified().unwrap(), meta.permissions()),
            )
        })
        .collect()
}

#[test]
fn startup_files_are_left_byte_for_byte() {
    let home = tempfile::tempdir().unwrap();
    let zdot = tempfile::tempdir().unwrap();
    let old = filetime_past();
    for (dir, name, block) in [
        (home.path(), ".bashrc", BASH_BLOCK),
        (home.path(), ".bash_profile", BASH_BLOCK),
        (home.path(), ".profile", BASH_BLOCK),
        (home.path(), ".zshrc", ZSH_BLOCK),
        (zdot.path(), ".zshrc", ZSH_BLOCK),
    ] {
        let path = dir.join(name);
        write(&path, format!("x=1\r\n{block}y=2").as_bytes());
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(old)
            .unwrap();
    }
    let (home_before, zdot_before) = (snapshot(home.path()), snapshot(zdot.path()));
    let listed = outside_items(home.path(), Some(zdot.path()), Language::Zh);
    assert_eq!(listed.len(), 5, "{listed:?}");
    assert_eq!(snapshot(home.path()), home_before);
    assert_eq!(snapshot(zdot.path()), zdot_before);
}

/// 一个过去的时刻：改过的文件修改时间会变成现在，比得出来。
fn filetime_past() -> SystemTime {
    SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000)
}

/// 读不了的启动文件：不让整个零碎失败，报一项说读不了（这个工具不悄悄吞）。root 跑的读得了，不看。
#[cfg(unix)]
#[test]
fn an_unreadable_startup_file_is_reported_not_fatal() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join(".bashrc");
    write(&path, BASH_BLOCK.as_bytes());
    write(&home.path().join(".profile"), BASH_BLOCK.as_bytes());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&path).is_ok() {
        return;
    }
    let listed = outside_items(home.path(), None, Language::Zh);
    let bashrc = find(&listed, "outside/.bashrc");
    assert!(note(bashrc).contains("读不了"), "{bashrc:?}");
    assert!(note(find(&listed, "outside/.profile")).contains("第 1～3 行"));
    let english = outside_items(home.path(), None, Language::En);
    let said = note(find(&english, "outside/.bashrc"));
    assert!(said.contains("Could not read"), "{said}");
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0,
        "权限不动"
    );
}

/// 英文的：没有汉字，行号、命令照样在；fish 那一项也换成新说法。
#[test]
fn startup_blocks_read_in_english() {
    let home = tempfile::tempdir().unwrap();
    let text = format!("a=1\n{BASH_BLOCK}b=2\n\n{BASH_BLOCK}");
    write(&home.path().join(".bashrc"), text.as_bytes());
    write(&home.path().join(".zshrc"), ZSH_BLOCK.as_bytes());
    write(&home.path().join(".config/fish/conf.d/miyu.fish"), b"");
    let listed = outside_items(home.path(), None, Language::En);
    assert_eq!(listed.len(), 3, "{listed:?}");
    for item in &listed {
        for text in [&item.source, &item.format, item.note.as_ref().unwrap()] {
            assert!(
                !text
                    .chars()
                    .any(|one| ('\u{4e00}'..='\u{9fff}').contains(&one)
                        || "，。；：（）、～".contains(one)),
                "{}：{text}",
                item.what
            );
        }
    }
    let bashrc = note(find(&listed, "outside/.bashrc"));
    assert!(bashrc.contains("2-4") && bashrc.contains("7-9"), "{bashrc}");
    assert!(
        bashrc.contains("miyu shellhook install --shell bash"),
        "{bashrc}"
    );
    let fish = note(find(&listed, "outside/.config/fish/conf.d/miyu.fish"));
    assert!(
        fish.contains("miyu shellhook install --shell fish"),
        "{fish}"
    );
}

/// fish 那一项对上新说法：新版 install 认得出旧版的会备份后清掉；别的数据根外面的照旧那一句。
#[test]
fn the_fish_file_says_install_cleans_it() {
    let home = tempfile::tempdir().unwrap();
    write(&home.path().join(".config/fish/conf.d/miyu.fish"), b"");
    write(&home.path().join(".local/state/miyu/x"), b"");
    let listed = outside_items(home.path(), None, Language::Zh);
    let fish = note(find(&listed, "outside/.config/fish/conf.d/miyu.fish"));
    assert!(
        fish.contains("miyu shellhook install --shell fish") && fish.contains("备份"),
        "{fish}"
    );
    let state = note(find(&listed, "outside/.local/state/miyu"));
    assert!(state.contains("确认是旧版的"), "{state}");
}

/// 两份报告里都看得到行号：给人的那一节、给 agent 的 JSON。
#[test]
fn both_reports_show_the_lines() {
    let home = tempfile::tempdir().unwrap();
    let text = format!("export A=1\n{BASH_BLOCK}");
    write(&home.path().join(".bashrc"), text.as_bytes());
    let report = crate::report::Report {
        format: crate::report::FORMAT,
        migrate: "0.1.0".to_string(),
        at: "2026-11-18T08:00:00Z".to_string(),
        archive: None,
        old_root: crate::report::OldRoot {
            path: "/home/u/.miyu".to_string(),
            layout: "home-layout-v1".to_string(),
            admin: None,
            rolled_back: false,
        },
        personas: Vec::new(),
        items: outside_items(home.path(), None, Language::Zh),
        pending: Vec::new(),
    };
    let human = report.human(Language::Zh);
    assert!(human.contains("- `~/.bashrc`：第 2～4 行"), "{human}");
    let agent = report.agent().unwrap();
    assert!(
        agent.contains("\"what\": \"outside/.bashrc\"") && agent.contains("第 2～4 行"),
        "{agent}"
    );
    println!("{human}\n{agent}");
}

/// 一段是一个结尾记号和它前面最近的那个开头记号之间（同终端 install 实际删的）：开着又遇到开头的，前一个开头算只有开头、
/// 不算一段，从新的开头重新算；孤立的结尾不算。
#[test]
fn a_block_starts_at_the_nearest_begin_before_its_end() {
    let home = tempfile::tempdir().unwrap();
    let begin = "# >>> miyu bash hook >>>\n";
    let end = "# <<< miyu bash hook <<<\n";
    write(
        &home.path().join(".bashrc"),
        format!("{begin}export PATH=$PATH:~/bin\n{begin}source /x\n{end}").as_bytes(),
    );
    write(
        &home.path().join(".bash_profile"),
        format!("{begin}{begin}source /x\n{end}{begin}a=1\n").as_bytes(),
    );
    write(
        &home.path().join(".profile"),
        format!("{end}a=1\n{begin}source /x\n{end}{end}").as_bytes(),
    );
    write(
        &home.path().join(".bash_login"),
        format!("a=1\n{end}").as_bytes(),
    );
    let listed = outside_items(home.path(), None, Language::Zh);
    let bashrc = note(find(&listed, "outside/.bashrc"));
    assert!(
        bashrc.contains("第 3～5 行") && !bashrc.contains("第 1～"),
        "{bashrc}"
    );
    let profile = note(find(&listed, "outside/.bash_profile"));
    assert!(
        profile.contains("第 2～4 行") && !profile.contains("第 1～") && !profile.contains("、"),
        "{profile}"
    );
    let plain = note(find(&listed, "outside/.profile"));
    assert!(
        plain.contains("第 3～5 行") && !plain.contains("、"),
        "{plain}"
    );
    assert!(
        !listed.iter().any(|item| item.what == "outside/.bash_login"),
        "{listed:?}"
    );
}

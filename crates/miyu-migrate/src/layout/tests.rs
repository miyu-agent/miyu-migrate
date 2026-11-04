//! 认布局：四种标记各认成哪一代、东西在哪；回滚过的照上一代；搬家没收尾的、不是旧版的、管理员名不对的拒。

use std::fs;

use super::*;

fn root_with(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, text) in files {
        let path = dir.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    dir
}

#[test]
fn the_home_layout_puts_the_session_database_in_the_admins_home() {
    let dir = root_with(&[
        (".home-layout-v1", "shorin\n"),
        (".resource-layout-v1", "1\n"),
        (".layout-v1", "1\n"),
    ]);
    let layout = read(dir.path()).unwrap();
    assert_eq!(layout.generation, Generation::Home);
    assert_eq!(layout.admin.as_deref(), Some("shorin"));
    assert_eq!(
        layout.conversation_db(),
        dir.path().join("home/shorin/conversation.db")
    );
    assert_eq!(layout.personas(), dir.path().join("personas"));
    assert_eq!(layout.skills(), dir.path().join("extensions/skills"));
    assert_eq!(layout.prompts(), dir.path().join("data/prompts"));
    assert_eq!(layout.profile(), dir.path().join("home/shorin/profile.md"));
    assert_eq!(
        layout.user_dir("pictures"),
        dir.path().join("home/shorin/pictures")
    );
    assert_eq!(layout.workspace(), dir.path().join("home/shorin/workspace"));
}

#[test]
fn older_generations_keep_things_where_they_were() {
    let resource = root_with(&[(".resource-layout-v1", "1\n"), (".layout-v1", "1\n")]);
    let layout = read(resource.path()).unwrap();
    assert_eq!(layout.generation, Generation::Resource);
    assert_eq!(
        layout.conversation_db(),
        resource.path().join("state/conversation.db")
    );
    assert_eq!(layout.skills(), resource.path().join("data/skills"));
    assert_eq!(
        layout.profile(),
        resource.path().join("data/identities/user-identity.md")
    );
    assert_eq!(layout.workspace(), resource.path().join("workspace"));
    let first = root_with(&[(".layout-v1", "1\n")]);
    let layout = read(first.path()).unwrap();
    assert_eq!(layout.generation, Generation::Layout);
    assert_eq!(layout.prompts(), first.path().join("config/prompts"));
    assert_eq!(
        layout.profile(),
        first.path().join("config/user-identity.md")
    );
    let unmarked = root_with(&[("config/config.jsonc", "{}")]);
    assert_eq!(
        read(unmarked.path()).unwrap().generation,
        Generation::Unmarked
    );
}

#[test]
fn a_rolled_back_home_layout_is_read_as_the_one_before() {
    let dir = root_with(&[
        (".home-layout-v1", "shorin\n"),
        (".home-layout-off", ""),
        (".resource-layout-v1", "1\n"),
    ]);
    let layout = read(dir.path()).unwrap();
    assert_eq!(
        (layout.generation, layout.rolled_back),
        (Generation::Resource, true)
    );
    assert_eq!(
        layout.conversation_db(),
        dir.path().join("state/conversation.db")
    );
}

#[test]
fn unfinished_moves_strangers_and_bad_names_are_refused() {
    let unfinished = root_with(&[
        (".home-layout-v1", "shorin\n"),
        (".home-layout-v1.journal.json", "{}"),
    ]);
    assert!(matches!(
        read(unfinished.path()),
        Err(LayoutError::Unfinished(_))
    ));
    let stranger = root_with(&[("notes.txt", "hi")]);
    assert!(matches!(read(stranger.path()), Err(LayoutError::NotOld(_))));
    let bad = root_with(&[(".home-layout-v1", "../etc\n")]);
    assert!(matches!(read(bad.path()), Err(LayoutError::BadAdmin(_))));
    let file = root_with(&[("x", "")]);
    assert!(matches!(
        read(&file.path().join("x")),
        Err(LayoutError::NotADirectory(_))
    ));
}

//! 打包、核对：打进去的照路径排、文件带大小和哈希；解出来逐个对得上；改了一个字节的、丢了的认得出；链接照原样存（Unix）。

use super::*;

fn old_root() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("home/shorin")).unwrap();
    fs::create_dir_all(root.join("state/empty")).unwrap();
    fs::write(root.join(".home-layout-v1"), "shorin\n").unwrap();
    fs::write(root.join("home/shorin/conversation.db"), vec![7u8; 100_000]).unwrap();
    fs::write(root.join("state/usage.db"), b"usage").unwrap();
    dir
}

#[test]
fn packing_lists_every_file_with_its_hash_and_unpacks_the_same() {
    let old = old_root();
    let out = tempfile::tempdir().unwrap();
    let archive = out.path().join("miyu-0.7.tar.zst");
    let manifest = pack(old.path(), &archive).unwrap();
    let paths: Vec<&str> = manifest
        .entries
        .iter()
        .map(|entry| entry.path.as_str())
        .collect();
    assert_eq!(
        paths,
        [
            ".home-layout-v1",
            "home",
            "home/shorin",
            "home/shorin/conversation.db",
            "state",
            "state/empty",
            "state/usage.db"
        ]
    );
    assert_eq!(manifest.totals(), (3, 7 + 100_000 + 5));
    let usage = manifest
        .entries
        .iter()
        .find(|entry| entry.path == "state/usage.db")
        .unwrap();
    assert_eq!(
        usage.sha256.as_deref(),
        Some(hex(&Sha256::digest(b"usage")).as_str())
    );
    assert!(
        !out.path().join("miyu-0.7.tar.partial").exists(),
        "临时文件改了名"
    );
    let copy = out.path().join("copy");
    unpack_and_check(&archive, &manifest, &copy).unwrap();
    assert_eq!(fs::read(copy.join("state/usage.db")).unwrap(), b"usage");
    assert!(copy.join("state/empty").is_dir());
}

#[test]
fn a_changed_or_missing_file_is_named() {
    let old = old_root();
    let out = tempfile::tempdir().unwrap();
    let archive = out.path().join("a.tar.zst");
    let mut manifest = pack(old.path(), &archive).unwrap();
    for entry in &mut manifest.entries {
        if entry.path == "state/usage.db" {
            entry.sha256 = Some("0".repeat(64));
        }
    }
    manifest.entries.push(Entry {
        path: "state/gone.db".to_string(),
        kind: "file".to_string(),
        bytes: Some(1),
        sha256: Some("0".repeat(64)),
        target: None,
        lossy: false,
    });
    match unpack_and_check(&archive, &manifest, &out.path().join("copy")) {
        Err(ArchiveError::Mismatch(bad)) => {
            let names: Vec<&str> = bad.iter().map(|(path, _)| path.as_str()).collect();
            assert_eq!(names, ["state/usage.db", "state/gone.db"]);
        }
        other => panic!("{other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn a_link_is_kept_as_a_link() {
    let old = old_root();
    std::os::unix::fs::symlink("/nowhere/at/all", old.path().join("state/link")).unwrap();
    let out = tempfile::tempdir().unwrap();
    let archive = out.path().join("a.tar.zst");
    let manifest = pack(old.path(), &archive).unwrap();
    let link = manifest
        .entries
        .iter()
        .find(|entry| entry.path == "state/link")
        .unwrap();
    assert_eq!(
        (link.kind.as_str(), link.target.as_deref()),
        ("symlink", Some("/nowhere/at/all"))
    );
    let copy = out.path().join("copy");
    unpack_and_check(&archive, &manifest, &copy).unwrap();
    assert_eq!(
        fs::read_link(copy.join("state/link")).unwrap(),
        Path::new("/nowhere/at/all")
    );
}

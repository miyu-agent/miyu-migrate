//! 测试共用：新版的数据根，核心在进程里、一条自己的线程里跑着（照 miyu-im 测试的起法；资源目录用钉住那份核心检出的
//! `resources/`，出厂的软件工程师照样本装进数据根）；照终端的样子连上它。

#![allow(dead_code, reason = "几个测试程序各用其中一部分")]
#![allow(clippy::unwrap_used, reason = "测试：起不来就该停")]

use std::path::Path;
use std::sync::Arc;

use miyu_migrate::core::Live;
use miyu_session::testkit::Script;
use miyu_store::env::{Env, Platform};
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;
use serde_json::Value;

/// 新版的数据根，核心在一条自己的线程里跑着。
pub struct Fresh {
    _dir: tempfile::TempDir,
    /// 数据根。
    pub root: DataRoot,
}

pub fn fresh() -> Fresh {
    fresh_in(&std::env::temp_dir())
}

/// 同 [`fresh`]，数据根放在 `parent` 下（演练的东西大，不放 `/tmp`）。
pub fn fresh_in(parent: &Path) -> Fresh {
    let dir = tempfile::tempdir_in(parent).unwrap();
    let env = Env {
        platform: Platform::current(),
        miyu_home: Some(dir.path().join("miyu").into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    };
    let root = DataRoot::locate(&env).unwrap();
    root.prepare().unwrap();
    miyu_session::testkit::install_sample_persona(&root);
    // 监听要在运行时里绑：核心整个在自己的线程、自己的运行时里起，绑好了才交回。
    let (ready, bound) = std::sync::mpsc::channel();
    let serving = root.clone();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let dirs = miyu_ipc::Dirs {
                runtime_dir: None,
                ..miyu_ipc::Dirs::current()
            };
            let opened = miyu_ipc::open(&serving, &dirs).unwrap();
            let resources =
                ResourceRoot::at(Path::new(miyu_client::WORKSPACE_ROOT).join("resources"));
            let core = Arc::new(miyu_endpoint::Core::new(
                serving.clone(),
                resources,
                Arc::new(Script::new(Vec::new())),
                miyu_tool::Catalog::default(),
                None,
                opened.token.clone(),
            ));
            ready.send(()).unwrap();
            miyu_endpoint::run(opened.listener, core).await;
        });
    });
    bound.recv().unwrap();
    Fresh { _dir: dir, root }
}

pub fn connect(root: &DataRoot) -> Live {
    Live::connect(root, || {
        std::process::Command::new("miyu-core-is-already-running")
    })
    .unwrap()
}

/// 一张 2×3 的 PNG 的头（认大小只读到 IHDR）。
pub fn png() -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    bytes.extend_from_slice(&2u32.to_be_bytes());
    bytes.extend_from_slice(&3u32.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

/// 数据根里会话 `id` 的日志，写成 JSON。
pub fn logged(root: &DataRoot, id: &str) -> Vec<Value> {
    let session = miyu_kernel::id::SessionId::parse(id).unwrap();
    miyu_store::log::read_events(&root.session_dir(&session))
        .unwrap()
        .iter()
        .map(|event| serde_json::to_value(event).unwrap())
        .collect()
}

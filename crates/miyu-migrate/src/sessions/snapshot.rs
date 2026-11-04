//! 策略快照（细则第一节第 8 条）：快照缺了核心的必要格就读不回来，不手写。每个新人格经协议建一个临时会话，读它第一条
//! `session.created` 的 `policy`，从数据根的 blob 库取出快照的字节，再删掉临时会话。同一个人格的存档共用这一份；快照里的
//! `persona` 决定导进来以后进哪个人格的回合库。

use serde_json::{Value, json};

use miyu_client::places::DataRoot;
use miyu_kernel::id::{ContentHash, SessionId};
use miyu_store::blob::Blobs;
use miyu_store::log::read_events;

use crate::core::{Core, Said};

/// 借来的一份快照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// `sha256:…`。
    pub hash: String,
    /// 字节。
    pub bytes: Vec<u8>,
}

/// 照 `persona`（`None` 是明着无人格）借一份快照。
///
/// # Errors
///
/// 核心拒了；临时会话的日志、快照读不到（写成 [`Said`]，代码是 `snapshot`）。
pub fn borrow(
    core: &mut dyn Core,
    root: &DataRoot,
    persona: Option<&str>,
) -> Result<Snapshot, Said> {
    let created = core.call(
        "session.create",
        json!({"persona": persona, "titles": false, "memory": "persona"}),
    )?;
    let id = created["session"].as_str().unwrap_or_default().to_string();
    let read = read_policy(root, &id);
    // 删掉临时会话：删不掉的只是回收处多一个空会话，不挡迁移。
    let deleted = core.call("session.delete", json!({"session": id}));
    let snapshot = read?;
    deleted?;
    Ok(snapshot)
}

/// 会话 `id` 第一条的 `policy`，和 blob 库里它的字节。
fn read_policy(root: &DataRoot, id: &str) -> Result<Snapshot, Said> {
    let failed = |why: String| Said {
        code: "snapshot".to_string(),
        message: why,
    };
    let session = SessionId::parse(id).map_err(|error| failed(error.to_string()))?;
    let events =
        read_events(&root.session_dir(&session)).map_err(|error| failed(format!("{error:?}")))?;
    let first = events
        .first()
        .map(|event| serde_json::to_value(event).unwrap_or(Value::Null))
        .unwrap_or(Value::Null);
    let hash = first["body"]["policy"]
        .as_str()
        .ok_or_else(|| failed("session.created has no policy".to_string()))?
        .to_string();
    let parsed = ContentHash::parse(&hash).map_err(|error| failed(error.to_string()))?;
    let bytes = Blobs::new(root.blobs())
        .get(&parsed)
        .map_err(|error| failed(format!("{error:?}")))?;
    Ok(Snapshot { hash, bytes })
}

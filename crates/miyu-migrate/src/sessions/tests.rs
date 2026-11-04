//! 会话：编号照旧编号算、同输入同编号、时间段是创建时刻；时刻写成 UTC 毫秒。一轮拼成的事件在 `tests/turns.rs`，场所会话（群、
//! 私聊、被挤出的轮）在 `tests/venue.rs`；这里是它们共用的。

mod turns;
mod venue;

use serde_json::Value;

use super::*;

#[test]
fn ids_are_uuid_v7_from_the_old_time_and_stable() {
    let id = ids::session_id(1_700_000_000_123, "home/a/conversation.db", "sess_1");
    assert_eq!(
        id,
        ids::session_id(1_700_000_000_123, "home/a/conversation.db", "sess_1")
    );
    assert_ne!(
        id,
        ids::session_id(1_700_000_000_123, "home/a/conversation.db", "sess_2")
    );
    assert_ne!(
        id,
        ids::session_id(1_700_000_000_123, "state/conversation.db", "sess_1")
    );
    assert_eq!(id.len(), 36);
    let hex: String = id.chars().filter(|ch| *ch != '-').collect();
    assert_eq!(
        &hex[..12],
        &format!("{:012x}", 1_700_000_000_123u64),
        "时间段"
    );
    assert_eq!(&hex[12..13], "7", "版本");
    assert!(matches!(&hex[16..17], "8" | "9" | "a" | "b"), "变体");
    assert!(miyu_kernel::id::SessionId::parse(&id).is_ok());
}

#[test]
fn times_are_utc_millis() {
    let time = ids::parse("2026-09-25T15:04:05.123456789+08:00").unwrap();
    assert_eq!(ids::at(time), "2026-09-25T07:04:05.123Z");
    assert_eq!(
        ids::at(ids::parse("2026-09-25T07:04:05+00:00").unwrap()),
        "2026-09-25T07:04:05.000Z"
    );
    assert_eq!(
        ids::at_millis(1_700_000_000_123).as_deref(),
        Some("2023-11-14T22:13:20.123Z")
    );
    assert!(ids::parse("昨天").is_none());
}

pub(super) fn session(name: &str) -> Session {
    Session {
        id: "sess_1".to_string(),
        persona: String::new(),
        name: name.to_string(),
        kind: "user".to_string(),
        owner: String::new(),
        parent: None,
        created_at: "2026-09-25T07:00:00+00:00".to_string(),
        updated_at: "2026-09-25T08:00:00+00:00".to_string(),
        depth: 0,
    }
}

pub(super) fn turn(id: &str, seq: i64) -> Turn {
    Turn {
        id: id.to_string(),
        seq,
        user_content: format!("{id} 原文"),
        user_timestamp: "2026-09-25T07:01:00+00:00".to_string(),
        assistant_content: format!("{id} 回复"),
        assistant_timestamp: Some("2026-09-25T07:02:00+00:00".to_string()),
        status: "completed".to_string(),
        provider: Some("deepseek".to_string()),
        model: Some("chat".to_string()),
        ..Turn::default()
    }
}

pub(super) const HEAD: Head<'static> = Head {
    venue: "local",
    policy: "sha256:0000000000000000000000000000000000000000000000000000000000000000",
    persona: Some("persona-1"),
    who: Who::Person,
    parent: None,
};

/// 每一条核心都读得懂，整份过得了核心的账本（载入时查的那一套）。
pub(super) fn ledger_ok(events: &[Value]) {
    let mut ledger = miyu_kernel::ledger::Ledger::default();
    for event in events {
        let parsed = miyu_kernel::event::Event::from_line(&event.to_string())
            .unwrap_or_else(|error| panic!("{event}: {error}"));
        ledger
            .append(&parsed)
            .unwrap_or_else(|error| panic!("{event}: {error}"));
    }
}

pub(super) fn kinds(events: &[Value]) -> Vec<&str> {
    ledger_ok(events);
    events
        .iter()
        .map(|event| event["kind"].as_str().unwrap())
        .collect()
}

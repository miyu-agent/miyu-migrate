//! 一轮拼成的事件：字、附件、工具流、失败的工具、中断、占位、摘要行、标题；暂存打包、账本不收的不打包。

use std::collections::HashMap;

use serde_json::{Value, json};

use super::super::*;
use super::{HEAD, kinds, ledger_ok, session, turn};

#[test]
fn a_plain_turn_is_said_started_replied_and_ended() {
    let mut first = turn("turn_a", 1);
    first.display_content = "给人看的".to_string();
    let built = build(&HEAD, &session("  旧名字 "), &[first], &HashMap::new());
    let events = &built.events;
    assert_eq!(
        kinds(events),
        vec![
            "session.created",
            "session.meta_changed",
            "message.user",
            "turn.started",
            "message.assistant",
            "turn.ended"
        ]
    );
    assert_eq!(events[0]["body"]["persona"], "persona-1");
    assert_eq!(events[0]["body"]["titles"], false);
    assert_eq!(events[0]["at"], "2026-09-25T07:00:00.000Z");
    assert_eq!(events[1]["body"]["title"], "旧名字");
    assert_eq!(events[2]["body"]["blocks"][0]["text"], "给人看的");
    assert_eq!(events[2]["cause"], "legacy-turn_a");
    assert_eq!(events[3]["turn"], 4);
    assert_eq!(events[3]["body"]["trigger"], 3);
    assert_eq!(
        events[4]["by"],
        json!({"kind": "model", "endpoint": "deepseek", "model": "chat"})
    );
    assert_eq!(events[4]["body"]["seen"], 4);
    assert_eq!(events[4]["at"], "2026-09-25T07:02:00.000Z");
    assert_eq!(events[5]["body"]["reason"], "completed");
    for (n, event) in events.iter().enumerate() {
        assert_eq!(event["seq"], n as u64 + 1);
    }
    assert_eq!(built.turns, 1);
}

#[test]
fn tool_rounds_become_calls_and_results_and_failures_are_errors() {
    let mut one = turn("turn_a", 1);
    one.tool_flow = Some(
        json!([
            {"assistant_content": "我看看", "assistant_reasoning": "想", "calls": [
                {"id": "x", "name": "read", "arguments": "{\"path\":\"a\"}", "output": "内容", "started_ms": 1_700_000_000_000u64, "finished_ms": 1_700_000_000_250u64},
                {"id": "y", "name": "read", "arguments": "{}", "output": "没有"}
            ]}
        ])
        .to_string(),
    );
    one.replay_journal = Some(
        json!([
            {"kind": "tool_result", "name": "read", "ok": true, "output": ""},
            {"kind": "tool_result", "name": "read", "ok": false, "output": ""}
        ])
        .to_string(),
    );
    let built = build(&HEAD, &session(""), &[one], &HashMap::new());
    let events = &built.events;
    assert_eq!(
        kinds(events),
        vec![
            "session.created",
            "message.user",
            "turn.started",
            "message.assistant",
            "tool.result",
            "tool.result",
            "message.assistant",
            "turn.ended"
        ]
    );
    let asked = &events[3]["body"];
    assert_eq!(
        asked["blocks"][0],
        json!({"type": "reasoning", "text": "想"})
    );
    assert_eq!(
        asked["blocks"][1],
        json!({"type": "text", "text": "我看看"})
    );
    assert_eq!(
        asked["blocks"][2],
        json!({"type": "tool_call", "call_id": "call_4_1", "name": "read", "args": "{\"path\":\"a\"}"})
    );
    assert_eq!(asked["blocks"][3]["call_id"], "call_4_2");
    assert_eq!(
        events[4]["by"],
        json!({"kind": "tool", "call_id": "call_4_1"})
    );
    assert_eq!(events[4]["body"]["status"], "ok");
    assert_eq!(events[4]["body"]["duration_ms"], 250);
    assert_eq!(events[4]["at"], "2023-11-14T22:13:20.250Z");
    assert_eq!(events[5]["body"]["status"], "error", "第二个 read 失败了");
    assert!(events[5]["body"].get("duration_ms").is_none());
    assert_eq!(events[6]["body"]["seen"], 6);
}

#[test]
fn interrupted_running_and_summary_turns() {
    let mut cut = turn("turn_a", 1);
    cut.status = "interrupted".to_string();
    cut.assistant_content = "说到一半\n<system-reminder>上一轮prompt已被中断，除非用户重新要求否则不要处理上一轮的prompt</system-reminder>".to_string();
    let mut summary = turn("summary_1", 2);
    summary.summary = true;
    let mut running = turn("turn_b", 3);
    running.status = "running".to_string();
    running.assistant_content = "<system-reminder>上一轮prompt正在由另一轮回复处理中，你只需要回应用户当前的prompt，不要处理上一轮的prompt</system-reminder>".to_string();
    running.provider = None;
    let built = build(
        &HEAD,
        &session(""),
        &[cut, summary, running],
        &HashMap::new(),
    );
    let events = &built.events;
    assert_eq!(
        kinds(events),
        vec![
            "session.created",
            "message.user",
            "turn.started",
            "message.assistant",
            "turn.ended",
            "message.user",
            "turn.started",
            "turn.ended"
        ]
    );
    assert_eq!(
        events[3]["body"]["blocks"][0]["text"], "说到一半",
        "中断的提示去掉"
    );
    assert_eq!(events[4]["body"]["reason"], "interrupted");
    assert_eq!(events[7]["body"]["reason"], "interrupted", "运行中的当中断");
    assert_eq!((built.turns, built.summaries), (2, 1));
}

#[test]
fn attachments_follow_the_words() {
    let media = HashMap::from([(
        "turn_a".to_string(),
        vec![
            Media {
                blob: format!("sha256:{}", "a".repeat(64)),
                name: "猫.png".to_string(),
                media_type: "image/png".to_string(),
                size: Some((2, 3)),
            },
            Media {
                blob: format!("sha256:{}", "b".repeat(64)),
                name: "a.pdf".to_string(),
                media_type: "application/pdf".to_string(),
                size: None,
            },
        ],
    )]);
    let built = build(&HEAD, &session(""), &[turn("turn_a", 1)], &media);
    ledger_ok(&built.events);
    let blocks = &built.events[1]["body"]["blocks"];
    assert_eq!(blocks[1]["type"], "image");
    assert_eq!(
        (blocks[1]["width"].clone(), blocks[1]["height"].clone()),
        (json!(2), json!(3))
    );
    assert_eq!(blocks[2]["type"], "file");
    assert_eq!(blocks[2]["name"], "a.pdf");
    assert_eq!(built.attachments, 2);
}

#[test]
fn the_stage_bundles_good_logs_and_refuses_what_the_ledger_refuses() {
    let dir = tempfile::tempdir().unwrap();
    let stage = Stage::new(dir.path()).unwrap();
    let id = ids::session_id(1_700_000_000_000, "home/a/conversation.db", "sess_1");
    let built = build(&HEAD, &session(""), &[turn("turn_a", 1)], &HashMap::new());
    let bundle = stage
        .bundle(&id, &built.events, "2026-09-25T08:00:00.000Z")
        .unwrap();
    assert!(bundle.is_file());
    // 回复不在回合里：核心读得懂这一条，账本不收。
    let mut broken = built.events[..2].to_vec();
    broken.push(json!({
        "seq": 3, "at": "2026-09-25T07:02:00.000Z", "kind": "message.assistant",
        "by": {"kind": "model", "endpoint": "legacy", "model": "legacy"},
        "body": {"blocks": [{"type": "text", "text": "嗯"}], "seen": 2},
    }));
    match stage.bundle(&id, &broken, "2026-09-25T08:00:00.000Z") {
        Err(StageError::Ledger(3, _)) => {}
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_last_round_that_is_the_reply_is_not_said_twice() {
    let mut one = turn("turn_a", 1);
    one.assistant_content = "好了".to_string();
    one.tool_flow = Some(
        json!([
            {"assistant_content": "我看看", "calls": [{"id": "x", "name": "read", "arguments": "{}", "output": "o"}]},
            {"assistant_content": "好了", "calls": []}
        ])
        .to_string(),
    );
    let built = build(&HEAD, &session(""), &[one], &HashMap::new());
    let replies: Vec<&Value> = built
        .events
        .iter()
        .filter(|event| event["kind"] == "message.assistant")
        .collect();
    assert_eq!(
        replies.len(),
        2,
        "工具那一轮、最后一轮，最后的回复不再写一遍"
    );
    assert_eq!(
        replies[1]["body"]["blocks"],
        json!([{"type": "text", "text": "好了"}])
    );
    ledger_ok(&built.events);
}

//! 会话：编号照旧编号算、同输入同编号、时间段是创建时刻；时刻写成 UTC 毫秒；一轮拼成的事件（字、附件、工具流、失败的工具、
//! 中断、占位、摘要行、标题）。

use std::collections::HashMap;

use serde_json::{Value, json};

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

fn session(name: &str) -> Session {
    Session {
        id: "sess_1".to_string(),
        persona: String::new(),
        name: name.to_string(),
        kind: "user".to_string(),
        owner: String::new(),
        parent: None,
        created_at: "2026-09-25T07:00:00+00:00".to_string(),
        updated_at: "2026-09-25T08:00:00+00:00".to_string(),
    }
}

fn turn(id: &str, seq: i64) -> Turn {
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

const HEAD: Head<'static> = Head {
    venue: "local",
    policy: "sha256:0000000000000000000000000000000000000000000000000000000000000000",
    persona: Some("persona-1"),
    who: Who::Person,
};

/// 每一条核心都读得懂，整份过得了核心的账本（载入时查的那一套）。
fn ledger_ok(events: &[Value]) {
    let mut ledger = miyu_kernel::ledger::Ledger::default();
    for event in events {
        let parsed = miyu_kernel::event::Event::from_line(&event.to_string())
            .unwrap_or_else(|error| panic!("{event}: {error}"));
        ledger
            .append(&parsed)
            .unwrap_or_else(|error| panic!("{event}: {error}"));
    }
}

fn kinds(events: &[Value]) -> Vec<&str> {
    ledger_ok(events);
    events
        .iter()
        .map(|event| event["kind"].as_str().unwrap())
        .collect()
}

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

/// 照旧版 `targeting.rs` 的样子拼的一轮群消息：上文一节（不拆）、新到的、同一个人早些的、随后补充的；她自己的一行。
const GROUP_TURN: &str = "[Prior group chat records]
[09:58] 老王(QQ:111) [msg=1]: 上文不拆
[09:59] [you] [msg=2]: 她说过的
[New messages received this turn]
[10:00] 小李(QQ:222) [msg=5]: 看这个\\n第二行
  quoted earlier message [msg=1] by 老王(QQ:111): \u{201c}上文不拆\u{201d}
  @mentions: [you]、老王(QQ:111)
[Earlier messages from the same sender this turn, in chronological order]
[09:59] 小李(QQ:222) [msg=4]: 先说一句
[09:59] 不显示号 [msg=6]: 没有号
[Follow-up messages sent later by the same sender, in chronological order]
[10:01] 小李(QQ:222) [msg=7]: 补一张图";

#[test]
fn group_turns_split_by_sender_and_skip_the_records_she_already_saw() {
    let lines = group::split(GROUP_TURN);
    let msgs: Vec<&str> = lines.iter().map(|line| line.msg.as_str()).collect();
    assert_eq!(msgs, vec!["5", "4", "6", "7"]);
    let first = &lines[0];
    assert_eq!(
        (first.qq.as_deref(), first.name.as_str()),
        (Some("222"), "小李")
    );
    assert_eq!(first.text, "看这个\n第二行", "JSON 转义读回来");
    assert_eq!(first.reply_to.as_deref(), Some("1"));
    assert_eq!(first.quoted.as_deref(), Some("上文不拆"));
    assert!(first.mentions_me);
    assert_eq!(first.mentions, vec!["111".to_string()]);
    assert_eq!(lines[2].qq, None, "不显示号的群拆不出号");
    assert!(group::split("随便一段字").is_empty());
    // 上文一节在后面也不拆；她自己的行在哪一节都不拆。
    let later = "[New messages received this turn]\n[10:00] 甲(QQ:1) [msg=1]: 一\n[09:59] [you] [msg=2]: 她的\n[Prior group chat records]\n[09:00] 乙(QQ:2) [msg=0]: 上文";
    let msgs: Vec<String> = group::split(later)
        .into_iter()
        .map(|line| line.msg)
        .collect();
    assert_eq!(msgs, vec!["1".to_string()]);
}

#[test]
fn followups_carry_their_sender_in_trusted_metadata() {
    let raw = "再问一句\n\nTrusted metadata for this QQ follow-up message: sender QQ=333; message ID=9; replied-to message ID=8; replied-to sender QQ=111";
    assert_eq!(
        group::followup(raw),
        (
            "再问一句".to_string(),
            Some("333".to_string()),
            Some("9".to_string()),
            Some("8".to_string())
        )
    );
    assert_eq!(
        group::followup("没有元数据"),
        ("没有元数据".to_string(), None, None, None)
    );
}

#[test]
fn evicted_turns_merge_back_by_time_keeping_each_order() {
    let at = |id: &str, time: &str| Turn {
        id: id.to_string(),
        user_timestamp: time.to_string(),
        ..Turn::default()
    };
    let kept = vec![
        at("k1", "2026-09-25T07:00:00Z"),
        at("k2", "2026-09-25T09:00:00Z"),
    ];
    let evicted = vec![
        at("e1", "2026-09-25T06:00:00Z"),
        at("e2", "2026-09-25T09:00:00Z"),
        at("e3", "2026-09-25T10:00:00Z"),
    ];
    let merged: Vec<String> = evicted::merge(kept, evicted)
        .into_iter()
        .map(|turn| turn.id)
        .collect();
    assert_eq!(
        merged,
        vec!["e1", "k1", "k2", "e2", "e3"],
        "同一时刻会话里剩下的在前"
    );
}

#[test]
fn a_group_turn_becomes_one_message_per_line_and_the_rest_join_the_turn() {
    let venue = "qq:999:group:555";
    let head = Head {
        venue,
        who: Who::Group("999"),
        ..HEAD
    };
    let mut one = turn("turn_g", 1);
    one.user_content = GROUP_TURN.to_string();
    one.display_content = "给人看的不用".to_string();
    one.followups = vec![
        "后来又说\n\nTrusted metadata for this QQ follow-up message: sender QQ=333; message ID=9"
            .to_string(),
    ];
    let built = build(&head, &session(""), &[one], &HashMap::new());
    let events = &built.events;
    assert_eq!(
        kinds(events),
        vec![
            "session.created",
            "message.user",
            "turn.started",
            "message.user",
            "message.user",
            "message.user",
            "message.user",
            "message.assistant",
            "turn.ended"
        ]
    );
    assert_eq!(events[0]["body"]["venue"], venue);
    assert_eq!(events[0]["body"]["confined"], true);
    let first = &events[1];
    assert!(first.get("turn").is_none(), "引起这一轮的不带 turn");
    assert_eq!(
        first["by"],
        json!({"kind": "external", "venue": venue, "id": "qq:222", "role": "member"})
    );
    assert_eq!(
        first["body"]["blocks"][0]["text"],
        "看这个\n第二行\n\n> 上文不拆"
    );
    assert_eq!(
        first["body"]["venue"],
        json!({"msg": "5", "name": "小李", "reply_to": "1", "mentions": ["qq:111"], "mentions_me": true, "show_ids": true})
    );
    assert_eq!(events[3]["turn"], 3);
    assert_eq!(events[4]["by"]["id"], "qq:unknown");
    assert_eq!(events[6]["by"]["id"], "qq:333");
    assert_eq!(events[6]["body"]["venue"], json!({"msg": "9"}));
    assert_eq!(built.unknown, 1);
}

#[test]
fn a_private_turn_is_said_by_the_other_person() {
    let venue = "qq:999:private:333";
    let head = Head {
        venue,
        who: Who::Private("333"),
        ..HEAD
    };
    let built = build(&head, &session(""), &[turn("turn_p", 1)], &HashMap::new());
    assert_eq!(kinds(&built.events)[1], "message.user");
    assert!(built.events[0]["body"].get("confined").is_none());
    assert_eq!(
        built.events[1]["by"],
        json!({"kind": "external", "venue": venue, "id": "qq:333", "role": "member"})
    );
    assert_eq!(built.events[1]["body"]["blocks"][0]["text"], "turn_p 原文");
}

//! 场所会话：群的一轮照号拆人、上文和她自己的不拆、并进来的话认可信元数据、被挤出的轮照时刻归并、私聊是对方说的。

use std::collections::HashMap;

use serde_json::json;

use super::super::*;
use super::{HEAD, kinds, session, turn};

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

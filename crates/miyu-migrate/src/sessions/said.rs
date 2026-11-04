//! 一轮里人说的话写成几条 `message.user`（细则第一节第 11 条）：
//!
//! - 本机会话：一条，`by person`，字取给人看的，附件接在后面；
//! - 私聊：一条，`by` 是对方（`external`，`qq:<号>`），字照原样；
//! - 群：照每行开头的号拆（[`group::split`]），一行一条，`by` 是那个人（拆不出号的 `qq:unknown`），`venue` 那一格记平台编号、
//!   名字、引用、@；拆不出一行的整段当一条 `qq:unknown`。附件接在第一条后面。
//!
//! 被挤出的轮里排着并进来的话（`followup:`、`before:`）各一条，号、编号照旧版附在正文后面的可信元数据。第一条是引起这一轮的，
//! 别的排在 `turn.started` 后面。

use serde_json::{Value, json};

use super::events::Media;
use super::group;
use super::old::Turn;

/// 这个会话里说话的是谁。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Who<'a> {
    /// 本机：人。
    Person,
    /// QQ 私聊：对方的号。
    Private(&'a str),
    /// QQ 群：机器人的号（她自己）。
    Group(&'a str),
}

/// 一条 `message.user`：谁说的、正文；拆不出号的记一笔。
#[derive(Debug, Clone, PartialEq)]
pub struct Said {
    /// `by`。
    pub by: Value,
    /// `body`。
    pub body: Value,
    /// 号是 `qq:unknown`。
    pub unknown: bool,
}

/// 一轮里人说的话。`venue` 是会话的场所。交回几条（至少一条）和附件几个。
pub fn said(venue: &str, who: Who<'_>, turn: &Turn, media: &[Media]) -> (Vec<Said>, u64) {
    let shown = if turn.display_content.trim().is_empty() {
        turn.user_content.as_str()
    } else {
        turn.display_content.as_str()
    };
    let mut all = match who {
        Who::Person => vec![plain(json!({"kind": "person"}), shown, None)],
        Who::Private(peer) => vec![plain(external(venue, Some(peer)), shown, None)],
        Who::Group(_) => {
            let lines = group::split(&turn.user_content);
            if lines.is_empty() {
                vec![plain(external(venue, None), shown, None)]
            } else {
                lines.iter().map(|line| from_line(venue, line)).collect()
            }
        }
    };
    for raw in &turn.followups {
        let (text, qq, msg, reply_to) = group::followup(raw);
        let peer = match who {
            Who::Private(peer) => Some(peer.to_string()),
            _ => qq,
        };
        let by = match who {
            Who::Person => json!({"kind": "person"}),
            _ => external(venue, peer.as_deref()),
        };
        let mut one = plain(by, &text, None);
        if let Some(msg) = msg {
            let mut place = json!({"msg": msg});
            if let Some(reply_to) = reply_to {
                place["reply_to"] = json!(reply_to);
            }
            one.body["venue"] = place;
        }
        one.unknown = who != Who::Person && peer.is_none();
        all.push(one);
    }
    let mut attached = 0;
    if let Some(first) = all.first_mut()
        && let Some(blocks) = first.body["blocks"].as_array_mut()
    {
        for one in media {
            blocks.push(block(one));
            attached += 1;
        }
    }
    (all, attached)
}

/// 一条只有字的：空的写一个空字块（`blocks` 不能是空的）。
fn plain(by: Value, text: &str, place: Option<Value>) -> Said {
    let unknown = by["id"] == "qq:unknown";
    let mut body = json!({"blocks": [{"type": "text", "text": text}]});
    if let Some(place) = place {
        body["venue"] = place;
    }
    Said { by, body, unknown }
}

/// 场所里的人：`qq:<号>`，没有号的 `qq:unknown`。
fn external(venue: &str, qq: Option<&str>) -> Value {
    let id = qq.map_or_else(|| "qq:unknown".to_string(), |qq| format!("qq:{qq}"));
    json!({"kind": "external", "venue": venue, "id": id, "role": "member"})
}

/// 群里拆出来的一行：引用的原文接在正文后面一段。
fn from_line(venue: &str, line: &group::Line) -> Said {
    let mut text = line.text.clone();
    if let Some(quoted) = &line.quoted {
        text.push_str("\n\n> ");
        text.push_str(quoted);
    }
    let mut place = json!({"msg": line.msg, "name": line.name});
    if let Some(reply_to) = &line.reply_to {
        place["reply_to"] = json!(reply_to);
    }
    if !line.mentions.is_empty() {
        let mentions: Vec<String> = line.mentions.iter().map(|qq| format!("qq:{qq}")).collect();
        place["mentions"] = json!(mentions);
    }
    if line.mentions_me {
        place["mentions_me"] = json!(true);
    }
    if line.qq.is_some() {
        place["show_ids"] = json!(true);
    }
    plain(external(venue, line.qq.as_deref()), &text, Some(place))
}

/// 一个附件的块：认出大小的图片写图片块，别的写文件块。
pub fn block(media: &Media) -> Value {
    match media.size {
        Some((width, height)) => json!({
            "type": "image",
            "blob": media.blob,
            "name": media.name,
            "media_type": media.media_type,
            "width": width,
            "height": height,
        }),
        None => json!({
            "type": "file",
            "blob": media.blob,
            "name": media.name,
            "media_type": media.media_type,
        }),
    }
}

//! 会话（`sessions`）、QQ 消息记录（`qq-records`）的字：旧格式一句话、迁完那一句。英文里接在上一句后面的，开头带一个空格。

use std::path::Path;

use super::Language;

/// 会话的字。
pub struct Sessions {
    /// 旧格式一句话。
    pub format: &'static str,
    /// 旧根里没有会话库。
    pub none: &'static str,
    /// 会话库比认得的新：它的版本、认得的版本。
    pub too_new: fn(i64, i64) -> String,
    /// 会话库读不了。
    pub unreadable: fn(&str) -> String,
    /// 暂存目录建不了。
    pub no_stage: fn(&str) -> String,
    /// 被挤出上下文的库几个读不了、哪几个。
    pub evicted_unreadable: fn(usize, &str) -> String,
    /// 几轮、几个附件、几条摘要行。
    pub turns: fn(u64, u64, u64) -> String,
    /// 子代理会话几个。
    pub children: fn(u64) -> String,
    /// 以前导过的几个。
    pub present: fn(u64) -> String,
    /// 人格没映射上的几个。
    pub unmapped: fn(u64) -> String,
    /// 只有展示转录的几轮。
    pub replay_only: fn(u64) -> String,
    /// 拼回去的被挤出的几轮。
    pub evicted: fn(u64) -> String,
    /// 群里拆不出是谁说的几条。
    pub unknown: fn(u64) -> String,
    /// 工具结果后面接的图、文件几样。
    pub tool_blocks: fn(u64) -> String,
    /// 对不上调用的图、文件几样。
    pub unplaced: fn(u64) -> String,
    /// 提问几次。
    pub questions: fn(u64) -> String,
}

/// QQ 消息记录的字。
pub struct Records {
    /// 旧格式一句话。
    pub format: &'static str,
    /// 旧根里没有。
    pub none: &'static str,
    /// 不是认得的那一版。
    pub wrong_version: fn(i64) -> String,
    /// 读不了。
    pub unreadable: fn(&str) -> String,
    /// 写不了转出来的文件。
    pub cannot_write: fn(&Path, &str) -> String,
    /// 转出几条消息、几件撤回，写在哪。
    pub converted: fn(u64, u64, &Path) -> String,
    /// 几条不是 QQ 的。
    pub foreign: fn(u64) -> String,
    /// im 新收了几条消息、几件群事件。
    pub added: fn(u64, u64) -> String,
    /// im 里本来就有的几条、编号被占的几条。
    pub same: fn(u64, u64) -> String,
    /// 收尾的句号。
    pub full_stop: &'static str,
    /// im 没导：它的头一行、改好以后跑的命令。
    pub refused: fn(&str, &str) -> String,
    /// 跑不了 im：系统的原话、装上以后跑的命令。
    pub cannot_run: fn(&str, &str) -> String,
}

static SESSIONS_ZH: Sessions = Sessions {
    format: "SQLite（user_version 1-40）：sessions 一行一个会话（kind user/ask/voice/subagent，owner 非空是成员的），turns 一行一轮（user_content/display_content、assistant_content/assistant_reasoning、tool_flow JSON、replay_journal JSON、is_summary 摘要行），user_attachments 附件（data 列或 state/attachments/<编号>/<文件名>），platform_session_bindings 绑场所",
    none: "旧根里没有会话库。",
    too_new: |version, newest| {
        format!("会话库的版本 {version} 比认得的 {newest} 新，整库留在压缩包里。")
    },
    unreadable: |error| format!("会话库读不了：{error}"),
    no_stage: |error| format!("暂存目录建不了：{error}"),
    evicted_unreadable: |n, scopes| {
        format!("被挤出上下文的库有 {n} 个读不了（{scopes}），留在压缩包里。")
    },
    turns: |turns, attachments, summaries| {
        format!("{turns} 轮，{attachments} 个附件；摘要行 {summaries} 条不导（是派生的）。")
    },
    children: |n| format!("子代理会话 {n} 个作子会话，和主会话一起导。"),
    present: |n| format!("其中 {n} 个会话以前就导过。"),
    unmapped: |n| format!("{n} 个会话的人格没映射上，导成了无人格的存档。"),
    replay_only: |n| format!("{n} 轮只有展示转录：工具照转录拼的，参数和输出被旧版截过。"),
    evicted: |n| format!("被挤出上下文的 {n} 轮拼回了原来的会话。"),
    unknown: |n| format!("群里 {n} 条话拆不出是谁说的，记成 qq:unknown。"),
    tool_blocks: |n| format!("工具结果后面接了 {n} 张图、文件。"),
    unplaced: |n| {
        format!(
            "工具出的图、文件有 {n} 样对不上调用（那一轮的工具细节没有了，例如被挤出的轮），没接上，留在压缩包里。"
        )
    },
    questions: |n| format!("提问 {n} 次写成了 ask_user。"),
};

static SESSIONS_EN: Sessions = Sessions {
    format: "SQLite (user_version 1-40): sessions, one row per session (kind user/ask/voice/subagent, a non-empty owner means a member's), turns, one row per turn (user_content/display_content, assistant_content/assistant_reasoning, tool_flow JSON, replay_journal JSON, is_summary for summary rows), user_attachments for attachments (the data column or state/attachments/<id>/<file name>), platform_session_bindings for venue bindings",
    none: "No session database in the old root.",
    too_new: |version, newest| {
        format!(
            "The session database is version {version}, newer than the {newest} this program knows; the whole database stays in the archive."
        )
    },
    unreadable: |error| format!("Cannot read the session database: {error}"),
    no_stage: |error| format!("Cannot create the staging folder: {error}"),
    evicted_unreadable: |n, scopes| {
        format!(
            "{n} database(s) of turns pushed out of context could not be read ({scopes}); they stay in the archive."
        )
    },
    turns: |turns, attachments, summaries| {
        format!(
            "{turns} turn(s), {attachments} attachment(s); {summaries} summary row(s) not imported (they are derived)."
        )
    },
    children: |n| {
        format!(
            "{n} subagent session(s) imported as child sessions together with their main session."
        )
    },
    present: |n| format!("{n} of the sessions had been imported before."),
    unmapped: |n| {
        format!(
            "{n} session(s) had an unmapped persona and were imported as archives with no persona."
        )
    },
    replay_only: |n| {
        format!(
            "{n} turn(s) only had a display transcript: tools were rebuilt from it, and their arguments and output had been cut by the old version."
        )
    },
    evicted: |n| format!("{n} turn(s) pushed out of context were put back into their sessions."),
    unknown: |n| {
        format!(
            "{n} group message(s) could not be tied to a sender and are recorded as qq:unknown."
        )
    },
    tool_blocks: |n| format!("{n} picture(s) or file(s) were attached after tool results."),
    unplaced: |n| {
        format!(
            "{n} picture(s) or file(s) from tools did not match a call (the tool details of that turn are gone, for example in turns pushed out of context); they were not attached and stay in the archive."
        )
    },
    questions: |n| format!("{n} question(s) were written as ask_user."),
};

static RECORDS_ZH: Records = Records {
    format: "SQLite（user_version 4）：messages 一行一条（platform onebot、account_id 机器人号、conversation_kind group/private、conversation_id、message_id、sender_id、sender_name、text、media_json、mentions_json、reply_to_message_id、is_bot、sent_at 秒、recalled_at 秒），recalls 撤回（operator_id），context_boundaries 不迁",
    none: "旧根里没有 QQ 消息记录。",
    wrong_version: |version| format!("消息记录库不是第 {version} 版，整库留在压缩包里。"),
    unreadable: |error| format!("消息记录库读不了：{error}"),
    cannot_write: |file, error| format!("写不了 {}：{error}", file.display()),
    converted: |messages, recalls, file| {
        format!(
            "转出 {messages} 条消息、{recalls} 件撤回（消息不在了的那几次），写在 `{}`。",
            file.display()
        )
    },
    foreign: |n| format!("{n} 条不是 QQ 的没转。"),
    added: |messages, events| format!("im 新收了 {messages} 条消息、{events} 件群事件"),
    same: |same, taken| {
        format!(
            "；{same} 条 im 里本来就有（新 im 并行接过的、以前导过的），{taken} 条编号被别的消息占了、留着 im 里的"
        )
    },
    full_stop: "。",
    refused: |first, command| {
        format!("im 没导（{first}）。装上 im 以后（或者照它说的改好以后）跑：`{command}`。")
    },
    cannot_run: |error, command| format!("跑不了 im（{error}）。装上 im 以后跑：`{command}`。"),
};

static RECORDS_EN: Records = Records {
    format: "SQLite (user_version 4): messages, one row per message (platform onebot, account_id the bot's number, conversation_kind group/private, conversation_id, message_id, sender_id, sender_name, text, media_json, mentions_json, reply_to_message_id, is_bot, sent_at seconds, recalled_at seconds), recalls (operator_id); context_boundaries is not migrated",
    none: "No QQ message records in the old root.",
    wrong_version: |version| {
        format!(
            "The message record database is not version {version}; the whole database stays in the archive."
        )
    },
    unreadable: |error| format!("Cannot read the message record database: {error}"),
    cannot_write: |file, error| format!("Cannot write {}: {error}", file.display()),
    converted: |messages, recalls, file| {
        format!(
            "Converted {messages} message(s) and {recalls} recall(s) (of messages that are gone) into `{}`.",
            file.display()
        )
    },
    foreign: |n| format!(" {n} message(s) not from QQ were not converted."),
    added: |messages, events| {
        format!(" im took in {messages} new message(s) and {events} group event(s)")
    },
    same: |same, taken| {
        format!(
            "; {same} were already in im (received by the new im in parallel, or imported before), {taken} had ids taken by other messages and im keeps its own"
        )
    },
    full_stop: ".",
    refused: |first, command| {
        format!(
            " im did not import them ({first}). After installing im (or fixing what it says), run: `{command}`."
        )
    },
    cannot_run: |error, command| {
        format!(" Cannot run im ({error}). After installing im, run: `{command}`.")
    },
};

/// 照语言挑会话的字。
#[must_use]
pub fn sessions(language: Language) -> &'static Sessions {
    language.pick(&SESSIONS_ZH, &SESSIONS_EN)
}

/// 照语言挑 QQ 消息记录的字。
#[must_use]
pub fn records(language: Language) -> &'static Records {
    language.pick(&RECORDS_ZH, &RECORDS_EN)
}

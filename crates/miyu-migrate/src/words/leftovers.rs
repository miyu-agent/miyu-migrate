//! 零碎（`leftovers`）的字：每一样的旧格式一句话、大小和条数、数据根外面的那几句。英文里接在上一句后面的，开头带一个空格。

use super::Language;

/// 零碎的字。
pub struct Leftovers {
    /// 用量。
    pub usage: &'static str,
    /// 上键历史。
    pub repl_history: &'static str,
    /// QQ 的文件、图片、头像。
    pub qq_media: &'static str,
    /// 配置的手动备份。
    pub config_backups: &'static str,
    /// 网页。
    pub web: &'static str,
    /// 语音。
    pub voice: &'static str,
    /// 定时提醒。
    pub alarms: &'static str,
    /// 别的旧状态。
    pub old_state: &'static str,
    /// 运行日志。
    pub logs: &'static str,
    /// 缓存。
    pub cache: &'static str,
    /// 知识库的向量。
    pub kb_index: &'static str,
    /// 别的派生的状态。
    pub derived_state: &'static str,
    /// 几个文件、多大。
    pub size: fn(u64, &str) -> String,
    /// 用量几条。
    pub usage_count: fn(i64) -> String,
    /// 会话库里的上键历史几条。
    pub repl_count: fn(i64) -> String,
    /// 提醒几条、还没到点的几条。
    pub alarms_count: fn(usize, usize) -> String,
    /// 成员的家目录在哪。
    pub members_source: &'static str,
    /// 成员的家目录是什么。
    pub members_format: &'static str,
    /// 几个成员。
    pub members_note: fn(usize) -> String,
    /// 一个插件几条键值。
    pub plugin_rows: fn(&str, i64) -> String,
    /// 插件键值那一句：几个插件并起来的。
    pub plugins: fn(&str) -> String,
    /// 表情引用、会话绑定叫什么。
    pub platform_tables: [&'static str; 2],
    /// 一张表几条。
    pub table_rows: fn(&str, i64) -> String,
    /// 平台的表在哪（会话库的相对路径）。
    pub platform_source: fn(&str) -> String,
    /// 平台的表是什么。
    pub platform_format: &'static str,
    /// 记忆的逐轮经历、日记、改动记录、旧向量叫什么。
    pub memory_kinds: [&'static str; 4],
    /// 一种几条。
    pub memory_rows: fn(&str, i64) -> String,
    /// 一个人格的几种。
    pub memory_scope: fn(&str, &str) -> String,
    /// 记忆的库在哪（人格目录的相对路径）。
    pub memory_source: fn(&str) -> String,
    /// 记忆剩下的是什么。
    pub memory_format: &'static str,
    /// 几句并起来以后的句号。
    pub full_stop: &'static str,
    /// `~/.local/bin/miyu` 是新版的。
    pub outside_new: &'static str,
    /// 数据根外面旧版装过的。
    pub outside_old: &'static str,
    /// 数据根外面的是什么。
    pub outside_format: &'static str,
}

static ZH: Leftovers = Leftovers {
    usage: "SQLite usage_records 一次请求一行（供应商、模型、词元、费用、时刻）；新版账本还没有导入入口，以后有了再补",
    repl_history: "终端的上键历史：一行一条输入；会话库里另有 repl_history 表",
    qq_media: "QQ 收到的文件、图片、头像原样存的文件；新 im 只存占位，不存内容",
    config_backups: "旧配置的手动备份（JSONC），内容同 config.jsonc 当时的样子",
    web: "旧网页的登录密码（哈希）、自定义样式（CSS）、登录会话、界面状态；新版网页另有登录和外观包",
    voice: "语音会话的编号、缓存；语音的配置在 config.jsonc 的 voice 段（配置那一项报过）",
    alarms: "JSON 数组，一条一个一次性提醒（没有重复的）：label 是要提醒的内容，due_at 是到点的时刻（Unix 秒，已经照当时的本地时区算好的绝对时刻），time 是人当时说的原话（「15:30」「10m」这类），status 是 scheduled 或 ringing，audio_file 是响铃的声音文件；新版对应的方法是 schedule.add（它的参数这里未核实），已经过了 due_at 的不用迁",
    old_state: "旧版自己的状态：更早的聊天库、表情用到的记录、技能草稿、子代理的检查点、思考档位、角色扮演提示的自动稿、会话的待办、压缩的折叠稿（内容在会话库里，会话那一类已经导了）",
    logs: "运行日志",
    cache: "缓存：表情向量、模型列表、链接预览、图表、剪贴板暂存、命令输出",
    kb_index: "知识库的切块和向量、旧版内置默认库",
    derived_state: "模型文件的缓存、提示词指纹、中转线状态、启动记录",
    size: |files, size| format!("{files} 个文件，{size}。"),
    usage_count: |n| format!("用量 {n} 条。"),
    repl_count: |n| format!("会话库里另有 repl_history {n} 条。"),
    alarms_count: |all, pending| format!("提醒 {all} 条，还没到点的 {pending} 条。"),
    members_source: "home/<别的用户>/",
    members_format: "成员各自的家目录：会话库、知识库、记账、工作目录（新版只给一个人用）",
    members_note: |n| format!("{n} 个成员的家目录，没迁（新版只给一个人用）。"),
    plugin_rows: |plugin, n| format!("{plugin} {n} 条"),
    plugins: |listed| format!("插件键值（{listed}），新 im 没有对应的，要重做"),
    platform_tables: ["表情引用", "会话绑定"],
    table_rows: |name, n| format!("{name} {n} 条"),
    platform_source: |db| {
        format!("{db}（platform_plugin_kv、platform_meme_refs、platform_session_bindings）")
    },
    platform_format: "platform_plugin_kv：(plugin_id, platform, account_id, conversation_kind, conversation_id, key) → value_json；platform_meme_refs：哪条消息用了哪张表情；platform_session_bindings：QQ 会话绑在哪个会话（会话那一类已经照它导了）",
    memory_kinds: ["逐轮经历", "日记", "改动记录", "旧向量"],
    memory_rows: |name, n| format!("{name} {n}"),
    memory_scope: |scope, counts| format!("`{scope}`：{counts}"),
    memory_source: |dir| format!("{dir}/<人格>/memory/memory.db"),
    memory_format: "episodes（retention short_term 是逐轮经历、long_term 是日记；content、created_at、origin_*）、memory_revisions、memory_embeddings、pending_events、skill_records；知识点 facts 记忆那一类已经导了",
    full_stop: "。",
    outside_new: "这是新版的 miyu，不是旧版的，别删。",
    outside_old: "旧版装过这个，确认是旧版的、用不着了再删；删不删由你定。",
    outside_format: "数据根外面的文件，不在压缩包里",
};

static EN: Leftovers = Leftovers {
    usage: "SQLite usage_records, one row per request (provider, model, tokens, cost, time); the new ledger has no import yet, to be added when it does",
    repl_history: "terminal input history for the up arrow, one input per line; the session database also has a repl_history table",
    qq_media: "files, pictures and avatars received over QQ, stored as they came; the new im keeps placeholders only, not the contents",
    config_backups: "manual backups of the old config (JSONC), each the same as config.jsonc at that time",
    web: "the old web page's login password (hashed), custom style (CSS), login sessions and UI state; the new web page has its own login and appearance packages",
    voice: "voice session ids and caches; the voice settings are in the voice section of config.jsonc (covered by the config item)",
    alarms: "a JSON array, one one-off reminder per entry (no repeats): label is what to remind of, due_at is when (Unix seconds, an absolute time already worked out in the local time zone of that moment), time is what the person said (like \"15:30\" or \"10m\"), status is scheduled or ringing, audio_file is the ringing sound; the matching new method is schedule.add (its parameters are not checked here), and ones past due_at need no migration",
    old_state: "the old version's own state: an earlier chat database, sticker usage records, skill drafts, subagent checkpoints, thinking levels, auto drafts of roleplay hints, session to-dos, compaction drafts (the contents are in the session database, already imported with sessions)",
    logs: "run logs",
    cache: "caches: sticker vectors, model lists, link previews, diagrams, clipboard scratch, command output",
    kb_index: "knowledge base chunks and vectors, the old built-in default knowledge base",
    derived_state: "model file caches, prompt fingerprints, relay state, launch records",
    size: |files, size| format!("{files} file(s), {size}."),
    usage_count: |n| format!(" {n} usage record(s)."),
    repl_count: |n| format!(" The session database also has {n} repl_history row(s)."),
    alarms_count: |all, pending| format!(" {all} reminder(s), {pending} not due yet."),
    members_source: "home/<other users>/",
    members_format: "each member's own home folder: session database, knowledge base, bookkeeping, workspace (the new version is for one person)",
    members_note: |n| {
        format!("{n} member home folder(s), not migrated (the new version is for one person).")
    },
    plugin_rows: |plugin, n| format!("{plugin}: {n}"),
    plugins: |listed| {
        format!(
            "plugin key-values ({listed}); the new im has nothing matching, so they have to be redone"
        )
    },
    platform_tables: ["sticker references", "session bindings"],
    table_rows: |name, n| format!("{n} {name}"),
    platform_source: |db| {
        format!("{db} (platform_plugin_kv, platform_meme_refs, platform_session_bindings)")
    },
    platform_format: "platform_plugin_kv: (plugin_id, platform, account_id, conversation_kind, conversation_id, key) → value_json; platform_meme_refs: which message used which sticker; platform_session_bindings: which session a QQ conversation is bound to (already used when importing sessions)",
    memory_kinds: [
        "per-turn episodes",
        "diary entries",
        "revisions",
        "old vectors",
    ],
    memory_rows: |name, n| format!("{n} {name}"),
    memory_scope: |scope, counts| format!("`{scope}`: {counts}"),
    memory_source: |dir| format!("{dir}/<persona>/memory/memory.db"),
    memory_format: "episodes (retention short_term is per-turn episodes, long_term is diary; content, created_at, origin_*), memory_revisions, memory_embeddings, pending_events, skill_records; facts were already imported with memories",
    full_stop: ".",
    outside_new: "This is the new miyu, not the old one. Do not delete it.",
    outside_old: "The old version installed this. Delete it once you are sure it is the old one and no longer needed; that is up to you.",
    outside_format: "a file outside the data root, not in the archive",
};

/// 照语言挑零碎的字。
#[must_use]
pub fn words(language: Language) -> &'static Leftovers {
    language.pick(&ZH, &EN)
}

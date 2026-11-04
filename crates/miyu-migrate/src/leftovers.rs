//! 零碎（DESIGN 第五节、core1 11-05 定的清单）：前面各类没管到的，每一样在报告里一项，写明在压缩包的哪个路径、旧格式一句话、多大
//! 或几条，给 agent 的那份照着就能接着迁。什么都不写进新数据根，不连核心。
//!
//! - 留在压缩包里的（`archived`）：用量、上键历史、成员的家目录（只数个数）、QQ 的图片文件头像、平台的表、记忆的经历和旧向量、配置的
//!   手动备份、网页、语音、定时提醒、别的旧状态。
//! - 派生的（`skipped`，`what` 以 `derived/` 开头）：新版自己重建，给人看的报告合成一行。
//! - 数据根外面的（`skipped`，`what` 以 `outside/` 开头）：只报在不在，删不删由人定；`~/.local/bin/miyu` 是新版包装脚本的照实说。

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

use crate::layout::Layout;
use crate::report::{Item, Status};

/// 一样零碎：报告里叫什么、在旧根的哪几个相对路径、旧格式一句话、要不要另外数库里的条数。
struct Entry {
    what: &'static str,
    paths: &'static [&'static str],
    format: &'static str,
    status: Status,
}

/// 留在压缩包里的。
const ARCHIVED: &[Entry] = &[
    Entry {
        what: "usage",
        paths: &[
            "state/usage.db",
            "state/usage-history.jsonl",
            "state/usage.json",
        ],
        format: "SQLite usage_records 一次请求一行（供应商、模型、词元、费用、时刻）；新版账本还没有导入入口，以后有了再补",
        status: Status::Archived,
    },
    Entry {
        what: "repl-history",
        paths: &["state/repl-history"],
        format: "终端的上键历史：一行一条输入；会话库里另有 repl_history 表",
        status: Status::Archived,
    },
    Entry {
        what: "qq-media",
        paths: &[
            "cache/platform_files",
            "cache/platform_images",
            "cache/qq-avatars",
        ],
        format: "QQ 收到的文件、图片、头像原样存的文件；新 im 只存占位，不存内容",
        status: Status::Archived,
    },
    Entry {
        what: "config-backups",
        paths: &["config/config.jsonc.bak*"],
        format: "旧配置的手动备份（JSONC），内容同 config.jsonc 当时的样子",
        status: Status::Archived,
    },
    Entry {
        what: "web",
        paths: &[
            "config/web-password",
            "config/webui-theme.css",
            "state/web-sessions.json",
            "state/web-ui.json",
        ],
        format: "旧网页的登录密码（哈希）、自定义样式（CSS）、登录会话、界面状态；新版网页另有登录和外观包",
        status: Status::Archived,
    },
    Entry {
        what: "voice",
        paths: &["state/voice-session-id", "cache/voice"],
        format: "语音会话的编号、缓存；语音的配置在 config.jsonc 的 voice 段（配置那一项报过）",
        status: Status::Archived,
    },
    Entry {
        what: "alarms",
        paths: &["state/alarms.json"],
        format: "JSON 数组，一条一个一次性提醒（没有重复的）：label 是要提醒的内容，due_at 是到点的时刻（Unix 秒，已经照当时的本地时区算好的绝对时刻），time 是人当时说的原话（「15:30」「10m」这类），status 是 scheduled 或 ringing，audio_file 是响铃的声音文件；新版对应的方法是 schedule.add（它的参数这里未核实），已经过了 due_at 的不用迁",
        status: Status::Archived,
    },
    Entry {
        what: "old-state",
        paths: &[
            "data/chat_history.db",
            "state/memes.db",
            "state/skill-drafts",
            "state/subagent-checkpoints",
            "state/session-thinking-variants",
            "state/thinking-variants.json",
            "state/persona-hints",
            "state/todos",
            "state/compact",
        ],
        format: "旧版自己的状态：更早的聊天库、表情用到的记录、技能草稿、子代理的检查点、思考档位、角色扮演提示的自动稿、会话的待办、压缩的折叠稿（内容在会话库里，会话那一类已经导了）",
        status: Status::Archived,
    },
];

/// 派生的，新版自己重建。
const DERIVED: &[Entry] = &[
    Entry {
        what: "derived/logs",
        paths: &["logs"],
        format: "运行日志",
        status: Status::Skipped,
    },
    Entry {
        what: "derived/cache",
        paths: &[
            "cache/meme-embeddings",
            "cache/models_cache.json",
            "cache/link-preview",
            "cache/diagrams",
            "cache/clipboard_images",
            "cache/clipboard_texts",
            "cache/command-output",
            "cache/tool-drill",
            "cache/jobs",
            "cache/sounds",
            "cache/logs",
        ],
        format: "缓存：表情向量、模型列表、链接预览、图表、剪贴板暂存、命令输出",
        status: Status::Skipped,
    },
    Entry {
        what: "derived/kb-index",
        paths: &[
            "data/kb/semantic_index.db",
            "data/kb/embedding-reindex.json",
            "cache/default-kb",
            "data/default-kb",
        ],
        format: "知识库的切块和向量、旧版内置默认库",
        status: Status::Skipped,
    },
    Entry {
        what: "derived/state",
        paths: &[
            "state/models",
            "state/prompt-fingerprints",
            "state/relay",
            "state/daemon-launch.json",
            "state/arch_news_last_seen.json",
            "state/thinking-variants.lock",
        ],
        format: "模型文件的缓存、提示词指纹、中转线状态、启动记录",
        status: Status::Skipped,
    },
];

/// 数据根外面旧版装过的（相对家目录）。
const OUTSIDE: &[&str] = &[
    ".local/bin/miyu",
    ".config/fish/conf.d/miyu.fish",
    ".gemini/config/agents/miyu",
    ".gemini/antigravity-cli",
    ".cache/miyu/voice-models",
    ".local/state/miyu",
];

/// 零碎这几项：`copy` 是解出来的副本，`home` 是这台机器的家目录（看数据根外面的，测试换成临时目录）。
pub fn items(copy: &Layout, home: Option<&Path>) -> Vec<Item> {
    let mut items = Vec::new();
    for entry in ARCHIVED.iter().chain(DERIVED) {
        let found = found(&copy.root, entry.paths);
        if found.is_empty() {
            continue;
        }
        let (files, bytes) = found.iter().fold((0u64, 0u64), |(files, bytes), path| {
            let (more, size) = size(path);
            (files + more, bytes + size)
        });
        let mut note = format!("{files} 个文件，{}。", human_size(bytes));
        if let Some(count) = counted(copy, entry.what) {
            note.push_str(&count);
        }
        items.push(item(
            entry.what,
            &found,
            &copy.root,
            entry.format,
            entry.status,
            note,
        ));
    }
    items.extend(members(copy));
    items.extend(platform(copy));
    items.extend(memory(copy));
    if let Some(home) = home {
        items.extend(outside(home));
    }
    items
}

/// 旧根里这几个路径真有的（`*` 结尾的照前缀找同一层的）。
fn found(root: &Path, paths: &[&str]) -> Vec<PathBuf> {
    let mut all = Vec::new();
    for path in paths {
        if let Some(prefix) = path.strip_suffix('*') {
            let full = root.join(prefix);
            let (dir, start) = (
                full.parent().map(Path::to_path_buf).unwrap_or_default(),
                full.file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default(),
            );
            let mut matched: Vec<PathBuf> = fs::read_dir(&dir)
                .map(|entries| {
                    entries
                        .filter_map(Result::ok)
                        .map(|entry| entry.path())
                        .filter(|path| {
                            path.file_name()
                                .is_some_and(|name| name.to_string_lossy().starts_with(&start))
                        })
                        .collect()
                })
                .unwrap_or_default();
            matched.sort();
            all.extend(matched);
        } else if root.join(path).exists() {
            all.push(root.join(path));
        }
    }
    all
}

/// 文件几个、多少字节（目录照里面的普通文件加）。
fn size(path: &Path) -> (u64, u64) {
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .fold((0, 0), |(files, bytes), entry| {
            (
                files + 1,
                bytes + entry.metadata().map_or(0, |meta| meta.len()),
            )
        })
}

/// 几样另外数库里的条数。
fn counted(copy: &Layout, what: &str) -> Option<String> {
    match what {
        "usage" => count(
            &copy.root.join("state/usage.db"),
            "SELECT COUNT(*) FROM usage_records",
        )
        .map(|n| format!("用量 {n} 条。")),
        "repl-history" => count(&copy.conversation_db(), "SELECT COUNT(*) FROM repl_history")
            .map(|n| format!("会话库里另有 repl_history {n} 条。")),
        "alarms" => {
            let text = fs::read_to_string(copy.root.join("state/alarms.json")).ok()?;
            let alarms: Vec<Value> = serde_json::from_str(&text).ok()?;
            let now = jiff::Timestamp::now().as_second();
            let pending = alarms
                .iter()
                .filter(|one| one["due_at"].as_i64().is_some_and(|due| due > now))
                .count();
            Some(format!(
                "提醒 {} 条，还没到点的 {pending} 条。",
                alarms.len()
            ))
        }
        _ => None,
    }
}

/// 照 `sql` 数一个数；库、表没有的是 `None`。
fn count(db: &Path, sql: &str) -> Option<i64> {
    let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_WRITE).ok()?;
    conn.query_row(sql, [], |row| row.get(0)).ok()
}

/// 成员的家目录：只数个数，不写名字。
fn members(copy: &Layout) -> Option<Item> {
    let admin = copy.admin.clone()?;
    let home = copy.root.join("home");
    let others = fs::read_dir(&home)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir() && entry.file_name().to_string_lossy() != admin)
        .count();
    (others > 0).then(|| Item {
        what: "members".to_string(),
        status: Status::Archived,
        source: "home/<别的用户>/".to_string(),
        format: "成员各自的家目录：会话库、知识库、记账、工作目录（新版只给一个人用）".to_string(),
        migrated: 0,
        skipped: Vec::new(),
        target: None,
        note: Some(format!(
            "{others} 个成员的家目录，没迁（新版只给一个人用）。"
        )),
    })
}

/// 会话库里平台的几张表：插件键值照插件数条数（好感度、情绪这类要在新 im 里重做），表情引用、会话绑定只数条数。
fn platform(copy: &Layout) -> Option<Item> {
    let db = copy.conversation_db();
    let conn = Connection::open_with_flags(&db, OpenFlags::SQLITE_OPEN_READ_WRITE).ok()?;
    let mut parts = Vec::new();
    if let Ok(mut statement) = conn.prepare(
        "SELECT plugin_id, COUNT(*) FROM platform_plugin_kv GROUP BY plugin_id ORDER BY plugin_id",
    ) {
        let plugins: Vec<String> = statement
            .query_map([], |row| {
                Ok(format!(
                    "{} {} 条",
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?
                ))
            })
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default();
        if !plugins.is_empty() {
            parts.push(format!(
                "插件键值（{}），新 im 没有对应的，要重做",
                plugins.join("、")
            ));
        }
    }
    for (table, name) in [
        ("platform_meme_refs", "表情引用"),
        ("platform_session_bindings", "会话绑定"),
    ] {
        if let Some(n) = count(&db, &format!("SELECT COUNT(*) FROM {table}")).filter(|n| *n > 0) {
            parts.push(format!("{name} {n} 条"));
        }
    }
    (!parts.is_empty()).then(|| Item {
        what: "platform-tables".to_string(),
        status: Status::Archived,
        source: format!("{}（platform_plugin_kv、platform_meme_refs、platform_session_bindings）", relative(&db, &copy.root)),
        format: "platform_plugin_kv：(plugin_id, platform, account_id, conversation_kind, conversation_id, key) → value_json；platform_meme_refs：哪条消息用了哪张表情；platform_session_bindings：QQ 会话绑在哪个会话（会话那一类已经照它导了）".to_string(),
        migrated: 0,
        skipped: Vec::new(),
        target: None,
        note: Some(format!("{}。", parts.join("；"))),
    })
}

/// 记忆的经历、日记、改动、旧向量：每个人格数一数。
fn memory(copy: &Layout) -> Option<Item> {
    let dir = copy.personas();
    let mut scopes: Vec<String> = fs::read_dir(&dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("memory/memory.db").is_file())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();
    scopes.sort();
    let mut parts = Vec::new();
    for scope in &scopes {
        let db = dir.join(scope).join("memory/memory.db");
        let mut counts = Vec::new();
        for (sql, name) in [
            (
                "SELECT COUNT(*) FROM episodes WHERE retention = 'short_term'",
                "逐轮经历",
            ),
            (
                "SELECT COUNT(*) FROM episodes WHERE retention != 'short_term'",
                "日记",
            ),
            ("SELECT COUNT(*) FROM memory_revisions", "改动记录"),
            ("SELECT COUNT(*) FROM memory_embeddings", "旧向量"),
        ] {
            if let Some(n) = count(&db, sql).filter(|n| *n > 0) {
                counts.push(format!("{name} {n}"));
            }
        }
        if !counts.is_empty() {
            parts.push(format!("`{scope}`：{}", counts.join("、")));
        }
    }
    (!parts.is_empty()).then(|| Item {
        what: "memory-rest".to_string(),
        status: Status::Archived,
        source: format!("{}/<人格>/memory/memory.db", relative(&dir, &copy.root)),
        format: "episodes（retention short_term 是逐轮经历、long_term 是日记；content、created_at、origin_*）、memory_revisions、memory_embeddings、pending_events、skill_records；知识点 facts 记忆那一类已经导了".to_string(),
        migrated: 0,
        skipped: Vec::new(),
        target: None,
        note: Some(format!("{}。", parts.join("；"))),
    })
}

/// 数据根外面旧版装过的：在的才报；`~/.local/bin/miyu` 是新版包装脚本（里面写着 miyu-remake）的照实说。
fn outside(home: &Path) -> Vec<Item> {
    OUTSIDE
        .iter()
        .filter(|path| home.join(path).exists())
        .map(|path| {
            let full = home.join(path);
            let newer = *path == ".local/bin/miyu"
                && fs::read(&full)
                    .is_ok_and(|bytes| String::from_utf8_lossy(&bytes).contains("miyu-remake"));
            let note = if newer {
                "这是新版的包装脚本，不是旧版的，别删。".to_string()
            } else {
                "旧版装过这个，确认是旧版的、用不着了再删；删不删由你定。".to_string()
            };
            Item {
                what: format!("outside/{path}"),
                status: Status::Skipped,
                source: format!("~/{path}"),
                format: "数据根外面的文件，不在压缩包里".to_string(),
                migrated: 0,
                skipped: Vec::new(),
                target: None,
                note: Some(note),
            }
        })
        .collect()
}

fn item(
    what: &str,
    found: &[PathBuf],
    root: &Path,
    format: &str,
    status: Status,
    note: String,
) -> Item {
    Item {
        what: what.to_string(),
        status,
        source: found
            .iter()
            .map(|path| relative(path, root))
            .collect::<Vec<_>>()
            .join("、"),
        format: format.to_string(),
        migrated: 0,
        skipped: Vec::new(),
        target: None,
        note: Some(note),
    }
}

fn relative(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// 字节数写成给人看的（KB、MB）。
pub fn human_size(bytes: u64) -> String {
    match bytes {
        0..1024 => format!("{bytes} 字节"),
        1024..1_048_576 => format!("{:.1} KB", bytes as f64 / 1024.0),
        _ => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
    }
}

#[cfg(test)]
mod tests;

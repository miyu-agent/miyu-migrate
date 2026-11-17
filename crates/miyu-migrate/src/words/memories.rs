//! 记忆（`memories`）的字：旧格式一句话、每个人格导了多少、整理的回执。

use super::Language;

/// 记忆的字。
pub struct Memories {
    /// 旧格式一句话。
    pub format: &'static str,
    /// 记忆库在哪（人格目录的相对路径）。
    pub source: fn(&str) -> String,
    /// 一个人格的库读不了：哪个、系统的原话。
    pub unreadable: fn(&str, &str) -> String,
    /// 一个人格没映射上：哪个、几条。
    pub unmapped: fn(&str, usize) -> String,
    /// 旧根里没有记忆库。
    pub none: &'static str,
    /// 没找到主人的号。
    pub no_owner: &'static str,
    /// 一个人格导完：旧人格、新人格、导进几条、以前导过几条。
    pub imported: fn(&str, &str, u64, u64) -> String,
    /// 接着：没导的（照原因数的）。
    pub not_imported: fn(&str) -> String,
    /// 接着：没写是谁、照推荐导的。
    pub guessed: fn(&str) -> String,
    /// 接着：号换不出、记成名字的几条。
    pub by_name: fn(u64) -> String,
    /// 接着：整理的回执，句号收尾。
    pub dream: fn(&str) -> String,
    /// 核心整批拒了，没整理：拒的代码。
    pub refused: fn(&str) -> String,
    /// 整理了：交进、作废、改了几条。
    pub dreamed: fn(u64, u64, u64) -> String,
    /// 整理没成：拒的代码。
    pub dream_failed: fn(&str) -> String,
    /// 整理等了五分钟还在跑。
    pub dream_busy: &'static str,
}

static ZH: Memories = Memories {
    format: "SQLite（没有 user_version）：facts 一行一条（content、memory_type fact/preference/relationship/task/self/other、status、updated_at、tags JSON 列表、subjects JSON [{name, principal}]、owner_principal），episodes 日记（owner_principal、origin_platform、origin_sender_id 换得出 QQ 号）、memory_revisions、memory_embeddings 只在压缩包里",
    source: |dir| format!("{dir}/<人格>/memory/memory.db"),
    unreadable: |scope, error| format!("`{scope}` 的库读不了（{error}），留在压缩包里。"),
    unmapped: |scope, n| format!("`{scope}` 没映射上新人格，{n} 条没导。"),
    none: "旧根里没有记忆库。",
    no_owner: "没找到主人的号（新版 `[external.bindings]`、旧版 `platforms.qq.admin_users` 都没有）：说的是谁的一律当别人。",
    imported: |scope, persona, imported, before| {
        format!("`{scope}` → `{persona}`：导进 {imported} 条，以前导过 {before} 条")
    },
    not_imported: |counts| format!("，没导 {counts}"),
    guessed: |counts| format!("；没写是谁、照推荐导的（建议→导成）{counts}"),
    by_name: |n| format!("；{n} 条号换不出、记成名字"),
    dream: |said| format!("；{said}。"),
    refused: |code| format!("核心拒了（{code}），没整理"),
    dreamed: |given, retired, revised| {
        format!("整理了：交进 {given}、作废 {retired}、改了 {revised}")
    },
    dream_failed: |code| format!("整理没成（{code}），以后照整理的间隔会再整理"),
    dream_busy: "整理还在跑（等了五分钟），以后照整理的间隔会再整理",
};

static EN: Memories = Memories {
    format: "SQLite (no user_version): facts, one row per fact (content, memory_type fact/preference/relationship/task/self/other, status, updated_at, tags a JSON list, subjects JSON [{name, principal}], owner_principal); episodes as diary (owner_principal, origin_platform, origin_sender_id gives the QQ number), memory_revisions and memory_embeddings are only in the archive",
    source: |dir| format!("{dir}/<persona>/memory/memory.db"),
    unreadable: |scope, error| {
        format!("Cannot read the database of `{scope}` ({error}); it stays in the archive.")
    },
    unmapped: |scope, n| {
        format!("`{scope}` was not mapped to a new persona; {n} fact(s) not imported.")
    },
    none: "No memory database in the old root.",
    no_owner: "The owner's number was not found (neither the new `[external.bindings]` nor the old `platforms.qq.admin_users`), so every fact about someone is treated as about another person.",
    imported: |scope, persona, imported, before| {
        format!("`{scope}` → `{persona}`: {imported} imported, {before} imported before")
    },
    not_imported: |counts| format!(", not imported {counts}"),
    guessed: |counts| {
        format!("; no subject given, imported as suggested (suggested→imported) {counts}")
    },
    by_name: |n| format!("; {n} with no number to map, recorded by name"),
    dream: |said| format!("; {said}."),
    refused: |code| format!("the core refused ({code}), not consolidated"),
    dreamed: |given, retired, revised| {
        format!("consolidated: {given} given, {retired} retired, {revised} revised")
    },
    dream_failed: |code| {
        format!("consolidation failed ({code}); it will run again at the usual interval")
    },
    dream_busy: "consolidation still running after five minutes; it will run again at the usual interval",
};

/// 照语言挑记忆的字。
#[must_use]
pub fn words(language: Language) -> &'static Memories {
    language.pick(&ZH, &EN)
}

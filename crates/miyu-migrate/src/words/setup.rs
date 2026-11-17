//! 配置（`config`）、赞助记录（`sponsors`）的字：旧格式一句话、写了多少、缺了哪个包或者太旧。

use super::Language;

/// 配置的字。
pub struct Settings {
    /// 旧格式一句话。
    pub format: &'static str,
    /// 迁到哪。
    pub target: &'static str,
    /// 旧根里没有读得懂的配置。
    pub none: &'static str,
    /// 写了几项设置、几个密钥。
    pub written: fn(usize, usize) -> String,
    /// 改了编号的供应商（并起来的）。
    pub renamed: fn(&str) -> String,
    /// 读回来不一样的几项。
    pub differ: fn(usize) -> String,
}

/// 赞助记录的字。
pub struct Sponsors {
    /// 旧格式一句话。
    pub format: &'static str,
    /// 在旧根的哪里（会话库的相对路径）。
    pub source: fn(&str) -> String,
    /// 旧版没有。
    pub none: &'static str,
    /// 插件没装：几笔、插件。
    pub not_installed: fn(usize, &str) -> String,
    /// 导了：几笔、新加几笔、本来就有几笔。
    pub imported: fn(usize, u64, u64) -> String,
    /// 对方太旧：几笔、怎么回事、拒的代码、升级哪一个。
    pub outdated: fn(usize, &str, &str, &str) -> String,
    /// 插件拒了：几笔、拒的代码、原话。
    pub refused: fn(usize, &str, &str) -> String,
    /// 总线上调不到插件的导入：总线上的名字。
    pub no_bus_name: fn(&str) -> String,
    /// 升级插件：插件。
    pub upgrade_plugin: fn(&str) -> String,
    /// 核心不认 `bus.call`。
    pub old_core: &'static str,
    /// 升级 Miyu。
    pub upgrade_core: &'static str,
}

static SETTINGS_ZH: Settings = Settings {
    format: "JSONC（去掉注释照 JSON 读，config_version 6）：providers[] 供应商（id、display_name、base_url、protocol、api_key 可多个、models、model_context_window、model_temperature、model_costs、default_model），active_provider，embedding，model_tiers，plugins.vision/web，platforms.qq.admin_users；属主档案在 profile.md",
    target: "config.set、secret.set",
    none: "旧根里没有读得懂的配置。",
    written: |settings, secrets| {
        format!("写了 {settings} 项设置、{secrets} 个密钥（报告只写名字）。")
    },
    renamed: |listed| format!("供应商编号照新版的写法改了：{listed}。"),
    differ: |n| format!("{n} 项读回来和写的不一样，照 `miyu config get` 看一眼。"),
};

static SETTINGS_EN: Settings = Settings {
    format: "JSONC (read as JSON without comments, config_version 6): providers[] (id, display_name, base_url, protocol, api_key possibly several, models, model_context_window, model_temperature, model_costs, default_model), active_provider, embedding, model_tiers, plugins.vision/web, platforms.qq.admin_users; the owner profile is in profile.md",
    target: "config.set, secret.set",
    none: "No readable config in the old root.",
    written: |settings, secrets| {
        format!("Wrote {settings} setting(s) and {secrets} secret(s) (the report only names them).")
    },
    renamed: |listed| format!("Provider ids were changed to the new version's form: {listed}."),
    differ: |n| {
        format!(
            "{n} setting(s) read back differently from what was written; check them with `miyu config get`."
        )
    },
};

static SPONSORS_ZH: Sponsors = Sponsors {
    format: "会话库（SQLite v32 起）的 sponsor_records：platform（onebot）、account_id、sponsor_id（QQ 号或名字）、sponsor_name、amount_minor（分）、currency、cny_minor、fx_rate（同币种写 0）、note、recorded_by、sponsored_at（RFC 3339）",
    source: |db| format!("{db}（sponsor_records）"),
    none: "旧版没有赞助记录。",
    not_installed: |n, plugin| {
        format!("{n} 笔赞助记录：赞助插件 `{plugin}` 没装，装上以后重跑迁移（重跑不会多出来）。")
    },
    imported: |n, added, skipped| {
        format!("{n} 笔赞助记录：新加 {added} 笔，{skipped} 笔账本里本来就有（以前导过、记过的）。")
    },
    outdated: |n, why, code, upgrade| {
        format!(
            "{n} 笔赞助记录：{why}（{code}），留在压缩包里；{upgrade}以后重跑迁移（重跑不会多出来）。"
        )
    },
    refused: |n, code, message| {
        format!("{n} 笔赞助记录没导进去（{code}：{message}），留在压缩包里。")
    },
    no_bus_name: |name| format!("总线上调不到赞助插件的 `{name}.import`，装着的插件太旧或者停用了"),
    upgrade_plugin: |plugin| format!("把赞助插件 `{plugin}` 升级（停用了的先启用）"),
    old_core: "这一版 Miyu 不认 `bus.call`，太旧",
    upgrade_core: "把 Miyu 升级",
};

static SPONSORS_EN: Sponsors = Sponsors {
    format: "sponsor_records in the session database (SQLite v32 and later): platform (onebot), account_id, sponsor_id (QQ number or name), sponsor_name, amount_minor (cents), currency, cny_minor, fx_rate (0 for the same currency), note, recorded_by, sponsored_at (RFC 3339)",
    source: |db| format!("{db} (sponsor_records)"),
    none: "The old version has no sponsor records.",
    not_installed: |n, plugin| {
        format!(
            "{n} sponsor record(s): the sponsor plugin `{plugin}` is not installed. Install it and run the migration again (that will not add duplicates)."
        )
    },
    imported: |n, added, skipped| {
        format!(
            "{n} sponsor record(s): {added} added, {skipped} already in the ledger (imported or recorded before)."
        )
    },
    outdated: |n, why, code, upgrade| {
        format!(
            "{n} sponsor record(s): {why} ({code}); they stay in the archive. {upgrade}, then run the migration again (that will not add duplicates)."
        )
    },
    refused: |n, code, message| {
        format!(
            "{n} sponsor record(s) were not imported ({code}: {message}); they stay in the archive."
        )
    },
    no_bus_name: |name| {
        format!(
            "`{name}.import` of the sponsor plugin cannot be called over the bus; the installed plugin is too old or turned off"
        )
    },
    upgrade_plugin: |plugin| {
        format!("Upgrade the sponsor plugin `{plugin}` (turn it on first if it is off)")
    },
    old_core: "this Miyu does not know `bus.call`; it is too old",
    upgrade_core: "Upgrade Miyu",
};

/// 照语言挑配置的字。
#[must_use]
pub fn settings(language: Language) -> &'static Settings {
    language.pick(&SETTINGS_ZH, &SETTINGS_EN)
}

/// 照语言挑赞助记录的字。
#[must_use]
pub fn sponsors(language: Language) -> &'static Sponsors {
    language.pick(&SPONSORS_ZH, &SPONSORS_EN)
}

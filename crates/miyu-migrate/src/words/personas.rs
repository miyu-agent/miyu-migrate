//! 压缩包（`archive`）、人格（`personas`）和人格那几样只报不迁的字：旧格式一句话、人格表里那一格的备注。

use super::Language;

/// 压缩包和人格的字。
pub struct Personas {
    /// 压缩包是什么。
    pub archive_format: &'static str,
    /// 几样不是普通文件、目录、链接，没打进去。
    pub archive_unpacked: fn(usize) -> String,
    /// 人格的旧格式。
    pub format: &'static str,
    /// `persona.toml`。
    pub settings: &'static str,
    /// 开发模式的提示词。
    pub dev_prompt: &'static str,
    /// 没选人格时的系统提示词。
    pub system_prompt: &'static str,
    /// 头像不在了。
    pub avatar: &'static str,
    /// 出厂的软件工程师被卸了：它的编号。
    pub no_engineer: fn(&str) -> String,
    /// `persona.set` 没交回编号。
    pub no_id: &'static str,
    /// 头像没传上：核心的原话。
    pub avatar_failed: fn(&str) -> String,
}

static ZH: Personas = Personas {
    archive_format: "旧数据根整个打成 tar.zst，逐文件 SHA-256 记在清单里",
    archive_unpacked: |n| format!("{n} 样不是普通文件、目录、链接，没打进去。"),
    format: "<名字>.md 是人设；hints/<scope>.md 角色扮演提示、dialogs/<scope>.md 示范对话（行首 user:/assistant:），空文件是停用；<名字>.json 的 avatar_path 是头像",
    settings: "TOML：[subsystems] memory/persona_reminder/voice/emotion，[plugins] enabled/scripts/skills/mcp 白名单；新版没有对应",
    dev_prompt: "开发模式的提示词（Markdown）；新版的软件工程师是出厂的，不改",
    system_prompt: "旧版没选人格时的自定义系统提示词（Markdown）；新版没有对应",
    avatar: "网页附带的 avatar_path 指的头像在旧根以外或者不在了",
    no_engineer: |engineer| format!("新版没有 {engineer}（出厂的包被卸了）"),
    no_id: "persona.set 没交回编号",
    avatar_failed: |said| format!("头像没传上：{said}"),
};

static EN: Personas = Personas {
    archive_format: "the whole old data root packed as tar.zst, with each file's SHA-256 in the manifest",
    archive_unpacked: |n| {
        format!("{n} item(s) were not regular files, folders or links and were not packed.")
    },
    format: "<name>.md is the persona; hints/<scope>.md roleplay hints, dialogs/<scope>.md example dialogs (lines start with user:/assistant:), an empty file means turned off; avatar_path in <name>.json is the avatar",
    settings: "TOML: [subsystems] memory/persona_reminder/voice/emotion, [plugins] enabled/scripts/skills/mcp allow lists; nothing matches in the new version",
    dev_prompt: "the developer mode prompt (Markdown); the new software engineer is built in and not changed",
    system_prompt: "the custom system prompt the old version used with no persona selected (Markdown); nothing matches in the new version",
    avatar: "the avatar that avatar_path from the web page points to is outside the old root or gone",
    no_engineer: |engineer| {
        format!("the new version has no {engineer} (its built-in package was uninstalled)")
    },
    no_id: "persona.set returned no id",
    avatar_failed: |said| format!("avatar upload failed: {said}"),
};

/// 照语言挑压缩包和人格的字。
#[must_use]
pub fn words(language: Language) -> &'static Personas {
    language.pick(&ZH, &EN)
}

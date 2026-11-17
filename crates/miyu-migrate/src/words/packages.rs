//! 技能、脚本、包（`skills`、`scripts`、`packages`）、登录态（`logins`）、表情包（`memes`）的字：都是「装上哪个包再迁」这一路的。

use super::Language;

/// 技能、脚本、包的字。
pub struct Skills {
    /// 技能的旧格式。
    pub format: &'static str,
    /// 技能迁到哪。
    pub target: &'static str,
    /// 改了名的技能：原来在哪、新名字。
    pub renamed: fn(&str, &str) -> String,
    /// 原来是哪个人格的：名字、人格。
    pub persona_owned: fn(&str, &str) -> String,
    /// 原来停用了的：名字。
    pub disabled: fn(&str) -> String,
    /// 有新包的脚本（并起来的）。
    pub ported: fn(&str) -> String,
    /// 新版格式不一样的自己的脚本：几个、给 agent 的那份叫什么。
    pub scripts_left: fn(usize, &str) -> String,
    /// 脚本的旧格式。
    pub scripts_format: &'static str,
    /// 包没有对应的：包名。
    pub no_package: fn(&str) -> String,
    /// 包管理器的旧格式。
    pub packages_format: &'static str,
    /// 包迁到哪。
    pub packages_target: &'static str,
    /// 包那一句（并起来的）。
    pub packages: fn(&str) -> String,
}

/// 登录态的字。
pub struct Logins {
    /// 在旧根的哪里。
    pub source: &'static str,
    /// 旧格式一句话。
    pub format: &'static str,
    /// 闲鱼的拷了：几个文件、包编号。
    pub goofish_copied: fn(u64, &str) -> String,
    /// 闲鱼的包没装：几个文件、包编号。
    pub goofish_missing: fn(usize, &str) -> String,
    /// B 站开播的有新包：包编号。
    pub live: fn(&str) -> String,
    /// 还没有包的几样（并起来的）。
    pub others: fn(&str) -> String,
    /// 旧根里没有。
    pub none: &'static str,
}

/// 表情包的字。
pub struct Memes {
    /// 旧格式一句话。
    pub format: &'static str,
    /// 在旧根的哪里（表情包目录的相对路径）。
    pub source: fn(&str) -> String,
    /// 迁到哪。
    pub target: &'static str,
    /// 旧根里没有。
    pub none: &'static str,
    /// 软件没装：包编号、几条、软件、装上以后跑的命令。
    pub not_installed: fn(&str, u64, &str, &str) -> String,
    /// 装上了：包编号、几条。
    pub installed: fn(&str, u64) -> String,
    /// 没装上：包编号、拒的代码、改好以后跑的命令。
    pub failed: fn(&str, &str, &str) -> String,
}

static SKILLS_ZH: Skills = Skills {
    format: "一个技能一个目录：SKILL.md（YAML 头 name、description）加资源；personas/<scope>/<名字>/ 是人格层；.disabled 是停用",
    target: "root/skills/<名字>/",
    renamed: |shown, name| format!("`{shown}` 新版里叫 `{name}`。"),
    persona_owned: |name, scope| {
        format!("`{name}` 原来是人格 `{scope}` 的（新版的技能不分人格）。")
    },
    disabled: |name| format!("`{name}` 原来停用了，新版里是开着的，不要的在设置里关掉。"),
    ported: |listed| format!("有新包的到官方源装：{listed}。"),
    scripts_left: |n, agent| {
        format!(
            "{n} 个自己写的脚本新版的格式不一样，原样留在压缩包里；要的时候照新版的写法（`docs/authors/scripts-skills.md`）改，或者让 agent 照 `{agent}` 改。"
        )
    },
    scripts_format: "一个脚本一个文件，头部注释写说明和参数，index.json 是覆盖层（id、path、parameters、load_policy……）；personas/<scope>/ 是人格层",
    no_package: |name| format!("`{name}` 新版没有对应的包"),
    packages_format: "JSON：{packages: {<包名>: {source, version, kind extension/persona, files [相对旧根的路径]}}}",
    packages_target: "官方源",
    packages: |listed| format!("旧版包管理器装的包不拷，到官方源装新的：{listed}。"),
};

static SKILLS_EN: Skills = Skills {
    format: "one folder per skill: SKILL.md (YAML header name, description) plus resources; personas/<scope>/<name>/ is the persona layer; .disabled means turned off",
    target: "root/skills/<name>/",
    renamed: |shown, name| format!("`{shown}` is called `{name}` in the new version."),
    persona_owned: |name, scope| {
        format!(
            "`{name}` belonged to persona `{scope}` (skills in the new version are not per persona)."
        )
    },
    disabled: |name| {
        format!(
            "`{name}` was turned off; it is on in the new version, so turn it off in settings if you do not want it."
        )
    },
    ported: |listed| {
        format!("Scripts with a new package; install it from the official source: {listed}.")
    },
    scripts_left: |n, agent| {
        format!(
            "{n} script(s) of your own use a format the new version does not, and stay in the archive as they were. When you need them, rewrite them the new way (`docs/authors/scripts-skills.md`), or have the agent do it from `{agent}`."
        )
    },
    scripts_format: "one file per script, a header comment with the description and parameters, index.json the override layer (id, path, parameters, load_policy ...); personas/<scope>/ is the persona layer",
    no_package: |name| format!("`{name}` has no package in the new version"),
    packages_format: "JSON: {packages: {<package>: {source, version, kind extension/persona, files [paths relative to the old root]}}}",
    packages_target: "official package source",
    packages: |listed| {
        format!(
            "Packages installed by the old package manager are not copied; install the new ones from the official source: {listed}."
        )
    },
};

static LOGINS_ZH: Logins = Logins {
    source: "cache/<工具>/",
    format: "goofish-search：session.json（Playwright storage_state）、<md5 前 12 位>.json（监控票据）；zhihu-search、xhs-search：Chromium 的 profile；bilibili：cookie",
    goofish_copied: |n, package| {
        format!(
            "闲鱼的登录态 {n} 个文件拷进了 `root/state/scripts/{package}/goofish/legacy/`；新包认不认旧版的格式没核实，要是还得重新登录，照新包的说明登一次。"
        )
    },
    goofish_missing: |n, package| {
        format!(
            "闲鱼的登录态 {n} 个文件：`{package}` 没装，装上以后重跑迁移，或者在新包里重新登录。"
        )
    },
    live: |package| {
        format!(
            "B 站开播的登录态有新包 `{package}`，旧版的格式没核实、没拷：装上新包以后在里面重新登录一次。"
        )
    },
    others: |listed| {
        format!("{listed} 新版还没有对应的包，登录态留在压缩包里（`cache/` 下），以后有了包再迁。")
    },
    none: "旧根里没有搜索脚本的登录态。",
};

static LOGINS_EN: Logins = Logins {
    source: "cache/<tool>/",
    format: "goofish-search: session.json (Playwright storage_state), <first 12 of md5>.json (watch tickets); zhihu-search, xhs-search: Chromium profiles; bilibili: cookies",
    goofish_copied: |n, package| {
        format!(
            "Copied {n} Goofish login file(s) into `root/state/scripts/{package}/goofish/legacy/`. Whether the new package reads the old format is not checked; if you still have to log in again, follow the new package's instructions once."
        )
    },
    goofish_missing: |n, package| {
        format!(
            "{n} Goofish login file(s): `{package}` is not installed. Install it and run the migration again, or log in again in the new package."
        )
    },
    live: |package| {
        format!(
            "Bilibili live streaming logins have a new package, `{package}`. The old format is not checked and was not copied: after installing the new package, log in there once."
        )
    },
    others: |listed| {
        format!(
            "{listed}: no package in the new version yet. The logins stay in the archive (under `cache/`) until there is one."
        )
    },
    none: "No search script logins in the old root.",
};

static MEMES_ZH: Memes = Memes {
    format: "data/memes/<库>/index.json（{library, version 2, memes:[{id sha256:…, name {zh,en}, file images/…, mime_type, animated, description, usage, tags, origin {platform, conversation_kind, conversation_id, sender_id, sender_name, message_id, sent_at, collected_at}}], disabled_ids}）加 images/ 下的图",
    source: |dir| format!("{dir}/<库>"),
    target: "package.install（memes 的内容包）",
    none: "旧根里没有自己的表情包库。",
    not_installed: |id, n, memes, line| {
        format!("`{id}` {n} 条：{memes} 没装，装上以后跑 `{line}`。")
    },
    installed: |id, n| format!("`{id}` 装上了，{n} 条。"),
    failed: |id, code, line| format!("`{id}` 没装上（{code}），改好以后跑 `{line}`。"),
};

static MEMES_EN: Memes = Memes {
    format: "data/memes/<library>/index.json ({library, version 2, memes:[{id sha256:…, name {zh,en}, file images/…, mime_type, animated, description, usage, tags, origin {platform, conversation_kind, conversation_id, sender_id, sender_name, message_id, sent_at, collected_at}}], disabled_ids}) plus the pictures under images/",
    source: |dir| format!("{dir}/<library>"),
    target: "package.install (a memes content package)",
    none: "No sticker libraries of your own in the old root.",
    not_installed: |id, n, memes, line| {
        format!(
            "`{id}`, {n} sticker(s): {memes} is not installed. After installing it, run `{line}`."
        )
    },
    installed: |id, n| format!("Installed `{id}`, {n} sticker(s)."),
    failed: |id, code, line| {
        format!("`{id}` was not installed ({code}). After fixing that, run `{line}`.")
    },
};

/// 照语言挑技能、脚本、包的字。
#[must_use]
pub fn skills(language: Language) -> &'static Skills {
    language.pick(&SKILLS_ZH, &SKILLS_EN)
}

/// 照语言挑登录态的字。
#[must_use]
pub fn logins(language: Language) -> &'static Logins {
    language.pick(&LOGINS_ZH, &LOGINS_EN)
}

/// 照语言挑表情包的字。
#[must_use]
pub fn memes(language: Language) -> &'static Memes {
    language.pick(&MEMES_ZH, &MEMES_EN)
}

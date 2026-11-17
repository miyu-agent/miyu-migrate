//! 给人看的字（施工 MG-21）：程序的输出、给人看的报告、报告的文件名，照语言给中文或英文。字都在这一层的小字表里，一张表一个
//! 模块（`cli` 帮助和报错、`report` 报告的框架、别的是各类迁完的那一句），每张表一份中文 `ZH`、一份英文 `EN`，用的地方拿
//! [`Language`] 挑一张。给 agent 读的那份的结构不变；会话、记忆、表情包这些写进新版的数据不跟着语言变。
//!
//! 语言：连上核心以后照握手回的 `language`（核心照界面语言、头报的系统语言算，`zh`、`en`、`ja`）；连上以前（帮助、连不上的
//! 报错）照系统语言（`LC_ALL`、`LC_MESSAGES`、`LANG`）。`zh` 开头的是中文，别的（日文也是）英文：包里没有日文字表。

pub mod cli;
pub mod files;
pub mod leftovers;
pub mod memories;
pub mod packages;
pub mod personas;
pub mod report;
pub mod sessions;
pub mod setup;

/// 给人看的字用哪种语言。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    /// 中文。
    #[default]
    Zh,
    /// 英文。
    En,
}

impl Language {
    /// 照一个语言代码或者区域设置（`zh`、`zh_CN.UTF-8`、`en_US.UTF-8`、`ja`、`C`）认：`zh` 开头的是中文，别的英文。
    #[must_use]
    pub fn from_code(code: &str) -> Language {
        if code.trim().to_ascii_lowercase().starts_with("zh") {
            Language::Zh
        } else {
            Language::En
        }
    }

    /// 系统语言：`LC_ALL`、`LC_MESSAGES`、`LANG` 头一个不空的（同核心握手报的 `locale`）；都没有的是英文。
    #[must_use]
    pub fn system() -> Language {
        Language::from_code(&miyu_client::open::locale())
    }

    /// 核心握手回的 `language`；没回的照系统语言。
    #[must_use]
    pub fn from_core(language: Option<&str>) -> Language {
        language.map_or_else(Language::system, Language::from_code)
    }

    /// 照语言挑一张字表。
    #[must_use]
    pub fn pick<T: ?Sized>(self, zh: &'static T, en: &'static T) -> &'static T {
        match self {
            Language::Zh => zh,
            Language::En => en,
        }
    }

    /// 几句并成一段：中文直接接上，英文中间空一格。
    #[must_use]
    pub fn sentences(self, parts: &[String]) -> String {
        parts.join(self.pick("", " "))
    }

    /// 几样列成一串：中文用「、」，英文用逗号。
    #[must_use]
    pub fn list<S: AsRef<str>>(self, items: &[S]) -> String {
        let items: Vec<&str> = items.iter().map(AsRef::as_ref).collect();
        items.join(self.pick("、", ", "))
    }

    /// 一句话的分句号：中文「；」，英文「; 」。
    #[must_use]
    pub fn semicolon(self) -> &'static str {
        self.pick("；", "; ")
    }

    /// 字节数写成给人看的（KB、MB）。
    #[must_use]
    pub fn human_size(self, bytes: u64) -> String {
        match bytes {
            0..1024 => format!("{bytes}{}", self.pick(" 字节", " bytes")),
            1024..1_048_576 => format!("{:.1} KB", bytes as f64 / 1024.0),
            _ => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
        }
    }
}

#[cfg(test)]
mod tests;

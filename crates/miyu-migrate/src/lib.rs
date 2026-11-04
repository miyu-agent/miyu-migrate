//! 从旧版 Miyu（0.7.x）迁到新版（设计见仓库 `docs/`）：先把旧目录打成压缩包、核对，再从解出来的副本逐类读、经核心的协议写进
//! 新版，最后出两份报告。迁不了的不硬迁，留在压缩包里、报告写明在哪。

pub mod archive;
pub mod core;
pub mod files;
pub mod jsonc;
pub mod knowledge;
pub mod layout;
pub mod lock;
pub mod logins;
pub mod memes;
pub mod memories;
pub mod personas;
pub mod records;
pub mod report;
pub mod run;
pub mod sessions;
pub mod settings;
pub mod skills;
pub mod sponsors;

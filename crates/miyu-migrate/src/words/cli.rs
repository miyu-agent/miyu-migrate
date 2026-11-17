//! 终端上的字：帮助、跑完印的两行、报错（`miyu migrate: ` 后面跟一句，英文小写开头、不带句号，中文照 GNU 中文翻译的写法）。

use std::path::Path;

use super::Language;

/// 终端上的字。
pub struct Cli {
    /// `miyu migrate --help`。
    pub help: &'static str,
    /// 不是经 `miyu migrate` 转交的（没有状态目录）。
    pub no_state_dir: &'static str,
    /// 连不上核心：核心（或连接）的原话。
    pub cannot_reach: fn(&str) -> String,
    /// 跑完印的两行：给人看的报告、给 agent 的。
    pub done: fn(&Path, &Path) -> String,
    /// 旧版还在跑，知道进程号的。
    pub still_running_pid: fn(u32) -> String,
    /// 旧版还在跑。
    pub still_running: &'static str,
    /// 这一次的目录已经有了。
    pub exists: fn(&Path) -> String,
    /// 盘不够：哪个目录、要几 MiB、有几 MiB。
    pub no_space: fn(&Path, u64, u64) -> String,
    /// 核心拒了：它的代码和原话。
    pub core_said: fn(&str) -> String,
    /// 旧目录不是目录。
    pub not_a_directory: fn(&Path) -> String,
    /// 不是旧版的数据目录。
    pub not_old: fn(&Path) -> String,
    /// 旧版搬家没收尾：搬家日志在哪。
    pub unfinished: fn(&Path) -> String,
    /// `.home-layout-v1` 里的用户名不对。
    pub bad_admin: fn(&str) -> String,
    /// 读不了：哪里、系统的原话。
    pub cannot_read: fn(&Path, &str) -> String,
    /// 解出来的和打进去的对不上：几个。
    pub mismatch: fn(usize) -> String,
}

static ZH: Cli = Cli {
    help: "用法：miyu migrate <旧目录>

把旧版 Miyu（0.7.x）的数据目录迁进这一版。先停掉旧版，再把旧目录挪开（例如 mv ~/.miyu ~/.miyu-0.7），然后跑这一条。
旧目录会整个打成压缩包留着；迁完看报告，确认没问题以后再删旧目录。
",
    no_state_dir: "没有设置 MIYU_STATE_DIR；请用 `miyu migrate <旧目录>` 运行",
    cannot_reach: |said| format!("无法连接核心：{said}"),
    done: |human, agent| {
        format!(
            "报告：{}\n给 agent 的：{}",
            human.display(),
            agent.display()
        )
    },
    still_running_pid: |pid| format!("旧版 Miyu 还在运行（进程号 {pid}），请先停止它"),
    still_running: "旧版 Miyu 还在运行，请先停止它",
    exists: |dir| format!("{} 已存在；请等一秒再运行", dir.display()),
    no_space: |at, need, have| {
        format!(
            "{} 所在磁盘空间不足，放不下压缩包和核对用的副本：需要 {need} MiB，可用 {have} MiB",
            at.display()
        )
    },
    core_said: |said| format!("核心拒绝了：{said}"),
    not_a_directory: |path| format!("{} 不是目录", path.display()),
    not_old: |path| format!("{} 不是旧版 Miyu 的数据目录", path.display()),
    unfinished: |path| {
        format!(
            "旧版 Miyu 没有搬完文件（{}）；请先启动一次旧版，让它自行恢复",
            path.display()
        )
    },
    bad_admin: |name| format!("家目录布局标记里的用户名无效：{name:?}"),
    cannot_read: |path, error| format!("无法读取 {}：{error}", path.display()),
    mismatch: |n| format!("解包后有 {n} 个文件与打包时不符"),
};

static EN: Cli = Cli {
    help: "Usage: miyu migrate <old-folder>

Migrate the data folder of old Miyu (0.7.x) into this version. Stop the old version first, move its folder aside
(for example: mv ~/.miyu ~/.miyu-0.7), then run this command.
The whole old folder is kept as a compressed archive. Read the report when it finishes, and delete the old folder
only after you have checked it.
",
    no_state_dir: "MIYU_STATE_DIR is not set; run this as `miyu migrate <old-folder>`",
    cannot_reach: |said| format!("cannot reach the core: {said}"),
    done: |human, agent| {
        format!(
            "Report: {}\nFor the agent: {}",
            human.display(),
            agent.display()
        )
    },
    still_running_pid: |pid| format!("old Miyu is still running (pid {pid}); stop it first"),
    still_running: "old Miyu is still running; stop it first",
    exists: |dir| {
        format!(
            "{} already exists; wait a second and run again",
            dir.display()
        )
    },
    no_space: |at, need, have| {
        format!(
            "not enough disk space for the archive and its check under {}: need {need} MiB, have {have} MiB",
            at.display()
        )
    },
    core_said: |said| format!("the core said {said}"),
    not_a_directory: |path| format!("{} is not a directory", path.display()),
    not_old: |path| format!("{} is not an old Miyu data folder", path.display()),
    unfinished: |path| {
        format!(
            "old Miyu did not finish moving its files ({}); start the old version once to let it recover",
            path.display()
        )
    },
    bad_admin: |name| format!("the home layout marker names an invalid user: {name:?}"),
    cannot_read: |path, error| format!("cannot read {}: {error}", path.display()),
    mismatch: |n| format!("{n} file(s) differ after unpacking"),
};

/// 照语言挑终端上的字。
#[must_use]
pub fn words(language: Language) -> &'static Cli {
    language.pick(&ZH, &EN)
}

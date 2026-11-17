//! 给人看的报告的框架：标题、每节的标题和那几句、状态、两份报告和留给人的目录叫什么。各类自己的那一句在各自的字表里。

use super::Language;

/// 报告的字。
pub struct Report {
    /// 给人看的那份的文件名。
    pub human_file: &'static str,
    /// 给 agent 读的那份的文件名。
    pub agent_file: &'static str,
    /// 留给人的东西放在新数据根 `root/documents/` 下的哪个目录。
    pub keep_dir: &'static str,
    /// 开头：迁移时刻、旧目录、布局。
    pub intro: fn(&str, &str, &str) -> String,
    /// 压缩包在哪。
    pub archive_intro: fn(&str) -> String,
    /// 人格一节的标题和表头。
    pub personas: &'static str,
    /// 人格怎么来的：新建、同名已有、出厂的软件工程师、没映射上。
    pub how: [&'static str; 4],
    /// 人格那一格后面接的备注。
    pub how_note: fn(&str) -> String,
    /// 「每一类」的标题。
    pub items: &'static str,
    /// 一类的小标题和头一句：哪一类、状态、迁了几条。
    pub item: fn(&str, &str, u64) -> String,
    /// 接着头一句：跳过几条。
    pub skipped: fn(usize) -> String,
    /// 头一句的句号（带换行）。
    pub end: &'static str,
    /// 迁到哪。
    pub target: fn(&str) -> String,
    /// 派生的缓存合成的那一节：几处。
    pub derived: fn(usize) -> String,
    /// 数据根外面的那一节的标题和头一句。
    pub outside: &'static str,
    /// 数据根外面的一行：路径和备注之间。
    pub outside_colon: &'static str,
    /// 没迁的那一节：标题、头一句（给 agent 的那份叫什么）、表头。
    pub left: fn(&str) -> String,
    /// 最后一节「旧数据根」的标题和头一句。
    pub old_root: fn(&str) -> String,
    /// 压缩包：在哪、几个文件、原来多大、哈希、清单在哪。
    pub archive: fn(&str, u64, &str, &str, &str) -> String,
    /// 最后一句：什么时候删旧目录。
    pub delete_later: &'static str,
    /// 状态：迁了、迁了一部分、没迁在压缩包里、不用迁。
    pub status: [&'static str; 4],
}

static ZH: Report = Report {
    human_file: "报告.md",
    agent_file: "给agent.json",
    keep_dir: "旧版 Miyu 迁移",
    intro: |at, path, layout| {
        format!("# 旧版 Miyu 迁移报告\n\n迁移时刻：{at}。旧数据目录：`{path}`（{layout}）。\n\n")
    },
    archive_intro: |path| {
        format!(
            "旧目录整个打成了压缩包：`{path}`。最后一节「旧数据根」写了什么时候可以删旧目录。\n\n"
        )
    },
    personas: "## 人格\n\n| 旧版 | 名字 | 新版 | 怎么来的 |\n|---|---|---|---|\n",
    how: ["新建", "已经有同名的", "出厂的软件工程师", "没映射上"],
    how_note: |note| format!("（{note}）"),
    items: "## 每一类\n\n",
    item: |what, status, n| format!("### {what}\n\n{status}，{n} 条"),
    skipped: |n| format!("，跳过 {n} 条"),
    end: "。\n",
    target: |target| format!("迁到：`{target}`。\n"),
    derived: |n| {
        format!(
            "### 派生的缓存\n\n{n} 处（日志、缓存、知识库的向量这些）不迁，新版自己重建；给 agent 的那份逐条列着。\n\n"
        )
    },
    outside: "## 数据根外面\n\n旧版在数据根外面装过这些，不在压缩包里：\n\n",
    outside_colon: "：",
    left: |agent| {
        format!(
            "## 没迁的\n\n要的时候，让 agent 照同一目录里的 `{agent}` 接着迁。\n\n| 东西 | 在压缩包里 | 旧格式 |\n|---|---|---|\n"
        )
    },
    old_root: |path| format!("## 旧数据根\n\n旧数据目录 `{path}` 迁移一个字节都没动。"),
    archive: |path, files, size, sha256, manifest| {
        format!(
            "完整的压缩包在 `{path}`：{files} 个文件、原来一共 {size}，整个包的 SHA-256 是 `{sha256}`；逐文件的清单在 `{manifest}`。"
        )
    },
    delete_later: "确认都迁过来了、新版用了一阵没问题以后，再删旧数据目录（压缩包留着）；删由你自己来，迁移程序不删。\n",
    status: ["迁了", "迁了一部分", "没迁，在压缩包里", "不用迁"],
};

static EN: Report = Report {
    human_file: "report.md",
    agent_file: "for-agent.json",
    keep_dir: "Old Miyu migration",
    intro: |at, path, layout| {
        format!(
            "# Old Miyu migration report\n\nMigrated at: {at}. Old data folder: `{path}` ({layout}).\n\n"
        )
    },
    archive_intro: |path| {
        format!(
            "The whole old folder was packed into an archive: `{path}`. The last section, \"Old data root\", says when you can delete the old folder.\n\n"
        )
    },
    personas: "## Personas\n\n| Old | Name | New | How |\n|---|---|---|---|\n",
    how: [
        "created",
        "a persona with this name already existed",
        "built-in software engineer",
        "not mapped",
    ],
    how_note: |note| format!(" ({note})"),
    items: "## By category\n\n",
    item: |what, status, n| format!("### {what}\n\n{status}: {n} item(s)"),
    skipped: |n| format!(", {n} skipped"),
    end: ".\n",
    target: |target| format!("Target: `{target}`.\n"),
    derived: |n| {
        format!(
            "### Derived caches\n\n{n} place(s) (logs, caches, knowledge base vectors and the like) are not migrated; the new version rebuilds them. The agent report lists each one.\n\n"
        )
    },
    outside: "## Outside the data root\n\nThe old version installed these outside its data root. They are not in the archive:\n\n",
    outside_colon: ": ",
    left: |agent| {
        format!(
            "## Not migrated\n\nWhen you need any of these, have the agent continue from `{agent}` in the same folder.\n\n| What | In the archive | Old format |\n|---|---|---|\n"
        )
    },
    old_root: |path| {
        format!(
            "## Old data root\n\nMigration did not change a single byte of the old data folder `{path}`. "
        )
    },
    archive: |path, files, size, sha256, manifest| {
        format!(
            "The full archive is `{path}`: {files} files, {size} in total before packing. The SHA-256 of the whole archive is `{sha256}`, and the per-file list is `{manifest}`. "
        )
    },
    delete_later: "Once you have checked that everything came over and the new version has worked for a while, delete the old data folder and keep the archive. Deleting is up to you; the migration never deletes it.\n",
    status: [
        "Migrated",
        "Partly migrated",
        "Not migrated, kept in the archive",
        "Nothing to migrate",
    ],
};

/// 照语言挑报告的字。
#[must_use]
pub fn words(language: Language) -> &'static Report {
    language.pick(&ZH, &EN)
}

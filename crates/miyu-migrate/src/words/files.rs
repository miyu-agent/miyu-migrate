//! 文件（`files`）、知识库（`knowledge`）的字：旧格式一句话、拷了多少、改了名的。

use super::Language;

/// 文件的字。
pub struct Files {
    /// 在旧根的哪几处。
    pub source: &'static str,
    /// 旧格式一句话。
    pub format: &'static str,
    /// 拷了几个、以前迁过几个。
    pub copied: fn(u64, u64) -> String,
    /// 同名不一样、改了名的（并起来的）。
    pub renamed: fn(&str) -> String,
    /// 拷完核对对不上的几个。
    pub mismatch: fn(u64) -> String,
    /// 以前的分享没有对应的。
    pub shares: &'static str,
    /// 旧根里没有。
    pub none: &'static str,
}

/// 知识库的字。
pub struct Knowledge {
    /// 旧格式一句话。
    pub format: &'static str,
    /// 迁到哪：新版的库编号。
    pub target: fn(&str) -> String,
    /// 旧根里没有。
    pub none: &'static str,
    /// 清单在、读不了：系统的原话。
    pub unreadable: fn(&str) -> String,
    /// 只有内置的默认库：几篇。
    pub builtin_only: fn(usize) -> String,
    /// 新建的库叫什么。
    pub name: &'static str,
    /// 新建的库的说明。
    pub description: &'static str,
    /// 建库没成：拒的代码。
    pub create_failed: fn(&str) -> String,
    /// 拷进去几篇、库编号、以前迁过几篇。
    pub copied: fn(u64, &str, u64) -> String,
    /// 新版现在认得几篇。
    pub documents: fn(u64) -> String,
    /// 第一次用之前要建索引。
    pub indexing: &'static str,
    /// 不是新版认的类型的几篇。
    pub other: fn(u64) -> String,
    /// 内置的默认库几篇不迁。
    pub builtin: fn(usize) -> String,
    /// 同名不一样、改了名的（并起来的）。
    pub renamed: fn(&str) -> String,
    /// 和旧清单的哈希对不上的几篇。
    pub mismatch: fn(u64) -> String,
}

static FILES_ZH: Files = Files {
    source: "home/<管>/{pictures,documents,artifacts,workspace}、data/pictures、workspace",
    format: "普通文件：图片、文档、工具产出的产物（artifacts/<旧会话编号>/）、工作目录",
    copied: |copied, present| format!("拷了 {copied} 个文件，{present} 个以前迁过。"),
    renamed: |listed| format!("新版里已经有同名不一样的，改了名：{listed}。"),
    mismatch: |n| format!("{n} 个拷完核对 SHA-256 对不上，照实写在这里。"),
    shares: "`shares/`（以前给别人的分享）新版没有对应的，留在压缩包里。",
    none: "旧根里没有这几样文件。",
};

static FILES_EN: Files = Files {
    source: "home/<admin>/{pictures,documents,artifacts,workspace}, data/pictures, workspace",
    format: "plain files: pictures, documents, artifacts made by tools (artifacts/<old session id>/), the workspace",
    copied: |copied, present| {
        format!("Copied {copied} file(s); {present} had been migrated before.")
    },
    renamed: |listed| {
        format!(
            "The new version already had different files with the same names, so these were renamed: {listed}."
        )
    },
    mismatch: |n| format!("{n} file(s) did not match their SHA-256 after copying."),
    shares: "`shares/` (things shared with others before) has nothing matching in the new version and stays in the archive.",
    none: "None of these files are in the old root.",
};

static KNOWLEDGE_ZH: Knowledge = Knowledge {
    format: "data/kb/files/ 下放原文；kb_meta.db 的 files（name 相对 files/、path 旧绝对路径、size_bytes、mtime、content_sha256）是清单；semantic_index.db 切块和向量（新版自己重建）；default-kb/ 开头的是旧版内置的默认库",
    target: |base| format!("root/kb/{base}/（kb.create）"),
    none: "旧根里没有知识库。",
    unreadable: |error| {
        format!("知识库的清单 kb_meta.db 读不了（{error}），文档没迁，留在压缩包里。")
    },
    builtin_only: |n| format!("旧版只有内置的默认库（{n} 篇，新版不迁）。"),
    name: "旧版知识库",
    description: "从旧版 Miyu 迁过来的文档",
    create_failed: |code| format!("建库没成（{code}），文档留在压缩包里。"),
    copied: |copied, base, present| {
        format!("{copied} 篇拷进了 `root/kb/{base}/`，{present} 篇以前迁过。")
    },
    documents: |n| format!("新版这个库现在认得 {n} 篇。"),
    indexing: "第一次列出、搜这个库之前新版会先建索引，篇数多要等一会儿，不是卡住了。",
    other: |n| format!("{n} 篇不是新版认的类型（只认 md、txt、pdf、html），照拷了，新版不建索引。"),
    builtin: |n| format!("旧版内置的默认库 {n} 篇不迁。"),
    renamed: |listed| format!("新版里已经有同名不一样的，改了名：{listed}。"),
    mismatch: |n| {
        format!("{n} 篇和旧清单记的 SHA-256 对不上（旧版记下以后又改过），照现在的文件拷了。")
    },
};

static KNOWLEDGE_EN: Knowledge = Knowledge {
    format: "originals under data/kb/files/; files in kb_meta.db (name relative to files/, path the old absolute path, size_bytes, mtime, content_sha256) is the list; semantic_index.db holds chunks and vectors (rebuilt by the new version); names starting with default-kb/ are the old built-in default knowledge base",
    target: |base| format!("root/kb/{base}/ (kb.create)"),
    none: "No knowledge base in the old root.",
    unreadable: |error| {
        format!(
            "Cannot read the knowledge base list kb_meta.db ({error}); the documents were not migrated and stay in the archive."
        )
    },
    builtin_only: |n| {
        format!(
            "The old version only had the built-in default knowledge base ({n} document(s), not migrated)."
        )
    },
    name: "Old Miyu knowledge base",
    description: "Documents migrated from old Miyu",
    create_failed: |code| {
        format!("Could not create the knowledge base ({code}); the documents stay in the archive.")
    },
    copied: |copied, base, present| {
        format!(
            "Copied {copied} document(s) into `root/kb/{base}/`; {present} had been migrated before."
        )
    },
    documents: |n| format!("The new version now sees {n} document(s) in this knowledge base."),
    indexing: "The new version builds the index before this knowledge base is first listed or searched. With many documents that takes a while; it is not stuck.",
    other: |n| {
        format!(
            "{n} document(s) are of types the new version does not index (only md, txt, pdf, html); they were copied anyway."
        )
    },
    builtin: |n| {
        format!("{n} document(s) of the old built-in default knowledge base are not migrated.")
    },
    renamed: |listed| {
        format!(
            "The new version already had different documents with the same names, so these were renamed: {listed}."
        )
    },
    mismatch: |n| {
        format!(
            "{n} document(s) did not match the SHA-256 in the old list (changed after the old version recorded them); the current files were copied."
        )
    },
};

/// 照语言挑文件的字。
#[must_use]
pub fn files(language: Language) -> &'static Files {
    language.pick(&FILES_ZH, &FILES_EN)
}

/// 照语言挑知识库的字。
#[must_use]
pub fn knowledge(language: Language) -> &'static Knowledge {
    language.pick(&KNOWLEDGE_ZH, &KNOWLEDGE_EN)
}

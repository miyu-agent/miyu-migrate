//! 旧配置的 JSONC：反斜杠结尾的字符串（Windows 路径）后面，末尾多的逗号照样去掉、字符串里的逗号不动（审计 #139）。

#![allow(clippy::unwrap_used, reason = "测试：读坏了就该停")]

use miyu_migrate::jsonc::parse;

/// 审计 #139：去末尾逗号的第二遍只看前一个字符是不是 `\` 来判断引号是不是被转义：`"C:\\x\\"` 的闭引号前是一个被转义的
/// `\`，被当成还在字符串里，之后引号状态整个反过来——末尾逗号留着，整份配置读不懂（报告写「没有配置」，模型、供应商、
/// 密钥全不迁）；或者把字符串里的 `, }` 当成末尾逗号删掉。
#[test]
fn audit_139_a_string_ending_in_a_backslash_does_not_flip_the_quotes() {
    let config = parse(r#"{"dir": "C:\\Users\\shorin\\", "model": "gpt-4o", }"#)
        .expect("末尾逗号要去掉，整份读得懂");
    assert_eq!(config["dir"], "C:\\Users\\shorin\\");
    assert_eq!(config["model"], "gpt-4o");

    let list = parse(r#"{"dir": "C:\\x\\", "list": [1, 2, ], "k": 1}"#).expect("读得懂");
    assert_eq!(list["list"], serde_json::json!([1, 2]));

    let kept = parse(r#"{"dir": "C:\\x\\", "note": "a, }", "k": 1}"#).expect("读得懂");
    assert_eq!(kept["note"], "a, }", "字符串里的逗号不能删");
}

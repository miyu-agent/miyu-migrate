//! 语言照代码认：`zh` 开头的中文，别的（`en`、`ja`、`C`、空的）英文；并句、列举照语言用各自的标点。

use super::*;

#[test]
fn codes_map_to_the_two_languages() {
    for code in ["zh", "zh_CN.UTF-8", "zh_TW", "ZH_cn"] {
        assert_eq!(Language::from_code(code), Language::Zh, "{code}");
    }
    for code in ["en", "en_US.UTF-8", "ja", "ja_JP.UTF-8", "C", "C.UTF-8", ""] {
        assert_eq!(Language::from_code(code), Language::En, "{code}");
    }
    assert_eq!(Language::from_core(Some("zh")), Language::Zh);
    assert_eq!(Language::from_core(Some("ja")), Language::En);
}

#[test]
fn sentences_and_lists_follow_the_language() {
    let parts = vec!["一。".to_string(), "二。".to_string()];
    assert_eq!(Language::Zh.sentences(&parts), "一。二。");
    let parts = vec!["One.".to_string(), "Two.".to_string()];
    assert_eq!(Language::En.sentences(&parts), "One. Two.");
    assert_eq!(Language::Zh.list(&["a", "b"]), "a、b");
    assert_eq!(Language::En.list(&["a", "b"]), "a, b");
}

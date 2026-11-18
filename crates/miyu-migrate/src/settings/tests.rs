//! 配置：供应商编号照新版的写法改、键里人起的名字带引号；旧配置排成哪几项设置、几个密钥，不转的为什么；新版已经有的供应商整家
//! 跳过。

use std::collections::BTreeSet;

use serde_json::{Value, json};

use super::*;

#[test]
fn ids_and_key_segments_follow_the_new_rules() {
    assert_eq!(new_id("DeepSeek"), "deepseek");
    assert_eq!(new_id("my relay.cn"), "my-relay-cn");
    assert_eq!(new_id("1api"), "p-1api");
    assert_eq!(new_id(&"x".repeat(40)).len(), 32);
    assert_eq!(segment("deepseek-v4"), "deepseek-v4");
    assert_eq!(segment("deepseek-v4.1-flash"), "\"deepseek-v4.1-flash\"");
    assert_eq!(segment("qq:10001"), "\"qq:10001\"");
    assert_eq!(segment(r#"a"b"#), r#""a\"b""#);
}

fn old() -> Value {
    json!({
        "config_version": 6,
        "active_provider": "Relay",
        "providers": [
            {"id": "claude-code", "display_name": "Claude Code", "base_url": "", "protocol": "claude-code", "enabled": true},
            {"id": "openai", "display_name": "OpenAI", "base_url": "https://api.openai.com/v1"},
            {"id": "anthropic", "display_name": "Anthropic", "base_url": "https://api.anthropic.com", "protocol": "anthropic", "api_key": "$env:ANTHROPIC_API_KEY"},
            {"id": "deepseek", "display_name": "DeepSeek", "base_url": "https://api.deepseek.com", "api_key": "sk-one, sk-two\nsk-three"},
            {"id": "Relay", "display_name": "我的中转", "base_url": "https://relay.example/v1", "api_key": "$env:RELAY_KEY",
             "default_model": "glm-5", "model_context_window": {"glm-5": 200000}, "model_temperature": {"glm-5": 0.6},
             "model_costs": {"glm-5.1": {"currency": "cny", "input": 2.0, "output": 8.0, "cache": 0.5}}},
            {"id": "ollama", "display_name": "Ollama", "base_url": "http://localhost:11434/v1", "models": ["qwen3"]},
            {"id": "kept", "display_name": "已经有", "base_url": "https://kept/v1", "api_key": "sk-kept"}
        ],
        "embedding": {"backend": "auto", "provider_id": "deepseek", "model": "embed-1"},
        "plugins": {
            "vision": {"vision_provider_id": "Relay", "vision_model": "glm-5v"},
            "web": {"tavily_api_keys": ["", "tv-key"], "firecrawl_api_keys": ["fc"], "searxng_base_url": "http://s.local"},
            "memes": {"enabled": true}
        },
        "model_tiers": {"lite": [{"provider_id": "deepseek", "model": "chat"}], "strong": [{"provider_id": "Relay", "model": "glm-5"}], "cheap": [{"provider_id": "deepseek", "model": "chat"}]},
        "platforms": {"qq": {"admin_users": [10001, "10002"], "access_token": "t"}},
        "context": {"trim_at_ratio": 0.7}
    })
}

#[test]
fn the_old_config_becomes_settings_and_secrets() {
    let existing = BTreeSet::from(["kept".to_string()]);
    let planned = plan(&old(), Some("  我是谁\n喜欢猫  "), &existing);
    let changes: Vec<(&str, &Value)> = planned
        .changes
        .iter()
        .map(|change| (change.key.as_str(), &change.value))
        .collect();
    let expected = vec![
        ("providers.deepseek.name", json!("DeepSeek")),
        (
            "providers.deepseek.base_url",
            json!("https://api.deepseek.com"),
        ),
        ("providers.deepseek.driver", json!("openai-chat")),
        (
            "providers.deepseek.key",
            json!({"secret": "provider-deepseek"}),
        ),
        ("providers.relay.name", json!("我的中转")),
        (
            "providers.relay.base_url",
            json!("https://relay.example/v1"),
        ),
        ("providers.relay.driver", json!("openai-chat")),
        ("providers.relay.key", json!({"env": "RELAY_KEY"})),
        ("providers.relay.models.glm-5.window", json!(200_000)),
        ("providers.relay.models.glm-5.temperature", json!(0.6)),
        ("providers.relay.models.\"glm-5.1\".price.input", json!(2.0)),
        (
            "providers.relay.models.\"glm-5.1\".price.output",
            json!(8.0),
        ),
        (
            "providers.relay.models.\"glm-5.1\".price.cache_read",
            json!(0.5),
        ),
        (
            "providers.relay.models.\"glm-5.1\".price.currency",
            json!("CNY"),
        ),
        ("providers.ollama.name", json!("Ollama")),
        (
            "providers.ollama.base_url",
            json!("http://localhost:11434/v1"),
        ),
        ("providers.ollama.driver", json!("openai-chat")),
        ("models.chat", json!("relay/glm-5")),
        ("models.embedding", json!("deepseek/embed-1")),
        ("models.vision", json!("relay/glm-5v")),
        ("pools.lite.models", json!(["deepseek/chat"])),
        ("pools.flagship.models", json!(["relay/glm-5"])),
        (
            "net.search.tavily.key",
            json!({"secret": "net-search-tavily"}),
        ),
        ("net.search.searxng.url", json!("http://s.local")),
        ("profile.about", json!("我是谁\n喜欢猫")),
        ("external.bindings.\"qq:10001\"", json!("owner")),
        ("external.bindings.\"qq:10002\"", json!("owner")),
    ];
    let expected: Vec<(&str, &Value)> = expected.iter().map(|(key, value)| (*key, value)).collect();
    assert_eq!(changes, expected);
    let names: Vec<&str> = planned
        .secrets
        .iter()
        .map(|secret| secret.name.as_str())
        .collect();
    assert_eq!(names, vec!["provider-deepseek", "net-search-tavily"]);
    assert_eq!(planned.secrets[0].value, "sk-one", "多个 key 只迁第一个");
    assert!(
        !format!("{planned:?}").contains("sk-one"),
        "Debug 不印密钥的值"
    );
    assert_eq!(
        planned.renamed,
        vec![("Relay".to_string(), "relay".to_string())]
    );
    let left: Vec<(&str, &str)> = planned
        .left
        .iter()
        .map(|(key, why)| (key.as_str(), why.as_str()))
        .collect();
    assert_eq!(
        left,
        vec![
            ("providers.claude-code", "cli-relay"),
            ("providers.deepseek.api_key", "more-keys-2"),
            ("providers.kept", "existing"),
            ("model_tiers.cheap", "no-match"),
            ("plugins.web.firecrawl_api_keys", "no-match"),
            ("context", "no-match"),
            ("plugins.memes", "no-match"),
            ("platforms.qq", "im"),
        ]
    );
}

#[test]
fn a_long_profile_and_a_local_embedding_are_left() {
    let mut config =
        json!({"embedding": {"backend": "local", "provider_id": "deepseek", "model": "x"}});
    config["providers"] =
        json!([{"id": "deepseek", "base_url": "https://api.deepseek.com", "api_key": "k"}]);
    let planned = plan(&config, Some(&"长".repeat(2001)), &BTreeSet::new());
    assert!(
        !planned
            .changes
            .iter()
            .any(|change| change.key == "models.embedding"),
        "本地的不转"
    );
    assert!(
        planned
            .left
            .contains(&("profile.md".to_string(), "too-long".to_string()))
    );
}

#[test]
fn an_anthropic_relay_keeps_its_driver() {
    let config = json!({"providers": [{"id": "relay", "protocol": "anthropic", "base_url": "https://r/v1", "api_key": "k"}]});
    let planned = plan(&config, None, &BTreeSet::new());
    let driver = planned
        .changes
        .iter()
        .find(|change| change.key == "providers.relay.driver")
        .map(|change| change.value.clone());
    assert_eq!(driver, Some(json!("anthropic")));
}

#[test]
fn a_price_with_both_cache_names_is_written_once() {
    let config = json!({"providers": [{"id": "relay", "base_url": "https://r/v1", "api_key": "k",
        "model_costs": {"m": {"input": 1.0, "output": 2.0, "cache_read": 0.1, "cache": 0.2}}}]});
    let planned = plan(&config, None, &BTreeSet::new());
    let cache: Vec<&Value> = planned
        .changes
        .iter()
        .filter(|change| change.key == "providers.relay.models.m.price.cache_read")
        .map(|change| &change.value)
        .collect();
    assert_eq!(cache, vec![&json!(0.1)], "新写法 cache_read 为准");
}

/// 审计 #142：`new_id` 把非 ASCII 一律换成 `-`，等长的中文供应商名都成了 `p---`：两家的设置写到同一个键下、两个密钥同名，
/// 后写的那家的 Key 盖掉前一家，两家都坏。旧版的中文编号是合法配置。
#[test]
fn audit_142_two_chinese_provider_ids_keep_their_own_keys() {
    let old = json!({
        "providers": [
            {"id": "智谱", "base_url": "https://zhipu.example/v1", "api_key": "key-zhipu"},
            {"id": "百炼", "base_url": "https://bailian.example/v1", "api_key": "key-bailian"},
        ]
    });
    let planned = plan(&old, None, &BTreeSet::new());
    let names: BTreeSet<&str> = planned
        .secrets
        .iter()
        .map(|one| one.name.as_str())
        .collect();
    assert_eq!(
        names.len(),
        planned.secrets.len(),
        "密钥名撞了：{:?}",
        planned.secrets
    );
    for (url, key) in [
        ("https://zhipu.example/v1", "key-zhipu"),
        ("https://bailian.example/v1", "key-bailian"),
    ] {
        let base = planned
            .changes
            .iter()
            .find(|one| one.value == json!(url))
            .and_then(|one| one.key.strip_suffix(".base_url"))
            .unwrap_or_else(|| panic!("{url} 没写进去：{:?}", planned.changes));
        let secret = planned
            .changes
            .iter()
            .rfind(|one| one.key == format!("{base}.key"))
            .and_then(|one| one.value["secret"].as_str())
            .unwrap_or_else(|| panic!("{base} 没有 key"));
        let value = planned
            .secrets
            .iter()
            .rfind(|one| one.name == secret)
            .map(|one| one.value.as_str());
        assert_eq!(value, Some(key), "{base}（{url}）的 key 被别家的盖了");
    }
}

/// 审计 #142：`segment` 带引号时只转义 `\` 和 `"`，换行、回车、制表这些控制字符原样放进 TOML 的引号键里，键写不进去。
#[test]
fn audit_142_control_characters_in_a_key_segment_are_escaped() {
    assert_eq!(segment("a\nb"), r#""a\nb""#);
    assert_eq!(segment("a\tb\rc"), r#""a\tb\rc""#);
    assert!(
        !segment("x\u{1}y").chars().any(char::is_control),
        "{:?}",
        segment("x\u{1}y")
    );
}

/// 中文编号的新编号每次一样、合新版的写法；两个旧编号还是换成了同一个的，后一家加 `-2`。
#[test]
fn chinese_ids_are_stable_and_colliding_ids_get_a_number() {
    let zhipu = new_id("智谱");
    assert_eq!(zhipu, new_id(" 智谱 "), "同一个旧编号每次一样");
    assert_ne!(zhipu, new_id("百炼"));
    assert!(zhipu.len() <= 32 && zhipu.starts_with(|ch: char| ch.is_ascii_lowercase()));
    assert!(new_id("智谱AI").starts_with("ai-"), "{}", new_id("智谱AI"));
    let long = new_id(&format!("{}智", "x".repeat(40)));
    assert!(long.len() <= 32, "截短了也留着后面那段：{long}");
    assert_ne!(long, new_id(&"x".repeat(40)));
    let old = json!({
        "providers": [
            {"id": "my relay", "base_url": "https://a.example/v1"},
            {"id": "my-relay", "base_url": "https://b.example/v1"},
        ]
    });
    let planned = plan(&old, None, &BTreeSet::new());
    let bases: Vec<&str> = planned
        .changes
        .iter()
        .filter_map(|one| one.key.strip_suffix(".base_url"))
        .collect();
    assert_eq!(bases, vec!["providers.my-relay", "providers.my-relay-2"]);
}

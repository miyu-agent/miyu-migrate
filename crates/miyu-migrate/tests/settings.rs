//! 配置迁进真核心（进程里起）：供应商、主对话、搜索、主人的号、关于你写进系统配置，密钥写进密钥文件；两份报告里没有密钥的值；
//! 再跑一遍全是新版已经有的，什么都不改。

#![allow(clippy::unwrap_used, reason = "测试：读坏了就该停")]

mod support;

use serde_json::{Value, json};

use miyu_migrate::core::Core;
use miyu_migrate::run::{self, Plan};
use support::{connect, fresh};

/// 旧配置里的 key：两份报告里都不能有它。
const KEY: &str = "sk-test-do-not-print-0123456789";

fn old_root() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".home-layout-v1"), "owner\n").unwrap();
    std::fs::create_dir_all(dir.path().join("home/owner")).unwrap();
    std::fs::write(dir.path().join("home/owner/conversation.db"), "").unwrap();
    std::fs::write(
        dir.path().join("home/owner/profile.md"),
        "喜欢猫\n写 Rust\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("config")).unwrap();
    let config = format!(
        r#"{{
  // 旧版的配置
  "config_version": 6,
  "active_provider": "deepseek",
  "providers": [
    {{"id": "deepseek", "display_name": "DeepSeek", "base_url": "https://api.deepseek.com", "api_key": "{KEY}",
      "default_model": "deepseek-chat", "model_context_window": {{"deepseek-chat": 128000}}}},
  ],
  "plugins": {{"web": {{"tavily_api_keys": ["{KEY}-tavily"]}}}},
  "platforms": {{"qq": {{"admin_users": [10001]}}}},
}}"#
    );
    std::fs::write(dir.path().join("config/config.jsonc"), config).unwrap();
    dir
}

fn settings_item(agent: &Value) -> Value {
    agent["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["what"] == "config")
        .unwrap()
        .clone()
}

#[test]
fn old_settings_fill_the_new_config_without_printing_keys() {
    let fresh = fresh();
    let old = old_root();
    let mut core = connect(&fresh.root);
    let plan = Plan {
        old: old.path().to_path_buf(),
        state: fresh.root.path().join("var/lib/migrate"),
        at: "2026-11-04T08:00:00Z".parse().unwrap(),
        root: fresh.root.clone(),
        records: miyu_migrate::records::Importer::new(|_| Err(std::io::Error::other("no im"))),
        home: None,
        space: run::free_space,
    };
    let done = run::run(&plan, &mut core).unwrap();
    for report in [&done.human, &done.agent] {
        let text = std::fs::read_to_string(report).unwrap();
        assert!(!text.contains(KEY), "{} 里有密钥的值", report.display());
    }
    let agent: Value =
        serde_json::from_str(&std::fs::read_to_string(&done.agent).unwrap()).unwrap();
    let item = settings_item(&agent);
    assert!(item.get("skipped").is_none(), "{item}");

    let got = core.call("config.get", json!({})).unwrap();
    let value = |key: &str| got["items"][key]["value"].clone();
    assert_eq!(
        value("models.chat"),
        json!("deepseek/deepseek-chat"),
        "{got}"
    );
    assert_eq!(
        value("providers.deepseek.base_url"),
        json!("https://api.deepseek.com")
    );
    assert_eq!(
        value("providers.deepseek.models.deepseek-chat.window"),
        json!(128_000)
    );
    assert_eq!(value("profile.about"), json!("喜欢猫\n写 Rust"));
    assert_eq!(
        value("external.bindings.\"qq:10001\""),
        json!("owner"),
        "{got}"
    );
    let secrets = core.call("secret.list", json!({})).unwrap();
    let set: Vec<&str> = secrets["secrets"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|one| one["set"] == true)
        .map(|one| one["name"].as_str().unwrap())
        .collect();
    assert_eq!(set, vec!["net-search-tavily", "provider-deepseek"]);
    assert!(
        !item["note"]
            .as_str()
            .unwrap()
            .contains("读回来和写的不一样"),
        "{item}"
    );

    let again = run::run(
        &Plan {
            at: "2026-11-04T08:00:01Z".parse().unwrap(),
            ..plan
        },
        &mut core,
    )
    .unwrap();
    let agent: Value =
        serde_json::from_str(&std::fs::read_to_string(again.agent).unwrap()).unwrap();
    let item = settings_item(&agent);
    assert_eq!(item["migrated"], 0, "{item}");
    let whys: Vec<&str> = item["skipped"]
        .as_array()
        .unwrap()
        .iter()
        .map(|skip| skip["why"].as_str().unwrap())
        .collect();
    assert!(
        whys.iter()
            .all(|why| *why == "already-set" || *why == "existing"),
        "{whys:?}"
    );
}

//! 旧配置 `config/config.jsonc` 换成新版的几项设置、几个密钥（细则第四节那张表）。只排要写什么，不写；不转的记下名字和为什么，
//! 不记值。

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

/// 旧版的供应商模板：没填 key（或者模型）的不转。
const TEMPLATES: &[&str] = &[
    "opencode",
    "opencodego",
    "openai",
    "anthropic",
    "deepseek",
    "gemini",
    "xiaomi",
    "minimax",
    "openrouter",
    "ollama",
    "lmstudio",
];
/// 旧版的 CLI 中转：新版没有对应的驱动，不转。
const RELAYS: &[&str] = &["claude-code", "codex", "antigravity", "codebuddy"];

/// 要写的一项设置。
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    /// 新版的键（TOML 点号连着，人起的名字照要求带引号）。
    pub key: String,
    /// 值（JSON 的写法）。
    pub value: Value,
}

/// 要写的一个密钥：名字、值（值只交给 `secret.set`，哪里都不印）。
#[derive(Clone, PartialEq, Eq)]
pub struct Secret {
    /// 名字。
    pub name: String,
    /// 值。
    pub value: String,
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Secret({})", self.name)
    }
}

/// 排好的。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Planned {
    /// 设置，照先后（密钥的引用排在密钥后面写，所以都在密钥之后）。
    pub changes: Vec<Change>,
    /// 密钥。
    pub secrets: Vec<Secret>,
    /// 不转的：哪一样（旧版的键、供应商编号），为什么。
    pub left: Vec<(String, String)>,
    /// 旧版的供应商编号改成了新版能用的写法的：旧的、新的。
    pub renamed: Vec<(String, String)>,
}

/// 照旧配置 `old`、属主档案 `profile`、新版已经有的供应商 `existing` 排。
pub fn plan(old: &Value, profile: Option<&str>, existing: &BTreeSet<String>) -> Planned {
    let mut planned = Planned::default();
    let ids = providers(old, existing, &mut planned);
    let model = |provider: &str, name: &str| -> Option<String> {
        let id = ids.get(provider.trim())?;
        let name = name.trim();
        (!name.is_empty()).then(|| format!("{id}/{name}"))
    };
    // 主对话。
    if let Some(active) = old["active_provider"].as_str() {
        let default = old["providers"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|one| one["id"].as_str() == Some(active))
            .and_then(|one| one["default_model"].as_str())
            .unwrap_or_default();
        if let Some(chosen) = model(active, default) {
            planned.changes.push(change("models.chat", json!(chosen)));
        }
    }
    // 向量：远程的才转（本地的新版有自己的）。
    let embedding = &old["embedding"];
    let backend = embedding["backend"].as_str().unwrap_or("auto");
    if matches!(backend, "remote" | "auto")
        && let Some(chosen) = model(
            embedding["provider_id"].as_str().unwrap_or_default(),
            embedding["model"].as_str().unwrap_or_default(),
        )
    {
        planned
            .changes
            .push(change("models.embedding", json!(chosen)));
    }
    // 看图。
    let vision = &old["plugins"]["vision"];
    if let Some(chosen) = model(
        vision["vision_provider_id"].as_str().unwrap_or_default(),
        vision["vision_model"].as_str().unwrap_or_default(),
    ) {
        planned.changes.push(change("models.vision", json!(chosen)));
    }
    // 挡位 → 池。
    let tiers = if old["model_tiers"].is_object() {
        &old["model_tiers"]
    } else {
        &old["subagent_tiers"]
    };
    for (old_name, aliases, pool) in [
        ("lite", &[][..], "lite"),
        ("standard", &["balanced"][..], "normal"),
        ("flagship", &["strong"][..], "flagship"),
    ] {
        let members = std::iter::once(old_name)
            .chain(aliases.iter().copied())
            .find_map(|name| tiers[name].as_array());
        let members: Vec<String> = members
            .into_iter()
            .flatten()
            .filter_map(|one| {
                model(
                    one["provider_id"].as_str().unwrap_or_default(),
                    one["model"].as_str().unwrap_or_default(),
                )
            })
            .collect();
        if !members.is_empty() {
            planned
                .changes
                .push(change(&format!("pools.{pool}.models"), json!(members)));
        }
    }
    if tiers["cheap"].as_array().is_some_and(|all| !all.is_empty()) {
        planned
            .left
            .push(("model_tiers.cheap".to_string(), "no-match".to_string()));
    }
    search(&old["plugins"]["web"], &mut planned);
    if let Some(about) = profile.map(str::trim).filter(|about| !about.is_empty()) {
        if about.chars().count() <= 2000 {
            planned.changes.push(change("profile.about", json!(about)));
        } else {
            planned
                .left
                .push(("profile.md".to_string(), "too-long".to_string()));
        }
    }
    for admin in old["platforms"]["qq"]["admin_users"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let qq = match admin {
            Value::Number(qq) => qq.to_string(),
            Value::String(qq) => qq.trim().to_string(),
            _ => continue,
        };
        if !qq.is_empty() {
            planned.changes.push(change(
                &format!("external.bindings.{}", segment(&format!("qq:{qq}"))),
                json!("owner"),
            ));
        }
    }
    rest(old, &mut planned);
    planned
}

/// 供应商：交回旧编号 → 新编号（转了的、新版本来就有的）。
fn providers(
    old: &Value,
    existing: &BTreeSet<String>,
    planned: &mut Planned,
) -> BTreeMap<String, String> {
    let mut ids = BTreeMap::new();
    let mut secrets = Vec::new();
    for provider in old["providers"].as_array().into_iter().flatten() {
        let Some(old_id) = provider["id"]
            .as_str()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        else {
            continue;
        };
        let protocol = provider["protocol"].as_str().unwrap_or("auto");
        if RELAYS.contains(&old_id)
            || RELAYS.iter().any(|relay| protocol.starts_with(relay))
            || protocol == "agy"
            || protocol == "cbc"
        {
            if provider["enabled"].as_bool() == Some(true) {
                planned
                    .left
                    .push((format!("providers.{old_id}"), "cli-relay".to_string()));
            }
            continue;
        }
        let key = provider["api_key"]
            .as_str()
            .map(str::trim)
            .unwrap_or_default();
        let filled_key = !key.is_empty()
            && !(old_id == "anthropic" && key == "$env:ANTHROPIC_API_KEY")
            && !(old_id == "opencode" && key == "public");
        let has_models = ["models", "custom_models"]
            .iter()
            .any(|list| provider[list].as_array().is_some_and(|all| !all.is_empty()));
        if TEMPLATES.contains(&old_id) && !filled_key && (old_id == "opencode" || !has_models) {
            continue;
        }
        let id = new_id(old_id);
        if id != old_id {
            planned.renamed.push((old_id.to_string(), id.clone()));
        }
        ids.insert(old_id.to_string(), id.clone());
        if existing.contains(&id) {
            planned
                .left
                .push((format!("providers.{old_id}"), "existing".to_string()));
            continue;
        }
        let base = format!("providers.{}", segment(&id));
        if let Some(name) = provider["display_name"]
            .as_str()
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            planned.changes.push(change(
                &format!("{base}.name"),
                json!(name.chars().take(64).collect::<String>()),
            ));
        }
        if let Some(url) = provider["base_url"]
            .as_str()
            .map(str::trim)
            .filter(|url| !url.is_empty())
        {
            planned
                .changes
                .push(change(&format!("{base}.base_url"), json!(url)));
        }
        let driver = if protocol == "anthropic" {
            "anthropic"
        } else {
            "openai-chat"
        };
        planned
            .changes
            .push(change(&format!("{base}.driver"), json!(driver)));
        if filled_key {
            let mut keys = key
                .split(['\n', ','])
                .map(str::trim)
                .filter(|one| !one.is_empty());
            if let Some(first) = keys.next() {
                if let Some(var) = first.strip_prefix("$env:") {
                    planned
                        .changes
                        .push(change(&format!("{base}.key"), json!({"env": var.trim()})));
                } else {
                    let name = format!("provider-{id}");
                    secrets.push(Secret {
                        name: name.clone(),
                        value: first.to_string(),
                    });
                    planned
                        .changes
                        .push(change(&format!("{base}.key"), json!({"secret": name})));
                }
            }
            let more = keys.count();
            if more > 0 {
                planned.left.push((
                    format!("providers.{old_id}.api_key"),
                    format!("more-keys-{more}"),
                ));
            }
        }
        models(provider, &base, planned);
    }
    planned.secrets.extend(secrets);
    ids
}

/// 每个模型的窗口、温度、价格。
fn models(provider: &Value, base: &str, planned: &mut Planned) {
    let mut names = BTreeSet::new();
    for table in ["model_context_window", "model_temperature", "model_costs"] {
        names.extend(
            provider[table]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(name, _)| name.clone()),
        );
    }
    for name in names {
        let at = format!("{base}.models.{}", segment(&name));
        if let Some(window) = provider["model_context_window"][&name]
            .as_u64()
            .filter(|window| *window > 0)
        {
            planned
                .changes
                .push(change(&format!("{at}.window"), json!(window)));
        }
        if let Some(temperature) = provider["model_temperature"][&name].as_f64() {
            planned
                .changes
                .push(change(&format!("{at}.temperature"), json!(temperature)));
        }
        let cost = &provider["model_costs"][&name];
        if cost.is_object() {
            for (old, new) in [
                ("input", "input"),
                ("output", "output"),
                ("cache_read", "cache_read"),
                ("cache", "cache_read"),
            ] {
                if let Some(price) = cost[old].as_f64() {
                    planned
                        .changes
                        .push(change(&format!("{at}.price.{new}"), json!(price)));
                }
            }
            let currency = match cost["currency"].as_str().map(str::to_lowercase).as_deref() {
                Some("cny" | "rmb" | "¥") => "CNY",
                _ => "USD",
            };
            planned
                .changes
                .push(change(&format!("{at}.price.currency"), json!(currency)));
        }
    }
    // 同一个模型 `cache` 和 `cache_read` 都写了的，留一个。
    let mut seen = BTreeSet::new();
    planned.changes.retain(|one| seen.insert(one.key.clone()));
}

/// 搜索的 key 和地址。
fn search(web: &Value, planned: &mut Planned) {
    for (old, service) in [("tavily_api_keys", "tavily"), ("exa_api_keys", "exa")] {
        if let Some(first) = web[old]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::trim)
            .find(|key| !key.is_empty())
        {
            let name = format!("net-search-{service}");
            planned.secrets.push(Secret {
                name: name.clone(),
                value: first.to_string(),
            });
            planned.changes.push(change(
                &format!("net.search.{service}.key"),
                json!({"secret": name}),
            ));
        }
    }
    if let Some(url) = web["searxng_base_url"]
        .as_str()
        .map(str::trim)
        .filter(|url| !url.is_empty())
    {
        planned
            .changes
            .push(change("net.search.searxng.url", json!(url)));
    }
    for old in ["firecrawl_api_keys", "anysearch_api_keys"] {
        if web[old].as_array().is_some_and(|all| !all.is_empty()) {
            planned
                .left
                .push((format!("plugins.web.{old}"), "no-match".to_string()));
        }
    }
}

/// 别的整段：新版没有对应的格，只报。
fn rest(old: &Value, planned: &mut Planned) {
    for section in [
        "context",
        "tools",
        "cache",
        "display",
        "notifications",
        "voice",
        "mcp",
        "skills",
    ] {
        if old[section].as_object().is_some_and(|map| !map.is_empty()) {
            planned
                .left
                .push((section.to_string(), "no-match".to_string()));
        }
    }
    let plugins: Map<String, Value> = old["plugins"].as_object().cloned().unwrap_or_default();
    for (name, value) in plugins {
        if name != "vision" && name != "web" && value.as_object().is_some_and(|map| !map.is_empty())
        {
            planned
                .left
                .push((format!("plugins.{name}"), "no-match".to_string()));
        }
    }
    let qq = old["platforms"]["qq"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    if qq.keys().any(|key| key != "admin_users") {
        planned
            .left
            .push(("platforms.qq".to_string(), "im".to_string()));
    }
}

fn change(key: &str, value: Value) -> Change {
    Change {
        key: key.to_string(),
        value,
    }
}

/// 新版供应商编号的写法（小写字母开头，小写字母、数字、`-`、`_`，最长 32 个字）：照不上的换成能用的。
pub fn new_id(old: &str) -> String {
    let mut id: String = old
        .trim()
        .chars()
        .map(|ch| {
            let ch = ch.to_ascii_lowercase();
            if ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect();
    if !id.starts_with(|ch: char| ch.is_ascii_lowercase()) {
        id.insert_str(0, "p-");
    }
    id.chars().take(32).collect()
}

/// 键里人起的一段：只有字母、数字、`-`、`_` 的照写，别的带双引号（照 TOML 转义）。
pub fn segment(name: &str) -> String {
    if !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        name.to_string()
    } else {
        format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

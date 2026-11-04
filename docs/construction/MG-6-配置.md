# MG-6 配置和密钥：只补新版没有的

照细则第四节那张表。旧版 `config/config.jsonc`（JSONC：去掉注释、末尾多的逗号再读）和属主档案，写进新版的系统配置和密钥文件。

## 做什么

1. **排**（`settings/plan.rs`，只排不写）：
   - 供应商：旧版的 CLI 中转（`claude-code`、`codex`、`antigravity`、`codebuddy`）不转、开着的报 `cli-relay`；出厂模板没填 key
     （`anthropic` 的 `$env:ANTHROPIC_API_KEY`、`opencode` 的 `public` 算没填）、也没加模型的不转。编号照新版的写法改（小写、
     非法字符换 `-`、不是字母开头的加 `p-`、最长 32），改了的报告写「旧 → 新」。新版已经有同编号的整家跳过（`existing`）。
   - 一家写 `name`、`base_url`、`driver`（`anthropic` 照写、别的 `openai-chat`）、`key`：`$env:VAR` 写 `{env = "VAR"}`，别的写
     `{secret = "provider-<编号>"}`、值进密钥；多个 key 只迁第一个，其余数出来（`more-keys-N`）。
   - 模型：`model_context_window` → `window`，`model_temperature` → `temperature`，`model_costs` → `price.*`（单位一样，每一百万 token；
     `cache` 是 `cache_read` 的旧名，两个都写了的照 `cache_read`），币种 `cny`/`rmb`/`¥` 写 `CNY`、别的 `USD`。
   - `active_provider` + 它的 `default_model` → `models.chat`；远程的向量 → `models.embedding`（本地的不转）；看图 → `models.vision`；
     挡位 `lite`/`standard`（`balanced`）/`flagship`（`strong`）→ `pools.lite/normal/flagship.models`，`cheap` 报 `no-match`。
   - 搜索：`tavily_api_keys`、`exa_api_keys` 的第一个 → 密钥 `net-search-tavily`/`net-search-exa` 和 `net.search.<家>.key` 的引用；
     `searxng_base_url` → `net.search.searxng.url`；`firecrawl`、`anysearch` 报 `no-match`。
   - 属主档案 → `profile.about`（超过 2000 字的不截，报 `too-long`）；`platforms.qq.admin_users` → `external.bindings."qq:<号>" = "owner"`。
   - 别的整段（`context`、`tools`、`cache`、`display`、`notifications`、`voice`、`mcp`、`skills`、`plugins` 的其余）报 `no-match`，
     `platforms.qq` 的其余报 `im`（归 im 管）。
2. **写**（`settings.rs`）：
   - 密钥先 `secret.list`，名字已经设了的跳过（`already-set`），没有的 `secret.set`。值只交给核心：报告、日志只写名字，`Secret`
     的 `Debug` 也只印名字。
   - 设置一项一发 `config.set`，每项带 `expect: {}`：回 `config_conflict` 的算新版已经配过（`already-set`），别的拒绝写原因码。
   - 写完 `config.get` 读回比一遍（数照数值比），对不上的数出来。

## 怎么验

- 单元：编号、键的引号；一份旧配置排成哪几项、几个密钥、不转的为什么（`Debug` 里没有密钥的值）；超长的关于你、本地的向量；
  anthropic 的驱动；两个缓存价的名字只写一个。
- 进程里起真核心（配置清单照真核心拼：包声明的项也在，`miyu-core` 开 `net`）：写进去的读得回来，密钥只有名字，两份报告里没有
  密钥的值；再跑一遍什么都不写。
- 演练（主人旧根的副本，`config.jsonc` 里像密钥的值先换成占位，演完连临时数据根的密钥文件一起删）：第一遍写了 80 项设置、18 个
  密钥，没有一项被新版拒，读回都对；第二遍 17 家供应商都是新版已经有的、另 4 项已经配过，什么都没写；不转的整段 22 处、QQ 的设置
  1 处。

## 坑

- 测试里的核心要照真核心那样拼配置清单（`Packaged::of` + `Config::load`，同 miyu-im 的测试），只用 `Core::new` 的默认清单的话
  供应商、模型、搜索这些包声明的键都是 `unknown_config_key`；搜索的键在 `miyu-core` 的 `net` 里。

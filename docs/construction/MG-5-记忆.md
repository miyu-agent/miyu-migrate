# MG-5 记忆：旧版的知识点导进新版的记忆

照细则第三节和记忆类别对照（记忆线 11-03 起草、core1 审过）。

## 做什么

1. **读**：旧根 `personas/<scope>/memory/memory.db`（老布局 `data/personas/…`）的 `facts`，没有 `user_version`，照列名读，缺列当默认。
   每个人格一个库，照会话用的那一张人格映射找新人格；映射不上的整个库不导，报告写 `unmapped`。
2. **不导的**：`status` 不是 `active` 的（`inactive`）、`self`（她自己的）、空的、正文超过 120 字的（`too-long`，不拆、不调模型）、
   时刻读不懂的（`bad-time`，核心会整批拒）。超长的写进给 agent 的报告 `pending`（`memory-too-long`：库、`facts`、行号、
   `origin`、原来的类），不带原文；以后改短了照同一个 `origin` 导，不会重复。
3. **说的是谁**：`subjects` 里和这一条 `owner_principal` 相同的那个，没有相同的取第一个；旧版的身份哈希照同一个库 `episodes` 的
   `owner_principal` → `origin_sender_id`（`origin_platform = onebot`）换成 `qq:<号>`，一个哈希换出几个号的当换不出；换不出的退到
   名字，名字也没有的当没写是谁。
4. **类**：说的是主人（号在主人的号里）→ `user`；说的是别人 → `person`（`who`）；没写是谁的：`preference` → `feedback`、
   `task` → `episode`、别的 → `reference`（判不了 `user` 还是 `reference` 的一律 `reference`，报告里数「建议→导成」）。
5. **主人的号**：新版 `config.get` 的 `[external.bindings]`（整张表一格、拆成 `external.bindings.<号>` 两种都认）写了的；没有的看
   旧配置 `config/config.jsonc` 的 `platforms.qq.admin_users`（JSONC：去掉注释、末尾多的逗号）。都没有的说的是谁的一律当别人，
   报告里写一句。
6. **其余格**：`text` 是 `content` 去两头空白；`at` 是 `updated_at`（读成时刻再写成 RFC 3339 的 UTC）；`keywords` 是 `tags` 用
   「, 」连起来，超过 200 字的在逗号处截；`origin` 是 `miyu-legacy:<旧人格>:facts:<id>`。
7. **导**：每个人格一批不超过 500 条 `memory.import {persona, items}`；整批被拒的（记忆软件包没装、`bad_params`）这个人格停下，
   那一批写 `refused`；再 `memory.dream {persona}`，回 `memory_busy` 的隔十秒再问、最多五分钟，等不到、整理没成的写进报告，不算
   失败。
8. 日记 `episodes`、改动记录、向量、`evicted_context.db` 只在压缩包里（会话那一类已经把被挤出的轮拼回去了）。

## 和对照表不一样的

- 对照表第 4 节说 `self` 列「原文」进报告：报告一律不带内容（细则第十二节），只写在压缩包的哪个库、`origin`、为什么。

## 怎么验

- 单元：判类的每一种、不导的每一种、身份哈希换号（一个哈希两个号的不认）、关键词截在逗号、分批、整理忙着等、整批被拒停下、
  主人的号先新配置再旧配置。
- 进程里起真核心：导进新建的 Miyu，`memory.list` 照类、原来的时刻对；再跑一遍全是以前导过的。
- 演练：主人旧根的副本两遍。`default` → 新建的 Miyu 导进 119 条，没导 `self` 2、超长 1，没写是谁的 22 条照推荐导成 `reference`，
  1 条号换不出记成名字；`dev` → 软件工程师导进 2 条，没导 `self` 4、超长 33；第二遍全是以前导过的。演练的核心没有模型，整理回
  `memory_unavailable`（报告照写）；演练的副本没有配置，没认出主人。和对照表 11-03 数的对得上（default 122 条、dev 39 条）。

# MG-3 会话：旧会话库迁成新版的会话存档

照细则第一节，分三步做，每一步都要在主人旧根的副本上演练。

| 步 | 做什么 |
|---|---|
| 上 | 本机会话（没绑场所的 `user`、`voice`）：字、工具、附件；快照、打包、导入、核对 |
| 中 | QQ 场所会话：私聊照原样，群的一轮照每行开头的号拆；被挤出上下文的轮拼回去 |
| 下 | 工具出的图、内嵌媒体、产物、提问、溢出、插话、中断轮的流水；子代理作子会话 |

## 上

**读**（解出来的副本，照列名读，缺的列当没有）
1. 会话库照布局找；`PRAGMA user_version` 比 40 新的整库不迁、报告写明；没有 `sessions`、`turns` 表的当空的。
2. 取 `kind` 是 `user`、`voice`、`owner` 空的会话；绑了场所的（`platform_session_bindings` 有它）留给「中」，`subagent` 留给「下」，
   `ask` 不迁，成员的（`owner` 非空）不迁。这几种报告里都数出来。
3. 一轮一行 `turns`，照 `seq` 排；`is_summary = 1` 的摘要行跳过，`hidden = 1` 的照常导。

**编号、快照**
4. 新会话编号：UUIDv7，时间段是 `created_at` 的毫秒，随机段取 `blake3("miyu-legacy\0" + 会话库相对旧根的路径 + "\0" + 旧编号)` 的前
   74 位。重跑同一个编号，导入回 `present`。
5. 快照：每个用到的新人格经 `session.create {persona}` 建一个临时会话，读它第一条的 `policy`，从数据根 `root/blobs/` 取快照，再
   `session.delete`。人格映射不上的旧 scope 用不带人格建的那一份，报告里写明。

**事件**（JSON 照 `kernel/events.md` 拼，交核心 crate 的 `Event::from_line` 读一遍、`SessionLog` 写，不手拼行）
6. `session.created`：`venue: local`、`policy`、`permission {workspace, read_only: false}`、`persona`、`titles: false`；名字不空的接
   `session.meta_changed {title}`（截到 200 字）。
7. 一轮：`message.user`（字取 `display_content`，空的取 `user_content`；`user_attachments` 里这一轮的接在后面，图片写图片块，别的
   写文件块，字节取 `data` 列或 `state/attachments/<编号>/<文件名>`）→ `turn.started {trigger}` → `tool_flow` 每一轮一条
   `message.assistant`（思考、字、`tool_call`，`call_id` 照账本重编 `call_<seq>_<k>`，`args` 是原串）加上每个调用一条
   `tool.result`（`status` 照 `replay_journal` 里同名的 `ok`，没有的 `ok`；`duration_ms` 照 `finished_ms - started_ms`）→ 最后一条
   `message.assistant`（`by` 是 `assistant_provider_id`/`assistant_model`，空的写 `legacy`）→ `turn.ended`（`completed` 照写，
   `interrupted`、`running` 写 `interrupted`）。运行中的占位正文当没有。
8. `at` 一律 UTC 毫秒；`cause` 写 `legacy-<旧回合编号>`；不写 `model.called`（用量先存档）。

**包、导入、核对**
9. 一个会话一个包：日志写进暂存目录，blob 放进暂存的 blob 库，交核心 crate 的 `bundle::write` 打包（引用了、库里没有的进
   `absent`）；`session.import {path, archive: true}`。
10. 每个会话数轮数、附件数，导入回 `sessions` 或 `present` 的算迁了；报告逐类写条数，没迁的写为什么。

## 中

1. 场所：`platform_session_bindings` 里平台是 `onebot` 的，`venue = qq:<机器人>:<种类>:<号>`；群的 `confined: true`。别的平台的整会话
   不导（报告写 `platform-<平台>`）。
2. 群的一轮照每行开头的号拆（细则第一节第 11 条，格式照旧版 `real_context/targeting.rs`）：只拆后三节，`[Prior group chat records]`
   不拆，她自己的 `[you]` 不拆；名字、编号、正文是 JSON 转义过的，照 JSON 读回。一行一条 `message.user`，`by` 是
   `external qq:<号>`（拆不出的 `qq:unknown`），`venue` 那一格记 `msg`、`name`、`reply_to`、`mentions`、`mentions_me`、`show_ids`；
   引用的原文接在正文后面一段。第一条引起这一轮，别的带这一轮的 `turn`、排在 `turn.started` 后面。一行都拆不出的整段一条
   `qq:unknown`。
3. 私聊：一条，`by` 是对方 `qq:<号>`。
4. 被挤出上下文的轮：读 `state/personas/<scope>/memory/evicted_context.db`，照 `source_id` 的 `<turn_id>:<哪一句>` 拼成一轮，照
   `origin_session_id` 插回原来的会话，和剩下的轮照人说话的时刻归并（两边各自的先后不变，同一时刻剩下的在前）。排着并进来的话
   （`followup:`、`before:`）各一条带这一轮的 `message.user`，号、编号照正文后面的可信元数据，没有的 `qq:unknown`。原来的会话不在了
   的、`origin_session_id` 是空的，照人格归进一个本机存档「旧版被挤出的对话」。

## 下

1. 工具的附带照调用的旧编号接在那一条 `tool.result` 的字后面：`image_assets`（图）、`turn_inline_media`（有字节的图写图片块，视频写
   文件块，`kind = text` 的视觉文字描述不接；没字节、`source` 是旧根里的路径的照副本读，读不到写一块字说没迁过来）、
   `artifact_assets`（文件）、溢出 `state/spill/<会话>/<turn>-<call>-<工具>.txt`（文件）。旧版接入 QQ 的工具出的图编号是
   `bridge_<工具名>_<第几张>`，对不上调用：照工具名排队，同名的调用照先后一次领一张。对不上调用的（那一轮没有工具细节了）数出来，
   留在压缩包里。
2. 提问 `question_exchanges`：一次写成一次来回，调 `ask_user`（参数是问的题），结果是「问：…／答：…」，排在工具流后面、最后的
   回复前面；最后的回复和工具流最后说的一样的照旧不写两遍。
3. 插话 `queued_prompts`（`consumed`、挂在这一轮的）：照 `seq` 各一条带这一轮的 `message.user`，排在开轮后面、第一条回复前面。
   本机的取给人看的字；场所会话取交给模型的原文，照可信元数据认号。
4. 只有展示转录的轮（没有 `tool_flow`）：照转录拼来回，字、思考接在这一次，调用排进这一次，结果对上这一次头一个还没结果的同名
   调用，`ok: false` 写 `error`；这一次的调用都有结果以后再说话、再调用是下一次。参数、输出被旧版截过，报告里数出来。
5. 子代理会话：顺着 `parent_session_id` 找到根（不是子代理的那一个），作子会话和根一个包：`parent` 是直接的父会话的新编号，
   `depth` 照 `sessions.depth`，旧库写 0 的照往上几层算。根不在要导的里面的报 `orphan-subagent`。
6. 运行中的轮：主人那份没有（0 轮），照中断处理，流水 `turn_journal_events` 不读。

## 怎么验

- 单元：编号（同输入同编号、时间段对）、时刻写法、每种轮拼出来的事件。
- 进程里起真核心：造一个旧会话库（带附件、工具、中断、摘要行、隐藏行），迁进去，`view.page` 打得开、`session.list` 里是存档；
  再跑一遍全是 `present`。
- 主人旧根的副本上演练两遍。上：本机会话 85 个、178 轮。中：一共 118 个会话（本机 85、QQ 33）、3028 轮，被挤出的 2607 轮全部拼回
  （和细则草稿 2 在拷贝上数的一样）；`qq:unknown` 6 条，都是被挤出的轮里没有可信元数据的 `before:` 行（旧版那时还不写），查过
  正文里也没有别的号的写法。第二遍全是以前导过的。下：3040 轮（加上 12 个子代理会话的 12 轮），子代理 12 个全作子会话（有一个
  是孙代理）；工具结果后面接了 161 张图、文件（工具出的图 37、内嵌的图 124），对不上调用的 2 样（一张那一轮没有工具流、一份溢出
  属于被挤出的轮），另 2 份溢出属于旧库里已经没有的会话、只在压缩包里；提问 18 次；只有展示转录的 24 轮。

## 坑

- **核心建目录只认数据根里面的**（核心施工 TMP-1：防的是往删掉了的数据根里写）：`SessionLog::create` 建会话目录、`Blobs::put`
  建 `<前两位>/` 和临时目录，往上找不到 `.miyu-layout` 的报 `NotFound`。迁移程序的暂存可能不在数据根里（状态目录是人指的），
  所以暂存的会话目录先自己建好再交给 `SessionLog`，暂存的 blob 照核心的布局（`blobs/<前两位>/<64 位>`）自己写、读还交给
  `Blobs`。测试 `a_state_folder_outside_the_data_root_still_imports` 守着这一条。以后别的工具从数据根外面造日志也会撞上。
  这就让迁移工具耦合了核心的 blob 布局：核心改 blob 布局时这里要跟着改（读和打包走 `Blobs`，布局一变暂存的读不出来，测试会红）。
  以后要是有第二个工具也要在数据根外面暂存，就在核心给 `Blobs` 加一个「数据根外的暂存」入口，别再各写一份。

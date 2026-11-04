# MG-4 QQ 消息记录：旧版的消息记录导进 im

照细则第二节。旧版 `data/platforms/onebot/message_history/history.sqlite3`（第 4 版）转成 im 的 JSON Lines（im 设计 04 第四节，
施工 I-127），交 `miyu im records import` 导进 im 的库。

## 做什么

1. **读**：`PRAGMA user_version` 要是 4，别的整库留在压缩包里、报告写明。`messages` 照 `id` 升序，`recalls` 整张表。
2. **一行消息**：`venue = qq:<account_id>:<conversation_kind>:<conversation_id>`、`msg` 照原样（合成的 `ingress-…`、`miyu-…`、
   `group_file_…` 也原样）、`sender = qq:<sender_id>`、`at = sent_at × 1000`、`text`、`name`（空的不写）、`reply_to`（空的不写）、
   `mentions`（字符串数组、对象数组两种都认，写 `qq:<号>`，读不懂的当没有）、`media`（`audio` 写 `voice`、`other` 写 `file`，
   `id` 取 `media_id`、没有的取 `label`，`name` 取 `label`，两样都没有的不写）、`is_bot = 1` 写 `mine: true`。
3. **撤回**：写在消息行上，`recalled_at` 以消息行的为准、没有的取 `recalls` 同键那一行；`recalled_by` 取那一行的
   `operator_id`。`recalls` 里有、`messages` 里没有的写一行 `happening`（`recalled`），照键排、排在全文件最后。
4. `platform` 不是 `onebot` 的跳过、数出来。`context_boundaries` 不迁。
5. **导**：整份写进这一次的目录 `qq-records.jsonl`（先写临时文件再挂上），交 `miyu im records import <文件>`（照 `MIYU_PROGRAM`，
   用英文说，照它的话数新收几条、本来就有几条、编号被占几条）。im 没装、导不了的：文件留着，报告写「装上 im 以后跑这一行」和
   im 说的头一句。迁移程序不停、不起 im（导入有 5 秒的忙等，im 开着也能导）。
6. **重导、和新 im 并行接过的重叠**：im 照 (场所, 平台编号) 去重，发的人、时刻一样的算本来就有，不一样的算编号被占、留着 im 里
   的（报告写 `partial`）。

## 怎么验

- 单元：一行消息的每一格、两种 @、几种媒体、她自己发的、撤回带谁撤的、消息不在了的撤回排最后、不是 QQ 的跳过、版本不对的不转；
  im 说的条数照数读；im 导成、导不了、没装三种报告。
- 演练：主人旧根的副本转出来，交真的 im（im main 的那一版，状态目录指到演练目录）导进主人现在 im 库的一份只读拷贝（主人 11-04
  准）：数新收、本来就有、编号被占几条；再导一遍应当全是本来就有。

## 演练的数

主人旧根的副本，im 是 im main `be8acd2` 编的 `miyu-im`（状态目录指到演练目录，`MIYU_HOME` 是演练的新数据根，资源目录取部署的
`src/resources`、包目录取 im 那一版的 `miyu-im/`），导进主人 im 库的一份只读拷贝（拷时 44996 条消息、1316 件群事件，`quick_check`
过）：

- 转出 211292 条消息、9 件撤回（消息不在了的）；
- 第一遍：新收 211291 条消息、9 件群事件，本来就有 0 条，编号被占 1 条（留着 im 里的）；im 的消息 44996 → 256287，正好加上；
  `quick_check` 过；
- 第二遍：新收 0，本来就有 211300，编号被占 1；
- 本来就有 0 条是对的：旧版最后一条记在 10-27 06:19（UTC），新 im 自己记的第一条在 10-27 06:20，桥是换过去的，没有并行接的那一段。

## 坑

- im 的程序要找资源目录、自己的包目录（字在 `miyu-im/data/human/`）：从 `miyu im` 转交时都有；演练直接跑 `miyu-im` 要给
  `MIYU_RESOURCES`、`MIYU_PACKAGE_DIR`，不给的报「找不到资源目录」，给了资源目录、没给包目录的帮助里印的是键名。

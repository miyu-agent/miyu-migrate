# MG-2 人格：连上新核心，旧人格映射成新人格

会话、记忆都要写新版的人格编号，所以人格先迁（细则第十三节第 3 条的顺序）。这一步另外顺手改 core1 审 MG-1 时记下的两处。

## 做什么

1. **连核心**：照 miyu-im 的写法用 `miyu_client::open::Core::connect` 连（读 `run/socket`、本机令牌，核心认成本机的人；没在跑就照
   `miyu core` 拉起）。数据根照 `DataRoot::locate`。请求编号每次换：`migrate-<进程号>-<时刻>-<n>`（同一个编号核心照旧回应不执行）。
   迁移逻辑不直接拿连接，拿一个「一问一答」的口子（`Core` trait），测试换成假的或进程里的真核心。
2. **读旧人格**（只读解出来的副本）：
   - `default`：旧版的 Miyu。提示词取迁移包带的旧版仓库原文（`legacy/miyu.md`、`miyu.hint.md`、`miyu-dialogs.md`，旧版
     `5df19e2f`）；`data/prompts/hints/default.md`、`dialogs/default.md` 存在的盖在上面（空的就是停用：写空的字、空的对）。
   - `dev`：映射到出厂的 `software-engineer`，不改它；`config/dev-prompt.md` 非空、不是那句旧默认的，报告里写明在压缩包的哪儿。
   - `data/prompts/<名字>.md`（`system-prompt.md` 除外）每一个：名字是文件名去掉 `.md`，scope 照旧版的规整（小写、非
     `[a-z0-9_-]` 换 `-`、去首尾 `-`，空或 `md` 的是 `persona-<blake3 前 12 位>`）；`hints/<scope>.md`、`dialogs/<scope>.md` 同上；
     `<名字>.json` 的 `avatar_path` 指的图（旧绝对路径照旧根换到副本里）。
   - 旧示范对话照旧版的读法拆成一对一对（行首 `user`/`assistant` 不分大小写、冒号全半角都行，没配上回复的尾巴丢掉），交 `pairs`
     由核心写成新写法。
   - `personas/<scope>/persona.toml` 的子系统、插件白名单：新版没有对应，只列进报告。
3. **建、映射**：先 `persona.list`。新版已经有同名人格的（名字一字不差）不建、映射到它、不改它；`dev` 映射到
   `software-engineer`（不在的报告里写明，映射成空）；其余经 `persona.set` 新建（不写编号）：`changes` 写 `persona.name`，
   `prompts` 写 `persona`、`reminders`、`examples`（只写有的）。有头像的先 `blob.put {path}`，再 `persona.set {persona, avatar}`。
   重跑：同名已经在了，就是映射，不重复建。
4. **报告**：`给agent.json` 加 `personas: [{old, name, new, how}]`（`how` 是 `created`、`existing`、`engineer`、`missing`）；
   `items` 加一项 `personas`；子系统、插件白名单、`dev-prompt.md`、`system-prompt.md` 进「没迁的」。
5. **core1 记的两处**：
   - 这一次的目录 `<状态目录>/<时刻>/` 已经有了的拒绝（不盖）。
   - 打包前看状态目录那块盘的剩余空间：要旧根大小的两倍（压缩包按最坏不压缩算，加上解出来核对的那一份），不够的动手前说要多少、
     有多少。
6. 解出来的副本留到各类都迁完再删（人格要从它读）。

## 不做

- 不迁会话、记忆（下一步）；不改出厂人格；成员的私有人格不迁。

## 怎么验

- 纯逻辑：scope 规整、示范对话拆法、旧人格清单（`hints`/`dialogs` 有、空、没有三种）的单元测试。
- 进程里起真核心（照 miyu-im 测试的起法，资源目录用钉住那份核心检出的 `resources/`）：迁一个造的旧根，`persona.list`/`persona.read`
  核对建出来的名字、提示词、示范对话；再跑一遍，什么都不建、映射照旧。
- 空间不够、目录已经有了：各一个测试，什么都不写。

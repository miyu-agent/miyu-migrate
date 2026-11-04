# miyu-migrate

把旧版 Miyu（0.7.x）的数据目录迁进新版 Miyu。新版的软件包：装上以后多一条 `miyu migrate`。

```
miyu pkg install miyu-migrate
mv ~/.miyu ~/.miyu-0.7        # 先停掉旧版，再把旧目录挪开
miyu migrate ~/.miyu-0.7
```

它先把旧目录整个打成压缩包、逐个文件核对，然后只从解出来的副本读，经新版核心的协议写进去，最后出两份报告：一份给人看，
一份给 agent 读（写明没迁的东西在压缩包里的哪个路径、旧格式是什么样，要的时候让 agent 接着迁）。旧目录不动。

迁移程序连新版核心的身份就是在终端里跑它的人（读数据根的 `run/token`），核心没在跑会照 `miyu core` 拉起。动手前会先查：旧版
停了没有（`daemon.lock`），状态目录那块盘够不够（要旧目录大小的两倍）。

现在迁的：

- **人格**：旧版的 Miyu 新建成名字叫 Miyu 的人格（提示词取旧版原文，`crates/miyu-migrate/legacy/`，你改过的角色扮演提示、
  示范对话盖在上面）；`data/prompts/` 里自己建的每一个新建一个；开发模式映射到出厂的软件工程师。新版里已经有同名人格的不建，
  直接用它。人格的子系统、插件开关新版没有对应，写进报告。
- **本机会话**：每个会话导成一个只读存档（能翻、能搜，不能接着聊），字、工具调用和结果、上传的附件都在；摘要行是派生的，
  不导。会话编号由旧编号算出来，重跑不会重复导。QQ 里的会话、子代理的会话还没迁（下一步）。

新版核心一个字节都不认旧格式：旧用户都迁完以后，这个包下架、仓库归档。

## 开发

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

许可证：AGPL-3.0-or-later，见 `LICENSE`。

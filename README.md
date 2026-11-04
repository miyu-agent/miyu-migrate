# miyu-migrate

把旧版 Miyu（0.7.x）的数据目录迁进新版 Miyu。新版的软件包：装上以后多一条 `miyu migrate`。

```
miyu pkg install miyu-migrate
mv ~/.miyu ~/.miyu-0.7        # 先停掉旧版，再把旧目录挪开
miyu migrate ~/.miyu-0.7
```

它先把旧目录整个打成压缩包、逐个文件核对，然后只从解出来的副本读，经新版核心的协议写进去，最后出两份报告：一份给人看，
一份给 agent 读（写明没迁的东西在压缩包里的哪个路径、旧格式是什么样，要的时候让 agent 接着迁）。旧目录不动。

新版核心一个字节都不认旧格式：旧用户都迁完以后，这个包下架、仓库归档。

## 开发

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

许可证：AGPL-3.0-or-later，见 `LICENSE`。

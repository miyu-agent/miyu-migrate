# MG-7 表情包：旧版的用户库做成 miyu-memes 的内容包

照细则第七节。叠在 MG-6 上（都改 `run.rs`）。

## 做什么

1. 只迁用户库 `data/memes/<库>/`（`index.json` 加 `images/…`）；出厂库不在数据根。条目的图不在这个库里的（覆盖出厂条目的）不算，
   `disabled_ids` 不管；一张自己的图都没有的库跳过、报 `no-own-images`。
2. 一个库做成一个内容包，放在这一次的目录 `legacy-memes-<库>/`：
   - 编号照包的写法（小写字母开头，小写字母、数字、`-`、`_`，最多 64 个）；库名一个能用的字都没有的（中文名）用 blake3 前 8 位。
   - `package.toml` 只写 `[package]`（`version = "1.0.0"`，`name` 照「语言到一句话」写成 `{ zh, en }` 的表）和
     `[content.miyu-memes] dir = "memes"`。
   - `memes/index.json` 写 `{"memes": [{file, description, usage, reason}]}`，图照 `file` 拷；`reason` 是「旧版表情包 <库>」，从聊天里
     收来的接上「收集自 QQ 群 <号> 的 <名字>（<号>），<收集时刻>」。`description`、`usage` 有空的那条跳过（`no-text`），`file` 跑出
     库外的跳过（`no-image`）。
3. `package.list` 里有 miyu-memes 的，经 `package.install {path}` 装（同编号再装是换成新的，重跑不多；miyu-memes 照图去重）；没装
   的包目录留着，报告写「装上 miyu-memes 以后跑 `miyu pkg install <路径>`」；核心拒了的写原因码和那一行。

## 怎么验

- 单元：编号、来源的写法、一个库做成的包（自己的图、缺字的、覆盖出厂的、跑出库外的）、只覆盖出厂的库没有包、装上/没装/拒了。
- 进程里起真核心：做好的包 `package.info {path}` 认得、`package.install {path}` 装得上。
- 演练（主人旧根的副本）：两个用户库 `miyu` 350 条、`miyuastrbot-md` 1 条，都做成了包，核心都认得；和原来 `index.json` 的条数、
  自己有图的条数一致（350/350、1/1），没有跳过的。演练的核心没装 miyu-memes，所以没装，报告写了那两行。

## 坑

- `[package] name` 写成一句字（`name = "…"`）核心整份不收（`package_invalid`）：要写成语言到一句话的表 `{ zh = …, en = … }`。集成
  测试一装就抓到了，单元测试只看字照不出来。

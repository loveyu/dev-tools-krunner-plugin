# CLAUDE.md

本文件为 Claude Code（claude.ai/code）在本仓库中工作时提供指引。

## 这是什么

一个 **Plasma 6 KRunner 的 DBus runner**（Rust + `zbus`）。KRunner 调用我们暴露的
session-bus 服务，我们对 `date`/`time` 查询返回多种时间格式，回车时把选中值复制到剪贴板。
MVP 范围是内联的 date/time；后续扩展了 `rand`（随机字符串）、`uuid`（UUID v4 生成）。
设计文档（原始需求 / `README.md`）描述了后续的外部插件管理器架构。

目标平台：KDE Plasma 6 + Wayland（在 6.3.6 / Frameworks 6.13 上开发验证）。

代码风格见 [`docs/CODE_STYLE.md`](docs/CODE_STYLE.md)（核心：注释统一用中文）。

## 常用命令

```bash
cargo build --release              # 构建（cargo 通过 rustup 装在 ~/.cargo）
cargo test                         # 单元测试（纯函数：命令解析 / 时间格式化）
cargo fmt                          # 按 rustfmt 标准格式化（会改写文件）
cargo clippy --all-targets -- -D warnings   # lint，警告当错误
./install.sh                       # 构建 + 部署 + 重启 KRunner（每次改代码后重跑）
./target/release/devtools-runner   # 前台运行（stderr 输出日志），用于调试
```

提交前三件套（格式化 + lint + 测试，按顺序跑一遍）：

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
```

> `rustfmt` / `clippy` 不在 rustup minimal profile 里，缺失时先 `rustup component add rustfmt clippy`。

单元测试覆盖纯函数逻辑（`suffixes_for_query` 的命令解析/精确优先、`value_of` 的格式化、
`build_matches` 的排序与 id 前缀）；与 KRunner/DBus 的集成验证见下文「调试/验证」。

## 调试 / 验证

```bash
# 内省我们暴露的接口/签名：
qdbus6 org.kde.devtools /runner
dbus-send --session --print-reply --dest=org.kde.devtools /runner \
    org.freedesktop.DBus.Introspectable.Introspect

# 直接调用 Match（绕过 KRunner）。注意看 int32（categoryRelevance）和 double（relevance）：
dbus-send --session --print-reply --dest=org.kde.devtools /runner \
    org.kde.krunner1.Match string:"date"

# 打开 KRunner 并只显示本 runner 的结果：
qdbus6 org.kde.krunner /App org.kde.krunner.App.querySingleRunner org.kde.devtools date

# 服务把 Match/Run 调用打到 stderr。DBus 自动激活时 stderr 会被丢弃——
# 所以想看「KRunner 是否真的在调我们」，请自己前台运行二进制（它占有 bus name），看终端。
```

安装位置（用户级，无需 root）：二进制 → `~/.local/bin/devtools-runner`；
KRunner 元数据 → `~/.local/share/krunner/dbusplugins/org.kde.devtools.desktop`；
D-Bus 激活 → `~/.local/share/dbus-1/services/org.kde.devtools.service`。
首次查询时 DBus 会自动激活服务，无需手动启动任何进程。

## KRunner DBus2 协议契约（关键、非显而易见）

- **接口是 `org.kde.krunner1`**（"DBus2" 协议），对象路径 `/runner`，bus name `org.kde.devtools`。
  它**不是** `org.kde.krunner.App`（那是 krunner 自身在 `/App` 暴露的客户端接口）。
  系统上的权威参考：`/usr/share/dbus-1/interfaces/kf6_org.kde.krunner1.xml`。
- **`Match(query: s) → a(sssida{sv})`** —— 每条 match 是结构体
  `(Id, Text, IconName, CategoryRelevance:i32, Relevance:f64, Properties:a{sv})`。
- 另有：`Run(matchId:s, actionId:s)`、`Actions() → a(sss)`、`Config() → a{sv}`、`Teardown()`。
- **第 4 个字段（`i`）是 `categoryRelevance`，不是 "type"。** 系统里那份
  `kf6_org.kde.krunner1.xml` 的注释把它写成 "Type"——**该注释已过时**。
  权威定义见 KRunner 框架 `src/dbusutils_p.h` 中的 `RemoteMatch`：
  `int categoryRelevance = ...::Lowest;`（默认 0）。传 0 会让所有结果掉到最底。
  我们传 `100`（`Highest`）。
- 关键的 `.desktop` 字段（从 `libKF6Runner.so` 里 grep 得出）：`X-Plasma-API=DBus2`、
  `X-Plasma-DBusRunner-Service=<busname>`、`X-Plasma-DBusRunner-Path=/runner`。
  runner 发现目录是 `~/.local/share/krunner/dbusplugins/`。
- 消费端会从 properties 字典读取的键：`subtext`、`category`、`urls`、`multiline`、`icon-data`、
  `actions`。（`categoryRelevance` 是结构体字段，不在字典里。）

## KRunner 结果排序（为什么结果排在那个位置）

来自 `resultsmodel.cpp` 的 `SortProxyModel::lessThan`：
- **类目级**（如 "应用程序"、"DevTools" 这些分组）：按 `(FavoriteIndex, CategoryRelevance)` 排序。
- **条目级**（同一类目内）：只按 `relevance` 排序。

推论：
- `QueryMatch::setCategoryRelevance` 被钳制在 `[0, 100]`；`setRelevance` 在 0 以上**不钳制**。
  所以把 relevance 调到很大只会重排「我们自己的类目内」顺序，无法把我们的类目抬到别的类目之上。
- `categoryRelevance = 100` 是上限。核心 runner（应用程序、系统设置）对其强匹配（如 "日期和时间"
  设置项）也用 `Highest`；平手时 KRunner 按加载/插入顺序排（核心 runner 在前），所以一个 DBus
  runner **无法用程序方式把自己排到其他 `Highest` 核心类目之上**。
- `FavoriteIndexRole` 标注为 `/// @internal`，只来自用户置顶过的 match id（KRunner 里的星标）——
  runner 无法设置。置顶是保证「绝对最前」的唯一途径。

## zbus 的坑（Rust）

- Cargo feature 名是 **`blocking`**，不是 `blocking-api`（`zbus = { version = "4",
  features = ["async-io", "blocking"] }`）。
- `OwnedValue` 没有 `From<String>`/`From<&str>`——要通过
  `OwnedValue::try_from(Value::from(s))` 包装。见 `main.rs` 里的 `str_value`。
- 接口方法用 PascalCase（`Match`、`Run`、`Actions`、`Config`、`Teardown`），使 D-Bus 成员名
  一一对应；`#![allow(non_snake_case)]` 关掉相应 lint。
- 阻塞式服务 = `ConnectionBuilder` 的 `.name().serve_at().build()` 之后接一个 sleep 循环。
  内部的 `async-io` 执行器在后台线程分发消息——**不需要手写 `receive_message` 循环**。

## 新增一经理解查询（以 `parse_*_query` 为入口的模块，如 `rand`、`uuid`）

这类模块的特点是输入格式统一为 `prefix[mode][length]`，解析过程全在各自模块内完成：

- **文件命名**：`src/<功能词>.rs`，用 snake_case 小写（如 `rand.rs`、`uuid.rs`）。
- **触发器命名习惯**：
  - 长前缀：功能的英文全称或其缩写（`date`、`time`、`unix`、`rand`、`uuid`）。
  - 短前缀：全称的首字母（`r` → rand、`u` → uuid、`ts` → timestamp）。
  - 全大写短前缀（如 `UC` / `UUID`）表示「大写输出」变体。
  - 单字符模式修饰符沿用 rand 的约定：`+` 可见字符、`n` 数字、`c` 小写/紧凑、`C` 大写。
- **入口函数**：`parse_*_query(query: &str) → Option<QueryParams>`。
- **构造结果**：`build_*_matches(params: &QueryParams) → Vec<KMatch>`。
- **Run 时再生**：`value_for_*_id(suffix: &str) → Option<String>`（match id 前缀由 `main.rs` 的 `value_for_id` 调度）。
- 在 `main.rs` 的 `Match` 里按调用优先级排列（越精确的越靠前）。

## 新增一种结果类型

在 `ITEMS`（`(id 后缀, 标题, 图标)`）里加一行，并在 `value_of` 加一个分支。
match id 形如 `date:<后缀>`，Run 时由 `value_for_id` 重新计算（时间始终取当前）。

## 新增一个触发命令

触发词与「展示哪些行」的关系由 `COMMANDS`（`(触发关键词列表, item 后缀列表)`）驱动，而
不再是一张写死的 `KEYWORDS`。匹配规则是双向前缀：查询等于/前缀于关键词，或关键词前缀于
查询（`q == k || q.starts_with(k) || k.starts_with(&q)`）。多个命令同时命中时，后缀求并集
去重，再由 `build_matches` 按 `ITEMS` 顺序输出。

- 复用已有结果：直接把新关键词加进对应命令的关键词列表（如 `ts`/`unix` 共用 `["unix"]`，
  `tms`/`tsm` 共用 `["unixms"]`）。
- 别名若与别的关键词有前缀关系（`tsm` 以 `ts` 开头），靠「精确命中优先于前缀命中」解决：
  只要存在任意精确命中，就只采用精确命中的命令，避免 `tsm` 连带触发秒级 `ts`。
- 其余前缀歧义（如 `t` 同时命中 `time`/`ts`/`tms`）由后缀并集去重自然消化。

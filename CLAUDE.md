# CLAUDE.md

本文件为 Claude Code（claude.ai/code）在本仓库中工作时提供指引。

## 这是什么

一个 **Plasma 6 KRunner 开发者工具箱**。轻量能力继续由 Rust + `zbus` Runner 内联处理；
复杂 JSON 与数据转换交互通过 `org.loveyu.DevTools` D-Bus 服务交给 `devtools-workerd`，由 Wry /
WebKitGTK 承载 Vue 3 + Naive UI 工作台。已有能力包括 date/time、rand、uuid、时间戳互转、JSON
Workbench、数据转换、OCR、条形码/二维码识别与生成、纯前端图片压缩、编辑与水印。

Runner 目标平台为 KDE Plasma 6；Worker 支持 Debian 13 KDE Wayland/X11 与 Windows 10+。

代码风格见 [`docs/CODE_STYLE.md`](docs/CODE_STYLE.md)（核心：注释统一用中文）。

## 常用命令

```bash
cd web/devtools-ui
fnm use 26
pnpm install --frozen-lockfile
pnpm check                         # 前端完整门禁并生成 dist/index.html
cd ../..

cargo build --release --workspace  # 构建（Worker 会嵌入前端 dist）
cargo test --workspace             # 所有 Rust crate 单元测试
cargo fmt --all                    # 按 rustfmt 标准格式化（会改写文件）
cargo clippy --workspace --all-targets -- -D warnings
./install.sh                       # 构建 + 部署 + 重启 KRunner（每次改代码后重跑）
./target/release/devtools-runner   # 前台运行（stderr 输出日志），用于调试
./target/release/devtools-workerd --settings  # 打开或激活 Worker 设置页
./target/release/devtools-workerd --launcher  # 打开独立工具启动器（无需 KRunner）
cargo check -p devtools-workerd --target x86_64-pc-windows-msvc
```

提交前三件套（格式化 + lint + 测试，按顺序跑一遍）：

```bash
pnpm --dir web/devtools-ui check
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

> `rustfmt` / `clippy` 不在 rustup minimal profile 里，缺失时先 `rustup component add rustfmt clippy`。

单元测试覆盖纯函数逻辑（`suffixes_for_query` 的命令解析/精确优先、`value_of` 的格式化、
`build_matches` 的排序与 id 前缀）；与 KRunner/DBus 的集成验证见下文「调试/验证」。

## Workbench / Worker 初版架构

- 根 package 仍是 `devtools-runner`；Workspace 成员位于 `crates/devtools-core`、
  `crates/devtools-tools`、`crates/devtools-workerd`。
- Runner 的 `src/json.rs` 优先识别 KRunner 输入框里的 JSON 对象/数组；也支持通过 Klipper D-Bus
  读取剪贴板（Wayland/X11 通用），失败时分别回落到 `wl-paste` / `xclip`。两种入口均限制为 2 MiB。
- KRunner 的 `Run` 只回传 match id，因此直接输入的 JSON 使用不含正文的 `json:inline:*` id，正文
  仅保存于最多 8 条的查询会话内存缓存，执行后单次消费并在 `Teardown` 清空；剪贴板入口仍在 Run
  阶段重新读取。两者最终都通过 `org.loveyu.DevTools.OpenTool("json", payload)` 交给 Worker，Runner
  不记录 JSON 原文，也不加载 GTK/WebKit。
- `src/data_convert.rs` 支持复制文本后输入 `convert`（最短前缀 `co`）或 `cv`；JSON 页面也可直接
  带当前结果切换到转换页。Worker 注册 `JsonTool` 与 `ConvertTool`，复用单个 Tao/Wry 窗口。前端是构建后嵌入的单个 HTML，不启动本地
  HTTP 服务；关闭窗口只隐藏，托盘与 D-Bus 服务继续存活。
- 转换器以异步 codec registry 组织。JSON、JSON5、YAML、XML、表格、Query、Cookie、Postman、
  URI/JWT、Base64/Gzip 与 URL Encode 优先由 TypeScript 实现；PHP 三种格式只在 Worker 探测到
  `php` 时启用，通过受限固定脚本执行。禁止 PHP 对象反序列化，不迁移旧版 `eval` 解析。
- `ocr` 通过 Worker 受限调用本机 Tesseract；`barcode` / `bar` / `qr` / `qrcode` 的识别通过
  ZBar。图片最多 10 MiB，子进程固定参数、30 秒超时、8 MiB 输出；命令缺失时仅禁用对应能力。
  QR、Code 128、Code 39、EAN-13 生成由 WebView 中的 `@bwip-js/browser` 纯前端完成。
- `compress` / `squoosh` / `image-compress` / `imgcompress` 打开图片压缩页。解码、缩放、
  JPEG/WebP/PNG 编码、前后对比与下载全部由 WebView TypeScript + Canvas 完成，不把图像发送给
  Rust、后端或网络；Rust 只负责工具注册和页面路由。
- `editor` / `image-editor` / `edit-image` / `imageedit` / `imgedit` 打开 TOAST UI Image Editor。
  图片选择、拖放、粘贴、编辑、复制 PNG 和 PNG/JPEG 导出均在 WebView 内完成，并关闭使用统计。
- `watermark` / `wm` / `image-watermark` 打开纯前端图片水印。文字/图片平铺水印、角度、透明度、
  间距、复制和导出均由 TypeScript + Canvas 完成。交互参考 TransparentLC/watermarker，但不复制其
  AGPL-3.0 源码；图片工具页面底部保留原项目入口，README 维护第三方代码来源与许可证说明。
- Worker 业务层固定为 `Application -> WindowManager -> WebViewManager / IPC -> Platform`；业务模块
  不得出现操作系统条件编译或 GTK/WebKitGTK/WebView2 API。`target_os` 选择只能位于
  `src/platform/mod.rs`，Linux 与 Windows 具体实现分别放在 `platform/linux.rs`、`platform/windows.rs`。
- 托盘基于 KDE StatusNotifierItem，菜单固定为设置/重启/退出。配置位于
  `$XDG_CONFIG_HOME/devtools/settings.json`（默认 `~/.config/devtools/settings.json`）；开机启动入口为
  `$XDG_CONFIG_HOME/autostart/org.loveyu.DevTools.desktop`。主题设置支持跟随系统（默认）、浅色和深色；
  语言支持自动识别（默认）、简体中文、繁体中文和英语，并同步到 WebView、Naive UI、图片编辑器与托盘菜单。
- Worker 另有可独立于 KRunner 使用的类 KRunner 启动器。启动器快捷键与原生快速输入快捷键均默认关闭；
  Linux Wayland 快捷键走 XDG GlobalShortcuts portal，X11/Windows 走平台注册接口。
- 原生快速输入不是 WebView：Linux 使用 GTK Entry，Windows 使用 Win32 Edit。窗口按指针所在显示器工作区
  裁剪尺寸和位置，Enter 后在原焦点应用回填；历史按 JSONL 写入用户数据目录。X11 回填依赖 `xdotool`，
  KDE Wayland 回填通过 XDG RemoteDesktop portal 取得键盘权限，Windows 使用 `SendInput`。
- WebView 单文件超过 WebView2 `NavigateToString` 限制，因此 Windows 使用 Wry 进程内自定义协议加载嵌入 HTML；
  Linux 仍直接使用 `with_html`。两者都不能启动 localhost 服务。
- i18n 消息 key 必须是唯一的 ASCII 语义 key，不得使用中文原文；简中/繁中/英语消息表 key 必须完全一致。
- 前端固定 fnm + Node 26 + pnpm 11；Vue SFC 样式只用 SCSS。`pnpm check` 的 warning 上限为 0，
  依次执行 peer 检查、Prettier、类型感知 ESLint、Stylelint、`vue-tsc`、Vitest 覆盖率、Vite 构建、
  `tsx` 单文件产物校验。
- Debian 13 构建 Worker 需要 `libgtk-3-dev` 和 `libwebkit2gtk-4.1-dev`。Wry 必须通过
  `WebViewBuilderExtUnix::build_gtk` 挂入 GTK 容器，才能同时支持 KDE Wayland 和 X11。

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

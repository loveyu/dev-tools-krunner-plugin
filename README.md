# krunner-devtools

一个面向 KDE Plasma 6 的开发者工具箱：轻量操作直接在 **KRunner Runner** 中完成，复杂操作由常驻 Worker 打开原生 WebView 工作台。

> **Alt + Space → 输入 `date` → 看到多个结果 → 回车复制**

## JSON Workbench

打开 KRunner 后，可以直接输入或粘贴 JSON 对象/数组，无需命令前缀，Runner 会自动识别：

```json
{"name":"loveyu","items":[1,2,3]}
```

也可以先复制合法 JSON，再输入 `json`（输入 `j` / `js` 等前缀时也会提示），从剪贴板打开同一个工作台。

选择“打开 JSON Workbench”后，Runner 通过 D-Bus 按需激活 `devtools-workerd`，在 WebView 中提供：

- 格式化与压缩预览
- JSON 树视图
- 按键、路径和值搜索
- 通过 Rust IPC 复制当前结果

JSONPath、编辑、Diff、Schema、导出等不在本次范围。JSON 内容不会写入日志、上传网络或自动持久化，单次 JSON 输入限制为 2 MiB。直接输入的正文只保存在 Runner 的有界查询会话缓存中，match ID 不包含正文，执行或会话结束后即清理。

## 数据转换

数据转换有两个入口：

- 复制待转换文本后，在 KRunner 输入 `convert`（支持 `co` 等前缀）或 `cv`。
- 在 JSON Workbench 点击“数据转换”；当前 JSON 会传入转换页，返回时原 JSON 页面状态保持不变。

转换页会自动探测来源格式，也允许手动指定来源/目标和交换两侧数据。首版优先在 WebView 内以 TypeScript 完成转换：

- JSON / JSON 深度解码 / JSON 压缩 / JSON5 风格 JS Object
- YAML / XML / CSV / TSV / TOML（输入）/ INI（输入）
- RFC1738 / RFC3986 Query String、Cookie、Postman Bulk、Raw Line、Plain Text
- URI（输入）、JWT（仅解码不验签）、Base64、Base64 + Gzip、URL Encode

PHP Serialize、PHP VarExport 和 PHP Array 通过 Rust IPC 调用本机 PHP CLI；只有启动时探测到 `php` 可执行文件才会启用。PHP 子进程使用固定脚本、禁用 php.ini、禁止反序列化对象，并带有 2 MiB 输入、8 MiB 输出和 5 秒超时限制。旧实现中依赖 `eval` 的 PHP VarExport 输入没有迁移；CSS 压缩、拼音与简繁转换也暂不纳入首版。

## OCR、条码与二维码

在 KRunner 输入 `ocr` 可打开本地文字识别页，支持选择、拖放或直接粘贴图片。页面可选择 Tesseract 语言和页面分割模式、过滤低置信度结果，并在预览图上显示文字框。OCR 由 Worker 受限调用本机 `tesseract`，图片不会上传网络。

输入 `barcode`（或 `bar` / `qr` / `qrcode`）可打开条码工作台：

- 识别：选择、拖放或粘贴图片，由 Worker 调用本机 `zbarimg`，支持返回多条结果并复制。
- 生成：在 WebView 内纯前端生成 QR Code、Code 128、Code 39 和 EAN-13，可导出 PNG，不依赖后端或本机命令。

图片输入限制为 10 MiB，只接受常见图片 MIME 类型；识别子进程有 30 秒超时和 8 MiB 输出上限。`tesseract` 或 `zbarimg` 缺失时，相应识别入口会显示安装提示，其他功能不受影响。

Worker 使用 KDE 系统托盘图标常驻，菜单固定为“设置 / 重启 / 退出”。设置页同样由 Vue 3 + Naive UI WebView 渲染，可控制：

- 是否显示系统托盘图标（默认开启）
- 是否随 KDE 用户会话开机启动（默认关闭）

隐藏托盘后仍可执行 `devtools-workerd --settings` 重新打开设置页。

## 日期时间

输入 `date` / `time`（或前缀 `da` / `tim`）显示：

| 结果 | 示例 |
| --- | --- |
| 当前时间 | `2026-07-28 10:31:07` |
| Unix 时间戳 | `1785205640` |
| Unix 时间戳 (ms) | `1785205640955` |
| RFC3339 | `2026-07-28T10:31:07+08:00` |
| ISO8601 | `2026-07-28T02:31:07Z` |
| UTC 时间 | `2026-07-28 02:31:07 UTC` |

快捷命令只返回单条结果：

| 命令 | 结果 |
| --- | --- |
| `ts` / `unix` | 秒级 Unix 时间戳 |
| `tms` / `tsm` | 毫秒级 Unix 时间戳 |

## 时间戳 ↔ 时间字符串互转

直接输入**时间戳**或**时间字符串**（无需任何前缀），插件按输入的形状自动判定方向：

| 输入 | 识别为 | 结果 |
| --- | --- | --- |
| `1722902400` | 秒级时间戳 | 本地时间 / UTC / RFC3339 / ISO8601 |
| `1722902400000` | 毫秒时间戳（≥13 位） | 本地时间 / UTC / RFC3339 / ISO8601 |
| `2024-08-06 12:00:00` | 本地时间字符串 | Unix 秒 / 毫秒时间戳 / 本地时间 |
| `2024-08-06T00:00:00Z` | RFC3339 / ISO8601 | Unix 秒 / 毫秒时间戳 / 本地时间 |
| `2024-08-06` | 仅日期（本地 00:00） | Unix 秒 / 毫秒时间戳 / 本地时间 |

- 纯数字位数 **≥ 13 视为毫秒**，否则视为秒；短于 9 位不触发，避免普通数字误判。
- 无时区的时间字符串按**本机时区**解释；时间戳方向默认**本地时区优先**，同时给出 UTC。
- 另支持 `YYYY/MM/DD`、RFC2822（邮件格式）等常见写法。

## 随机字符串

输入 `rand` / `r` + 模式 + 长度（未指定长度默认 16 位），大小写不敏感：

| 命令 | 模式 | 示例 |
| --- | --- | --- |
| `rand` / `r` / `r16` / `rand 32` | 字母数字 a-zA-Z0-9 | `r16` |
| `rand+` / `r+16` / `rand+ 32` | 可见字符含符号 | `r+16` |
| `rn` / `rn16` / `randn 8` | 仅数字 | `rn` |
| `rc` / `rc16` / `randc 8` | 仅小写字母 | `rc` |
| `rC` / `rC16` / `randC 8` | 仅大写字母（注意大写 `C`） | `rC` |

**回车**复制选中项，并弹出桌面通知。

## 运行环境

- Debian 13 + KDE Plasma 6（Wayland 为主，保留 X11 兼容）
- `rustc` / `cargo`（`install.sh` 会在缺失时自动用 rustup 安装到用户目录）
- fnm + Node.js 26 + pnpm 11
- `libgtk-3-dev`、`libwebkit2gtk-4.1-dev`（Wry / WebKitGTK 4.1 编译依赖）
- `wl-clipboard`（`wl-copy`）、`notify-send`
- `php`（可选；仅用于启用 PHP 格式转换）
- `tesseract-ocr`、`tesseract-ocr-eng`、`tesseract-ocr-chi-sim`（可选；OCR）
- `zbar-tools`（可选；条形码和二维码识别）

## 安装

```bash
sudo apt install libgtk-3-dev libwebkit2gtk-4.1-dev \
  tesseract-ocr tesseract-ocr-eng tesseract-ocr-chi-sim zbar-tools
git clone <this-repo> krunner-plugin
cd krunner-plugin
./install.sh
```

`install.sh` 会：

1. 用 fnm 的 Node 26 执行前端全量门禁并生成单文件 WebView 产物
2. 编译 Cargo Workspace，安装 `devtools-runner` 与 `devtools-workerd` 到 `~/.local/bin/`
3. 放置 KRunner 元数据 → `~/.local/share/krunner/dbusplugins/org.kde.devtools.desktop`
4. 放置 Runner / Worker 两个 D-Bus 自动激活服务
5. 重启 KRunner

之后 **无需手动启动任何进程**：KRunner 查询时，D-Bus 会按需激活服务。

## 使用

- 打开 KRunner（默认 `Alt+Space`），直接粘贴 JSON 对象/数组即可自动识别；也可以复制 JSON 后输入 `json`。
- 复制任意待转换文本后输入 `convert` / `cv`，打开数据转换页。
- 输入 `ocr` 打开文字识别；输入 `barcode` / `bar` / `qr` / `qrcode` 打开条码识别与生成。
- 输入 `date` / `time` / `da` / `tim` 看时间格式，或 `ts` / `tms` / `unix` 取时间戳；也可以**直接粘贴时间戳或时间字符串**双向互转，回车复制。
- 命令行触发（打开 KRunner 并只显示本 runner 结果）：

  ```bash
  qdbus6 org.kde.krunner /App org.kde.krunner.App.querySingleRunner org.kde.devtools date
  ```

## 架构

采用 Cargo Workspace 和两个独立进程。Runner 保持轻量，不加载 GTK/WebKit；复杂工作台交互通过 D-Bus 交给 Worker：

```
KRunner Query
    │  (D-Bus 方法调用)
    ▼
devtools-runner  (org.kde.devtools  @  /runner  :  org.kde.krunner1)
    │
    ├─ Match("{...}" / "[...]") -> 直接识别 KRunner Input JSON
    ├─ Match("json")            -> 检测 KDE Clipboard JSON
    ├─ Match("convert" / "cv") -> 读取 KDE Clipboard Text
    ├─ Match("ocr")             -> 打开本机 OCR
    ├─ Match("barcode" / "qr") -> 打开条码识别与生成
    ├─ Run("json:inline:*" / "json:open")
    │       │  org.loveyu.DevTools.OpenTool("json", payload)
    │       ▼
    │  devtools-workerd -> Tool Registry -> JsonTool / ConvertTool / Media Tools
    │                                      │
    │                               Wry/WebKitGTK
    │                                      │
    │                               Vue 3 + Naive UI
    ├─ Run("convert:open") -> OpenTool("convert", clipboard)
    ├─ 其他 Run(matchId) -> wl-copy + notify-send
    ├─ Actions()      -> a(sss)
    ├─ Config()       -> a{sv}
    └─ Teardown()
```

Worker 默认复用一个窗口和 WebView，关闭窗口只隐藏，进程与托盘继续运行。Web 静态资源构建为单个 `dist/index.html` 并编译进 Worker，不启动 localhost 服务。系统剪贴板、配置和自启动文件均由 Rust IPC 控制。

匹配结构在总线上的签名（来自 `/usr/share/dbus-1/interfaces/kf6_org.kde.krunner1.xml`）：

```
a(sssida{sv})
 │  │  │  │  │   └─ Properties: subtext(复制内容), category ...
 │  │  │  │  └──── Relevance (double)
 │  │  │  └─────── CategoryRelevance (int32)
 │  │  └────────── IconName
 │  └───────────── Text  (KRunner 第一行)
 └──────────────── Id    (Run 时回传)
```

关键文件：

| 文件 | 作用 |
| --- | --- |
| `src/main.rs` | Runner 主体：`org.kde.krunner1` 接口实现、调度分发 |
| `src/json.rs` | JSON 查询、直接输入会话缓存与 Worker D-Bus 调用 |
| `src/data_convert.rs` | `convert` / `cv` 查询与转换工作台调用 |
| `src/media.rs` | OCR / 条码 KRunner 触发词与 Worker 调用 |
| `src/clipboard.rs` | Klipper / Wayland / X11 的共享剪贴板读取 |
| `src/time.rs` | 日期时间查询逻辑（COMMANDS / ITEMS / value_of 等） |
| `src/rand.rs` | 随机字符串生成（RandMode / parse_rand_query 等） |
| `crates/devtools-core` | Context / Action / Tool / Settings 公共协议 |
| `crates/devtools-tools` | 与 UI 解耦的 JsonTool / ConvertTool 业务入口 |
| `crates/devtools-workerd` | D-Bus、单窗口 WebView、托盘、设置、自启动与受限媒体处理 |
| `web/devtools-ui` | Vue 3 + TypeScript + Naive UI + SCSS 工作台 |
| `assets/org.kde.devtools.desktop` | KRunner DBus-runner 元数据（`X-Plasma-API=DBus2` 等） |
| `assets/*.service` | Runner / Worker D-Bus 激活服务模板 |
| `install.sh` | 编译 + 安装 + 重启 KRunner |

## 开发与调试

```bash
cd web/devtools-ui
fnm use 26
pnpm install --frozen-lockfile
pnpm check

cd ../..
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release --workspace

# 直接跑起来（前台，带日志到 stderr）
./target/release/devtools-runner

# 在另一个终端直接调它的方法（不经过 KRunner 也能验证）
dbus-send --session --print-reply --dest=org.kde.devtools /runner \
    org.kde.krunner1.Match string:"date"
dbus-send --session --print-reply --dest=org.kde.devtools /runner \
    org.freedesktop.DBus.Introspectable.Introspect

# 查看注册的接口/签名是否正确
qdbus6 org.kde.devtools /runner

# Worker 接口与设置页
qdbus6 org.loveyu.DevTools /org/loveyu/DevTools
./target/release/devtools-workerd --settings
```

前端严格门禁包含 peer 依赖检查、Prettier、类型感知 ESLint、Stylelint、`vue-tsc`、Vitest 功能测试与关键 JSON/媒体逻辑 100% 覆盖率、Vite 构建和 `tsx` 单文件产物校验。GitHub CI 会在 push / pull request 上重复执行前端门禁以及 Rust 格式、Clippy、测试和 release 构建。修改任意模块后，重新执行 `./install.sh` 即可部署。

# krunner-devtools

一个跨平台开发者工具箱：Linux/KDE 下可从 **KRunner Runner** 使用，Linux（Wayland/X11）与 Windows 也可由常驻 Worker 的独立启动器打开 WebView 工作台。

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

## 图片压缩

在 KRunner 输入 `compress`、`squoosh`、`image-compress` 或 `imgcompress` 可打开图片压缩页。交互参考 Squoosh，但实现保持最小化且完全位于 WebView 前端：图片不会传给 Rust、后端服务或网络。

- 支持选择、拖放和从剪贴板粘贴 PNG、JPEG、WebP、BMP、GIF。
- 使用浏览器 Canvas 在本地编码为 WebP、JPEG 或 PNG；JPEG/WebP 可调质量。
- 按最大宽高等比缩小，提供原图/压缩图滑杆对比、体积变化、输出尺寸和文件下载。
- 单张输入限制为 25 MiB，输出最长边限制为 8192 px、总像素限制为 4000 万；GIF 输出为静态图片。

该功能不使用 Worker 的媒体子进程。Rust 仅负责 KRunner、工具注册表和 WebView 页面路由，图像字节始终留在前端运行时。

## 图片编辑

在 KRunner 输入 `editor`、`image-editor`、`edit-image`、`imageedit` 或 `imgedit` 可打开纯前端图片编辑器。页面集成 TOAST UI Image Editor，支持裁剪、缩放、翻转、旋转、绘制、形状、文字、滤镜、撤销与重做；编辑结果可复制为 PNG，或按 PNG / JPEG 导出。图片只在 WebView 内处理，并已关闭 TOAST UI 的使用统计。

## 图片水印

在 KRunner 或独立启动器输入 `watermark`、`wm` 或 `image-watermark` 可打开纯前端图片水印页。功能交互参考 [TransparentLC/watermarker](https://github.com/TransparentLC/watermarker)，实现由本项目使用 Vue 3、TypeScript 与 Canvas 独立完成，没有复制其 AGPL-3.0 源码。

- 支持选择、拖放或粘贴原图，可使用文字或另一张图片作为水印。
- 水印按全图平铺，支持字号/图片宽度、颜色、透明度、旋转角度、水平与垂直间距。
- 支持 PNG、JPEG、WebP 输出及质量设置，可复制结果或下载文件。
- 图片仅在 WebView 中解码、渲染与编码，不会传给 Rust、后端或网络。

图片压缩、图片编辑和图片水印页底部均保留打开对应原项目的入口。

## 文本加解密

在 KRunner 或独立启动器输入 `crypto`、`encrypt`、`decrypt`、`cipher` 或 `aes` 可打开纯前端文本加解密页。功能范围参考 [IT Tools](https://github.com/CorentinTh/it-tools)，本项目独立实现两个互不影响的加密/解密工作区，支持 AES、TripleDES、Rabbit 与 RC4 的 CryptoJS/OpenSSL 口令格式；密钥与正文不会离开 WebView，页面底部保留原项目入口。

CryptoJS 已停止维护，其口令格式也不提供密文认证；TripleDES、Rabbit 与 RC4 只用于兼容旧数据，不建议用于新数据或高敏感场景。页面会持续显示此限制，不把兼容性工具包装成现代安全方案。

## 图片与视频元数据

输入 `exif`、`metadata` 或 `mediainfo` 可打开媒体元数据查看器。选择的任何文件路径（包括图片、MP4/MOV、MKV/WebM、AVI、MPEG-TS 等视频）都会直接交给设置中选定的 Rust 后端，不经剪贴板，也不会把视频读入 WebView：

- 内置模式（默认）：编译进 Worker 的 `revelo` 默认 BSD-2-Clause 构建，以内存映射读取路径，不需要系统命令，支持媒体容器/编解码信息及常见 EXIF/IPTC/XMP。
- 外部模式：受限调用 PATH 中的 `exiftool`，使用固定 JSON 参数、无 shell、30 秒超时与 16 MiB 输出上限，可读取更完整的格式和厂商标签；未安装时设置项不可选。

页面按元数据分组展示并支持搜索、复制字段及导出为 JSON。内置构建没有启用 `revelo` 的 `exiftool-tables` 特性，因此不会把 GPL/Artistic 的 ExifTool 深度标签表编入二进制。页面底部保留 ExifTool 与 revelo 项目入口。

## 颜色选择器

输入 `color`、`colour`、`picker` 或 `eyedropper` 可打开颜色选择器。WebView 内提供固定 HSV 色板、HEX/RGB/HSL 转换和最近颜色历史；“屏幕取色”会暂时隐藏 WebView，再从任意已连接显示器选择像素，完成或取消后恢复原窗口。

Linux Wayland/X11 使用 XDG Desktop Portal `PickColor`，由 KDE 门户负责多屏交互与授权；Windows 使用平台层读取全局桌面像素，单击确认、Esc 取消。相关实现只位于 `Platform` 层，应用和工具业务代码不包含操作系统条件分支。

## 独立启动器与原生快速输入

`devtools-workerd --launcher` 可在没有 KRunner 的桌面环境或 Windows 中打开类 KRunner 工具启动器；Linux/KDE 即使安装了 KRunner 也可同时使用。启动器支持工具命令、中英文关键词检索，并会把直接输入的 JSON 对象/数组自动送入 JSON Workbench。

设置中提供两个彼此独立、默认关闭的全局快捷键：

- 工具启动器快捷键：唤出 WebView 工具搜索窗口，默认值为 `Ctrl+Alt+Space`。
- 原生快速输入快捷键：唤出不使用 WebView 的轻量输入框，默认值为 `Ctrl+Alt+KeyI`。输入框自动聚焦并按指针所在显示器工作区裁剪位置和尺寸；`Enter` 将内容回填原应用，`↑` / `↓` 浏览本次和历史输入。

原生快速输入历史按 JSONL 追加保存：Linux 为 `$XDG_DATA_HOME/devtools/quick-input-history.jsonl`（默认 `~/.local/share/devtools/quick-input-history.jsonl`），Windows 为 `%LOCALAPPDATA%\devtools\quick-input-history.jsonl`。X11 使用 `xdotool` 回到原窗口并输入；KDE Wayland 按系统安全模型通过 XDG RemoteDesktop 门户申请键盘注入权限，首次使用会出现授权界面；Windows 使用 Win32 原生窗口与 `SendInput`。

Worker 使用系统托盘/Windows 通知区域图标常驻，菜单固定为“设置 / 重启 / 退出”，左键打开独立启动器。设置页同样由 Vue 3 + Naive UI WebView 渲染，可控制：

- 是否显示系统托盘图标（默认开启）
- 是否随桌面用户会话开机启动（默认关闭）
- 工具启动器全局快捷键（默认关闭）
- 原生快速输入快捷键及输入框宽高（默认关闭）
- 界面主题：跟随系统（默认）、浅色或深色
- 界面语言：自动识别（默认）、简体中文、繁体中文或英语；WebView、Naive UI、图片编辑器和托盘菜单使用同一设置
- 元数据后端：内置 revelo（默认）或系统外部 ExifTool

隐藏托盘后仍可执行 `devtools-workerd --settings` 重新打开设置页。
也可执行 `devtools-workerd --quick-input` 直接唤出原生快速输入框，便于脚本调用和故障恢复。

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

- Debian 13 + KDE Plasma 6（Wayland 为主，保留 X11 兼容）；Worker 同时支持 Windows 10+
- `rustc` / `cargo`（`install.sh` 会在缺失时自动用 rustup 安装到用户目录）
- fnm + Node.js 26 + pnpm 11
- `libgtk-3-dev`、`libwebkit2gtk-4.1-dev`（Wry / WebKitGTK 4.1 编译依赖）
- `wl-clipboard`（`wl-copy`）、`notify-send`
- `xdotool`（仅 X11 原生快速输入回填）
- `php`（可选；仅用于启用 PHP 格式转换）
- `tesseract-ocr`、`tesseract-ocr-eng`、`tesseract-ocr-chi-sim`（可选；OCR）
- `zbar-tools`（可选；条形码和二维码识别）
- `libimage-exiftool-perl`（可选；用于启用外部 ExifTool 元数据后端）

## 安装

```bash
sudo apt install libgtk-3-dev libwebkit2gtk-4.1-dev \
  xdotool tesseract-ocr tesseract-ocr-eng tesseract-ocr-chi-sim zbar-tools \
  libimage-exiftool-perl
git clone <this-repo> krunner-plugin
cd krunner-plugin
./install.sh
```

`install.sh` 会：

1. 用 fnm 的 Node 26 执行前端全量门禁并生成单文件 WebView 产物
2. 编译 Cargo Workspace，安装 `devtools-runner` 与 `devtools-workerd` 到 `~/.local/bin/`
3. 放置 KRunner 元数据 → `~/.local/share/krunner/dbusplugins/org.kde.devtools.desktop`
4. 放置独立应用入口 → `~/.local/share/applications/org.loveyu.DevTools.desktop`
5. 放置 Runner / Worker 两个 D-Bus 自动激活服务；检测到 KRunner 时才执行重启

之后 **无需手动启动任何进程**：有 KRunner 时查询会经 D-Bus 按需激活服务；没有 KRunner 时可从应用菜单打开 DevTools。

Windows 只构建 Worker（不构建 KDE Runner）：

```powershell
cd web\devtools-ui
pnpm install --frozen-lockfile
pnpm check
cd ..\..
cargo build --release -p devtools-workerd
.\target\release\devtools-workerd.exe --launcher
```

### 前端联调

开发时可以让 Worker 直接加载 Vite 页面并自动打开 Web Inspector，发布模式仍使用编译内置的单文件页面。联调脚本从 7173 起自动探测空闲端口，Vite 可访问后将端口写入系统临时目录的 `devtools-workerd-vite.port`：

```bash
# 自动启动 Vite 和 Worker
fnm exec --using 26 pnpm --dir web/devtools-ui dev:worker
```

也可以手动分开启动；Worker 会读取前端写入的临时端口文件：

```bash
# 终端 1
fnm exec --using 26 pnpm --dir web/devtools-ui dev

# 终端 2
DEVTOOLS_WEBVIEW_DEBUG=1 cargo run -p devtools-workerd -- --launcher
```

手动指定时，`DEVTOOLS_WEBVIEW_URL` 优先于 `DEVTOOLS_WEBVIEW_PORT` 和临时文件，例如
`DEVTOOLS_WEBVIEW_DEBUG=1 DEVTOOLS_WEBVIEW_PORT=17173 cargo run -p devtools-workerd -- --launcher`。
为避免远程页面获得本机 IPC 权限，调试 URL 只允许 `localhost`、IPv4/IPv6 loopback 的 HTTP/HTTPS 地址。

## 使用

- 打开 KRunner（默认 `Alt+Space`），直接粘贴 JSON 对象/数组即可自动识别；也可以复制 JSON 后输入 `json`。
- 无 KRunner 或在 Windows 中，运行 `devtools-workerd --launcher`；也可在设置中启用独立启动器全局快捷键。
- 复制任意待转换文本后输入 `convert` / `cv`，打开数据转换页。
- 输入 `ocr` 打开文字识别；输入 `barcode` / `bar` / `qr` / `qrcode` 打开条码识别与生成。
- 输入 `compress` / `squoosh` / `image-compress` / `imgcompress` 打开纯前端图片压缩。
- 输入 `editor` / `image-editor` / `edit-image` / `imageedit` / `imgedit` 打开纯前端图片编辑器。
- 输入 `watermark` / `wm` / `image-watermark` 打开纯前端图片水印。
- 输入 `crypto` / `encrypt` / `decrypt` 打开多算法文本加解密。
- 输入 `exif` / `metadata` / `mediainfo` 查看图片或视频路径元数据。
- 输入 `color` / `picker` / `eyedropper` 打开固定色板和跨屏幕取色。
- 输入 `date` / `time` / `da` / `tim` 看时间格式，或 `ts` / `tms` / `unix` 取时间戳；也可以**直接粘贴时间戳或时间字符串**双向互转，回车复制。
- 命令行触发（打开 KRunner 并只显示本 runner 结果）：

  ```bash
  qdbus6 org.kde.krunner /App org.kde.krunner.App.querySingleRunner org.kde.devtools date
  ```

## 架构

采用 Cargo Workspace 和两个独立进程。Linux Runner 保持轻量，不加载 GTK/WebKit；复杂工作台交互通过 D-Bus 交给跨平台 Worker。Worker 也可以完全脱离 KRunner，以应用入口、托盘或全局快捷键运行：

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
    ├─ Match("compress")        -> 打开纯前端图片压缩
    ├─ Match("editor")          -> 打开纯前端图片编辑器
    ├─ Match("watermark" / "wm") -> 打开纯前端图片水印
    ├─ Match("crypto")          -> 打开纯前端多算法加解密
    ├─ Match("exif")            -> 打开图片/视频元数据查看器
    ├─ Match("color")           -> 打开固定色板与原生屏幕取色
    ├─ Run("json:inline:*" / "json:open")
    │       │  org.loveyu.DevTools.OpenTool("json", payload)
    │       ▼
    │  devtools-workerd -> Tool Registry -> JsonTool / ConvertTool / Media Tools
    │                                      │
    │                               WindowManager
    │                                      │
    │                               WebViewManager
    │                                      │
    │                          Platform implementation
    │                         Linux             Windows
    │                   GTK / WebKitGTK         WebView2
    │                                      │
    │                               Vue 3 + Naive UI
    ├─ Run("convert:open") -> OpenTool("convert", clipboard)
    ├─ 其他 Run(matchId) -> wl-copy + notify-send
    ├─ Actions()      -> a(sss)
    ├─ Config()       -> a{sv}
    └─ Teardown()
```

Worker 默认复用一个窗口和 WebView，关闭窗口只隐藏，进程与托盘继续运行。Web 静态资源构建为单个 `dist/index.html` 并编译进 Worker；Windows 通过进程内自定义协议提供给 WebView2，Linux 直接交给 WebKitGTK，两者都不启动 localhost 服务。原生快速输入使用独立 GTK/Win32 窗口，不加载 WebView。系统剪贴板、配置和自启动文件均由 Rust IPC 控制；图片压缩、编辑、水印与文本加解密直接在前端完成。元数据路径由独立 Rust 线程交给内置 revelo 或受限 ExifTool；屏幕取色会隐藏 WebView 并委托平台层完成。

Worker 的业务层不包含 `target_os` 条件分支，也不直接引用 WebKitGTK 或 WebView2。平台选择只发生在 `platform/mod.rs`，具体 API 只存在于 `platform/linux.rs` 与 `platform/windows.rs`：

```text
devtools-workerd
├── Application          应用生命周期与事件编排
├── WindowManager        WebView 工作区与原生快速输入窗口协调
├── WebViewManager       页面路由与前端事件分发
├── IPC                  稳定 JSON 协议与业务工具路由
└── Platform
    ├── Linux            GTK / WebKitGTK、StatusNotifierItem、D-Bus、门户
    └── Windows          Win32 / WebView2、通知区域、SendInput
```

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
| `src/media.rs` | OCR / 条码 / 图片工具 / 加解密 / 元数据 / 颜色选择 KRunner 触发词与 Worker 调用 |
| `src/clipboard.rs` | Klipper / Wayland / X11 的共享剪贴板读取 |
| `src/time.rs` | 日期时间查询逻辑（COMMANDS / ITEMS / value_of 等） |
| `src/rand.rs` | 随机字符串生成（RandMode / parse_rand_query 等） |
| `crates/devtools-core` | Context / Action / Tool / Settings 公共协议 |
| `crates/devtools-tools` | 与 UI 解耦的 JsonTool / ConvertTool 业务入口 |
| `crates/devtools-workerd/src/application.rs` | 平台无关的 Worker 生命周期与事件编排 |
| `crates/devtools-workerd/src/window_manager.rs` | 协调 WebView 工作区与非 WebView 原生快速输入窗口 |
| `crates/devtools-workerd/src/webview_manager.rs` | 平台无关的页面路由、状态同步与前端事件分发 |
| `crates/devtools-workerd/src/ipc.rs` | WebView JSON 协议与工具请求到应用事件的映射 |
| `crates/devtools-workerd/src/metadata_processor.rs` | 内置 revelo 与受限外部 ExifTool 路径解析线程 |
| `crates/devtools-workerd/src/color_picker.rs` | 跨平台屏幕取色结果模型 |
| `crates/devtools-workerd/src/platform/` | Linux GTK/WebKitGTK 与 Windows Win32/WebView2 的全部平台实现 |
| `web/devtools-ui` | Vue 3 + TypeScript + Naive UI + SCSS 工作台 |
| `assets/org.kde.devtools.desktop` | KRunner DBus-runner 元数据（`X-Plasma-API=DBus2` 等） |
| `assets/*.service` | Runner / Worker D-Bus 激活服务模板 |
| `assets/org.loveyu.DevTools.desktop.in` | 无 KRunner 时使用的独立应用菜单入口 |
| `install.sh` | 编译 + 安装 + 重启 KRunner |

## 第三方开源项目与代码来源

前端直接使用的第三方代码均通过 pnpm 锁定版本，完整清单以 `web/devtools-ui/package.json` 与 `pnpm-lock.yaml` 为准；Rust 依赖以各 `Cargo.toml` 与 `Cargo.lock` 为准。与本次图片/转换能力直接相关的主要项目如下：

| 项目 | 本项目用途 | 使用方式 | 许可证 |
| --- | --- | --- | --- |
| [Vue 3](https://github.com/vuejs/core)、[Naive UI](https://github.com/tusen-ai/naive-ui) | WebView 前端与组件库 | 直接引入 npm 包 | MIT |
| [TOAST UI Image Editor](https://github.com/nhn/tui.image-editor) | 图片编辑 | 直接引入 `tui-image-editor` 包，关闭使用统计 | MIT |
| [bwip-js](https://github.com/metafloor/bwip-js) | 条码与二维码生成 | 直接引入 `@bwip-js/browser` 包 | MIT |
| [fast-xml-parser / builder / validator](https://github.com/NaturalIntelligence/fast-xml-parser)、[yaml](https://github.com/eemeli/yaml)、[JSON5](https://github.com/json5/json5)、[Papa Parse](https://github.com/mholt/PapaParse)、[pako](https://github.com/nodeca/pako)、[qs](https://github.com/ljharb/qs)、[smol-toml](https://github.com/squirrelchat/smol-toml) | 数据转换 | 直接引入 npm 包 | MIT / ISC / Zlib / BSD-3-Clause，分别遵循各项目许可证 |
| [Wry](https://github.com/tauri-apps/wry)、[Tao](https://github.com/tauri-apps/tao)、[tray-icon](https://github.com/tauri-apps/tray-icon)、[Wayclip/global-hotkey](https://github.com/Wayclip/global-hotkey)、[ashpd](https://github.com/bilelmoussaoui/ashpd) | WebView、窗口、托盘与全局快捷键 | 直接引入 Rust crates；Windows 与 Linux/X11 使用 global-hotkey，Linux/Wayland 使用 ashpd 管理 XDG GlobalShortcuts session，并沿用 Wayclip 的 preferred_trigger 按键映射；平台细节封装在 `Platform` 层 | Apache-2.0 / MIT，以各 crate 声明为准 |
| [Squoosh](https://github.com/GoogleChromeLabs/squoosh) | 图片压缩交互参考 | 未复制或引入其源码；本项目使用 Canvas 独立实现 | Apache-2.0（原项目） |
| [TransparentLC/watermarker](https://github.com/TransparentLC/watermarker) | 图片水印能力与交互参考 | 未复制或引入其源码；本项目使用 Canvas/TypeScript 独立实现 | AGPL-3.0（原项目） |
| [IT Tools](https://github.com/CorentinTh/it-tools) | 多算法加解密功能与交互参考 | 未复制或引入其源码；本项目独立实现，页面保留原项目入口 | GPL-3.0（原项目） |
| [CryptoJS](https://github.com/brix/crypto-js) | AES / TripleDES / Rabbit / RC4 兼容格式 | 直接引入 `crypto-js`；项目已停止维护，UI 明确提示风险 | MIT |
| [revelo](https://github.com/vbasky/revelo) | 编译内置的图片/视频元数据解析 | 直接引入默认特性集之外的 `mmap`；未启用 `exiftool-tables` | BSD-2-Clause |
| [ExifTool](https://exiftool.org/) | 可选外部完整元数据读取 | 仅运行用户系统中的命令，不链接或打包；页面保留项目入口 | Artistic-1.0 / GPL-1.0-or-later |

数据转换、OCR 与条码识别的功能范围和交互迁移自内部 `tools-console` 项目，其中数据转换参考路径为 `/data/code/private/tools-console/src/views/helper/data-convert`。本项目没有照搬旧后端：浏览器可完成的转换已用 TypeScript 重新实现；OCR、条码识别和可选 PHP 格式通过受限 Rust 子进程桥接；依赖后端且非必要、存在已知问题或本机缺少对应程序的逻辑没有迁移。

图片压缩、图片编辑、图片水印、加解密和元数据页面底部均提供对应原项目入口。Tesseract OCR、ZBar、ExifTool、PHP CLI、`xdotool` 等属于用户环境中的可选外部程序，Worker 仅以受限子进程调用，不把它们的代码链接或打包进本项目。

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
./target/release/devtools-workerd --launcher
./target/release/devtools-workerd --quick-input
```

前端严格门禁包含 peer 依赖检查、Prettier、类型感知 ESLint、Stylelint、`vue-tsc`、Vitest 功能测试与关键 JSON/媒体/图片压缩/启动器/i18n 模型逻辑 100% 覆盖率、Vite 构建和 `tsx` 单文件产物校验。i18n 门禁要求简中、繁中、英语 key 完全一致且 key 不含中文。GitHub CI 会在 push / pull request 上重复执行 Linux 前端与 Rust 全量门禁，并在 Windows runner 原生构建、检查和 lint Worker。修改任意模块后，重新执行 `./install.sh` 即可部署 Linux 版本。

# krunner-devtools

一个 KDE Plasma 6 的 **KRunner Runner 插件**，把常用开发小工具（日期/时间戳、随机字符串等）整合进 KRunner：

> **Alt + Space → 输入 `date` → 看到多个结果 → 回车复制**

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

- KDE Plasma 6（Wayland）—— 在 Plasma 6.3.6 + Wayland 上开发验证
- `rustc` / `cargo`（`install.sh` 会在缺失时自动用 rustup 安装到用户目录）
- `wl-clipboard`（`wl-copy`）、`notify-send`

## 安装

```bash
git clone <this-repo> krunner-plugin
cd krunner-plugin
./install.sh
```

`install.sh` 会：

1. 编译 `devtools-runner`（release）→ 安装到 `~/.local/bin/devtools-runner`
2. 放置 KRunner 元数据 → `~/.local/share/krunner/dbusplugins/org.kde.devtools.desktop`
3. 放置 D-Bus 自动激活服务 → `~/.local/share/dbus-1/services/org.kde.devtools.service`
4. 重启 KRunner

之后 **无需手动启动任何进程**：KRunner 查询时，D-Bus 会按需激活服务。

## 使用

- 打开 KRunner（默认 `Alt+Space`），输入 `date` / `time` / `da` / `tim` 看时间格式，或 `ts` / `tms` / `unix` 取时间戳；也可以**直接粘贴时间戳或时间字符串**双向互转，回车复制。
- 命令行触发（打开 KRunner 并只显示本 runner 结果）：

  ```bash
  qdbus6 org.kde.krunner /App org.kde.krunner.App.querySingleRunner org.kde.devtools date
  ```

## 架构

采用 **DBus2 协议**（`org.kde.krunner1`，Plasma 6 的新协议）。Runner 作为独立的 session-bus 服务常驻，KRunner 通过 D-Bus 调用它：

```
KRunner Query
    │  (D-Bus 方法调用)
    ▼
devtools-runner  (org.kde.devtools  @  /runner  :  org.kde.krunner1)
    │
    ├─ Match(query)   -> a(sssida{sv})   每条 = (Id, Text, Icon, Type, Relevance, Properties)
    ├─ Run(matchId)   -> wl-copy + notify-send
    ├─ Actions()      -> a(sss)
    ├─ Config()       -> a{sv}
    └─ Teardown()
```

匹配结构在总线上的签名（来自 `/usr/share/dbus-1/interfaces/kf6_org.kde.krunner1.xml`）：

```
a(sssida{sv})
 │  │  │  │  │   └─ Properties: subtext(复制内容), category ...
 │  │  │  │  └──── Relevance (double)
 │  │  │  └─────── Type (int32)
 │  │  └────────── IconName
 │  └───────────── Text  (KRunner 第一行)
 └──────────────── Id    (Run 时回传)
```

关键文件：

| 文件 | 作用 |
| --- | --- |
| `src/main.rs` | Runner 主体：`org.kde.krunner1` 接口实现、调度分发 |
| `src/time.rs` | 日期时间查询逻辑（COMMANDS / ITEMS / value_of 等） |
| `src/rand.rs` | 随机字符串生成（RandMode / parse_rand_query 等） |
| `Cargo.toml` | 依赖：`zbus`(DBus)、`chrono`(时间)、`rand`(随机) |
| `assets/org.kde.devtools.desktop` | KRunner DBus-runner 元数据（`X-Plasma-API=DBus2` 等） |
| `assets/org.kde.devtools.service` | D-Bus 激活服务模板（`@EXEC@` 由 install.sh 替换） |
| `install.sh` | 编译 + 安装 + 重启 KRunner |

## 开发与调试

```bash
cargo build --release

# 直接跑起来（前台，带日志到 stderr）
./target/release/devtools-runner

# 在另一个终端直接调它的方法（不经过 KRunner 也能验证）
dbus-send --session --print-reply --dest=org.kde.devtools /runner \
    org.kde.krunner1.Match string:"date"
dbus-send --session --print-reply --dest=org.kde.devtools /runner \
    org.freedesktop.DBus.Introspectable.Introspect

# 查看注册的接口/签名是否正确
qdbus6 org.kde.devtools /runner
```

修改 `src/main.rs` 后，重新 `./install.sh` 即可。



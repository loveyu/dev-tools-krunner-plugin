# krunner-devtools

一个 KDE Plasma 6 的 **KRunner Runner 插件**，把常用开发小工具（日期/时间戳等）整合进 KRunner：

> **Alt + Space → 输入 `date` → 看到多个结果 → 回车复制**

当前为 **MVP**：实现 `date` / `time` 相关的时间输出，以及 `ts` / `tms` / `unix` 时间戳快捷命令，验证整条链路（KRunner → 匹配 → 多结果 → 一键复制）。后续按设计文档逐步加入 uuid / hash / json / 外部插件管理等能力。

## 当前能力

输入 `date` / `time`（或前缀 `da` / `tim`）显示：

| 结果 | 示例 |
| --- | --- |
| 当前时间 | `2026-07-28 10:31:07` |
| Unix 时间戳 | `1785205640` |
| Unix 时间戳 (ms) | `1785205640955` |
| RFC3339 | `2026-07-28T10:31:07+08:00` |
| ISO8601 | `2026-07-28T02:31:07Z` |
| UTC 时间 | `2026-07-28 02:31:07 UTC` |

快捷命令只返回单条结果，适合「打完即复制」：

| 命令 | 结果 |
| --- | --- |
| `ts` / `unix` | 秒级 Unix 时间戳 |
| `tms` / `tsm` | 毫秒级 Unix 时间戳 |

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

- 打开 KRunner（默认 `Alt+Space`），输入 `date` / `time` / `da` / `tim` 看时间格式，或 `ts` / `tms` / `unix` 取时间戳，回车复制。
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
| `src/main.rs` | Runner 主体：`org.kde.krunner1` 接口实现、匹配逻辑、复制/通知 |
| `Cargo.toml` | 依赖：`zbus`(DBus)、`chrono`(时间) |
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

## 路线图（对齐设计文档）

- **MVP（当前）**：Runner 主体 + `date`/`time`，全链路打通。
- **V1.1**：外部插件目录 `~/.local/share/krunner-devtools/plugins/<id>/` + `plugin.toml`，Runner 作为「插件管理器」fork/exec 外部可执行文件并解析其 JSON 输出；uuid / hash / base64 / json 等。
- **V1.2+**：缓存、图标、alias、热加载；V2 的 `action` 扩展（open/url/exec）与富文本/图片。

> V1.1 起的输出协议（`items[].title/value`，以及 V2 的 `subtitle/copy/icon/action`）已在设计文档中定义，本 Runner 届时只需把外部插件 JSON 转成本 Runner 的 `a(sssida{sv})` 结构即可，协议向后兼容。

# 代码风格

本文件约定本仓库（Rust 侧）的代码风格。新增 / 修改代码时请遵守； review 时也以此为准。

## 注释语言（强制）

- **所有代码注释统一使用中文**（`//!`、`///` 文档注释，以及 `//` 行内注释）。
- 标识符、日志消息、面向总线的字符串可以保留英文（见下文「例外」），但**注释文字**必须中文。
- 例外（**不算注释**，不在本条约束范围内，但建议同步中文化面向最终用户可见的文案）：
  - DBus bus name / 对象路径 / 接口名 / 签名（`org.kde.devtools`、`/runner`、`a(sssida{sv})` 等，协议规定，不可改）。
  - `eprintln!` 输出到 stderr 的调试日志（`devtools-runner: ...`，见「日志」一节）。
  - DBus 错误返回串（如 `unknown match id: ...`）。
  - 面向用户的通知文案（`notify()` 的 summary/body）建议用中文，例如 `Copied` → `已复制`。

## 注释写法

- **文档注释 `///`、`//!`**：写「这段代码做什么」以及「为什么这么做」，尤其要记下非显而易见的决策与踩过的坑。面向未来读代码的人，而不是重复类型签名。
- **行内注释 `//`**：只在该行行为不直观时点缀，点明意图而非复述代码。
- 协议相关、易踩坑的硬知识（字段含义、排序规则、zbus 的坑）写在 `CLAUDE.md`，代码处用简短中文注释 + 指向 `CLAUDE.md` 即可，避免重复维护。
- 例：见 `src/main.rs` 顶部对 `categoryRelevance` 的说明、各 `const` / 函数的 `///`。

## 命名

遵循 Rust 惯例（`rustfmt` 默认）：

- 变量、函数、方法：`snake_case`。
- 类型、Trait、Enum 变体：`UpperCamelCase`。
- 常量、静态：`SCREAMING_SNAKE_CASE`。
- **DBus 接口方法例外**：`#[zbus::interface]` 下的方法用 `UpperCamelCase`（`Match`、`Run`、`Actions`、`Config`、`Teardown`），使 D-Bus 成员名与协议一一对应；文件顶部用 `#![allow(non_snake_case)]` 关闭对应 lint。原因见 `CLAUDE.md`。

## 日志

- 统一前缀 `devtools-runner:`，输出到 `stderr`，用 `eprintln!`。
- 日志语言不强制（key=value 风格便于 grep），但**保持与现状一致**——当前是英文短句，新增日志沿用同一风格，不要在同一文件里中英混排。

## 错误处理

- 可恢复错误用 `Result`；不可恢复、确信不会发生的内部不变量用 `.expect("说明为何不可能")`，说明文字中文（如 `src/main.rs` 的 `str_value`）。
- 外部命令（`wl-copy` / `notify-send`）失败只打日志、不中断主流程——runner 必须始终能响应 KRunner。

## 格式化

- 提交前跑 `cargo fmt`，不要引入 `rustfmt` 会抹掉的格式差异。

## 与其他文档的关系

| 文档 | 记什么 |
| --- | --- |
| `CLAUDE.md` | 非显而易见的协议契约、踩坑、构建/调试回路、架构决策 |
| `docs/CODE_STYLE.md`（本文档） | 通用编码风格与约定 |
| `README.md` | 面向用户的能力说明与安装使用 |

三者分工不同，改了对应内容请同步更新；本文件只管「怎么写代码」，不重复 `CLAUDE.md` 的协议细节。

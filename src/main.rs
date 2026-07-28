//! DevTools KRunner Runner
//!
//! 一个 Plasma 6 的 DBus runner（DBus2 协议）。KRunner 通过 session bus 调用
//! 我们的服务（bus name `org.kde.devtools`，对象路径 `/runner`），我们响应
//! `org.kde.krunner1` 的方法调用。
//!
//! MVP 范围：输入 `date` / `time`（或前缀 `da` / `tim`）展示多种时间格式；
//! `ts` / `unix` 直接给出秒级 Unix 时间戳，`tms` 给出毫秒级。回车复制选中项，
//! 并弹出桌面通知。
//!
//! match 结构在总线上的形状为
//! `(Id, Text, IconName, CategoryRelevance, Relevance, Properties)`，
//! 序列化后的 DBus 签名为 `a(sssida{sv})`。
//!
//! 注意：第 4 个字段（`i`）是 `categoryRelevance`，**不是** "type"——系统里那份
//! `kf6_org.kde.krunner1.xml` 把它标注成 "Type"，该注释已过时。权威定义见
//! KRunner 框架的 `dbusutils_p.h`：`RemoteMatch::categoryRelevance` 默认为
//! `Lowest`（= 0）。传 0 会让所有结果落入最低排序桶、沉到最底，因此我们传
//! `Highest`（= 100）。

#![allow(non_snake_case)]

use std::collections::HashMap;
use std::process::Command;
use std::time::Duration;

use chrono::{DateTime, Local, Utc};
use zbus::blocking::ConnectionBuilder;
use zbus::zvariant::{OwnedValue, Value};

/// 我们注册的 DBus bus name。
const SERVICE_NAME: &str = "org.kde.devtools";
/// KRunner 调用的对象路径（需与 .desktop 里的 `X-Plasma-DBusRunner-Path` 一致）。
const OBJECT_PATH: &str = "/runner";
/// 用于对我们结果分组的 KRunner 类目。
const CATEGORY: &str = "DevTools";
/// `KRunner::QueryMatch::CategoryRelevance::Highest`（见 dbusutils_p.h）。
/// 取 Highest 让结果排在靠前位置，而不是落入默认的 Lowest 桶。
const CATEGORY_RELEVANCE: i32 = 100;

/// 本 runner 认识的命令：`(触发关键词, 要展示的 item 后缀)`。
/// 当查询等于某关键词、以某关键词为前缀、或被某关键词以前缀包含时即命中——
/// 因此 `da`/`date` 命中 `date`，而 `ts`/`unix`、`tms`/`tsm` 各自只映射到单条时间戳行。
/// 输入过程中可能同时命中多个命令（如 `t` 同时命中 `time`、`ts`、`tms`），
/// 此时把它们的后缀求并集去重，再由 `build_matches` 按 `ITEMS` 顺序输出。
const COMMANDS: &[(&[&str], &[&str])] = &[
    (&["date", "time"], &["local", "unix", "unixms", "rfc3339", "iso8601", "utc"]),
    (&["ts", "unix"], &["unix"]),
    (&["tms", "tsm"], &["unixms"]),
];

/// 每一条结果的静态定义：`(id 后缀, 标题, 图标名)`。
/// 具体取值在查询时计算，保证永远是当前值。
const ITEMS: &[(&str, &str, &str)] = &[
    ("local", "当前时间", "clock"),
    ("unix", "Unix 时间戳", "preferences-system-time"),
    ("unixms", "Unix 时间戳 (ms)", "preferences-system-time"),
    ("rfc3339", "RFC3339", "text-x-generic"),
    ("iso8601", "ISO8601", "text-x-generic"),
    ("utc", "UTC 时间", "clock"),
];

/// `(Id, Text, IconName, CategoryRelevance, Relevance, Properties)` → DBus `a(sssida{sv})`。
type KMatch = (String, String, String, i32, f64, HashMap<String, OwnedValue>);

struct DevTools;

/// 把字符串包装成 `a{sv}` 字典里用的 `OwnedValue`（DBus variant `v`）。
/// `OwnedValue` 只对基础类型实现了 `From`，因此这里经由 `Value` 中转。
fn str_value(s: impl Into<String>) -> OwnedValue {
    OwnedValue::try_from(Value::from(s.into()))
        .expect("OwnedValue::try_from(Value) is infallible")
}

/// 根据某个固定时间点，计算给定后缀对应的可复制取值。
fn value_of(suffix: &str, now: &DateTime<Local>, utc: &DateTime<Utc>) -> String {
    match suffix {
        "local" => now.format("%Y-%m-%d %H:%M:%S").to_string(),
        "unix" => now.timestamp().to_string(),
        "unixms" => now.timestamp_millis().to_string(),
        "rfc3339" => now.format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
        "iso8601" => utc.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        "utc" => utc.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        _ => String::new(),
    }
}

/// 按给定的 item 后缀构造结果列表，输出顺序与 `ITEMS` 一致。
fn build_matches(suffixes: &[&str]) -> Vec<KMatch> {
    let now = Local::now();
    let utc = Utc::now();
    ITEMS
        .iter()
        .filter(|(s, _, _)| suffixes.contains(s))
        .enumerate()
        .map(|(i, (suffix, title, icon))| {
            let value = value_of(suffix, &now, &utc);
            let mut props = HashMap::new();
            props.insert("subtext".to_string(), str_value(value));
            props.insert("category".to_string(), str_value(CATEGORY));
            (
                format!("date:{suffix}"),
                (*title).to_string(),
                (*icon).to_string(),
                CATEGORY_RELEVANCE, // categoryRelevance：Highest → 排在靠前位置
                1.0 - 0.03 * i as f64, // relevance：决定同一类目内的先后顺序
                props,
            )
        })
        .collect()
}

/// 本次查询应该展示哪些 item 后缀？
/// 查询未命中任何命令时返回空 vec（KRunner 随即不显���本 runner 的结果）。
/// 多个命中命令的后缀会求并集去重，再由 `build_matches` 按 `ITEMS` 顺序还原。
///
/// 精确匹配（`q` 等于某关键词）优先于前缀匹配：只要存在任意精确命中，就只采用精确命中的
/// 命令。这样 `tsm` 不会因为以 `ts` 开头而连带触发秒级命令，能干净地等于 `tms`。
fn suffixes_for_query(query: &str) -> Vec<&'static str> {
    let q = query.trim().to_lowercase();
    if q.len() < 2 {
        return Vec::new();
    }
    let exact_hit = COMMANDS
        .iter()
        .any(|(kws, _)| kws.iter().any(|k| q == *k));
    let mut matched: Vec<&str> = Vec::new();
    for (keywords, suffixes) in COMMANDS {
        let hit = if exact_hit {
            keywords.iter().any(|k| q == *k)
        } else {
            keywords
                .iter()
                .any(|k| q.starts_with(k) || k.starts_with(&q))
        };
        if hit {
            for s in *suffixes {
                if !matched.contains(s) {
                    matched.push(s);
                }
            }
        }
    }
    matched
}

/// 根据我们此前产出的 match id 反解出要复制的取值。
fn value_for_id(id: &str) -> Option<String> {
    let suffix = id.strip_prefix("date:")?;
    let now = Local::now();
    let utc = Utc::now();
    let value = value_of(suffix, &now, &utc);
    (!value.is_empty()).then_some(value)
}

/// 把文本复制到 Wayland 剪贴板。`wl-copy` 会作为剪贴板拥有者继续存活，
/// 因此这里 spawn 后即放手，不去阻塞等待它结束。
fn copy_to_clipboard(text: &str) {
    match Command::new("wl-copy").arg(text).spawn() {
        Ok(_) => {}
        Err(e) => eprintln!("devtools-runner: wl-copy failed: {e}"),
    }
}

/// 弹出一个桌面通知。
fn notify(summary: &str, body: &str) {
    if let Err(e) = Command::new("notify-send")
        .args(["--app-name", "DevTools", "--icon", "edit-copy", summary, body])
        .spawn()
    {
        eprintln!("devtools-runner: notify-send failed: {e}");
    }
}

/// `org.kde.krunner1` DBus 接口。
#[zbus::interface(name = "org.kde.krunner1")]
impl DevTools {
    /// 返回某次查询匹配到的结果。
    fn Match(&self, query: &str) -> Vec<KMatch> {
        let suffixes = suffixes_for_query(query);
        let items = build_matches(&suffixes);
        eprintln!("devtools-runner: Match {query:?} -> {} item(s)", items.len());
        items
    }

    /// 支持的动作。MVP 阶段没有额外动作（默认动作即复制）。
    fn Actions(&self) -> Vec<(String, String, String)> {
        Vec::new()
    }

    /// 对某条 match 执行默认动作（复制 + 通知）。
    fn Run(&self, match_id: &str, _action_id: &str) -> zbus::fdo::Result<()> {
        match value_for_id(match_id) {
            Some(value) => {
                eprintln!("devtools-runner: copy '{match_id}' -> {value}");
                copy_to_clipboard(&value);
                notify("Copied", &value);
                Ok(())
            }
            None => Err(zbus::fdo::Error::Failed(format!(
                "unknown match id: {match_id}"
            ))),
        }
    }

    /// 运行时 runner 配置。返回空表示让 runner 自行决定 relevance。
    fn Config(&self) -> HashMap<String, OwnedValue> {
        HashMap::new()
    }

    /// 一次 match 会话结束时调用。当前没有需要清理的资源。
    fn Teardown(&self) {}
}

fn main() {
    eprintln!("devtools-runner: starting {SERVICE_NAME} at {OBJECT_PATH}");
    let _connection = match ConnectionBuilder::session()
        .and_then(|b| b.name(SERVICE_NAME))
        .and_then(|b| b.serve_at(OBJECT_PATH, DevTools))
        .and_then(|b| b.build())
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("devtools-runner: failed to start: {e}");
            std::process::exit(1);
        }
    };
    eprintln!("devtools-runner: ready");

    // 连接内部执行器会在后台线程分发收到的 DBus 消息到对象服务，
    // 这里只需让进程保持存活即可。
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    /// 取一个固定时刻，避免测试依赖「当前时间」或本机时区。
    /// 2000-01-01T00:00:00Z 的 Unix epoch = 946684800（秒）/ 946684800000（毫秒）。
    fn fixed() -> (DateTime<Local>, DateTime<Utc>) {
        let utc = Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();
        let local = utc.with_timezone(&Local);
        (local, utc)
    }

    #[test]
    fn timestamp_keywords_are_one_liners() {
        assert_eq!(suffixes_for_query("ts"), vec!["unix"]);
        assert_eq!(suffixes_for_query("unix"), vec!["unix"]);
        assert_eq!(suffixes_for_query("tms"), vec!["unixms"]);
        assert_eq!(suffixes_for_query("tsm"), vec!["unixms"]);
    }

    #[test]
    fn exact_match_beats_prefix() {
        // `tsm` 以 `ts` 开头，但精确命中 tsm 后不应连带触发秒级 ts。
        assert_eq!(suffixes_for_query("tsm"), vec!["unixms"]);
        assert_eq!(suffixes_for_query("ts"), vec!["unix"]);
    }

    #[test]
    fn date_time_keyword_shows_full_list() {
        assert_eq!(suffixes_for_query("date").len(), 6);
        assert_eq!(suffixes_for_query("time").len(), 6);
        assert_eq!(suffixes_for_query("da").len(), 6);
        assert_eq!(suffixes_for_query("tim").len(), 6);
    }

    #[test]
    fn unambiguous_prefixes() {
        assert_eq!(suffixes_for_query("tm"), vec!["unixms"]);
        assert_eq!(suffixes_for_query("uni"), vec!["unix"]);
    }

    #[test]
    fn query_is_trimmed_and_lowercased() {
        assert_eq!(suffixes_for_query("  TS  "), vec!["unix"]);
        assert_eq!(suffixes_for_query("Tms"), vec!["unixms"]);
        assert_eq!(suffixes_for_query("DATE").len(), 6);
    }

    #[test]
    fn rejects_too_short_or_unknown() {
        assert!(suffixes_for_query("").is_empty());
        assert!(suffixes_for_query("t").is_empty()); // 单字符被 len<2 拦截
        assert!(suffixes_for_query("xyz").is_empty());
    }

    #[test]
    fn value_of_formats_fixed_time() {
        let (now, utc) = fixed();
        assert_eq!(value_of("unix", &now, &utc), "946684800");
        assert_eq!(value_of("unixms", &now, &utc), "946684800000");
        assert_eq!(value_of("iso8601", &now, &utc), "2000-01-01T00:00:00Z");
        assert_eq!(value_of("utc", &now, &utc), "2000-01-01 00:00:00 UTC");
        assert_eq!(value_of("nope", &now, &utc), "");
        // local / rfc3339 带本机时区偏移，不在此固定断言。
    }

    #[test]
    fn build_matches_orders_like_items_and_prefixes_ids() {
        // 乱序、含未知后缀的输入：输出按 ITEMS 顺序、id 带 date: 前缀、未知项被过滤。
        let matches = build_matches(&["unixms", "unix", "missing"]);
        let ids: Vec<&str> = matches.iter().map(|(id, _, _, _, _, _)| id.as_str()).collect();
        assert_eq!(ids, vec!["date:unix", "date:unixms"]);
    }
}

//! DevTools KRunner Runner
//!
//! 一个 Plasma 6 的 DBus runner（DBus2 协议）。KRunner 通过 session bus 调用
//! 我们的服务（bus name `org.kde.devtools`，对象路径 `/runner`），我们响应
//! `org.kde.krunner1` 的方法调用。
//!
//! 功能：输入 `date` / `time` / `now`（或前缀）展示多种时间格式；
//! `uuid` / `u` 生成 UUID v1/v4/v7；`rand 16` / `r16` 等
//! 生成随机字符串；直接输入**时间戳**（`1722931200` / `1722931200000`）
//! 或**日期字符串**（`2024-08-06 12:00:00` / RFC3339 等）则双向互转；
//! 直接输入 JSON 对象/数组，或在剪贴板为有效 JSON 时输入 `json`，通过 D-Bus
//! 打开 JSON Workbench；
//! 回车复制选中项并弹出桌面通知。
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

mod clipboard;
mod convert;
mod data_convert;
mod json;
mod media;
mod rand;
mod time;
mod uuid;

use std::collections::HashMap;
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

use chrono::{Local, Utc};
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

/// `(Id, Text, IconName, CategoryRelevance, Relevance, Properties)` → DBus `a(sssida{sv})`。
type KMatch = (
    String,
    String,
    String,
    i32,
    f64,
    HashMap<String, OwnedValue>,
);

#[derive(Default)]
struct DevTools {
    inline_json: Mutex<json::InlineContextCache>,
}

/// 把字符串包装成 `a{sv}` 字典里用的 `OwnedValue`（DBus variant `v`）。
/// `OwnedValue` 只对基础类型实现了 `From`，因此这里经由 `Value` 中转。
fn str_value(s: impl Into<String>) -> OwnedValue {
    OwnedValue::try_from(Value::from(s.into())).expect("OwnedValue::try_from(Value) is infallible")
}

/// 根据 match id 反解出要复制的取值（被 Run 调用）。
/// 支持 `date:<suffix>`（时间）、`rand:<mode>:<length>`（随机字符串）、
/// `uuid:<version>:<format>`（UUID）、`conv:<kind>:<secs>:<fmt>`（时间戳 ↔ 时间
/// 字符串互转）四种 id 格式。
fn value_for_id(id: &str) -> Option<String> {
    if let Some(suffix) = id.strip_prefix("conv:") {
        return convert::value_for_convert_id(suffix);
    }
    if let Some(suffix) = id.strip_prefix("date:") {
        let now = Local::now();
        let utc = Utc::now();
        let value = time::value_of(suffix, &now, &utc);
        return (!value.is_empty()).then_some(value);
    }
    if let Some(suffix) = id.strip_prefix("rand:") {
        return rand::value_for_rand_id(suffix);
    }
    if let Some(suffix) = id.strip_prefix("uuid:") {
        return uuid::value_for_uuid_id(suffix);
    }
    None
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
        .args([
            "--app-name",
            "DevTools",
            "--icon",
            "edit-copy",
            summary,
            body,
        ])
        .spawn()
    {
        eprintln!("devtools-runner: notify-send failed: {e}");
    }
}

/// `org.kde.krunner1` DBus 接口。
#[zbus::interface(name = "org.kde.krunner1")]
impl DevTools {
    /// 返回某次查询匹配到的结果。
    /// 优先按输入「形状」识别时间戳 ↔ 时间字符串互转（裸数字 / 日期串），
    /// 优先识别直接输入的 JSON，其次处理 JSON 剪贴板命令、UUID、随机字符串，
    /// 最后回落到时间查询。
    fn Match(&self, query: &str) -> Vec<KMatch> {
        let json_item = self
            .inline_json
            .lock()
            .ok()
            .and_then(|mut cache| json::match_for_query(query, &mut cache));
        if let Some(item) = json_item {
            // 直接输入可能包含敏感正文，日志仅记录命中类型，不记录 query。
            eprintln!("devtools-runner: Match -> 1 json item");
            return vec![item];
        }
        if let Some(item) = data_convert::match_for_query(query) {
            eprintln!("devtools-runner: Match -> 1 convert item");
            return vec![item];
        }
        if let Some(item) = media::match_for_query(query) {
            eprintln!("devtools-runner: Match -> 1 media item");
            return vec![item];
        }
        if let Some(convert_query) = convert::parse_convert_query(query) {
            let items = convert::build_convert_matches(&convert_query);
            eprintln!(
                "devtools-runner: Match {query:?} -> {} convert item(s)",
                items.len()
            );
            return items;
        }
        if let Some(uuid_query) = uuid::parse_uuid_query(query) {
            let items = uuid::build_uuid_matches(&uuid_query);
            eprintln!("devtools-runner: Match {query:?} -> 1 uuid item");
            return items;
        }
        if let Some(rand) = rand::parse_rand_query(query) {
            let items = rand::build_rand_matches(&rand);
            eprintln!("devtools-runner: Match {query:?} -> 1 rand item");
            return items;
        }
        let suffixes = time::suffixes_for_query(query);
        let items = time::build_matches(&suffixes);
        eprintln!(
            "devtools-runner: Match {query:?} -> {} item(s)",
            items.len()
        );
        items
    }

    /// 支持的动作。MVP 阶段没有额外动作（默认动作即复制）。
    fn Actions(&self) -> Vec<(String, String, String)> {
        Vec::new()
    }

    /// 对某条 match 执行默认动作（复制 + 通知）。
    fn Run(&self, match_id: &str, _action_id: &str) -> zbus::fdo::Result<()> {
        if json::handles_match_id(match_id) {
            let mut cache = self
                .inline_json
                .lock()
                .map_err(|_| zbus::fdo::Error::Failed("JSON context lock poisoned".to_owned()))?;
            return json::open_workbench(match_id, &mut cache)
                .map_err(|error| zbus::fdo::Error::Failed(error.to_string()));
        }
        if match_id == data_convert::OPEN_MATCH_ID {
            return data_convert::open_workbench()
                .map_err(|error| zbus::fdo::Error::Failed(error.to_string()));
        }
        if media::handles_match_id(match_id) {
            return media::open_tool(match_id)
                .map_err(|error| zbus::fdo::Error::Failed(error.to_string()));
        }
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

    /// 一次 match 会话结束时清理直接输入的 JSON，保证正文只在必要期间驻留内存。
    fn Teardown(&self) {
        if let Ok(mut cache) = self.inline_json.lock() {
            cache.clear();
        }
    }
}

fn main() {
    eprintln!("devtools-runner: starting {SERVICE_NAME} at {OBJECT_PATH}");
    let _connection = match ConnectionBuilder::session()
        .and_then(|b| b.name(SERVICE_NAME))
        .and_then(|b| b.serve_at(OBJECT_PATH, DevTools::default()))
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

//! 时间/日期相关的查询逻辑。
//!
//! 由 `COMMANDS` 驱动的双向前缀匹配 → item 后缀 → `build_matches` 按 `ITEMS`
//! 顺序输出结果。用户在 KRunner 里输入 `date`、`time`、`ts`、`tms` 等都会
//! 命中这里的逻辑。

use std::collections::HashMap;

use chrono::{DateTime, Local, Utc};

use crate::{str_value, KMatch, CATEGORY, CATEGORY_RELEVANCE};

/// 本 runner 认识的命令：`(触发关键词, 要展示的 item 后缀)`。
/// 当查询等于某关键词、以某关键词为前缀、或被某关键词以前缀包含时即命中——
/// 因此 `da`/`date` 命中 `date`，而 `ts`/`unix`、`tms`/`tsm` 各自只映射到单条时间戳行。
/// 输入过程中可能同时命中多个命令（如 `t` 同时命中 `time`、`ts`、`tms`），
/// 此时把它们的后缀求并集去重，再由 `build_matches` 按 `ITEMS` 顺序输出。
const COMMANDS: &[(&[&str], &[&str])] = &[
    (
        &["date", "time"],
        &["local", "unix", "unixms", "rfc3339", "iso8601", "utc"],
    ),
    (&["ts", "unix"], &["unix"]),
    (&["tms", "tsm"], &["unixms"]),
    (&["now"], &["local", "hhmmss"]),
];

/// 每一条结果的静态定义：`(id 后缀, 标题, 图标名)`。
/// 具体取值在查询时计算，保证永远是当前值。
const ITEMS: &[(&str, &str, &str)] = &[
    ("local", "当前时间", "clock"),
    ("hhmmss", "当前时间 (仅时分秒)", "clock"),
    ("unix", "Unix 时间戳", "preferences-system-time"),
    ("unixms", "Unix 时间戳 (ms)", "preferences-system-time"),
    ("rfc3339", "RFC3339", "text-x-generic"),
    ("iso8601", "ISO8601", "text-x-generic"),
    ("utc", "UTC 时间", "clock"),
];

/// 根据某个固定时间点，计算给定后缀对应的可复制取值。
pub fn value_of(suffix: &str, now: &DateTime<Local>, utc: &DateTime<Utc>) -> String {
    match suffix {
        "local" => now.format("%Y-%m-%d %H:%M:%S").to_string(),
        "hhmmss" => now.format("%H:%M:%S").to_string(),
        "unix" => now.timestamp().to_string(),
        "unixms" => now.timestamp_millis().to_string(),
        "rfc3339" => now.format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
        "iso8601" => utc.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        "utc" => utc.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        _ => String::new(),
    }
}

/// 按给定的 item 后缀构造结果列表，输出顺序与 `ITEMS` 一致。
pub fn build_matches(suffixes: &[&str]) -> Vec<KMatch> {
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
                CATEGORY_RELEVANCE,
                1.0 - 0.03 * i as f64,
                props,
            )
        })
        .collect()
}

/// 本次查询应该展示哪些 item 后缀？
/// 查询未命中任何命令时返回空 vec（KRunner 随即不显示本 runner 的结果）。
/// 多个命中命令的后缀会求并集去重，再由 `build_matches` 按 `ITEMS` 顺序还原。
///
/// 精确匹配（`q` 等于某关键词）优先于前缀匹配：只要存在任意精确命中，就只采用精确命中的
/// 命令。这样 `tsm` 不会因为以 `ts` 开头而连带触发秒级命令，能干净地等于 `tms`。
pub fn suffixes_for_query(query: &str) -> Vec<&'static str> {
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
        assert!(suffixes_for_query("t").is_empty());
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
    }

    #[test]
    fn now_command_shows_local_and_hhmmss() {
        assert_eq!(suffixes_for_query("now"), vec!["local", "hhmmss"]);
        assert_eq!(suffixes_for_query("no"), vec!["local", "hhmmss"]);
    }

    #[test]
    fn value_of_hhmmss() {
        let (now, utc) = fixed();
        // 本地时间的时分秒取决于时区，这里只验证格式为 HH:MM:SS
        let v = value_of("hhmmss", &now, &utc);
        assert_eq!(v.len(), 8);
        assert_eq!(&v[2..3], ":");
        assert_eq!(&v[5..6], ":");
    }

    #[test]
    fn uppercase_queries_match() {
        assert_eq!(suffixes_for_query("DATE").len(), 6);
        assert_eq!(suffixes_for_query("NOW").len(), 2);
    }

    #[test]
    fn build_matches_orders_like_items_and_prefixes_ids() {
        let matches = build_matches(&["unixms", "unix", "missing"]);
        let ids: Vec<&str> = matches.iter().map(|(id, _, _, _, _, _)| id.as_str()).collect();
        assert_eq!(ids, vec!["date:unix", "date:unixms"]);
    }
}

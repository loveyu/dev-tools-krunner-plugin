//! 时间戳 ↔ 可读时间字符串的双向转换查询逻辑。
//!
//! 与 `rand` / `uuid` 等「带前缀」的查询不同，本模块**不依赖固定触发词**，
//! 而是根据输入的「形状」自动判定方向：
//!
//! - **输入是纯数字**（且长度 ≥ 9）→ 当作 Unix 时间戳。位数 ≥ 13 视为毫秒，
//!   否则视为秒；展示本地时区（优先）、UTC 以及 RFC3339 / ISO8601 等可读时间。
//! - **输入含分隔符**（看起来像日期字符串）→ 尝试用多种常见格式解析，
//!   成功后展示对应的 Unix 秒 / 毫秒时间戳（以及本地时间作为解析确认）。
//!
//! 两个方向的结果都由同一个「绝对秒数」派生，因此 match id 里只编码
//! `(方向, 秒数, 格式)`，`Run` 时据此原样重建——确定性取值，无需把原始
//! 输入串塞进 id。

use std::collections::HashMap;

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};

use crate::{str_value, KMatch, CATEGORY, CATEGORY_RELEVANCE};

/// 时间戳方向展示的几行结果：`(格式 key, 标题, 图标)`。
/// 本地时区排在最前（用户最关心），其后是 UTC 与两种标准交换格式。
const TS_ITEMS: &[(&str, &str, &str)] = &[
    ("local", "本地时间", "clock"),
    ("utc", "UTC 时间", "clock"),
    ("rfc3339", "RFC3339", "text-x-generic"),
    ("iso8601", "ISO8601", "text-x-generic"),
];

/// 日期字符串方向展示的几行结果：`(格式 key, 标题, 图标)`。
const DT_ITEMS: &[(&str, &str, &str)] = &[
    ("unix", "Unix 时间戳 (秒)", "preferences-system-time"),
    ("unixms", "Unix 时间戳 (毫秒)", "preferences-system-time"),
    ("local", "本地时间", "clock"),
];

/// 解析后的转换查询。两个变体都已归一化为「绝对 Unix 秒」。
#[derive(Debug, Clone, PartialEq)]
pub enum ConvertQuery {
    /// 输入是 Unix 时间戳（秒或毫秒），展示可读时间。
    Timestamp { secs: i64 },
    /// 输入是日期字符串，展示对应时间戳。
    Datetime { secs: i64 },
}

/// 尝试从用户输入解析出转换查询。不匹配时返回 `None`。
///
/// 判定顺序：先看是否「纯数字时间戳」，再看是否「可解析的日期字符串」。
/// 二者互斥——纯数字串不会进入日期解析（避免 `20240806` 这类紧凑写法与
/// 时间戳混淆）；日期字符串至少含一个非数字分隔符。
pub fn parse_convert_query(query: &str) -> Option<ConvertQuery> {
    let s = query.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(secs) = parse_timestamp(s) {
        return Some(ConvertQuery::Timestamp { secs });
    }
    if let Some(secs) = parse_datetime(s) {
        return Some(ConvertQuery::Datetime { secs });
    }
    None
}

/// 把「方向 + 秒数 + 格式 key」渲染成可复制的取值。`build` 与 `Run` 重建共用。
fn render(kind: &str, secs: i64, fmt: &str) -> Option<String> {
    let dt_utc = Utc.timestamp_opt(secs, 0).single()?;
    let dt_local = dt_utc.with_timezone(&Local);
    match (kind, fmt) {
        ("ts", "local") => Some(dt_local.format("%Y-%m-%d %H:%M:%S").to_string()),
        ("ts", "utc") => Some(dt_utc.format("%Y-%m-%d %H:%M:%S UTC").to_string()),
        ("ts", "rfc3339") => Some(dt_local.format("%Y-%m-%dT%H:%M:%S%:z").to_string()),
        ("ts", "iso8601") => Some(dt_utc.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
        ("dt", "unix") => Some(secs.to_string()),
        // 毫秒由秒乘 1000 得到；secs 极大时可能溢出，届时该行干脆不出现。
        ("dt", "unixms") => Some(secs.checked_mul(1000)?.to_string()),
        ("dt", "local") => Some(dt_local.format("%Y-%m-%d %H:%M:%S").to_string()),
        _ => None,
    }
}

/// 构造转换查询对应的 KRunner 结果。
pub fn build_convert_matches(query: &ConvertQuery) -> Vec<KMatch> {
    let (kind, secs, items) = match query {
        ConvertQuery::Timestamp { secs } => ("ts", *secs, TS_ITEMS),
        ConvertQuery::Datetime { secs } => ("dt", *secs, DT_ITEMS),
    };
    items
        .iter()
        .enumerate()
        .filter_map(|(i, (fmt, title, icon))| {
            let value = render(kind, secs, fmt)?;
            let mut props = HashMap::new();
            props.insert("subtext".to_string(), str_value(value));
            props.insert("category".to_string(), str_value(CATEGORY));
            Some((
                format!("conv:{kind}:{secs}:{fmt}"),
                (*title).to_string(),
                (*icon).to_string(),
                CATEGORY_RELEVANCE,
                1.0 - 0.03 * i as f64,
                props,
            ))
        })
        .collect()
}

/// 根据 conv id 后缀（`"<kind>:<secs>:<fmt>"`）重建取值（被 Run 调用）。
/// `secs` 与 `fmt` 都不含 `:`，故按 `:` 三段切分即可。
pub fn value_for_convert_id(suffix: &str) -> Option<String> {
    let mut parts = suffix.splitn(3, ':');
    let kind = parts.next()?;
    let secs: i64 = parts.next()?.parse().ok()?;
    let fmt = parts.next()?;
    render(kind, secs, fmt)
}

/// 解析纯数字时间戳为「绝对 Unix 秒」。
///
/// - 长度 < 9 或含非数字字符 → 不是时间戳（返回 `None`）。
/// - 长度 ≥ 13 → 毫秒（`1e12` ≈ 2001-09-09，13 位起几乎只可能是毫秒）。
/// - 否则 → 秒。
/// - 再用合理时间窗口（1970 .. 2300）兜底，过滤掉位数对不上但数值离谱的输入。
///
/// 极长数字串 `i64` 解析溢出时直接判失败，不会 panic。
fn parse_timestamp(s: &str) -> Option<i64> {
    if s.len() < 9 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let v: i64 = s.parse().ok()?;
    let secs = if s.len() >= 13 { v / 1000 } else { v };
    // 2300-01-01T00:00:00Z ≈ 10_459_718_400 秒。
    if !(0..=10_459_718_400).contains(&secs) {
        return None;
    }
    Some(secs)
}

/// 把日期字符串解析为「绝对 Unix 秒」。至少需含一个非数字分隔符。
///
/// 解析顺序（先精确、后宽松）：
/// 1. RFC3339 —— 覆盖 ISO8601 带时区偏移 / `Z` / 小数秒等机器可读格式；
/// 2. RFC2822 —— 邮件风格（`Tue, 01 Jul 2003 10:52:37 +0200`）；
/// 3. 常见无时区格式 —— 解析为 naive 时刻后**按本地时区**解释；
/// 4. 仅日期 —— 当作本地时区 00:00:00。
fn parse_datetime(s: &str) -> Option<i64> {
    // 纯数字（无分隔符）不在这里处理，避免与时间戳判定冲突。
    if !s.bytes().any(|b| !b.is_ascii_digit()) {
        return None;
    }

    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp());
    }
    if let Ok(dt) = DateTime::parse_from_rfc2822(s) {
        return Some(dt.timestamp());
    }

    // 含日期 + 时间成分的格式（naive，按本地时区解释）。
    let datetime_formats = [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M",
        "%Y/%m/%d %H:%M:%S",
        "%Y/%m/%d %H:%M",
    ];
    for fmt in datetime_formats {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(s, fmt) {
            return Local
                .from_local_datetime(&ndt)
                .single()
                .map(|dt| dt.timestamp());
        }
    }

    // 仅日期格式（本地时区 00:00:00）。
    let date_formats = ["%Y-%m-%d", "%Y/%m/%d"];
    for fmt in date_formats {
        if let Ok(nd) = NaiveDate::parse_from_str(s, fmt) {
            let ndt = nd.and_hms_opt(0, 0, 0)?;
            return Local
                .from_local_datetime(&ndt)
                .single()
                .map(|dt| dt.timestamp());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `2024-08-06T00:00:00Z` 的 Unix 秒，时区无关，便于断言。
    const SECS_2024_08_06: i64 = 1_722_902_400;

    #[test]
    fn parses_seconds_timestamp() {
        assert_eq!(
            parse_convert_query(&SECS_2024_08_06.to_string()),
            Some(ConvertQuery::Timestamp {
                secs: SECS_2024_08_06
            })
        );
        // 9 位（1973 起）也算秒
        assert!(matches!(
            parse_convert_query("100000000"),
            Some(ConvertQuery::Timestamp { secs: 100_000_000 })
        ));
    }

    #[test]
    fn parses_millis_timestamp() {
        let ms = SECS_2024_08_06 * 1000;
        assert_eq!(
            parse_convert_query(&ms.to_string()),
            Some(ConvertQuery::Timestamp {
                secs: SECS_2024_08_06
            })
        );
    }

    #[test]
    fn rejects_short_or_huge_numbers() {
        // 太短：不像时间戳
        assert!(parse_convert_query("12345678").is_none());
        assert!(parse_convert_query("123").is_none());
        // 超出合理窗口（年份远超 2300）→ 即便位数像也拒绝
        assert!(parse_convert_query("99000000000").is_none());
    }

    #[test]
    fn parses_rfc3339_with_various_offsets() {
        // Z
        assert_eq!(
            parse_convert_query("2024-08-06T00:00:00Z"),
            Some(ConvertQuery::Datetime {
                secs: SECS_2024_08_06
            })
        );
        // +00:00
        assert_eq!(
            parse_convert_query("2024-08-06T00:00:00+00:00"),
            Some(ConvertQuery::Datetime {
                secs: SECS_2024_08_06
            })
        );
        // +08:00 对应 UTC 00:00 → 同一绝对秒
        assert_eq!(
            parse_convert_query("2024-08-06T08:00:00+08:00"),
            Some(ConvertQuery::Datetime {
                secs: SECS_2024_08_06
            })
        );
    }

    #[test]
    fn parses_naive_datetime_as_local() {
        // 带时间的常见格式应被识别为 Datetime（具体秒数依赖本机时区，仅断言方向）
        assert!(matches!(
            parse_convert_query("2024-08-06 12:34:56"),
            Some(ConvertQuery::Datetime { .. })
        ));
        assert!(matches!(
            parse_convert_query("2024-08-06T12:34:56"),
            Some(ConvertQuery::Datetime { .. })
        ));
        assert!(matches!(
            parse_convert_query("2024/08/06 12:34:56"),
            Some(ConvertQuery::Datetime { .. })
        ));
        assert!(matches!(
            parse_convert_query("2024-08-06"),
            Some(ConvertQuery::Datetime { .. })
        ));
    }

    #[test]
    fn rejects_non_datetime_text() {
        assert!(parse_convert_query("hello").is_none());
        assert!(parse_convert_query("2024-13-99").is_none());
        assert!(parse_convert_query("not-a-date").is_none());
    }

    #[test]
    fn render_timestamp_formats() {
        assert_eq!(
            render("ts", SECS_2024_08_06, "iso8601"),
            Some("2024-08-06T00:00:00Z".to_string())
        );
        assert_eq!(
            render("ts", SECS_2024_08_06, "utc"),
            Some("2024-08-06 00:00:00 UTC".to_string())
        );
    }

    #[test]
    fn render_datetime_formats() {
        assert_eq!(
            render("dt", SECS_2024_08_06, "unix"),
            Some("1722902400".to_string())
        );
        assert_eq!(
            render("dt", SECS_2024_08_06, "unixms"),
            Some("1722902400000".to_string())
        );
    }

    #[test]
    fn build_timestamp_matches_order_and_ids() {
        let m = build_convert_matches(&ConvertQuery::Timestamp {
            secs: SECS_2024_08_06,
        });
        let ids: Vec<&str> = m.iter().map(|(id, _, _, _, _, _)| id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "conv:ts:1722902400:local",
                "conv:ts:1722902400:utc",
                "conv:ts:1722902400:rfc3339",
                "conv:ts:1722902400:iso8601",
            ]
        );
    }

    #[test]
    fn build_datetime_matches_has_unix_rows() {
        let m = build_convert_matches(&ConvertQuery::Datetime {
            secs: SECS_2024_08_06,
        });
        assert_eq!(m.len(), 3);
        assert_eq!(m[0].0, "conv:dt:1722902400:unix");
        assert_eq!(m[1].0, "conv:dt:1722902400:unixms");
    }

    #[test]
    fn convert_id_roundtrip() {
        assert_eq!(
            value_for_convert_id("ts:1722902400:iso8601"),
            Some("2024-08-06T00:00:00Z".to_string())
        );
        assert_eq!(
            value_for_convert_id("dt:1722902400:unixms"),
            Some("1722902400000".to_string())
        );
        // 未知格式 / 非法秒 → None
        assert!(value_for_convert_id("ts:abc:local").is_none());
        assert!(value_for_convert_id("ts:1722902400:nope").is_none());
    }
}

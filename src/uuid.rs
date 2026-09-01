//! UUID 生成查询逻辑。
//!
//! 支持的输入格式（大小写不敏感，`C` 大写敏感表示大写模式，数字表示版本）：
//! - `uuid` / `u` — UUID v4 标准格式（小写，带连字符）
//! - `u4` / `uuid4` — UUID v4（显式）
//! - `u7` / `uuid7` — UUID v7（时间有序，含 Unix 毫秒时间戳）
//! - `u1` / `uuid1` — UUID v1（基于时间 + 随机节点 ID）
//! - `uc` / `uuidc` — UUID v4 紧凑格式（小写，无连字符）
//! - `UC` / `uC` / `uuidC` — UUID v4 大写格式（大写，带连字符）
//! - `u4c` / `u7C` 等 — 版本 + 格式可组合

use std::collections::HashMap;

use rand::Rng;
use uuid::Uuid;

use crate::{str_value, KMatch, CATEGORY, CATEGORY_RELEVANCE};

/// UUID 版本。
#[derive(Debug, Clone, PartialEq)]
pub enum UuidVersion {
    V1,
    V4,
    V7,
}

impl UuidVersion {
    fn as_str(&self) -> &'static str {
        match self {
            UuidVersion::V1 => "v1",
            UuidVersion::V4 => "v4",
            UuidVersion::V7 => "v7",
        }
    }

    fn from_str(s: &str) -> Option<UuidVersion> {
        match s {
            "v1" => Some(UuidVersion::V1),
            "v4" => Some(UuidVersion::V4),
            "v7" => Some(UuidVersion::V7),
            _ => None,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            UuidVersion::V1 => "v1",
            UuidVersion::V4 => "v4",
            UuidVersion::V7 => "v7",
        }
    }
}

/// UUID 输出格式。
#[derive(Debug, Clone, PartialEq)]
pub enum UuidFormat {
    /// 标准格式（小写，带连字符）: `550e8400-e29b-41d4-a716-446655440000`
    Standard,
    /// 紧凑格式（小写，无连字符）: `550e8400e29b41d4a716446655440000`
    Compact,
    /// 大写格式（大写，带连字符）: `550E8400-E29B-41D4-A716-446655440000`
    Upper,
}

impl UuidFormat {
    fn as_str(&self) -> &'static str {
        match self {
            UuidFormat::Standard => "standard",
            UuidFormat::Compact => "compact",
            UuidFormat::Upper => "upper",
        }
    }

    fn from_str(s: &str) -> Option<UuidFormat> {
        match s {
            "standard" => Some(UuidFormat::Standard),
            "compact" => Some(UuidFormat::Compact),
            "upper" => Some(UuidFormat::Upper),
            _ => None,
        }
    }

    fn suffix(&self, version: &UuidVersion) -> &'static str {
        match (version, self) {
            (_, UuidFormat::Standard) => "小写，带连字符",
            (_, UuidFormat::Compact) => "紧凑格式，无连字符",
            (_, UuidFormat::Upper) => "大写，带连字符",
        }
    }
}

/// 解析后的 UUID 查询参数。
pub struct UuidQuery {
    pub version: UuidVersion,
    pub format: UuidFormat,
}

/// 根据版本和格式生成 UUID 字符串。
pub fn generate_uuid(version: &UuidVersion, format: &UuidFormat) -> String {
    let uuid = match version {
        UuidVersion::V1 => {
            let mut node_id = [0u8; 6];
            rand::thread_rng().fill(&mut node_id);
            Uuid::now_v1(&node_id)
        }
        UuidVersion::V4 => Uuid::new_v4(),
        UuidVersion::V7 => Uuid::now_v7(),
    };
    match format {
        UuidFormat::Standard => uuid.hyphenated().to_string(),
        UuidFormat::Compact => uuid.simple().to_string(),
        UuidFormat::Upper => uuid.hyphenated().to_string().to_uppercase(),
    }
}

/// 尝试从用户输入中解析出 UUID 查询。不匹配时返回 `None`。
///
/// 解析规则（基于字符，非字节）：
/// 1. 前缀必须是 `u` 或 `uuid`（大小写不敏感）
/// 2. 前缀之后：
///    - 空 → v4 Standard
///    - 数字 '1'/'4'/'7' → 对应版本，可选后续格式修饰符
///    - 'c' → v4 Compact
///    - 'C' → v4 Upper
/// 3. 额外规则：仅前缀且全大写（如 `UUID`）→ v4 Upper
pub fn parse_uuid_query(query: &str) -> Option<UuidQuery> {
    let raw = query.trim();
    if raw.is_empty() {
        return None;
    }

    let q = raw.to_lowercase();
    let prefix_len = if q.starts_with("uuid") {
        4
    } else if q.starts_with('u') {
        1
    } else {
        return None;
    };

    // 小写串的前缀长度可能与原串的字节边界不重合（多字节字符小写变形），
    // 用 get 安全切片，越界即视为不匹配，避免 D-Bus 回调里 panic。
    let after = raw.get(prefix_len..)?.trim();

    parse_version_format(after).or_else(|| {
        // 无修饰符：全大写 → v4 Upper，否则 v4 Standard
        if after.is_empty() {
            if raw.chars().all(|c| c.is_ascii_uppercase()) && raw.len() > 1 {
                Some(UuidQuery {
                    version: UuidVersion::V4,
                    format: UuidFormat::Upper,
                })
            } else {
                Some(UuidQuery {
                    version: UuidVersion::V4,
                    format: UuidFormat::Standard,
                })
            }
        } else {
            None
        }
    })
}

/// 解析 `after` 部分的 版本号 + 格式修饰符。
/// 支持：`"7"`, `"4c"`, `"7C"`, `"c"`, `"C"` 等。
fn parse_version_format(after: &str) -> Option<UuidQuery> {
    if after.is_empty() {
        return None;
    }
    let chars: Vec<char> = after.chars().collect();
    let mut pos = 0usize;
    let mut version = None;
    let mut format = None;

    // 解析版本数字（0-1 位）
    if pos < chars.len() && chars[pos].is_ascii_digit() {
        version = Some(match chars[pos] {
            '1' => UuidVersion::V1,
            '4' => UuidVersion::V4,
            '7' => UuidVersion::V7,
            _ => return None,
        });
        pos += 1;
    }

    // 解析格式修饰符（0-1 位）
    if pos < chars.len() {
        match chars[pos] {
            'c' => format = Some(UuidFormat::Compact),
            'C' => format = Some(UuidFormat::Upper),
            _ => return None,
        }
        pos += 1;
    }

    // 不允许余留字符
    if pos != chars.len() {
        return None;
    }

    Some(UuidQuery {
        version: version.unwrap_or(UuidVersion::V4),
        format: format.unwrap_or(UuidFormat::Standard),
    })
}

/// 构造 UUID 查询对应的 KRunner 结果（仅单条 match）。
pub fn build_uuid_matches(query: &UuidQuery) -> Vec<KMatch> {
    let value = generate_uuid(&query.version, &query.format);
    let mut props = HashMap::new();
    props.insert("subtext".to_string(), str_value(value.clone()));
    props.insert("category".to_string(), str_value(CATEGORY));

    let id = format!("uuid:{}:{}", query.version.as_str(), query.format.as_str());
    let title = format!(
        "UUID {} ({})",
        query.version.label(),
        query.format.suffix(&query.version)
    );

    vec![(
        id,
        title,
        "code-class".to_string(),
        CATEGORY_RELEVANCE,
        0.97,
        props,
    )]
}

/// 根据 UUID id 后缀（`"<version>:<format>"` 格式）重新生成 UUID。
pub fn value_for_uuid_id(suffix: &str) -> Option<String> {
    let (version_str, format_str) = suffix.rsplit_once(':')?;
    let version = UuidVersion::from_str(version_str)?;
    let format = UuidFormat::from_str(format_str)?;
    Some(generate_uuid(&version, &format))
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_query {
        ($query:expr, $ver:pat, $fmt:pat) => {
            let q = parse_uuid_query($query).expect(concat!("parse failed: ", $query));
            assert!(
                matches!(q.version, $ver),
                "{:?}: expected version {}, got {:?}",
                $query,
                stringify!($ver),
                q.version
            );
            assert!(
                matches!(q.format, $fmt),
                "{:?}: expected format {}, got {:?}",
                $query,
                stringify!($fmt),
                q.format
            );
        };
    }

    #[test]
    fn parse_default() {
        assert_query!("u", UuidVersion::V4, UuidFormat::Standard);
        assert_query!("uuid", UuidVersion::V4, UuidFormat::Standard);
    }

    #[test]
    fn parse_explicit_version() {
        assert_query!("u4", UuidVersion::V4, UuidFormat::Standard);
        assert_query!("uuid4", UuidVersion::V4, UuidFormat::Standard);
        assert_query!("u7", UuidVersion::V7, UuidFormat::Standard);
        assert_query!("uuid7", UuidVersion::V7, UuidFormat::Standard);
        assert_query!("u1", UuidVersion::V1, UuidFormat::Standard);
        assert_query!("uuid1", UuidVersion::V1, UuidFormat::Standard);
    }

    #[test]
    fn parse_compact() {
        assert_query!("uc", UuidVersion::V4, UuidFormat::Compact);
        assert_query!("uuidc", UuidVersion::V4, UuidFormat::Compact);
    }

    #[test]
    fn parse_upper() {
        assert_query!("UC", UuidVersion::V4, UuidFormat::Upper);
        assert_query!("uC", UuidVersion::V4, UuidFormat::Upper);
        assert_query!("uuidC", UuidVersion::V4, UuidFormat::Upper);
        assert_query!("UUID", UuidVersion::V4, UuidFormat::Upper);
    }

    #[test]
    fn parse_version_plus_format() {
        assert_query!("u4c", UuidVersion::V4, UuidFormat::Compact);
        assert_query!("u7c", UuidVersion::V7, UuidFormat::Compact);
        assert_query!("u1c", UuidVersion::V1, UuidFormat::Compact);
        assert_query!("u4C", UuidVersion::V4, UuidFormat::Upper);
        assert_query!("u7C", UuidVersion::V7, UuidFormat::Upper);
        assert_query!("u1C", UuidVersion::V1, UuidFormat::Upper);
    }

    #[test]
    fn parse_case_insensitive_prefix() {
        assert_query!("U", UuidVersion::V4, UuidFormat::Standard);
        assert_query!("U7", UuidVersion::V7, UuidFormat::Standard);
        assert_query!("U4c", UuidVersion::V4, UuidFormat::Compact);
    }

    #[test]
    fn parse_invalid() {
        assert!(parse_uuid_query("ua").is_none());
        assert!(parse_uuid_query("u z").is_none());
        assert!(parse_uuid_query("xyz").is_none());
        assert!(parse_uuid_query("ud").is_none());
        assert!(parse_uuid_query("uid").is_none());
        assert!(parse_uuid_query("u9").is_none());
        assert!(parse_uuid_query("u47").is_none());
        assert!(parse_uuid_query("u4x").is_none());
    }

    #[test]
    fn generate_v4() {
        let s = generate_uuid(&UuidVersion::V4, &UuidFormat::Standard);
        assert_eq!(s.len(), 36);
        assert!(s.contains('-'));
        // v4 版本位 = 4
        assert_eq!(&s[14..15], "4");
    }

    #[test]
    fn generate_v7() {
        let s = generate_uuid(&UuidVersion::V7, &UuidFormat::Standard);
        assert_eq!(s.len(), 36);
        assert!(s.contains('-'));
        // v7 版本位 = 7
        assert_eq!(&s[14..15], "7");
    }

    #[test]
    fn generate_v1() {
        let s = generate_uuid(&UuidVersion::V1, &UuidFormat::Standard);
        assert_eq!(s.len(), 36);
        assert!(s.contains('-'));
        // v1 版本位 = 1
        assert_eq!(&s[14..15], "1");
    }

    #[test]
    fn generate_compact() {
        let s = generate_uuid(&UuidVersion::V4, &UuidFormat::Compact);
        assert_eq!(s.len(), 32);
        assert!(!s.contains('-'));
    }

    #[test]
    fn generate_upper() {
        let s = generate_uuid(&UuidVersion::V4, &UuidFormat::Upper);
        assert_eq!(s.len(), 36);
        assert!(s.contains('-'));
        assert!(s
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-'));
    }

    #[test]
    fn generate_different_each_call() {
        let a = generate_uuid(&UuidVersion::V4, &UuidFormat::Standard);
        let b = generate_uuid(&UuidVersion::V4, &UuidFormat::Standard);
        assert_ne!(a, b);
    }

    #[test]
    fn id_roundtrip() {
        let q = parse_uuid_query("u7c").unwrap();
        let matches = build_uuid_matches(&q);
        let id = &matches[0].0;
        assert!(id.starts_with("uuid:v7:compact"));
        let suffix = id.strip_prefix("uuid:").unwrap();
        let value = value_for_uuid_id(suffix).unwrap();
        assert_eq!(value.len(), 32);
        assert!(!value.contains('-'));
    }
}

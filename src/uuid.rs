//! UUID 生成查询逻辑。
//!
//! 支持的输入格式（大小写不敏感，但 `C` 大写 C 敏感表示大写模式）：
//! - `uuid` / `u` — UUID v4 标准格式（小写，带连字符）
//! - `uc` / `uuidc` — UUID v4 紧凑格式（小写，无连字符）
//! - `UC` / `uC` / `uuidC` — UUID v4 大写格式（大写，带连字符）

use std::collections::HashMap;

use rand::Rng;

use crate::{str_value, KMatch, CATEGORY, CATEGORY_RELEVANCE};

/// UUID 的格式化模式。
#[derive(Debug, Clone, PartialEq)]
pub enum UuidMode {
    /// 标准格式（小写，带连字符）: `550e8400-e29b-41d4-a716-446655440000`
    Standard,
    /// 紧凑格式（小写，无连字符）: `550e8400e29b41d4a716446655440000`
    Compact,
    /// 大写格式（大写，带连字符）: `550E8400-E29B-41D4-A716-446655440000`
    Upper,
}

impl UuidMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            UuidMode::Standard => "standard",
            UuidMode::Compact => "compact",
            UuidMode::Upper => "upper",
        }
    }

    pub fn from_str(s: &str) -> Option<UuidMode> {
        match s {
            "standard" => Some(UuidMode::Standard),
            "compact" => Some(UuidMode::Compact),
            "upper" => Some(UuidMode::Upper),
            _ => None,
        }
    }

    fn title(&self) -> &'static str {
        match self {
            UuidMode::Standard => "UUID v4 (小写，带连字符)",
            UuidMode::Compact => "UUID v4 (紧凑格式，无连字符)",
            UuidMode::Upper => "UUID v4 (大写，带连字符)",
        }
    }
}

/// 生成原始的 UUID v4 字节（16 字节）。
/// 按 RFC 9562 设置 version (4) 和 variant (10xx) 位。
fn generate_uuid_bytes() -> [u8; 16] {
    let mut rng = rand::thread_rng();
    let mut bytes = [0u8; 16];
    rng.fill(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    bytes
}

/// 根据模式把 UUID 字节格式化为目标字符串。
pub fn format_uuid(bytes: &[u8; 16], mode: &UuidMode) -> String {
    match mode {
        UuidMode::Standard => {
            let s: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
            format!(
                "{}-{}-{}-{}-{}",
                &s[0..8],
                &s[8..12],
                &s[12..16],
                &s[16..20],
                &s[20..32],
            )
        }
        UuidMode::Compact => bytes.iter().map(|b| format!("{b:02x}")).collect(),
        UuidMode::Upper => {
            let s: String = bytes.iter().map(|b| format!("{b:02X}")).collect();
            format!(
                "{}-{}-{}-{}-{}",
                &s[0..8],
                &s[8..12],
                &s[12..16],
                &s[16..20],
                &s[20..32],
            )
        }
    }
}

/// 尝试从用户输入中解析出 UUID 查询。不匹配时返回 `None`。
///
/// 大小写不敏感（前缀 `u` / `uuid` 匹配时忽略大小写），
/// 但 `C`（大写）保留用于表示大写模式——`uc` → Compact、`UC` / `uC` → Upper。
/// 额外规则：全部大写的完整查询（如 `UUID`，无修饰符余留）视为 Upper。
pub fn parse_uuid_query(query: &str) -> Option<UuidMode> {
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

    let after = raw[prefix_len..].trim();

    // 无修饰符：若原始查询全部为大写（如 `UUID`）则视为 Upper，否则 Standard
    if after.is_empty() {
        if raw.len() > 1 && raw.chars().all(|c| c.is_ascii_uppercase()) {
            return Some(UuidMode::Upper);
        }
        return Some(UuidMode::Standard);
    }

    let first = after.chars().next()?;
    match first {
        'c' => {
            if after[first.len_utf8()..].trim().is_empty() {
                return Some(UuidMode::Compact);
            }
            None
        }
        'C' => {
            if after[first.len_utf8()..].trim().is_empty() {
                return Some(UuidMode::Upper);
            }
            None
        }
        _ => None,
    }
}

/// 构造 UUID 查询对应的 KRunner 结果（仅单条 match）。
pub fn build_uuid_matches(mode: &UuidMode) -> Vec<KMatch> {
    let bytes = generate_uuid_bytes();
    let value = format_uuid(&bytes, mode);
    let mut props = HashMap::new();
    props.insert("subtext".to_string(), str_value(value.clone()));
    props.insert("category".to_string(), str_value(CATEGORY));

    let id = format!("uuid:{}", mode.as_str());
    let title = mode.title().to_string();

    vec![(
        id,
        title,
        "code-class".to_string(),
        CATEGORY_RELEVANCE,
        0.97,
        props,
    )]
}

/// 根据 UUID id 后缀（`"<mode>"` 格式）重新生成 UUID。
pub fn value_for_uuid_id(suffix: &str) -> Option<String> {
    let mode = UuidMode::from_str(suffix)?;
    let bytes = generate_uuid_bytes();
    Some(format_uuid(&bytes, &mode))
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_mode {
        ($query:expr, $mode:pat) => {
            let m = parse_uuid_query($query).expect(concat!("parse failed: ", $query));
            assert!(
                matches!(m, $mode),
                "{:?}: expected mode {}, got {:?}",
                $query,
                stringify!($mode),
                m
            );
        };
    }

    #[test]
    fn parse_uuid_standard() {
        assert_mode!("u", UuidMode::Standard);
        assert_mode!("uuid", UuidMode::Standard);
        assert_mode!("Uuid", UuidMode::Standard);
    }

    #[test]
    fn parse_uuid_compact() {
        assert_mode!("uc", UuidMode::Compact);
        assert_mode!("uuidc", UuidMode::Compact);
        assert_mode!("Uc", UuidMode::Compact);
    }

    #[test]
    fn parse_uuid_upper() {
        assert_mode!("UC", UuidMode::Upper);
        assert_mode!("uC", UuidMode::Upper);
        assert_mode!("uuidC", UuidMode::Upper);
        assert_mode!("UUID", UuidMode::Upper);
    }

    #[test]
    fn parse_uuid_invalid() {
        assert!(parse_uuid_query("ua").is_none());
        assert!(parse_uuid_query("u z").is_none());
        assert!(parse_uuid_query("xyz").is_none());
        assert!(parse_uuid_query("ud").is_none());
        assert!(parse_uuid_query("uid").is_none());
    }

    #[test]
    fn format_uuid_length() {
        let bytes = generate_uuid_bytes();
        let s = format_uuid(&bytes, &UuidMode::Standard);
        assert_eq!(s.len(), 36);
        assert!(s.contains('-'));
        assert!(s.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));

        let s = format_uuid(&bytes, &UuidMode::Compact);
        assert_eq!(s.len(), 32);
        assert!(!s.contains('-'));
        assert!(s.chars().all(|c| c.is_ascii_hexdigit()));

        let s = format_uuid(&bytes, &UuidMode::Upper);
        assert_eq!(s.len(), 36);
        assert!(s.contains('-'));
        assert!(s.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-'));
    }

    #[test]
    fn uuid_version_and_variant() {
        // 验证生成的 UUID 是 v4 版本和 RFC 变体
        let bytes = generate_uuid_bytes();
        assert_eq!(bytes[6] >> 4, 4, "UUID 版本字段应为 4");
        assert!(bytes[8] >> 6 == 2, "UUID 变体字段应为 10xx");
    }

    #[test]
    fn generate_uuid_different_each_call() {
        let a = generate_uuid_bytes();
        let b = generate_uuid_bytes();
        assert_ne!(a, b);
    }

    #[test]
    fn uuid_id_roundtrip() {
        let m = parse_uuid_query("u").unwrap();
        let matches = build_uuid_matches(&m);
        let id = &matches[0].0;
        assert!(id.starts_with("uuid:standard"));
        let suffix = id.strip_prefix("uuid:").unwrap();
        let value = value_for_uuid_id(suffix).unwrap();
        assert_eq!(value.len(), 36);
        assert!(value.chars().any(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'));
    }
}

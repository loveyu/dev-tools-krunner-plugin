//! 随机字符串生成查询逻辑。
//!
//! 支持的输入格式（大小写不敏感，但 rC/randC 大写 C 敏感表示大写字母）：
//! - `r16` / `rand16` / `rand 16` — 随机字母数字（a-z、A-Z、0-9），指定长度
//! - `r+16` / `rand+16` — 随机可见字符（含常见符号 !..~）
//! - `rn16` / `randn16` — 随机数字
//! - `rc16` / `randc16` — 随机小写字母
//! - `rC16` / `randC16` — 随机大写字母
//!
//! 最大长度限制 256。

use std::collections::HashMap;

use rand::Rng;

use crate::{str_value, KMatch, CATEGORY, CATEGORY_RELEVANCE};

/// 随机字符串的生成模式。
#[derive(Debug, Clone, PartialEq)]
pub enum RandMode {
    /// a-z、A-Z、0-9
    AlphaNum,
    /// 可见字符（含符号，!..~）
    Visible,
    /// 仅数字 0-9
    Digits,
    /// 仅小写字母 a-z
    Lower,
    /// 仅大写字母 A-Z
    Upper,
}

impl RandMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            RandMode::AlphaNum => "alphanum",
            RandMode::Visible => "visible",
            RandMode::Digits => "digits",
            RandMode::Lower => "lower",
            RandMode::Upper => "upper",
        }
    }

    pub fn from_str(s: &str) -> Option<RandMode> {
        match s {
            "alphanum" => Some(RandMode::AlphaNum),
            "visible" => Some(RandMode::Visible),
            "digits" => Some(RandMode::Digits),
            "lower" => Some(RandMode::Lower),
            "upper" => Some(RandMode::Upper),
            _ => None,
        }
    }

    fn title(&self, length: usize) -> String {
        match self {
            RandMode::AlphaNum => format!("随机字母数字 ({} 位)", length),
            RandMode::Visible => format!("随机可见字符 ({} 位)", length),
            RandMode::Digits => format!("随机数字 ({} 位)", length),
            RandMode::Lower => format!("随机小写字母 ({} 位)", length),
            RandMode::Upper => format!("随机大写字母 ({} 位)", length),
        }
    }
}

/// 解析后的随机查询参数。
pub struct RandQuery {
    pub mode: RandMode,
    pub length: usize,
}

/// 尝试从用户输入中解析出随机查询。不匹配时返回 `None`。
///
/// 大小写不敏感，但 **`C`（大写）保留用于表示大写字母模式**——
/// 所以 `rc` → Lower、`rC` / `RC` → Upper。
///
/// 解析逻辑：跳过字母前缀（`r` 或 `rand`，大小写不敏感），然后看第一个
/// 非字母非空格字符，根据它决定模式。
pub fn parse_rand_query(query: &str) -> Option<RandQuery> {
    let raw = query.trim();
    if raw.len() < 3 {
        return None;
    }

    // 确认以 `r` 或 `rand` 开头（大小写不敏感）
    let q = raw.to_lowercase();
    let prefix_len = if q.starts_with("rand") { 4 } else if q.starts_with('r') { 1 } else { return None; };

    // 跳过前缀后面的空白符，定位到第一个有效字符
    let after = raw[prefix_len..].trim_start();
    let chars: Vec<char> = after.chars().collect();
    if chars.is_empty() {
        return None;
    }

    // 根据第一个字符确定模式和数值起点
    let (mode, num_offset) = match chars[0] {
        '+' => (RandMode::Visible, 1),
        'n' | 'N' => (RandMode::Digits, 1),
        'C' => (RandMode::Upper, 1),
        'c' => (RandMode::Lower, 1),
        ch if ch.is_ascii_digit() => (RandMode::AlphaNum, 0),
        _ => return None,
    };

    // 空白作分隔符，跳过数值之前的空格
    let num_str: String = chars[num_offset..].iter().collect();
    let length = num_str.trim_start().parse::<usize>().ok()?;
    if length == 0 || length > 256 {
        return None;
    }

    Some(RandQuery { mode, length })
}

/// 根据模式和长度生成一条随机字符串。
pub fn generate_rand(mode: &RandMode, length: usize) -> String {
    let mut rng = rand::thread_rng();
    match mode {
        RandMode::AlphaNum => {
            let chars: Vec<char> = (b'0'..=b'9')
                .chain(b'A'..=b'Z')
                .chain(b'a'..=b'z')
                .map(|c| c as char)
                .collect();
            (0..length)
                .map(|_| chars[rng.gen_range(0..chars.len())])
                .collect()
        }
        RandMode::Visible => {
            let chars: Vec<char> = (b'!'..=b'~').map(|c| c as char).collect();
            (0..length)
                .map(|_| chars[rng.gen_range(0..chars.len())])
                .collect()
        }
        RandMode::Digits => (0..length)
            .map(|_| (b'0' + rng.gen_range(0u8..10)) as char)
            .collect(),
        RandMode::Lower => (0..length)
            .map(|_| (b'a' + rng.gen_range(0u8..26)) as char)
            .collect(),
        RandMode::Upper => (0..length)
            .map(|_| (b'A' + rng.gen_range(0u8..26)) as char)
            .collect(),
    }
}

/// 构造随机查询对应的 KRunner 结果（仅单条 match）。
pub fn build_rand_matches(rand: &RandQuery) -> Vec<KMatch> {
    let value = generate_rand(&rand.mode, rand.length);
    let mut props = HashMap::new();
    props.insert("subtext".to_string(), str_value(value.clone()));
    props.insert("category".to_string(), str_value(CATEGORY));

    let id = format!("rand:{}:{}", rand.mode.as_str(), rand.length);
    let title = rand.mode.title(rand.length);

    vec![(
        id,
        title,
        "code-class".to_string(),
        CATEGORY_RELEVANCE,
        0.97,
        props,
    )]
}

/// 根据 rand id 后缀（`"<mode>:<length>"` 格式）重新生成随机字符串。
pub fn value_for_rand_id(suffix: &str) -> Option<String> {
    let (mode_str, len_str) = suffix.rsplit_once(':')?;
    let mode = RandMode::from_str(mode_str)?;
    let length = len_str.parse::<usize>().ok()?;
    Some(generate_rand(&mode, length))
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_mode {
        ($query:expr, $mode:pat, $len:expr) => {
            let r = parse_rand_query($query).expect(concat!("parse failed: ", $query));
            assert!(
                matches!(r.mode, $mode),
                "{:?}: expected mode {}, got {:?}",
                $query,
                stringify!($mode),
                r.mode
            );
            assert_eq!(r.length, $len, "{:?}: wrong length", $query);
        };
    }

    #[test]
    fn parse_rand_alphanum() {
        assert_mode!("r16", RandMode::AlphaNum, 16);
        assert_mode!("r 32", RandMode::AlphaNum, 32);
        assert_mode!("rand16", RandMode::AlphaNum, 16);
        assert_mode!("rand 8", RandMode::AlphaNum, 8);
        assert_mode!("R16", RandMode::AlphaNum, 16);
        assert_mode!("RAND32", RandMode::AlphaNum, 32);
    }

    #[test]
    fn parse_rand_visible() {
        assert_mode!("r+16", RandMode::Visible, 16);
        assert_mode!("r+ 32", RandMode::Visible, 32);
        assert_mode!("rand+16", RandMode::Visible, 16);
        assert_mode!("rand+ 8", RandMode::Visible, 8);
    }

    #[test]
    fn parse_rand_digits() {
        assert_mode!("rn16", RandMode::Digits, 16);
        assert_mode!("rn 8", RandMode::Digits, 8);
        assert_mode!("randn16", RandMode::Digits, 16);
    }

    #[test]
    fn parse_rand_lower() {
        assert_mode!("rc16", RandMode::Lower, 16);
        assert_mode!("randc16", RandMode::Lower, 16);
    }

    #[test]
    fn parse_rand_upper() {
        assert_mode!("rC16", RandMode::Upper, 16);
        assert_mode!("randC16", RandMode::Upper, 16);
        // 大小写只影响 C 的模式语义：rC → Upper，忽略剩余大小写
        assert_mode!("RC16", RandMode::Upper, 16);
    }

    #[test]
    fn parse_rand_invalid() {
        assert!(parse_rand_query("r").is_none());
        assert!(parse_rand_query("ra").is_none());
        assert!(parse_rand_query("rand").is_none());
        assert!(parse_rand_query("r0").is_none());
        assert!(parse_rand_query("r1000").is_none());
        assert!(parse_rand_query("xyz").is_none());
        assert!(parse_rand_query("ab").is_none());
    }

    #[test]
    fn generate_rand_correct_length() {
        assert_eq!(generate_rand(&RandMode::Digits, 10).len(), 10);
        assert_eq!(generate_rand(&RandMode::Lower, 20).len(), 20);
        assert_eq!(generate_rand(&RandMode::Upper, 5).len(), 5);
        assert_eq!(generate_rand(&RandMode::AlphaNum, 32).len(), 32);
        assert_eq!(generate_rand(&RandMode::Visible, 64).len(), 64);
    }

    #[test]
    fn generate_rand_chars_in_range() {
        let s = generate_rand(&RandMode::Digits, 100);
        assert!(s.chars().all(|c| c.is_ascii_digit()));

        let s = generate_rand(&RandMode::Lower, 100);
        assert!(s.chars().all(|c| c.is_ascii_lowercase()));

        let s = generate_rand(&RandMode::Upper, 100);
        assert!(s.chars().all(|c| c.is_ascii_uppercase()));

        let s = generate_rand(&RandMode::AlphaNum, 100);
        assert!(s.chars().all(|c| c.is_ascii_alphanumeric()));

        let s = generate_rand(&RandMode::Visible, 100);
        assert!(s.chars().all(|c| c.is_ascii_graphic() || c == ' '));
    }

    #[test]
    fn generate_rand_different_each_call() {
        let a = generate_rand(&RandMode::AlphaNum, 100);
        let b = generate_rand(&RandMode::AlphaNum, 100);
        assert_ne!(a, b);
    }

    #[test]
    fn rand_id_roundtrip() {
        let r = parse_rand_query("r16").unwrap();
        let matches = build_rand_matches(&r);
        let id = &matches[0].0;
        assert!(id.starts_with("rand:alphanum:16"));
        let suffix = id.strip_prefix("rand:").unwrap();
        let value = value_for_rand_id(suffix).unwrap();
        assert_eq!(value.len(), 16);
        assert!(value.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}

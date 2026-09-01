use std::collections::VecDeque;
use std::error::Error;
use std::io;

use devtools_core::{Context, WORKER_INTERFACE, WORKER_OBJECT_PATH, WORKER_SERVICE_NAME};
use zbus::blocking::{Connection, Proxy};

use crate::{clipboard, str_value, KMatch, CATEGORY, CATEGORY_RELEVANCE};

pub const OPEN_MATCH_ID: &str = "json:open";
const INLINE_MATCH_ID_PREFIX: &str = "json:inline:";
const QUERY_KEYWORD: &str = "json";
const MAX_INLINE_CONTEXTS: usize = 8;

/// KRunner 的 Run 调用只携带 match id，因此在一次查询会话内短暂保存直接输入的 JSON。
///
/// 缓存有严格条数上限，正文不会进入 match id，也不会写入磁盘；Teardown 时会主动清空。
#[derive(Debug, Default)]
pub struct InlineContextCache {
    next_id: u64,
    entries: VecDeque<(String, String)>,
}

pub fn handles_match_id(match_id: &str) -> bool {
    match_id == OPEN_MATCH_ID || match_id.starts_with(INLINE_MATCH_ID_PREFIX)
}

impl InlineContextCache {
    fn insert(&mut self, payload: String) -> String {
        let match_id = format!("{INLINE_MATCH_ID_PREFIX}{}", self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        self.entries.push_back((match_id.clone(), payload));

        if self.entries.len() > MAX_INLINE_CONTEXTS {
            self.entries.pop_front();
        }

        match_id
    }

    fn take(&mut self, match_id: &str) -> Option<String> {
        let index = self
            .entries
            .iter()
            .position(|(candidate, _)| candidate == match_id)?;
        self.entries.remove(index).map(|(_, payload)| payload)
    }

    /// 把已消费的正文放回缓存（OpenTool 失败时恢复，供重按 Enter 重试）。
    fn restore(&mut self, match_id: &str, payload: String) {
        if self
            .entries
            .iter()
            .any(|(candidate, _)| candidate == match_id)
        {
            return;
        }
        self.entries.push_back((match_id.to_owned(), payload));
        if self.entries.len() > MAX_INLINE_CONTEXTS {
            self.entries.pop_front();
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// 优先识别 KRunner 输入框中的结构化 JSON，再回落到 `j/json + 剪贴板`。
pub fn match_for_query(query: &str, cache: &mut InlineContextCache) -> Option<KMatch> {
    if let Some(item) = match_for_inline_query(query, cache) {
        return Some(item);
    }
    if !matches_query(query) {
        return None;
    }
    let clipboard = clipboard::read_text().ok()?;
    match_for_clipboard(query, &clipboard)
}

/// 根据 match 来源取得 JSON，并通过 D-Bus 交给 Worker。
pub fn open_workbench(
    match_id: &str,
    cache: &mut InlineContextCache,
) -> Result<(), Box<dyn Error>> {
    let from_inline = match_id.starts_with(INLINE_MATCH_ID_PREFIX);
    let payload = if match_id == OPEN_MATCH_ID {
        // 剪贴板入口在 Run 阶段重新读取，避免缓存用户剪贴板内容。
        clipboard::read_text()?
    } else if from_inline {
        cache
            .take(match_id)
            .ok_or_else(|| io::Error::other("JSON input context expired"))?
    } else {
        return Err(io::Error::other("unknown JSON match id").into());
    };
    if let Err(error) = dispatch_open_tool(&payload) {
        // OpenTool 失败（如 Worker 未部署）时把正文放回缓存：OpenTool 幂等，
        // 用户重按 Enter 即可重试，而不是让刚输入的 JSON 凭空丢失。
        if from_inline {
            cache.restore(match_id, payload);
        }
        return Err(error);
    }
    Ok(())
}

fn dispatch_open_tool(payload: &str) -> Result<(), Box<dyn Error>> {
    let context = Context::from_json_text(payload.to_owned())?;
    let connection = Connection::session()?;
    let proxy = Proxy::new(
        &connection,
        WORKER_SERVICE_NAME,
        WORKER_OBJECT_PATH,
        WORKER_INTERFACE,
    )?;
    proxy.call::<_, _, ()>("OpenTool", &("json", context.raw_text()))?;
    Ok(())
}

fn match_for_inline_query(query: &str, cache: &mut InlineContextCache) -> Option<KMatch> {
    let context = Context::from_json_text(query.to_owned()).ok()?;
    let is_structured = matches!(
        &context,
        Context::Json { value, .. } if value.is_object() || value.is_array()
    );
    if !is_structured {
        // 避免裸数字抢占时间戳、true/null 等普通查询；自动识别仅针对对象和数组。
        return None;
    }

    let match_id = cache.insert(context.raw_text().to_owned());
    Some(build_match(
        match_id,
        "打开 KRunner 输入的 JSON；内容只在本机内存处理",
    ))
}

fn match_for_clipboard(query: &str, clipboard: &str) -> Option<KMatch> {
    if !matches_query(query) || Context::from_json_text(clipboard).is_err() {
        return None;
    }

    Some(build_match(
        OPEN_MATCH_ID.to_owned(),
        "从剪贴板打开合法 JSON；内容只在本机处理",
    ))
}

fn build_match(match_id: String, subtext: &str) -> KMatch {
    let mut properties = std::collections::HashMap::new();
    properties.insert("subtext".to_owned(), str_value(subtext));
    properties.insert("category".to_owned(), str_value(CATEGORY));

    (
        match_id,
        "打开 JSON Workbench".to_owned(),
        "application-json".to_owned(),
        CATEGORY_RELEVANCE,
        1.0,
        properties,
    )
}

fn matches_query(query: &str) -> bool {
    let query = query.trim().to_ascii_lowercase();
    !query.is_empty() && QUERY_KEYWORD.starts_with(&query)
}

#[cfg(test)]
mod tests {
    use devtools_core::MAX_JSON_BYTES;

    use super::*;

    #[test]
    fn matches_json_keyword_prefix_case_insensitively() {
        assert!(matches_query("j"));
        assert!(matches_query(" JSON "));
        assert!(!matches_query(""));
        assert!(!matches_query("jsonpath"));
    }

    #[test]
    fn builds_match_only_for_valid_json_clipboard() {
        let item = match_for_clipboard("json", "{\"a\":1}").expect("应命中 JSON");

        assert_eq!(item.0, OPEN_MATCH_ID);
        assert_eq!(item.1, "打开 JSON Workbench");
        assert!(match_for_clipboard("json", "not json").is_none());
    }

    #[test]
    fn recognizes_structured_json_directly_from_query() {
        let mut cache = InlineContextCache::default();
        let object = match_for_inline_query("{\"name\":\"loveyu\"}", &mut cache)
            .expect("对象 JSON 应自动命中");
        let array = match_for_inline_query("[1,2,3]", &mut cache).expect("数组 JSON 应自动命中");

        assert!(object.0.starts_with(INLINE_MATCH_ID_PREFIX));
        assert!(array.0.starts_with(INLINE_MATCH_ID_PREFIX));
        assert!(!object.0.contains("loveyu"));
        assert_eq!(
            cache.take(&object.0).as_deref(),
            Some("{\"name\":\"loveyu\"}")
        );
        assert_eq!(cache.take(&array.0).as_deref(), Some("[1,2,3]"));
    }

    #[test]
    fn direct_json_detection_does_not_steal_scalar_queries() {
        let mut cache = InlineContextCache::default();

        assert!(match_for_inline_query("1722931200", &mut cache).is_none());
        assert!(match_for_inline_query("true", &mut cache).is_none());
        assert!(match_for_inline_query("null", &mut cache).is_none());
        assert!(match_for_inline_query("\"text\"", &mut cache).is_none());
    }

    #[test]
    fn inline_context_cache_is_bounded_and_single_use() {
        let mut cache = InlineContextCache::default();
        let first_id = cache.insert("{\"index\":0}".to_owned());
        let mut newest_id = String::new();
        for index in 1..=MAX_INLINE_CONTEXTS {
            newest_id = cache.insert(format!("{{\"index\":{index}}}"));
        }

        assert!(cache.take(&first_id).is_none());
        assert!(cache.take(&newest_id).is_some());
        assert!(cache.take(&newest_id).is_none());
    }

    #[test]
    fn rejects_oversized_json_clipboard() {
        let clipboard = format!("\"{}\"", "x".repeat(MAX_JSON_BYTES));

        assert!(match_for_clipboard("json", &clipboard).is_none());
    }

    #[test]
    fn restore_makes_failed_open_retryable() {
        let mut cache = InlineContextCache::default();
        let match_id = cache.insert("{\"a\":1}".to_owned());
        let payload = cache.take(&match_id).expect("应可消费");

        // OpenTool 失败回插后，同一 match id 可再次消费，重试不被「已过期」挡住。
        cache.restore(&match_id, payload);
        assert_eq!(cache.take(&match_id).as_deref(), Some("{\"a\":1}"));
        // 重复回插不产生重复条目（后续 take 只有一次命中）。
        cache.restore(&match_id, "{\"a\":1}".to_owned());
        cache.restore(&match_id, "{\"a\":1}".to_owned());
        let duplicates = cache
            .entries
            .iter()
            .filter(|(candidate, _)| candidate == &match_id)
            .count();
        assert_eq!(duplicates, 1);
    }
}

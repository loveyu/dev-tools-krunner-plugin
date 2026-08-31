use std::error::Error;

use devtools_core::{
    Context, MAX_TEXT_BYTES, WORKER_INTERFACE, WORKER_OBJECT_PATH, WORKER_SERVICE_NAME,
};
use zbus::blocking::{Connection, Proxy};

use crate::{clipboard, str_value, KMatch, CATEGORY, CATEGORY_RELEVANCE};

pub const OPEN_MATCH_ID: &str = "convert:open";

/// `convert` / `cv` 命中且剪贴板存在可处理文本时，返回转换工作台入口。
pub fn match_for_query(query: &str) -> Option<KMatch> {
    if !matches_query(query) {
        return None;
    }
    let clipboard = clipboard::read_text().ok()?;
    match_for_clipboard(query, &clipboard)
}

/// Run 阶段重新读取剪贴板并打开转换页。
pub fn open_workbench() -> Result<(), Box<dyn Error>> {
    let clipboard = clipboard::read_text()?;
    let context = Context::from_text(clipboard)?;
    let connection = Connection::session()?;
    let proxy = Proxy::new(
        &connection,
        WORKER_SERVICE_NAME,
        WORKER_OBJECT_PATH,
        WORKER_INTERFACE,
    )?;
    proxy.call::<_, _, ()>("OpenTool", &("convert", context.raw_text()))?;
    Ok(())
}

fn match_for_clipboard(query: &str, clipboard: &str) -> Option<KMatch> {
    if !matches_query(query) || clipboard.trim().is_empty() || clipboard.len() > MAX_TEXT_BYTES {
        return None;
    }

    let mut properties = std::collections::HashMap::new();
    properties.insert(
        "subtext".to_owned(),
        str_value("从剪贴板打开数据转换；内容只在本机处理"),
    );
    properties.insert("category".to_owned(), str_value(CATEGORY));

    Some((
        OPEN_MATCH_ID.to_owned(),
        "打开数据转换".to_owned(),
        "transform-move".to_owned(),
        CATEGORY_RELEVANCE,
        1.0,
        properties,
    ))
}

fn matches_query(query: &str) -> bool {
    let query = query.trim().to_ascii_lowercase();
    query == "cv" || (query.len() >= 2 && "convert".starts_with(&query))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_convert_keyword_and_short_alias() {
        assert!(matches_query("co"));
        assert!(matches_query("CONVERT"));
        assert!(matches_query("cv"));
        assert!(!matches_query("c"));
        assert!(!matches_query("converter"));
    }

    #[test]
    fn requires_non_empty_bounded_clipboard_text() {
        let item = match_for_clipboard("convert", "key=value").expect("文本应命中转换工具");

        assert_eq!(item.0, OPEN_MATCH_ID);
        assert_eq!(item.1, "打开数据转换");
        assert!(match_for_clipboard("convert", "  \n").is_none());
        assert!(match_for_clipboard("convert", &"x".repeat(MAX_TEXT_BYTES + 1)).is_none());
    }
}

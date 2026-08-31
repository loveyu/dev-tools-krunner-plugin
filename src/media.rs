use std::error::Error;

use devtools_core::{WORKER_INTERFACE, WORKER_OBJECT_PATH, WORKER_SERVICE_NAME};
use zbus::blocking::{Connection, Proxy};

use crate::{str_value, KMatch, CATEGORY, CATEGORY_RELEVANCE};

pub const OCR_MATCH_ID: &str = "ocr:open";
pub const BARCODE_MATCH_ID: &str = "barcode:open";
pub const IMAGE_COMPRESS_MATCH_ID: &str = "image-compress:open";
pub const IMAGE_EDITOR_MATCH_ID: &str = "image-editor:open";
pub const WATERMARK_MATCH_ID: &str = "watermark:open";

/// 根据 OCR、条形码、二维码、图片压缩、图片编辑和图片水印触发词构造页面入口。
pub fn match_for_query(query: &str) -> Option<KMatch> {
    let tool = tool_for_query(query)?;
    let (id, title, icon, subtext) = match tool {
        "ocr" => (
            OCR_MATCH_ID,
            "打开 OCR 文字识别",
            "insert-text",
            "粘贴或选择图片，使用本机 Tesseract 识别文字",
        ),
        "barcode" => (
            BARCODE_MATCH_ID,
            "打开条码与二维码工具",
            "view-barcode",
            "识别或生成条形码与二维码",
        ),
        "image-compress" => (
            IMAGE_COMPRESS_MATCH_ID,
            "打开图片压缩工具",
            "image-x-generic",
            "在 WebView 内压缩、对比并导出图片",
        ),
        "image-editor" => (
            IMAGE_EDITOR_MATCH_ID,
            "打开图片编辑器",
            "image-x-generic",
            "使用 TOAST UI Image Editor 编辑、复制和导出图片",
        ),
        "watermark" => (
            WATERMARK_MATCH_ID,
            "打开图片水印工具",
            "image-x-generic",
            "在 WebView 内添加平铺文字或图片水印",
        ),
        _ => return None,
    };
    let mut properties = std::collections::HashMap::new();
    properties.insert("subtext".to_owned(), str_value(subtext));
    properties.insert("category".to_owned(), str_value(CATEGORY));

    Some((
        id.to_owned(),
        title.to_owned(),
        icon.to_owned(),
        CATEGORY_RELEVANCE,
        1.0,
        properties,
    ))
}

pub fn handles_match_id(match_id: &str) -> bool {
    matches!(
        match_id,
        OCR_MATCH_ID
            | BARCODE_MATCH_ID
            | IMAGE_COMPRESS_MATCH_ID
            | IMAGE_EDITOR_MATCH_ID
            | WATERMARK_MATCH_ID
    )
}

pub fn open_tool(match_id: &str) -> Result<(), Box<dyn Error>> {
    let tool = match match_id {
        OCR_MATCH_ID => "ocr",
        BARCODE_MATCH_ID => "barcode",
        IMAGE_COMPRESS_MATCH_ID => "image-compress",
        IMAGE_EDITOR_MATCH_ID => "image-editor",
        WATERMARK_MATCH_ID => "watermark",
        _ => return Err(format!("unknown media match id: {match_id}").into()),
    };
    let connection = Connection::session()?;
    let proxy = Proxy::new(
        &connection,
        WORKER_SERVICE_NAME,
        WORKER_OBJECT_PATH,
        WORKER_INTERFACE,
    )?;
    proxy.call::<_, _, ()>("OpenTool", &(tool, ""))?;
    Ok(())
}

fn tool_for_query(query: &str) -> Option<&'static str> {
    let query = query.trim().to_ascii_lowercase();
    if query == "ocr" || (query.len() >= 2 && "ocr".starts_with(&query)) {
        return Some("ocr");
    }
    if matches!(query.as_str(), "bar" | "qr")
        || (query.len() >= 2 && ("barcode".starts_with(&query) || "qrcode".starts_with(&query)))
    {
        return Some("barcode");
    }
    if query == "compress"
        || query == "squoosh"
        || (query.len() >= 3
            && ("image-compress".starts_with(&query)
                || "compress-image".starts_with(&query)
                || "imgcompress".starts_with(&query)))
    {
        return Some("image-compress");
    }
    if query == "editor"
        || (query.len() >= 3
            && ("image-editor".starts_with(&query)
                || "edit-image".starts_with(&query)
                || "imageedit".starts_with(&query)
                || "imgedit".starts_with(&query)))
    {
        return Some("image-editor");
    }
    if matches!(query.as_str(), "watermark" | "wm")
        || (query.len() >= 3
            && ("watermark".starts_with(&query) || "image-watermark".starts_with(&query)))
    {
        return Some("watermark");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_ocr_prefix() {
        assert_eq!(
            match_for_query("oc").map(|item| item.0),
            Some(OCR_MATCH_ID.to_owned())
        );
        assert_eq!(
            match_for_query("OCR").map(|item| item.0),
            Some(OCR_MATCH_ID.to_owned())
        );
        assert!(match_for_query("o").is_none());
    }

    #[test]
    fn matches_barcode_and_qr_aliases() {
        for query in ["bar", "barc", "barcode", "qr", "qrc", "qrcode"] {
            assert_eq!(
                match_for_query(query).map(|item| item.0),
                Some(BARCODE_MATCH_ID.to_owned())
            );
        }
        assert!(match_for_query("barcode-reader").is_none());
    }

    #[test]
    fn recognizes_only_media_match_ids() {
        assert!(handles_match_id(OCR_MATCH_ID));
        assert!(handles_match_id(BARCODE_MATCH_ID));
        assert!(handles_match_id(IMAGE_COMPRESS_MATCH_ID));
        assert!(handles_match_id(IMAGE_EDITOR_MATCH_ID));
        assert!(handles_match_id(WATERMARK_MATCH_ID));
        assert!(!handles_match_id("json:open"));
    }

    #[test]
    fn matches_image_compression_aliases() {
        for query in [
            "compress",
            "squoosh",
            "ima",
            "image-compress",
            "com",
            "compress-image",
            "img",
            "imgcompress",
        ] {
            assert_eq!(
                match_for_query(query).map(|item| item.0),
                Some(IMAGE_COMPRESS_MATCH_ID.to_owned())
            );
        }
        assert!(match_for_query("im").is_none());
        assert!(match_for_query("compressor").is_none());
    }

    #[test]
    fn matches_image_editor_aliases() {
        for query in [
            "editor",
            "image-editor",
            "edi",
            "edit-image",
            "imageedit",
            "imge",
            "imgedit",
        ] {
            assert_eq!(
                match_for_query(query).map(|item| item.0),
                Some(IMAGE_EDITOR_MATCH_ID.to_owned())
            );
        }
        assert!(match_for_query("ed").is_none());
        assert!(match_for_query("photo-editor").is_none());
    }

    #[test]
    fn matches_image_watermark_aliases() {
        for query in ["wm", "wat", "watermark", "image-watermark"] {
            assert_eq!(
                match_for_query(query).map(|item| item.0),
                Some(WATERMARK_MATCH_ID.to_owned())
            );
        }
        assert!(match_for_query("wa").is_none());
        assert!(match_for_query("watermarked").is_none());
    }
}

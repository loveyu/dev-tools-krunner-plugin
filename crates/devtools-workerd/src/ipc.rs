use devtools_core::Settings;
use serde::Deserialize;

use crate::application::UserEvent;
use crate::media_processor::{MediaOperation, MediaOptions};
use crate::native_converter::{ConversionDirection, NativeFormat};
use crate::registry::ToolRegistry;

/// WebView 发送给应用层的稳定 JSON 协议。
#[derive(Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum WebRequest {
    FrontendReady,
    ClipboardWrite {
        text: String,
    },
    OpenExternal {
        url: String,
    },
    NativeConvert {
        request_id: String,
        format: NativeFormat,
        direction: ConversionDirection,
        payload: String,
    },
    MediaProcess {
        request_id: String,
        operation: MediaOperation,
        image_base64: String,
        mime_type: String,
        #[serde(default)]
        options: MediaOptions,
    },
    MetadataPick {
        request_id: String,
    },
    MetadataImage {
        request_id: String,
        image_base64: String,
        mime_type: String,
    },
    ColorPick {
        request_id: String,
    },
    SettingsGet,
    WindowHide,
    SettingsUpdate {
        settings: Settings,
    },
}

pub fn parse_web_request(payload: &str) -> Result<WebRequest, serde_json::Error> {
    serde_json::from_str(payload)
}

/// 将平台 IPC 的工具请求转换为应用事件，平台层不感知业务页面。
pub fn tool_event(registry: &ToolRegistry, tool: &str, payload: &str) -> Result<UserEvent, String> {
    let result = registry
        .execute(tool, payload)
        .map_err(|error| error.to_string())?;
    match tool {
        "json" => Ok(UserEvent::OpenJson(result.payload)),
        "convert" => Ok(UserEvent::OpenConvert(result.payload)),
        "ocr" => Ok(UserEvent::OpenOcr),
        "barcode" => Ok(UserEvent::OpenBarcode),
        "image-compress" => Ok(UserEvent::OpenImageCompress),
        "image-editor" => Ok(UserEvent::OpenImageEditor),
        "watermark" => Ok(UserEvent::OpenWatermark),
        "crypto" => Ok(UserEvent::OpenCrypto),
        "metadata" => Ok(UserEvent::OpenMetadata),
        "color" => Ok(UserEvent::OpenColor),
        _ => Err(format!("unsupported tool: {tool}")),
    }
}

#[cfg(test)]
mod tests {
    use devtools_core::{LanguageMode, ThemeMode};

    use super::*;

    #[test]
    fn parses_webview_settings_request() {
        let request = parse_web_request(
            r#"{"type":"settingsUpdate","settings":{"showTray":false,"autostart":true,"theme":"dark","language":"zh-TW"}}"#,
        )
        .expect("设置请求应可解析");

        assert!(matches!(
            request,
            WebRequest::SettingsUpdate {
                settings: Settings {
                    show_tray: false,
                    autostart: true,
                    theme: ThemeMode::Dark,
                    language: LanguageMode::TraditionalChinese,
                    ..
                }
            }
        ));
        assert!(matches!(
            parse_web_request(
                r#"{"type":"metadataImage","requestId":"meta-2","imageBase64":"AA==","mimeType":"image/png"}"#
            ),
            Ok(WebRequest::MetadataImage { request_id, mime_type, .. })
                if request_id == "meta-2" && mime_type == "image/png"
        ));
    }

    #[test]
    fn parses_open_external_request() {
        assert!(matches!(
            parse_web_request(r#"{"type":"openExternal","url":"https://github.com/TransparentLC/watermarker"}"#),
            Ok(WebRequest::OpenExternal { url }) if url == "https://github.com/TransparentLC/watermarker"
        ));
        assert!(parse_web_request(r#"{"type":"openExternal","url":"file:///etc/passwd"}"#).is_ok());
    }

    #[test]
    fn parses_media_processing_request() {
        let request = parse_web_request(
            r#"{"type":"mediaProcess","requestId":"m-1","operation":"ocr","imageBase64":"aGVsbG8=","mimeType":"image/png","options":{"language":"eng","pageSegmentationMode":6,"minimumConfidence":50}}"#,
        )
        .expect("媒体处理请求应可解析");

        assert!(matches!(
            request,
            WebRequest::MediaProcess {
                operation: MediaOperation::Ocr,
                ..
            }
        ));
    }

    #[test]
    fn parses_native_conversion_camel_case_fields() {
        let request = parse_web_request(
            r#"{"type":"nativeConvert","requestId":"n-1","format":"php-array","direction":"stringify","payload":"[]"}"#,
        )
        .expect("native 转换请求应可解析");

        assert!(matches!(
            request,
            WebRequest::NativeConvert {
                request_id,
                format: NativeFormat::Array,
                direction: ConversionDirection::Stringify,
                payload
            } if request_id == "n-1" && payload == "[]"
        ));
    }

    #[test]
    fn maps_every_registered_tool_to_an_application_event() {
        let registry = ToolRegistry::standard();
        assert!(matches!(
            tool_event(&registry, "json", "{\"ok\":true}"),
            Ok(UserEvent::OpenJson(_))
        ));
        assert!(matches!(
            tool_event(&registry, "convert", "a=1"),
            Ok(UserEvent::OpenConvert(_))
        ));
        assert!(matches!(
            tool_event(&registry, "ocr", ""),
            Ok(UserEvent::OpenOcr)
        ));
        assert!(matches!(
            tool_event(&registry, "barcode", ""),
            Ok(UserEvent::OpenBarcode)
        ));
        assert!(matches!(
            tool_event(&registry, "image-compress", ""),
            Ok(UserEvent::OpenImageCompress)
        ));
        assert!(matches!(
            tool_event(&registry, "image-editor", ""),
            Ok(UserEvent::OpenImageEditor)
        ));
        assert!(matches!(
            tool_event(&registry, "watermark", ""),
            Ok(UserEvent::OpenWatermark)
        ));
        assert!(matches!(
            tool_event(&registry, "crypto", ""),
            Ok(UserEvent::OpenCrypto)
        ));
        assert!(matches!(
            tool_event(&registry, "metadata", ""),
            Ok(UserEvent::OpenMetadata)
        ));
        assert!(matches!(
            tool_event(&registry, "color", ""),
            Ok(UserEvent::OpenColor)
        ));
    }

    #[test]
    fn parses_metadata_and_color_picker_requests() {
        assert!(matches!(
            parse_web_request(r#"{"type":"metadataPick","requestId":"meta-1"}"#),
            Ok(WebRequest::MetadataPick { request_id }) if request_id == "meta-1"
        ));
        assert!(matches!(
            parse_web_request(r#"{"type":"colorPick","requestId":"color-1"}"#),
            Ok(WebRequest::ColorPick { request_id }) if request_id == "color-1"
        ));
    }
}

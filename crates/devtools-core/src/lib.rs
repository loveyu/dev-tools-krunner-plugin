//! DevTools 各进程共享的模型与协议常量。

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// GUI Worker 在 session bus 上注册的服务名。
pub const WORKER_SERVICE_NAME: &str = "org.loveyu.DevTools";
/// GUI Worker 暴露对象的路径。
pub const WORKER_OBJECT_PATH: &str = "/org/loveyu/DevTools";
/// GUI Worker 的 D-Bus 接口名。
pub const WORKER_INTERFACE: &str = "org.loveyu.DevTools";
/// Runner 与 Worker 允许接收的最大文本，避免剪贴板意外拖垮桌面会话。
pub const MAX_TEXT_BYTES: usize = 2 * 1024 * 1024;
/// JSON 沿用统一文本上限。
pub const MAX_JSON_BYTES: usize = MAX_TEXT_BYTES;

/// 当前请求携带的上下文。
#[derive(Debug, Clone, PartialEq)]
pub enum Context {
    Empty,
    Json { raw: String, value: Value },
    Text { raw: String },
}

impl Context {
    /// 把剪贴板文本识别为 JSON 上下文，并应用统一的输入大小限制。
    pub fn from_json_text(raw: impl Into<String>) -> Result<Self, ContextError> {
        let raw = raw.into();
        validate_size(&raw)?;

        let value = serde_json::from_str(&raw).map_err(ContextError::InvalidJson)?;
        Ok(Self::Json { raw, value })
    }

    /// 构造普通文本上下文，并应用统一输入大小限制。
    pub fn from_text(raw: impl Into<String>) -> Result<Self, ContextError> {
        let raw = raw.into();
        validate_size(&raw)?;
        Ok(Self::Text { raw })
    }

    /// 返回上下文的原始文本。
    pub fn raw_text(&self) -> &str {
        match self {
            Self::Empty => "",
            Self::Json { raw, .. } | Self::Text { raw } => raw,
        }
    }
}

fn validate_size(raw: &str) -> Result<(), ContextError> {
    if raw.len() > MAX_TEXT_BYTES {
        return Err(ContextError::TooLarge {
            actual: raw.len(),
            maximum: MAX_TEXT_BYTES,
        });
    }
    Ok(())
}

/// 用户希望对上下文执行的操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Inspect,
    Convert,
    RecognizeText,
    ProcessBarcode,
    CompressImage,
    EditImage,
    WatermarkImage,
}

/// Worker 交给具体工具执行的请求。
#[derive(Debug, Clone, PartialEq)]
pub struct ToolRequest {
    pub context: Context,
    pub action: Action,
}

/// 工具完成准备后交给 UI 的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResult {
    pub payload: String,
}

/// 业务工具的最小抽象，避免 Worker 与具体 JSON 逻辑直接耦合。
pub trait Tool: Send + Sync {
    fn id(&self) -> &'static str;
    fn can_handle(&self, context: &Context, action: Action) -> bool;
    fn execute(&self, request: ToolRequest) -> Result<ToolResult, ToolError>;
}

/// 工具无法处理请求时返回的稳定错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolError {
    message: String,
}

impl ToolError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for ToolError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ToolError {}

/// 上下文识别失败的原因。
#[derive(Debug)]
pub enum ContextError {
    TooLarge { actual: usize, maximum: usize },
    InvalidJson(serde_json::Error),
}

impl Display for ContextError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { actual, maximum } => {
                write!(
                    formatter,
                    "JSON is too large: {actual} bytes (maximum {maximum})"
                )
            }
            Self::InvalidJson(error) => write!(formatter, "invalid JSON: {error}"),
        }
    }
}

impl Error for ContextError {}

/// Worker 设置页读写的持久化配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub show_tray: bool,
    pub autostart: bool,
    #[serde(default)]
    pub global_shortcut_enabled: bool,
    #[serde(default = "default_global_shortcut")]
    pub global_shortcut: String,
    #[serde(default)]
    pub quick_input_enabled: bool,
    #[serde(default = "default_quick_input_shortcut")]
    pub quick_input_shortcut: String,
    #[serde(default = "default_quick_input_width")]
    pub quick_input_width: u32,
    #[serde(default = "default_quick_input_height")]
    pub quick_input_height: u32,
    #[serde(default)]
    pub theme: ThemeMode,
    #[serde(default)]
    pub language: LanguageMode,
}

/// WebView 的外观主题；默认跟随 KDE/系统配色。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

/// WebView 显示语言；自动模式由前端根据浏览器/桌面语言解析。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LanguageMode {
    #[default]
    #[serde(rename = "system")]
    System,
    #[serde(rename = "zh-CN")]
    SimplifiedChinese,
    #[serde(rename = "zh-TW")]
    TraditionalChinese,
    #[serde(rename = "en-US")]
    English,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            show_tray: true,
            autostart: false,
            global_shortcut_enabled: false,
            global_shortcut: default_global_shortcut(),
            quick_input_enabled: false,
            quick_input_shortcut: default_quick_input_shortcut(),
            quick_input_width: default_quick_input_width(),
            quick_input_height: default_quick_input_height(),
            theme: ThemeMode::System,
            language: LanguageMode::System,
        }
    }
}

fn default_global_shortcut() -> String {
    "Ctrl+Alt+Space".to_owned()
}

fn default_quick_input_shortcut() -> String {
    "Ctrl+Alt+KeyI".to_owned()
}

fn default_quick_input_width() -> u32 {
    560
}

fn default_quick_input_height() -> u32 {
    56
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_context_keeps_raw_text_and_parsed_value() {
        let context = Context::from_json_text("{\n  \"a\": 1\n}").expect("JSON 应可解析");

        assert_eq!(context.raw_text(), "{\n  \"a\": 1\n}");
        let Context::Json { value, .. } = context else {
            panic!("应构造 JSON 上下文");
        };
        assert_eq!(value["a"], 1);
    }

    #[test]
    fn text_context_keeps_raw_text() {
        let context = Context::from_text("a=1&b=2").expect("普通文本应可构造");

        assert_eq!(
            context,
            Context::Text {
                raw: "a=1&b=2".to_owned()
            }
        );
        assert_eq!(context.raw_text(), "a=1&b=2");
    }

    #[test]
    fn empty_context_has_empty_raw_text() {
        assert_eq!(Context::Empty.raw_text(), "");
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(matches!(
            Context::from_json_text("{invalid}"),
            Err(ContextError::InvalidJson(_))
        ));
    }

    #[test]
    fn rejects_json_over_size_limit() {
        let raw = format!("\"{}\"", "x".repeat(MAX_JSON_BYTES));

        assert!(matches!(
            Context::from_json_text(raw),
            Err(ContextError::TooLarge { .. })
        ));
    }

    #[test]
    fn settings_without_theme_and_language_keep_system_defaults() {
        let settings: Settings = serde_json::from_str(r#"{"showTray":false,"autostart":true}"#)
            .expect("旧版设置应继续可读取");

        assert_eq!(settings.theme, ThemeMode::System);
        assert_eq!(settings.language, LanguageMode::System);
        assert!(!settings.global_shortcut_enabled);
        assert_eq!(settings.global_shortcut, "Ctrl+Alt+Space");
        assert!(!settings.quick_input_enabled);
        assert_eq!(settings.quick_input_shortcut, "Ctrl+Alt+KeyI");
        assert_eq!(settings.quick_input_width, 560);
        assert_eq!(settings.quick_input_height, 56);
    }
}

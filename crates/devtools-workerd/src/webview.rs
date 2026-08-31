use devtools_core::Settings;
use gtk::gdk;
use serde::Serialize;
use tao::event_loop::{EventLoop, EventLoopProxy};
use tao::platform::unix::WindowExtUnix;
use tao::window::{Window, WindowBuilder};
use wry::{WebView, WebViewBuilder, WebViewBuilderExtUnix};

use crate::media_processor::{MediaCapabilities, MediaProcessingResult};
use crate::native_converter::{ConverterCapabilities, NativeConversionResult};
use crate::{UserEvent, WebRequest};

const WEB_APP: &str = include_str!("../../../web/devtools-ui/dist/index.html");

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InitialState {
    version: &'static str,
    settings: Settings,
    converter_capabilities: ConverterCapabilities,
    media_capabilities: MediaCapabilities,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonDetail<'a> {
    payload: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsDetail<'a> {
    settings: Settings,
    error: Option<&'a str>,
}

/// 单窗口 WebView 管理器。JSON 工作台与设置页复用同一个前端实例。
pub struct WorkspaceWindow {
    window: Window,
    webview: WebView,
}

impl WorkspaceWindow {
    pub fn new(
        event_loop: &EventLoop<UserEvent>,
        proxy: EventLoopProxy<UserEvent>,
        settings: Settings,
        converter_capabilities: ConverterCapabilities,
        media_capabilities: MediaCapabilities,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let window = WindowBuilder::new()
            .with_title("DevTools")
            .with_inner_size(tao::dpi::LogicalSize::new(1180.0, 760.0))
            .with_min_inner_size(tao::dpi::LogicalSize::new(720.0, 520.0))
            .with_visible(false)
            .build(event_loop)?;

        let initial_state = InitialState {
            version: env!("CARGO_PKG_VERSION"),
            settings,
            converter_capabilities,
            media_capabilities,
        };
        let initial_state =
            serde_json::to_string(&initial_state).expect("InitialState 只含可序列化字段");
        let initialization_script = format!(
            "window.__DEVTOOLS_INITIAL_STATE__ = {initial_state};\n\
             window.__DEVTOOLS_PENDING_EVENTS__ = [];"
        );
        let web_proxy = proxy;
        let builder = WebViewBuilder::new()
            .with_html(WEB_APP)
            .with_clipboard(false)
            .with_initialization_script(&initialization_script)
            .with_ipc_handler(move |request| {
                let _ = web_proxy.send_event(UserEvent::WebMessage(request.body().to_owned()));
            });
        let container = window
            .default_vbox()
            .ok_or("Tao GTK window does not expose a default container")?;
        let webview = builder.build_gtk(container)?;

        Ok(Self { window, webview })
    }

    pub fn id(&self) -> tao::window::WindowId {
        self.window.id()
    }

    pub fn open_json(&self, payload: &str) {
        self.dispatch("devtools:open-json", &JsonDetail { payload });
        self.show();
    }

    pub fn open_convert(&self, payload: &str) {
        self.dispatch("devtools:open-convert", &JsonDetail { payload });
        self.show();
    }

    pub fn open_ocr(&self) {
        self.dispatch("devtools:open-ocr", &());
        self.show();
    }

    pub fn open_barcode(&self) {
        self.dispatch("devtools:open-barcode", &());
        self.show();
    }

    pub fn open_settings(&self, settings: Settings) {
        self.send_settings(settings, None);
        self.dispatch("devtools:open-settings", &());
        self.show();
    }

    pub fn send_settings(&self, settings: Settings, error: Option<&str>) {
        self.dispatch("devtools:settings", &SettingsDetail { settings, error });
    }

    pub fn send_native_conversion_result(&self, result: &NativeConversionResult) {
        self.dispatch("devtools:native-convert-result", result);
    }

    pub fn send_media_processing_result(&self, result: &MediaProcessingResult) {
        self.dispatch("devtools:media-process-result", result);
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    pub fn copy_to_clipboard(&self, text: &str) {
        let clipboard = gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD);
        clipboard.set_text(text);
        clipboard.store();
    }

    fn show(&self) {
        self.window.set_visible(true);
        self.window.set_focus();
        if let Err(error) = self.webview.focus() {
            eprintln!("devtools-workerd: failed to focus webview: {error}");
        }
    }

    fn dispatch(&self, event_name: &str, detail: &impl Serialize) {
        let event_name = serde_json::to_string(event_name).expect("事件名应可序列化");
        let detail = serde_json::to_string(detail).expect("事件数据应可序列化");
        let script = format!(
            "window.__DEVTOOLS_DISPATCH__\n\
             ? window.__DEVTOOLS_DISPATCH__({event_name}, {detail})\n\
             : window.__DEVTOOLS_PENDING_EVENTS__.push({{ name: {event_name}, detail: {detail} }});"
        );
        if let Err(error) = self.webview.evaluate_script(&script) {
            eprintln!("devtools-workerd: failed to dispatch web event: {error}");
        }
    }
}

/// 解析来自 WebView 的 JSON IPC，请求类型受 serde 标签约束。
pub fn parse_web_request(payload: &str) -> Result<WebRequest, serde_json::Error> {
    serde_json::from_str(payload)
}

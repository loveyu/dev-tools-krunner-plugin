use devtools_core::Settings;
use serde::Serialize;
use tao::event_loop::{EventLoop, EventLoopProxy};
use tao::window::{Window, WindowBuilder};
use wry::{WebView, WebViewBuilder};

use crate::media_processor::{MediaCapabilities, MediaProcessingResult};
use crate::metadata_processor::{MetadataCapabilities, MetadataProcessingResult};
use crate::native_converter::{ConverterCapabilities, NativeConversionResult};
use crate::platform;
use crate::UserEvent;

const WEB_APP: &str = include_str!("../../../web/devtools-ui/dist/index.html");

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InitialState {
    version: &'static str,
    settings: Settings,
    converter_capabilities: ConverterCapabilities,
    media_capabilities: MediaCapabilities,
    metadata_capabilities: MetadataCapabilities,
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

/// WebView 管理器。JSON 工作台与设置页复用同一个前端实例。
pub struct WebViewManager {
    window: Window,
    webview: WebView,
}

impl WebViewManager {
    pub fn new(
        event_loop: &EventLoop<UserEvent>,
        proxy: EventLoopProxy<UserEvent>,
        settings: Settings,
        converter_capabilities: ConverterCapabilities,
        media_capabilities: MediaCapabilities,
        metadata_capabilities: MetadataCapabilities,
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
            metadata_capabilities,
        };
        let initial_state =
            serde_json::to_string(&initial_state).expect("InitialState 只含可序列化字段");
        let initialization_script = format!(
            "window.__DEVTOOLS_INITIAL_STATE__ = {initial_state};\n\
             window.__DEVTOOLS_PENDING_EVENTS__ = [];"
        );
        let web_proxy = proxy;
        let builder = platform::configure_webview(WebViewBuilder::new(), WEB_APP);
        let builder = builder
            .with_clipboard(true)
            .with_initialization_script(&initialization_script)
            .with_ipc_handler(move |request| {
                let _ = web_proxy.send_event(UserEvent::WebMessage(request.body().to_owned()));
            });
        let webview = platform::build_webview(builder, &window)?;

        Ok(Self { window, webview })
    }

    pub fn id(&self) -> tao::window::WindowId {
        self.window.id()
    }

    pub fn open_json(&self, payload: &str) {
        self.use_workspace_size();
        self.dispatch("devtools:open-json", &JsonDetail { payload });
        self.show();
    }

    pub fn open_convert(&self, payload: &str) {
        self.use_workspace_size();
        self.dispatch("devtools:open-convert", &JsonDetail { payload });
        self.show();
    }

    pub fn open_ocr(&self) {
        self.use_workspace_size();
        self.dispatch("devtools:open-ocr", &());
        self.show();
    }

    pub fn open_barcode(&self) {
        self.use_workspace_size();
        self.dispatch("devtools:open-barcode", &());
        self.show();
    }

    pub fn open_image_compress(&self) {
        self.use_workspace_size();
        self.dispatch("devtools:open-image-compress", &());
        self.show();
    }

    pub fn open_image_editor(&self) {
        self.use_workspace_size();
        self.dispatch("devtools:open-image-editor", &());
        self.show();
    }

    pub fn open_watermark(&self) {
        self.use_workspace_size();
        self.dispatch("devtools:open-watermark", &());
        self.show();
    }

    pub fn open_crypto(&self) {
        self.use_workspace_size();
        self.dispatch("devtools:open-crypto", &());
        self.show();
    }

    pub fn open_metadata(&self) {
        self.use_workspace_size();
        self.dispatch("devtools:open-metadata", &());
        self.show();
    }

    pub fn open_color(&self) {
        self.use_workspace_size();
        self.dispatch("devtools:open-color", &());
        self.show();
    }

    pub fn open_settings(&self, settings: Settings) {
        self.use_workspace_size();
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

    pub fn send_metadata_processing_result(&self, result: &MetadataProcessingResult) {
        self.dispatch("devtools:metadata-process-result", result);
    }

    pub fn send_color_pick_result(&self, result: &crate::ColorPickResult) {
        self.dispatch("devtools:color-pick-result", result);
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    pub fn restore(&self) {
        self.show();
    }

    pub fn copy_to_clipboard(&self, text: &str) {
        platform::copy_text(text);
    }

    pub fn open_launcher(&self) {
        self.window
            .set_inner_size(tao::dpi::LogicalSize::new(760.0, 480.0));
        self.dispatch("devtools:open-launcher", &());
        self.show();
    }

    fn show(&self) {
        self.window.set_visible(true);
        self.window.set_focus();
        if let Err(error) = self.webview.focus() {
            eprintln!("devtools-workerd: failed to focus webview: {error}");
        }
    }

    fn use_workspace_size(&self) {
        self.window
            .set_inner_size(tao::dpi::LogicalSize::new(1180.0, 760.0));
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

use devtools_core::Settings;
use tao::event_loop::{EventLoop, EventLoopProxy};

use crate::media_processor::{MediaCapabilities, MediaProcessingResult};
use crate::native_converter::{ConverterCapabilities, NativeConversionResult};
use crate::platform::{QuickInputInjector, QuickInputWindow};
use crate::webview_manager::WebViewManager;
use crate::UserEvent;

/// 统一协调 WebView 工作区与不使用 WebView 的原生快速输入窗口。
pub struct WindowManager {
    webview: WebViewManager,
    quick_input: QuickInputWindow,
    quick_input_injector: QuickInputInjector,
}

impl WindowManager {
    pub fn new(
        event_loop: &EventLoop<UserEvent>,
        proxy: EventLoopProxy<UserEvent>,
        settings: Settings,
        converter_capabilities: ConverterCapabilities,
        media_capabilities: MediaCapabilities,
        quick_input_history: Vec<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            webview: WebViewManager::new(
                event_loop,
                proxy.clone(),
                settings,
                converter_capabilities,
                media_capabilities,
            )?,
            quick_input: QuickInputWindow::new(proxy, quick_input_history)?,
            quick_input_injector: QuickInputInjector::new(),
        })
    }

    pub fn id(&self) -> tao::window::WindowId {
        self.webview.id()
    }

    pub fn open_launcher(&self) {
        self.webview.open_launcher();
    }

    pub fn open_quick_input(&self, settings: &Settings) {
        self.quick_input.show(settings);
    }

    pub fn submit_quick_input(
        &self,
        text: &str,
        target_window: Option<&str>,
    ) -> Result<(), String> {
        self.quick_input_injector.inject(text, target_window)
    }

    pub fn open_json(&self, payload: &str) {
        self.webview.open_json(payload);
    }

    pub fn open_convert(&self, payload: &str) {
        self.webview.open_convert(payload);
    }

    pub fn open_ocr(&self) {
        self.webview.open_ocr();
    }

    pub fn open_barcode(&self) {
        self.webview.open_barcode();
    }

    pub fn open_image_compress(&self) {
        self.webview.open_image_compress();
    }

    pub fn open_image_editor(&self) {
        self.webview.open_image_editor();
    }

    pub fn open_watermark(&self) {
        self.webview.open_watermark();
    }

    pub fn open_settings(&self, settings: Settings) {
        self.webview.open_settings(settings);
    }

    pub fn send_settings(&self, settings: Settings, error: Option<&str>) {
        self.webview.send_settings(settings, error);
    }

    pub fn copy_to_clipboard(&self, text: &str) {
        self.webview.copy_to_clipboard(text);
    }

    pub fn send_native_conversion_result(&self, result: &NativeConversionResult) {
        self.webview.send_native_conversion_result(result);
    }

    pub fn send_media_processing_result(&self, result: &MediaProcessingResult) {
        self.webview.send_media_processing_result(result);
    }

    pub fn hide(&self) {
        self.webview.hide();
    }
}

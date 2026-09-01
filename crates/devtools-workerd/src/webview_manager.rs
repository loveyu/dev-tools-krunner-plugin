use devtools_core::Settings;
use serde::Serialize;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::Duration;
use tao::dpi::LogicalSize;
use tao::event_loop::{EventLoop, EventLoopProxy};
use tao::monitor::MonitorHandle;
use tao::window::{Window, WindowBuilder};
use url::{Host, Url};
use wry::{WebView, WebViewBuilder};

use crate::media_processor::{MediaCapabilities, MediaProcessingResult};
use crate::metadata_processor::{MetadataCapabilities, MetadataProcessingResult};
use crate::native_converter::{ConverterCapabilities, NativeConversionResult};
use crate::platform;
use crate::UserEvent;

const WEB_APP: &str = include_str!("../../../web/devtools-ui/dist/index.html");
const WORKSPACE_SIZE: LogicalSize<f64> = LogicalSize::new(1180.0, 760.0);
const LAUNCHER_SIZE: LogicalSize<f64> = LogicalSize::new(760.0, 480.0);
const DEVELOPMENT_PORT_FILE: &str = "devtools-workerd-vite.port";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InitialState {
    version: &'static str,
    settings: Settings,
    system_locale: devtools_core::LanguageMode,
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

struct WebViewDebug {
    enabled: bool,
    development_url: Option<String>,
}

impl WebViewDebug {
    fn from_environment() -> Result<Self, Box<dyn std::error::Error>> {
        let enabled = environment_flag("DEVTOOLS_WEBVIEW_DEBUG");
        if !enabled {
            return Ok(Self {
                enabled,
                development_url: None,
            });
        }

        let value = development_url_value()?;
        let url = Url::parse(&value)?;
        if !is_loopback_http_url(&url) {
            return Err("DEVTOOLS_WEBVIEW_URL 只允许本机 HTTP/HTTPS 开发服务器".into());
        }
        Ok(Self {
            enabled,
            development_url: Some(url.to_string()),
        })
    }
}

fn development_url_value() -> Result<String, Box<dyn std::error::Error>> {
    if let Ok(value) = std::env::var("DEVTOOLS_WEBVIEW_URL") {
        return Ok(value);
    }
    let (port, from_file) = match std::env::var("DEVTOOLS_WEBVIEW_PORT") {
        Ok(value) => (value, false),
        Err(_) => (
            std::fs::read_to_string(development_port_path()).map_err(|_| {
                "调试模式未找到端口；请先启动 pnpm dev，或设置 DEVTOOLS_WEBVIEW_URL/DEVTOOLS_WEBVIEW_PORT"
            })?,
            true,
        ),
    };
    let port_number = development_port(&port)?;
    if !development_server_is_reachable(port_number) {
        if from_file {
            let _ = std::fs::remove_file(development_port_path());
        }
        return Err(format!("前端调试端口 {port_number} 当前不可访问").into());
    }
    development_url_for_port(&port)
}

fn development_url_for_port(value: &str) -> Result<String, Box<dyn std::error::Error>> {
    let port = development_port(value)?;
    Ok(format!("http://127.0.0.1:{port}"))
}

fn development_port(value: &str) -> Result<u16, Box<dyn std::error::Error>> {
    let port: u16 = value.trim().parse()?;
    if port == 0 {
        return Err("DEVTOOLS_WEBVIEW_PORT 必须大于 0".into());
    }
    Ok(port)
}

fn development_server_is_reachable(port: u16) -> bool {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    TcpStream::connect_timeout(&address, Duration::from_millis(350)).is_ok()
}

fn development_port_path() -> PathBuf {
    std::env::temp_dir().join(DEVELOPMENT_PORT_FILE)
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
        let debug = WebViewDebug::from_environment()?;
        let monitor = event_loop.primary_monitor();
        let initial_size = fit_to_monitor(WORKSPACE_SIZE, monitor.as_ref());
        let window = WindowBuilder::new()
            .with_title("DevTools")
            .with_window_icon(Some(crate::app_icon::window_icon()?))
            .with_inner_size(initial_size)
            .with_min_inner_size(LogicalSize::new(420.0, 360.0))
            .with_visible(false)
            .build(event_loop)?;

        let initial_state = InitialState {
            version: env!("CARGO_PKG_VERSION"),
            settings,
            system_locale: platform::system_language(),
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
        let builder = platform::configure_webview(
            WebViewBuilder::new(),
            WEB_APP,
            debug.development_url.as_deref(),
        );
        let builder = builder
            .with_devtools(debug.enabled)
            .with_clipboard(true)
            .with_initialization_script(&initialization_script)
            .with_ipc_handler(move |request| {
                let _ = web_proxy.send_event(UserEvent::WebMessage(request.body().to_owned()));
            });
        let webview = platform::build_webview(builder, &window)?;
        if debug.enabled {
            webview.open_devtools();
            eprintln!(
                "devtools-workerd: frontend debug mode uses {}",
                debug.development_url.as_deref().unwrap_or_default()
            );
        }

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
        self.resize_for(LAUNCHER_SIZE);
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
        self.resize_for(WORKSPACE_SIZE);
    }

    fn resize_for(&self, desired: LogicalSize<f64>) {
        let monitor = self
            .window
            .current_monitor()
            .or_else(|| self.window.primary_monitor());
        self.window
            .set_inner_size(fit_to_monitor(desired, monitor.as_ref()));
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

fn fit_to_monitor(desired: LogicalSize<f64>, monitor: Option<&MonitorHandle>) -> LogicalSize<f64> {
    let Some(monitor) = monitor else {
        return desired;
    };
    let available: LogicalSize<f64> = monitor.size().to_logical(monitor.scale_factor());
    fit_to_available(desired, available)
}

fn fit_to_available(desired: LogicalSize<f64>, available: LogicalSize<f64>) -> LogicalSize<f64> {
    LogicalSize::new(
        desired.width.min(available.width * 0.92),
        desired.height.min(available.height * 0.86),
    )
}

fn environment_flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
}

fn is_loopback_http_url(url: &Url) -> bool {
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    match url.host() {
        Some(Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desired_size_is_kept_without_a_monitor() {
        assert_eq!(fit_to_monitor(WORKSPACE_SIZE, None), WORKSPACE_SIZE);
    }

    #[test]
    fn desired_size_is_reduced_to_leave_screen_margins() {
        assert_eq!(
            fit_to_available(WORKSPACE_SIZE, LogicalSize::new(800.0, 600.0)),
            LogicalSize::new(736.0, 516.0),
        );
    }

    #[test]
    fn development_url_only_accepts_loopback_http_servers() {
        for value in [
            "http://127.0.0.1:5173",
            "http://localhost:4173/path",
            "https://[::1]:5173",
        ] {
            assert!(is_loopback_http_url(&Url::parse(value).unwrap()));
        }
        for value in ["https://example.com", "file:///tmp/index.html"] {
            assert!(!is_loopback_http_url(&Url::parse(value).unwrap()));
        }
    }

    #[test]
    fn development_port_builds_a_loopback_url() {
        assert_eq!(
            development_url_for_port("7173\n").unwrap(),
            "http://127.0.0.1:7173"
        );
        assert!(development_url_for_port("0").is_err());
        assert!(development_url_for_port("invalid").is_err());
    }
}

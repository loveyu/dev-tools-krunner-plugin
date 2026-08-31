//! DevTools GUI Worker：承载 D-Bus、WebView、系统托盘和桌面设置。

#![allow(non_snake_case)]

mod media_processor;
mod native_converter;
mod registry;
mod settings;
mod tray;
mod webview;

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use devtools_core::{Settings, WORKER_INTERFACE, WORKER_OBJECT_PATH, WORKER_SERVICE_NAME};
use media_processor::{MediaOperation, MediaOptions, MediaProcessingResult, MediaProcessor};
use native_converter::{
    ConversionDirection, NativeConversionResult, NativeConverter, NativeFormat,
};
use registry::ToolRegistry;
use serde::Deserialize;
use settings::SettingsStore;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tray::TrayManager;
use webview::{parse_web_request, WorkspaceWindow};
use zbus::blocking::{Connection, ConnectionBuilder, Proxy};

enum UserEvent {
    OpenJson(String),
    OpenConvert(String),
    OpenOcr,
    OpenBarcode,
    OpenSettings,
    WebMessage(String),
    NativeConversionFinished(NativeConversionResult),
    MediaProcessingFinished(MediaProcessingResult),
    Restart,
    Quit,
}

#[derive(Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum WebRequest {
    FrontendReady,
    ClipboardWrite {
        text: String,
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
    SettingsGet,
    SettingsUpdate {
        settings: Settings,
    },
}

struct WorkerService {
    proxy: EventLoopProxy<UserEvent>,
    registry: Arc<ToolRegistry>,
    webview_ready: Arc<AtomicBool>,
}

#[zbus::interface(name = "org.loveyu.DevTools")]
impl WorkerService {
    fn OpenTool(&self, tool: &str, payload: &str) -> zbus::fdo::Result<()> {
        let result = self
            .registry
            .execute(tool, payload)
            .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))?;
        let event = match tool {
            "json" => UserEvent::OpenJson(result.payload),
            "convert" => UserEvent::OpenConvert(result.payload),
            "ocr" => UserEvent::OpenOcr,
            "barcode" => UserEvent::OpenBarcode,
            _ => {
                return Err(zbus::fdo::Error::Failed(format!(
                    "unsupported tool: {tool}"
                )))
            }
        };
        self.proxy
            .send_event(event)
            .map_err(|_| zbus::fdo::Error::Failed("worker event loop is closed".to_owned()))
    }

    fn OpenSettings(&self) -> zbus::fdo::Result<()> {
        self.proxy
            .send_event(UserEvent::OpenSettings)
            .map_err(|_| zbus::fdo::Error::Failed("worker event loop is closed".to_owned()))
    }

    fn Restart(&self) -> zbus::fdo::Result<()> {
        self.proxy
            .send_event(UserEvent::Restart)
            .map_err(|_| zbus::fdo::Error::Failed("worker event loop is closed".to_owned()))
    }

    fn Quit(&self) -> zbus::fdo::Result<()> {
        self.proxy
            .send_event(UserEvent::Quit)
            .map_err(|_| zbus::fdo::Error::Failed("worker event loop is closed".to_owned()))
    }

    fn IsWebViewReady(&self) -> bool {
        self.webview_ready.load(Ordering::Acquire)
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("devtools-workerd: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let open_settings = std::env::args()
        .skip(1)
        .any(|argument| argument == "--settings");
    if open_settings && request_existing_worker("OpenSettings").is_ok() {
        return Ok(());
    }

    ensure_display_available()?;
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let store = SettingsStore::from_environment()?;
    let mut current_settings = store.load();
    let executable = std::env::current_exe()?;
    if let Err(error) = store.sync_autostart(current_settings.autostart, &executable) {
        eprintln!("devtools-workerd: failed to sync autostart entry: {error}");
    }

    let native_converter = NativeConverter::start(proxy.clone());
    let media_processor = MediaProcessor::start(proxy.clone());
    let workspace = WorkspaceWindow::new(
        &event_loop,
        proxy.clone(),
        current_settings,
        native_converter.capabilities().clone(),
        media_processor.capabilities().clone(),
    )?;
    let mut tray = TrayManager::new(proxy.clone());
    if let Err(error) = tray.set_visible(current_settings.show_tray) {
        eprintln!("devtools-workerd: failed to create tray icon: {error}");
        current_settings.show_tray = false;
    }

    let registry = Arc::new(ToolRegistry::standard());
    let webview_ready = Arc::new(AtomicBool::new(false));
    let service = WorkerService {
        proxy: proxy.clone(),
        registry,
        webview_ready: Arc::clone(&webview_ready),
    };
    let _connection = ConnectionBuilder::session()?
        .name(WORKER_SERVICE_NAME)?
        .serve_at(WORKER_OBJECT_PATH, service)?
        .build()?;
    eprintln!("devtools-workerd: ready");

    if open_settings {
        let _ = proxy.send_event(UserEvent::OpenSettings);
    }

    event_loop.run(move |event, _target, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(UserEvent::OpenJson(payload)) => workspace.open_json(&payload),
            Event::UserEvent(UserEvent::OpenConvert(payload)) => workspace.open_convert(&payload),
            Event::UserEvent(UserEvent::OpenOcr) => workspace.open_ocr(),
            Event::UserEvent(UserEvent::OpenBarcode) => workspace.open_barcode(),
            Event::UserEvent(UserEvent::OpenSettings) => workspace.open_settings(current_settings),
            Event::UserEvent(UserEvent::WebMessage(payload)) => match parse_web_request(&payload) {
                Ok(WebRequest::FrontendReady) => {
                    webview_ready.store(true, Ordering::Release);
                }
                Ok(WebRequest::ClipboardWrite { text }) => workspace.copy_to_clipboard(&text),
                Ok(WebRequest::NativeConvert {
                    request_id,
                    format,
                    direction,
                    payload,
                }) => {
                    if let Err(error) =
                        native_converter.submit(request_id.clone(), format, direction, payload)
                    {
                        workspace.send_native_conversion_result(&NativeConversionResult::error(
                            request_id, error,
                        ));
                    }
                }
                Ok(WebRequest::MediaProcess {
                    request_id,
                    operation,
                    image_base64,
                    mime_type,
                    options,
                }) => {
                    if let Err(error) = media_processor.submit(
                        request_id.clone(),
                        operation,
                        image_base64,
                        mime_type,
                        options,
                    ) {
                        workspace.send_media_processing_result(&MediaProcessingResult::error(
                            request_id, error,
                        ));
                    }
                }
                Ok(WebRequest::SettingsGet) => {
                    workspace.send_settings(current_settings, None);
                }
                Ok(WebRequest::SettingsUpdate { settings: next }) => {
                    match update_settings(&store, &executable, &mut tray, current_settings, next) {
                        Ok(()) => {
                            current_settings = next;
                            workspace.send_settings(current_settings, None);
                        }
                        Err(error) => {
                            eprintln!("devtools-workerd: failed to update settings: {error}");
                            workspace.send_settings(current_settings, Some(&error));
                        }
                    }
                }
                Err(error) => eprintln!("devtools-workerd: invalid webview IPC request: {error}"),
            },
            Event::UserEvent(UserEvent::NativeConversionFinished(result)) => {
                workspace.send_native_conversion_result(&result);
            }
            Event::UserEvent(UserEvent::MediaProcessingFinished(result)) => {
                workspace.send_media_processing_result(&result);
            }
            Event::UserEvent(UserEvent::Restart) => restart_worker(&mut tray, &executable),
            Event::UserEvent(UserEvent::Quit) => {
                tray.shutdown();
                std::process::exit(0);
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
                ..
            } if window_id == workspace.id() => workspace.hide(),
            _ => {}
        }
    });
}

fn update_settings(
    store: &SettingsStore,
    executable: &Path,
    tray: &mut TrayManager,
    previous: Settings,
    next: Settings,
) -> Result<(), String> {
    store
        .apply(previous, next, executable)
        .map_err(|error| error.to_string())?;
    if let Err(error) = tray.set_visible(next.show_tray) {
        let _ = store.apply(next, previous, executable);
        return Err(error.to_string());
    }
    Ok(())
}

fn request_existing_worker(method: &str) -> zbus::Result<()> {
    let connection = Connection::session()?;
    let proxy = Proxy::new(
        &connection,
        WORKER_SERVICE_NAME,
        WORKER_OBJECT_PATH,
        WORKER_INTERFACE,
    )?;
    proxy.call::<_, _, ()>(method, &())
}

fn restart_worker(tray: &mut TrayManager, executable: &Path) {
    tray.shutdown();
    use std::os::unix::process::CommandExt;
    let error = Command::new(executable).exec();
    eprintln!("devtools-workerd: failed to restart: {error}");
}

fn ensure_display_available() -> Result<(), &'static str> {
    let has_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let has_x11 = std::env::var_os("DISPLAY").is_some();
    if has_wayland || has_x11 {
        Ok(())
    } else {
        Err("neither WAYLAND_DISPLAY nor DISPLAY is available")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_webview_settings_request() {
        let request = parse_web_request(
            r#"{"type":"settingsUpdate","settings":{"showTray":false,"autostart":true}}"#,
        )
        .expect("设置请求应可解析");

        assert!(matches!(
            request,
            WebRequest::SettingsUpdate {
                settings: Settings {
                    show_tray: false,
                    autostart: true
                }
            }
        ));
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
    fn parses_barcode_request_with_empty_options() {
        let request = parse_web_request(
            r#"{"type":"mediaProcess","requestId":"m-2","operation":"barcode","imageBase64":"aGVsbG8=","mimeType":"image/png","options":{}}"#,
        )
        .expect("条码请求应可解析");

        assert!(matches!(
            request,
            WebRequest::MediaProcess {
                operation: MediaOperation::Barcode,
                ..
            }
        ));
    }
}

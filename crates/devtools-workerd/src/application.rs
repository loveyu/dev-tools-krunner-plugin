use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use devtools_core::Settings;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};

use crate::global_shortcut::ShortcutManager;
use crate::ipc::{parse_web_request, WebRequest};
use crate::media_processor::{MediaProcessingResult, MediaProcessor};
use crate::metadata_processor::{MetadataProcessingResult, MetadataProcessor};
use crate::native_converter::{NativeConversionResult, NativeConverter};
use crate::platform::{self, TrayManager};
use crate::quick_input::HistoryStore;
use crate::registry::ToolRegistry;
use crate::settings::SettingsStore;
use crate::window_manager::WindowManager;

/// 应用层事件不携带任何平台实现类型。
pub enum UserEvent {
    OpenLauncher,
    OpenQuickInput,
    QuickInputSubmitted {
        text: String,
        target_window: Option<String>,
    },
    OpenJson(String),
    OpenConvert(String),
    OpenOcr,
    OpenBarcode,
    OpenImageCompress,
    OpenImageEditor,
    OpenWatermark,
    OpenCrypto,
    OpenMetadata,
    OpenColor,
    OpenSettings,
    WebMessage(String),
    NativeConversionFinished(NativeConversionResult),
    MediaProcessingFinished(MediaProcessingResult),
    MetadataProcessingFinished(MetadataProcessingResult),
    ColorPickingFinished(crate::ColorPickResult),
    Restart,
    Quit,
}

/// Worker 生命周期与业务事件的统一编排入口。
pub struct Application;

impl Application {
    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let arguments = std::env::args().skip(1).collect::<Vec<_>>();
        let open_settings = arguments.iter().any(|argument| argument == "--settings");
        let open_launcher = arguments.iter().any(|argument| argument == "--launcher");
        let open_quick_input = arguments.iter().any(|argument| argument == "--quick-input");
        if open_settings && platform::request_existing_worker("OpenSettings") {
            return Ok(());
        }
        if open_launcher && platform::request_existing_worker("OpenLauncher") {
            return Ok(());
        }
        if open_quick_input && platform::request_existing_worker("OpenQuickInput") {
            return Ok(());
        }

        platform::ensure_display_available()?;
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
        let metadata_processor = MetadataProcessor::start(proxy.clone());
        let history_store = HistoryStore::from_environment()?;
        let windows = WindowManager::new(
            &event_loop,
            proxy.clone(),
            current_settings.clone(),
            native_converter.capabilities().clone(),
            media_processor.capabilities().clone(),
            metadata_processor.capabilities().clone(),
            history_store.load(),
        )?;
        let mut tray = TrayManager::new(proxy.clone());
        if let Err(error) = tray.set_visible(current_settings.show_tray, current_settings.language)
        {
            eprintln!("devtools-workerd: failed to create tray icon: {error}");
            current_settings.show_tray = false;
        }

        let mut shortcut = ShortcutManager::new(proxy.clone())?;
        if let Err(error) = shortcut.apply(&current_settings) {
            eprintln!("devtools-workerd: failed to register global shortcut: {error}");
            current_settings.global_shortcut_enabled = false;
            current_settings.quick_input_enabled = false;
        }

        let registry = Arc::new(ToolRegistry::standard());
        let webview_ready = Arc::new(AtomicBool::new(false));
        let ipc_guard = platform::start_ipc(proxy.clone(), registry, Arc::clone(&webview_ready))?;
        eprintln!("devtools-workerd: ready");

        if open_settings {
            windows.open_settings(current_settings.clone());
        }
        if open_launcher {
            windows.open_launcher();
        }
        if open_quick_input {
            eprintln!("devtools-workerd: opening native quick input");
            windows.open_quick_input(&current_settings);
        }

        event_loop.run(move |event, _target, control_flow| {
            let _keep_ipc_alive = &ipc_guard;
            *control_flow = ControlFlow::Wait;
            match event {
                Event::UserEvent(UserEvent::OpenLauncher) => windows.open_launcher(),
                Event::UserEvent(UserEvent::OpenQuickInput) => {
                    windows.open_quick_input(&current_settings)
                }
                Event::UserEvent(UserEvent::QuickInputSubmitted {
                    text,
                    target_window,
                }) => {
                    eprintln!("devtools-workerd: native quick input submitted");
                    if let Err(error) = history_store.append(&text) {
                        eprintln!("devtools-workerd: failed to save quick input history: {error}");
                    }
                    if let Err(error) = windows.submit_quick_input(&text, target_window.as_deref())
                    {
                        eprintln!("devtools-workerd: failed to inject quick input: {error}");
                    }
                }
                Event::UserEvent(UserEvent::OpenJson(payload)) => windows.open_json(&payload),
                Event::UserEvent(UserEvent::OpenConvert(payload)) => windows.open_convert(&payload),
                Event::UserEvent(UserEvent::OpenOcr) => windows.open_ocr(),
                Event::UserEvent(UserEvent::OpenBarcode) => windows.open_barcode(),
                Event::UserEvent(UserEvent::OpenImageCompress) => windows.open_image_compress(),
                Event::UserEvent(UserEvent::OpenImageEditor) => windows.open_image_editor(),
                Event::UserEvent(UserEvent::OpenWatermark) => windows.open_watermark(),
                Event::UserEvent(UserEvent::OpenCrypto) => windows.open_crypto(),
                Event::UserEvent(UserEvent::OpenMetadata) => windows.open_metadata(),
                Event::UserEvent(UserEvent::OpenColor) => windows.open_color(),
                Event::UserEvent(UserEvent::OpenSettings) => {
                    windows.open_settings(current_settings.clone())
                }
                Event::UserEvent(UserEvent::WebMessage(payload)) => {
                    match parse_web_request(&payload) {
                        Ok(WebRequest::FrontendReady) => {
                            webview_ready.store(true, Ordering::Release);
                        }
                        Ok(WebRequest::ClipboardWrite { text }) => windows.copy_to_clipboard(&text),
                        Ok(WebRequest::NativeConvert {
                            request_id,
                            format,
                            direction,
                            payload,
                        }) => {
                            if let Err(error) = native_converter.submit(
                                request_id.clone(),
                                format,
                                direction,
                                payload,
                            ) {
                                windows.send_native_conversion_result(
                                    &NativeConversionResult::error(request_id, error),
                                );
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
                                windows.send_media_processing_result(
                                    &MediaProcessingResult::error(request_id, error),
                                );
                            }
                        }
                        Ok(WebRequest::MetadataPick { request_id }) => {
                            if let Some(path) = platform::pick_metadata_path() {
                                if let Err(error) = metadata_processor.submit(
                                    request_id.clone(),
                                    path,
                                    current_settings.metadata_backend,
                                ) {
                                    windows.send_metadata_processing_result(
                                        &MetadataProcessingResult::error(request_id, error),
                                    );
                                }
                            } else {
                                windows.send_metadata_processing_result(
                                    &MetadataProcessingResult::error(
                                        request_id,
                                        "file selection was cancelled",
                                    ),
                                );
                            }
                        }
                        Ok(WebRequest::MetadataImage {
                            request_id,
                            image_base64,
                            mime_type,
                        }) => {
                            if let Err(error) = metadata_processor.submit_image(
                                request_id.clone(),
                                image_base64,
                                mime_type,
                                current_settings.metadata_backend,
                            ) {
                                windows.send_metadata_processing_result(
                                    &MetadataProcessingResult::error(request_id, error),
                                );
                            }
                        }
                        Ok(WebRequest::ColorPick { request_id }) => {
                            windows.hide();
                            platform::start_screen_color_picker(request_id, proxy.clone());
                        }
                        Ok(WebRequest::SettingsGet) => {
                            windows.send_settings(current_settings.clone(), None);
                        }
                        Ok(WebRequest::WindowHide) => windows.hide(),
                        Ok(WebRequest::SettingsUpdate { settings: next }) => match update_settings(
                            &store,
                            &executable,
                            &mut tray,
                            &mut shortcut,
                            &current_settings,
                            &next,
                        ) {
                            Ok(()) => {
                                current_settings = next;
                                windows.send_settings(current_settings.clone(), None);
                            }
                            Err(error) => {
                                eprintln!("devtools-workerd: failed to update settings: {error}");
                                windows.send_settings(current_settings.clone(), Some(&error));
                            }
                        },
                        Err(error) => {
                            eprintln!("devtools-workerd: invalid webview IPC request: {error}")
                        }
                    }
                }
                Event::UserEvent(UserEvent::NativeConversionFinished(result)) => {
                    windows.send_native_conversion_result(&result);
                }
                Event::UserEvent(UserEvent::MediaProcessingFinished(result)) => {
                    windows.send_media_processing_result(&result);
                }
                Event::UserEvent(UserEvent::MetadataProcessingFinished(result)) => {
                    windows.send_metadata_processing_result(&result);
                }
                Event::UserEvent(UserEvent::ColorPickingFinished(result)) => {
                    windows.restore();
                    windows.send_color_pick_result(&result);
                }
                Event::UserEvent(UserEvent::Restart) => {
                    tray.shutdown();
                    platform::restart(&executable);
                }
                Event::UserEvent(UserEvent::Quit) => {
                    tray.shutdown();
                    std::process::exit(0);
                }
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    window_id,
                    ..
                } if window_id == windows.id() => windows.hide(),
                _ => {}
            }
        });
    }
}

fn update_settings(
    store: &SettingsStore,
    executable: &Path,
    tray: &mut TrayManager,
    shortcut: &mut ShortcutManager,
    previous: &Settings,
    next: &Settings,
) -> Result<(), String> {
    validate_settings(next)?;
    shortcut.apply(next)?;
    store.apply(previous, next, executable).map_err(|error| {
        let _ = shortcut.apply(previous);
        error.to_string()
    })?;
    if let Err(error) = tray.set_visible(next.show_tray, next.language) {
        let _ = store.apply(next, previous, executable);
        let _ = shortcut.apply(previous);
        return Err(error);
    }
    Ok(())
}

fn validate_settings(settings: &Settings) -> Result<(), String> {
    if !(240..=1600).contains(&settings.quick_input_width) {
        return Err("quick input width must be between 240 and 1600".to_owned());
    }
    if !(40..=240).contains(&settings.quick_input_height) {
        return Err("quick input height must be between 40 and 240".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_quick_input_dimensions() {
        let mut settings = Settings::default();
        assert!(validate_settings(&settings).is_ok());

        settings.quick_input_width = 239;
        assert_eq!(
            validate_settings(&settings).expect_err("过小宽度应拒绝"),
            "quick input width must be between 240 and 1600"
        );
        settings.quick_input_width = 560;
        settings.quick_input_height = 241;
        assert_eq!(
            validate_settings(&settings).expect_err("过大高度应拒绝"),
            "quick input height must be between 40 and 240"
        );
    }
}

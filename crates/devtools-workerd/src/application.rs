use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use devtools_core::Settings;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};

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
    /// 后台快捷键注册的结果；授权弹窗可能让注册耗时长达 60s，故经事件回传。
    ShortcutApplied {
        next: Box<Settings>,
        result: Result<(), String>,
        /// 启动路径：成功无需持久化（设置本就来自磁盘），失败仅内存禁用；
        /// 设置更新路径：成功才持久化，失败保持原设置并回显错误。
        from_startup: bool,
    },
    Restart,
    Quit,
}

/// Worker 生命周期与业务事件的统一编排入口。
pub struct Application;

impl Application {
    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        // global_hotkey 依赖的环境变量必须先于一切线程启动设置（见 prepare_environment 注释）。
        ShortcutManager::prepare_environment();
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
        let mut event_loop_builder = EventLoopBuilder::<UserEvent>::with_user_event();
        platform::configure_event_loop(&mut event_loop_builder);
        let event_loop = event_loop_builder.build();
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

        let shortcut = ShortcutManager::new(proxy.clone())?;
        // 注册可能等待 KDE 授权弹窗（上限 60s），改为后台执行，结果经
        // ShortcutApplied 事件回传；校验失败在启动期同步禁用并继续。
        if let Err(error) = shortcut.validate(&current_settings) {
            eprintln!("devtools-workerd: invalid global shortcut settings: {error}");
            current_settings.global_shortcut_enabled = false;
            current_settings.quick_input_enabled = false;
        } else if let Err(error) =
            submit_shortcut_apply(&shortcut, proxy.clone(), current_settings.clone(), true)
        {
            eprintln!("devtools-workerd: failed to schedule global shortcut registration: {error}");
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
                Event::UserEvent(UserEvent::QuickInputSubmitted { text }) => {
                    eprintln!("devtools-workerd: native quick input submitted");
                    if let Err(error) = history_store.append(&text) {
                        eprintln!("devtools-workerd: failed to save quick input history: {error}");
                    }
                    if let Err(error) = windows.copy_to_clipboard(&text) {
                        eprintln!("devtools-workerd: failed to copy quick input: {error}");
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
                        Ok(WebRequest::ClipboardWrite { text }) => {
                            if let Err(error) = windows.copy_to_clipboard(&text) {
                                eprintln!("devtools-workerd: failed to copy webview text: {error}");
                            }
                        }
                        Ok(WebRequest::OpenExternal { url }) => {
                            if let Err(error) = platform::open_external_url(&url) {
                                eprintln!("devtools-workerd: failed to open external url: {error}");
                            }
                        }
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
                                    &MetadataProcessingResult::cancelled(request_id),
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
                        Ok(WebRequest::SettingsUpdate { settings: next }) => {
                            match schedule_settings_update(
                                &store,
                                &executable,
                                &mut tray,
                                &shortcut,
                                &proxy,
                                &current_settings,
                                next,
                            ) {
                                Ok(()) => { /* 涉及快捷键时由 ShortcutApplied 事件收尾 */
                                }
                                Err(error) => {
                                    eprintln!(
                                        "devtools-workerd: failed to update settings: {error}"
                                    );
                                    windows.send_settings(current_settings.clone(), Some(&error));
                                }
                            }
                        }
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
                Event::UserEvent(UserEvent::ShortcutApplied {
                    next,
                    result,
                    from_startup,
                }) => match result {
                    Ok(()) if !from_startup => {
                        // 快捷键注册成功才持久化；previous 取当前内存值，
                        // 排队期间的多次更新也能得到正确的 autostart 差量。
                        let previous = current_settings.clone();
                        match commit_settings(&store, &executable, &mut tray, &previous, &next) {
                            Ok(()) => {
                                current_settings = *next;
                                windows.send_settings(current_settings.clone(), None);
                            }
                            Err(error) => {
                                eprintln!("devtools-workerd: failed to update settings: {error}");
                                windows.send_settings(current_settings.clone(), Some(&error));
                            }
                        }
                    }
                    Ok(()) => { /* 启动注册成功：设置本就来自磁盘，无需持久化 */
                    }
                    Err(error) => {
                        eprintln!("devtools-workerd: failed to register global shortcut: {error}");
                        if from_startup {
                            // 启动失败仅禁用内存态，磁盘不动，下次启动重试。
                            current_settings.global_shortcut_enabled = false;
                            current_settings.quick_input_enabled = false;
                        }
                        windows.send_settings(current_settings.clone(), Some(&error));
                    }
                },
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

/// 处理设置更新：快捷键有变化时先排队后台注册（注册成功经事件收尾），
/// 其余直接同步提交。校验失败立即返回错误，不产生任何副作用。
fn schedule_settings_update(
    store: &SettingsStore,
    executable: &Path,
    tray: &mut TrayManager,
    shortcut: &ShortcutManager,
    proxy: &EventLoopProxy<UserEvent>,
    previous: &Settings,
    next: Settings,
) -> Result<(), String> {
    validate_settings(&next)?;
    if shortcuts_differ(previous, &next) {
        return submit_shortcut_apply(shortcut, proxy.clone(), next, false);
    }
    commit_settings(store, executable, tray, previous, &next)?;
    Ok(())
}

/// 提交一次后台快捷键注册，结果经用户事件回传。
fn submit_shortcut_apply(
    shortcut: &ShortcutManager,
    proxy: EventLoopProxy<UserEvent>,
    next: Settings,
    from_startup: bool,
) -> Result<(), String> {
    let next = Box::new(next);
    let snapshot = (*next).clone();
    shortcut.apply_async(snapshot, move |result| {
        let _ = proxy.send_event(UserEvent::ShortcutApplied {
            next,
            result,
            from_startup,
        });
    })
}

/// 完成设置持久化与托盘同步；不涉及快捷键（注册在 worker 线程）。
fn commit_settings(
    store: &SettingsStore,
    executable: &Path,
    tray: &mut TrayManager,
    previous: &Settings,
    next: &Settings,
) -> Result<(), String> {
    store
        .apply(previous, next, executable)
        .map_err(|error| error.to_string())?;
    if let Err(error) = tray.set_visible(next.show_tray, next.language) {
        // 托盘失败仅回滚持久化设置；快捷键注册独立于本函数，不受影响。
        let _ = store.apply(next, previous, executable);
        return Err(error.to_string());
    }
    Ok(())
}

/// 快捷键相关字段是否有变化；变化时注册须经 worker 线程（可能等待授权弹窗）。
fn shortcuts_differ(previous: &Settings, next: &Settings) -> bool {
    previous.global_shortcut_enabled != next.global_shortcut_enabled
        || previous.global_shortcut != next.global_shortcut
        || previous.quick_input_enabled != next.quick_input_enabled
        || previous.quick_input_shortcut != next.quick_input_shortcut
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

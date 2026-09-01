use std::cell::{Cell, RefCell};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use ashpd::desktop::global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut};
use ashpd::desktop::remote_desktop::{DeviceType, KeyState, RemoteDesktop, SelectDevicesOptions};
use ashpd::desktop::{Color, CreateSessionOptions, PersistMode, Session};
use ashpd::AppID;
use devtools_core::{
    LanguageMode, Settings, WORKER_INTERFACE, WORKER_OBJECT_PATH, WORKER_SERVICE_NAME,
};
use futures::StreamExt;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use gtk::gdk;
use gtk::prelude::*;
use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::StandardItem;
use tao::event_loop::EventLoopProxy;
use tao::platform::unix::WindowExtUnix;
use tao::window::Window;
use tokio::sync::mpsc as tokio_mpsc;
use wry::{WebView, WebViewBuilder, WebViewBuilderExtUnix};
use zbus::blocking::{Connection, ConnectionBuilder, Proxy};

use crate::ipc::tool_event;
use crate::registry::ToolRegistry;
use crate::ColorPickResult;
use crate::UserEvent;

pub fn configure_webview<'a>(
    builder: WebViewBuilder<'a>,
    html: &'static str,
    development_url: Option<&str>,
) -> WebViewBuilder<'a> {
    if let Some(url) = development_url {
        builder.with_url(url)
    } else {
        builder.with_html(html)
    }
}

pub fn build_webview(
    builder: WebViewBuilder<'_>,
    window: &Window,
) -> Result<WebView, Box<dyn std::error::Error>> {
    let container = window
        .default_vbox()
        .ok_or("Tao GTK window does not expose a default container")?;
    Ok(builder.build_gtk(container)?)
}

pub fn copy_text(text: &str) {
    let clipboard = gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD);
    clipboard.set_text(text);
    clipboard.store();
}

pub fn pick_metadata_path() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Choose an image, video, or media file")
        .pick_file()
}

/// KDE Wayland 与 X11 都通过 Screenshot Portal 的 PickColor 进入跨屏幕取色模式。
pub fn start_screen_color_picker(request_id: String, proxy: EventLoopProxy<UserEvent>) {
    thread::spawn(move || {
        let result = tokio::runtime::Runtime::new()
            .map_err(|error| error.to_string())
            .and_then(|runtime| {
                runtime.block_on(async {
                    let request = Color::pick()
                        .send()
                        .await
                        .map_err(|error| error.to_string())?;
                    match request.response() {
                        Ok(color) => Ok(ColorPickResult::success(
                            request_id.clone(),
                            normalized_channel(color.red()),
                            normalized_channel(color.green()),
                            normalized_channel(color.blue()),
                        )),
                        Err(ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)) => {
                            Ok(ColorPickResult::cancelled(request_id.clone()))
                        }
                        Err(error) => Err(error.to_string()),
                    }
                })
            })
            .unwrap_or_else(|error| ColorPickResult::error(request_id, error));
        let _ = proxy.send_event(UserEvent::ColorPickingFinished(result));
    });
}

fn normalized_channel(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

pub fn ensure_display_available() -> Result<(), &'static str> {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some() {
        Ok(())
    } else {
        Err("neither WAYLAND_DISPLAY nor DISPLAY is available")
    }
}

type ShortcutEventHandler = Arc<dyn Fn(GlobalHotKeyEvent) + Send + Sync>;

/// Linux 全局快捷键后端；Wayland 使用 Portal，X11 复用 global-hotkey。
pub struct GlobalShortcutBackend {
    inner: GlobalShortcutBackendInner,
}

enum GlobalShortcutBackendInner {
    Portal(PortalShortcutBackend),
    X11 {
        manager: GlobalHotKeyManager,
        registered: Vec<HotKey>,
    },
}

impl GlobalShortcutBackend {
    pub fn new(
        handler: impl Fn(GlobalHotKeyEvent) + Send + Sync + 'static,
    ) -> Result<Self, String> {
        let handler: ShortcutEventHandler = Arc::new(handler);
        let inner = if use_wayland_shortcut_portal() {
            GlobalShortcutBackendInner::Portal(PortalShortcutBackend::new(handler)?)
        } else {
            let library_handler = Arc::clone(&handler);
            GlobalHotKeyEvent::set_event_handler(Some(move |event| library_handler(event)));
            GlobalShortcutBackendInner::X11 {
                manager: GlobalHotKeyManager::new().map_err(|error| error.to_string())?,
                registered: Vec::new(),
            }
        };
        Ok(Self { inner })
    }

    pub fn replace(&mut self, next: &[HotKey]) -> Result<(), String> {
        match &mut self.inner {
            GlobalShortcutBackendInner::Portal(manager) => manager.replace(next),
            GlobalShortcutBackendInner::X11 {
                manager,
                registered,
            } => replace_library_hotkeys(manager, registered, next),
        }
    }
}

fn use_wayland_shortcut_portal() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok_and(|value| !value.is_empty())
        && !std::env::var("GDK_BACKEND").is_ok_and(|value| value == "x11")
}

fn replace_library_hotkeys(
    manager: &GlobalHotKeyManager,
    registered: &mut Vec<HotKey>,
    next: &[HotKey],
) -> Result<(), String> {
    if next == registered {
        return Ok(());
    }
    let previous = registered.clone();
    if !previous.is_empty() {
        manager
            .unregister_all(&previous)
            .map_err(|error| error.to_string())?;
    }
    if !next.is_empty() {
        if let Err(error) = manager.register_all(next) {
            if !previous.is_empty() {
                let _ = manager.register_all(&previous);
            }
            return Err(error.to_string());
        }
    }
    *registered = next.to_vec();
    Ok(())
}

struct PortalShortcutBackend {
    sender: tokio_mpsc::UnboundedSender<PortalShortcutCommand>,
}

enum PortalShortcutCommand {
    Replace {
        hotkeys: Vec<HotKey>,
        response: mpsc::SyncSender<Result<(), String>>,
    },
    Shutdown,
}

struct ActivePortalShortcutSession {
    session: Session<GlobalShortcuts>,
    path: String,
}

impl PortalShortcutBackend {
    fn new(handler: ShortcutEventHandler) -> Result<Self, String> {
        let (sender, receiver) = tokio_mpsc::unbounded_channel();
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        thread::spawn(move || run_portal_shortcut_thread(receiver, handler, ready_sender));
        ready_receiver
            .recv()
            .map_err(|_| "global shortcuts portal thread stopped during startup".to_owned())??;
        Ok(Self { sender })
    }

    fn replace(&self, hotkeys: &[HotKey]) -> Result<(), String> {
        let (response, result) = mpsc::sync_channel(1);
        self.sender
            .send(PortalShortcutCommand::Replace {
                hotkeys: hotkeys.to_vec(),
                response,
            })
            .map_err(|_| "global shortcuts portal thread is unavailable".to_owned())?;
        result
            .recv()
            .map_err(|_| "global shortcuts portal thread stopped before replying".to_owned())?
    }
}

impl Drop for PortalShortcutBackend {
    fn drop(&mut self) {
        let _ = self.sender.send(PortalShortcutCommand::Shutdown);
    }
}

fn run_portal_shortcut_thread(
    receiver: tokio_mpsc::UnboundedReceiver<PortalShortcutCommand>,
    handler: ShortcutEventHandler,
    ready: mpsc::SyncSender<Result<(), String>>,
) {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = ready.send(Err(format!(
                "failed to create global shortcuts runtime: {error}"
            )));
            return;
        }
    };
    runtime.block_on(run_portal_shortcut_loop(receiver, handler, ready));
}

async fn run_portal_shortcut_loop(
    mut receiver: tokio_mpsc::UnboundedReceiver<PortalShortcutCommand>,
    handler: ShortcutEventHandler,
    ready: mpsc::SyncSender<Result<(), String>>,
) {
    if let Ok(app_id) = AppID::from_str("org.loveyu.DevTools") {
        if let Err(error) = ashpd::register_host_app(app_id).await {
            eprintln!("devtools-workerd: failed to register portal app id: {error}");
        }
    }
    let proxy = match GlobalShortcuts::new().await {
        Ok(proxy) => proxy,
        Err(error) => {
            let _ = ready.send(Err(format!(
                "failed to start global shortcuts portal proxy: {error}"
            )));
            return;
        }
    };
    let mut activated = match proxy.receive_activated().await {
        Ok(stream) => stream,
        Err(error) => {
            let _ = ready.send(Err(format!(
                "failed to receive global shortcut activation events: {error}"
            )));
            return;
        }
    };
    let mut deactivated = match proxy.receive_deactivated().await {
        Ok(stream) => stream,
        Err(error) => {
            let _ = ready.send(Err(format!(
                "failed to receive global shortcut deactivation events: {error}"
            )));
            return;
        }
    };
    if ready.send(Ok(())).is_err() {
        return;
    }

    let mut active_session: Option<ActivePortalShortcutSession> = None;
    loop {
        tokio::select! {
            command = receiver.recv() => match command {
                Some(PortalShortcutCommand::Replace { hotkeys, response }) => {
                    let result = replace_portal_shortcuts(&proxy, &mut active_session, &hotkeys).await;
                    let _ = response.send(result);
                }
                Some(PortalShortcutCommand::Shutdown) | None => break,
            },
            event = activated.next() => match event {
                Some(event) => dispatch_portal_shortcut_event(
                    active_session.as_ref(),
                    event.session_handle().as_str(),
                    event.shortcut_id(),
                    HotKeyState::Pressed,
                    &handler,
                ),
                None => break,
            },
            event = deactivated.next() => match event {
                Some(event) => dispatch_portal_shortcut_event(
                    active_session.as_ref(),
                    event.session_handle().as_str(),
                    event.shortcut_id(),
                    HotKeyState::Released,
                    &handler,
                ),
                None => break,
            },
        }
    }
    if let Some(active) = active_session {
        let _ = active.session.close().await;
    }
}

async fn replace_portal_shortcuts(
    proxy: &GlobalShortcuts,
    active: &mut Option<ActivePortalShortcutSession>,
    hotkeys: &[HotKey],
) -> Result<(), String> {
    if let Some(previous) = active.take() {
        if let Err(error) = previous.session.close().await {
            eprintln!(
                "devtools-workerd: old global shortcuts session was already unavailable: {error}"
            );
        }
    }
    if hotkeys.is_empty() {
        return Ok(());
    }

    let session = proxy
        .create_session(CreateSessionOptions::default())
        .await
        .map_err(|error| format!("failed to create global shortcuts session: {error}"))?;
    let path = serde_json::to_value(&session)
        .map_err(|error| format!("failed to serialize global shortcuts session: {error}"))?
        .as_str()
        .ok_or("global shortcuts session path is not a string")?
        .to_owned();
    let shortcuts = hotkeys
        .iter()
        .map(|hotkey| {
            NewShortcut::new(hotkey.id().to_string(), hotkey.into_string())
                .preferred_trigger(hotkey_to_wayland_trigger(*hotkey).as_deref())
        })
        .collect::<Vec<_>>();
    let request = proxy
        .bind_shortcuts(&session, &shortcuts, None, BindShortcutsOptions::default())
        .await
        .map_err(|error| format!("failed to bind global shortcuts: {error}"))?;
    request
        .response()
        .map_err(|error| format!("global shortcuts request was rejected: {error}"))?;
    *active = Some(ActivePortalShortcutSession { session, path });
    Ok(())
}

fn dispatch_portal_shortcut_event(
    active: Option<&ActivePortalShortcutSession>,
    session_path: &str,
    shortcut_id: &str,
    state: HotKeyState,
    handler: &ShortcutEventHandler,
) {
    if let Some(id) = portal_shortcut_event_id(
        active.map(|active| active.path.as_str()),
        session_path,
        shortcut_id,
    ) {
        handler(GlobalHotKeyEvent { id, state });
    }
}

fn portal_shortcut_event_id(
    active_session_path: Option<&str>,
    event_session_path: &str,
    shortcut_id: &str,
) -> Option<u32> {
    (active_session_path == Some(event_session_path))
        .then(|| shortcut_id.parse().ok())
        .flatten()
}

/// 将 keyboard-types 按键转换为 XDG GlobalShortcuts preferred_trigger。
/// 映射与 wayclip-global-hotkey 0.7.0 的 Wayland 后端保持兼容。
fn hotkey_to_wayland_trigger(hotkey: HotKey) -> Option<String> {
    let mut modifiers = String::new();
    if hotkey.mods.contains(Modifiers::CONTROL) {
        modifiers.push_str("CTRL+");
    }
    if hotkey.mods.contains(Modifiers::SHIFT) {
        modifiers.push_str("SHIFT+");
    }
    if hotkey.mods.contains(Modifiers::ALT) {
        modifiers.push_str("ALT+");
    }
    if hotkey.mods.intersects(Modifiers::SUPER | Modifiers::META) {
        modifiers.push_str("LOGO+");
    }

    let key = match hotkey.key {
        Code::KeyA => "a",
        Code::KeyB => "b",
        Code::KeyC => "c",
        Code::KeyD => "d",
        Code::KeyE => "e",
        Code::KeyF => "f",
        Code::KeyG => "g",
        Code::KeyH => "h",
        Code::KeyI => "i",
        Code::KeyJ => "j",
        Code::KeyK => "k",
        Code::KeyL => "l",
        Code::KeyM => "m",
        Code::KeyN => "n",
        Code::KeyO => "o",
        Code::KeyP => "p",
        Code::KeyQ => "q",
        Code::KeyR => "r",
        Code::KeyS => "s",
        Code::KeyT => "t",
        Code::KeyU => "u",
        Code::KeyV => "v",
        Code::KeyW => "w",
        Code::KeyX => "x",
        Code::KeyY => "y",
        Code::KeyZ => "z",
        Code::Backslash => "backslash",
        Code::BracketLeft => "bracketleft",
        Code::BracketRight => "bracketright",
        Code::Backquote => "grave",
        Code::Comma => "comma",
        Code::Digit0 => "0",
        Code::Digit1 => "1",
        Code::Digit2 => "2",
        Code::Digit3 => "3",
        Code::Digit4 => "4",
        Code::Digit5 => "5",
        Code::Digit6 => "6",
        Code::Digit7 => "7",
        Code::Digit8 => "8",
        Code::Digit9 => "9",
        Code::Equal => "equal",
        Code::Minus => "minus",
        Code::Period => "period",
        Code::Quote => "apostrophe",
        Code::Semicolon => "semicolon",
        Code::Slash => "slash",
        Code::Backspace => "BackSpace",
        Code::CapsLock => "Caps_Lock",
        Code::Enter => "Return",
        Code::Space => "space",
        Code::Tab => "Tab",
        Code::Delete => "Delete",
        Code::End => "End",
        Code::Home => "Home",
        Code::Insert => "Insert",
        Code::PageDown => "Page_Down",
        Code::PageUp => "Page_Up",
        Code::ArrowDown => "Down",
        Code::ArrowLeft => "Left",
        Code::ArrowRight => "Right",
        Code::ArrowUp => "Up",
        Code::Numpad0 => "KP_0",
        Code::Numpad1 => "KP_1",
        Code::Numpad2 => "KP_2",
        Code::Numpad3 => "KP_3",
        Code::Numpad4 => "KP_4",
        Code::Numpad5 => "KP_5",
        Code::Numpad6 => "KP_6",
        Code::Numpad7 => "KP_7",
        Code::Numpad8 => "KP_8",
        Code::Numpad9 => "KP_9",
        Code::NumpadAdd => "KP_Add",
        Code::NumpadDecimal => "KP_Decimal",
        Code::NumpadDivide => "KP_Divide",
        Code::NumpadMultiply => "KP_Multiply",
        Code::NumpadSubtract => "KP_Subtract",
        Code::Escape => "Escape",
        Code::PrintScreen => "Print",
        Code::ScrollLock => "Scroll_Lock",
        Code::NumLock => "Num_Lock",
        Code::F1 => "F1",
        Code::F2 => "F2",
        Code::F3 => "F3",
        Code::F4 => "F4",
        Code::F5 => "F5",
        Code::F6 => "F6",
        Code::F7 => "F7",
        Code::F8 => "F8",
        Code::F9 => "F9",
        Code::F10 => "F10",
        Code::F11 => "F11",
        Code::F12 => "F12",
        Code::AudioVolumeDown => "XF86AudioLowerVolume",
        Code::AudioVolumeMute => "XF86AudioMute",
        Code::AudioVolumeUp => "XF86AudioRaiseVolume",
        Code::MediaPlay => "XF86AudioPlay",
        Code::MediaPause => "XF86AudioPause",
        Code::MediaStop => "XF86AudioStop",
        Code::MediaTrackNext => "XF86AudioNext",
        Code::MediaTrackPrevious => "XF86AudioPrev",
        Code::Pause => "Pause",
        _ => return None,
    };
    modifiers.push_str(key);
    Some(modifiers)
}

pub fn restart(executable: &Path) {
    use std::os::unix::process::CommandExt;

    let error = Command::new(executable).exec();
    eprintln!("devtools-workerd: failed to restart: {error}");
}

pub fn config_root_from_environment() -> io::Result<PathBuf> {
    if let Some(path) = std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(|home| PathBuf::from(home).join(".config"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))
}

pub fn data_root_from_environment() -> io::Result<PathBuf> {
    if let Some(path) = std::env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(|home| PathBuf::from(home).join(".local/share"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))
}

pub fn autostart_path(config_root: &Path) -> PathBuf {
    config_root
        .join("autostart")
        .join("org.loveyu.DevTools.desktop")
}

pub fn autostart_entry(executable: &Path) -> String {
    let executable = quote_desktop_exec(executable);
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=DevTools Worker\n\
         Comment=DevTools JSON Workbench GUI worker\n\
         Exec={executable}\n\
         Icon=applications-development\n\
         Terminal=false\n\
         NoDisplay=true\n\
         OnlyShowIn=KDE;\n\
         X-KDE-autostart-after=panel\n"
    )
}

fn quote_desktop_exec(executable: &Path) -> String {
    let escaped = executable
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('`', "\\`")
        .replace('$', "\\$");
    format!("\"{escaped}\"")
}

/// Linux 原生快速输入窗口使用 GTK，不创建 WebView。
pub struct QuickInputWindow {
    window: gtk::Window,
    entry: gtk::Entry,
    history: Rc<RefCell<Vec<String>>>,
    history_cursor: Rc<Cell<usize>>,
    target_window: Rc<RefCell<Option<String>>>,
}

impl QuickInputWindow {
    pub fn new(
        proxy: EventLoopProxy<UserEvent>,
        initial_history: Vec<String>,
    ) -> Result<Self, String> {
        let window = gtk::Window::new(gtk::WindowType::Toplevel);
        window.set_title("DevTools Quick Input");
        window.set_decorated(false);
        window.set_keep_above(true);
        window.set_skip_pager_hint(true);
        window.set_skip_taskbar_hint(true);
        window.set_type_hint(gdk::WindowTypeHint::Utility);
        window.set_default_size(560, 56);
        window.set_resizable(true);
        window.set_position(gtk::WindowPosition::Mouse);

        let entry = gtk::Entry::new();
        entry.set_hexpand(true);
        entry.set_vexpand(true);
        entry.set_margin_top(8);
        entry.set_margin_bottom(8);
        entry.set_margin_start(12);
        entry.set_margin_end(12);
        window.add(&entry);

        let history = Rc::new(RefCell::new(initial_history));
        let history_cursor = Rc::new(Cell::new(history.borrow().len()));
        let target_window = Rc::new(RefCell::new(None));

        let activate_window = window.clone();
        let activate_history = Rc::clone(&history);
        let activate_cursor = Rc::clone(&history_cursor);
        let activate_target = Rc::clone(&target_window);
        let activate_proxy = proxy;
        entry.connect_activate(move |entry| {
            let text = entry.text().to_string();
            activate_window.hide();
            entry.set_text("");
            if text.is_empty() {
                return;
            }
            activate_history.borrow_mut().push(text.clone());
            activate_cursor.set(activate_history.borrow().len());
            let _ = activate_proxy.send_event(UserEvent::QuickInputSubmitted {
                text,
                target_window: activate_target.borrow_mut().take(),
            });
        });

        let key_window = window.clone();
        let key_history = Rc::clone(&history);
        let key_cursor = Rc::clone(&history_cursor);
        entry.connect_key_press_event(move |entry, event| {
            let key = event.keyval();
            if key == gdk::keys::constants::Escape {
                key_window.hide();
                return gtk::glib::Propagation::Stop;
            }
            let values = key_history.borrow();
            if key == gdk::keys::constants::Up && !values.is_empty() {
                let next = key_cursor.get().saturating_sub(1);
                key_cursor.set(next);
                entry.set_text(&values[next]);
                entry.set_position(-1);
                return gtk::glib::Propagation::Stop;
            }
            if key == gdk::keys::constants::Down && !values.is_empty() {
                let next = (key_cursor.get() + 1).min(values.len());
                key_cursor.set(next);
                entry.set_text(values.get(next).map_or("", String::as_str));
                entry.set_position(-1);
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });

        Ok(Self {
            window,
            entry,
            history,
            history_cursor,
            target_window,
        })
    }

    pub fn show(&self, settings: &Settings) {
        *self.target_window.borrow_mut() = capture_x11_target();
        let (width, height, position) = bounded_geometry(settings);
        self.window.set_default_size(width, height);
        self.window.set_size_request(width, height);
        self.window.resize(width, height);
        if let Some((x, y)) = position {
            self.window.move_(x, y);
        } else {
            self.window.set_position(gtk::WindowPosition::Mouse);
        }
        self.entry
            .set_placeholder_text(Some(placeholder(settings.language)));
        self.entry.set_text("");
        self.history_cursor.set(self.history.borrow().len());
        self.window.show_all();
        self.window.present();
        self.entry.grab_focus();
    }
}

pub struct QuickInputInjector {
    portal_sender: Option<mpsc::Sender<String>>,
}

impl QuickInputInjector {
    pub fn new() -> Self {
        let portal_sender = is_wayland().then(spawn_portal_typer);
        Self { portal_sender }
    }

    pub fn inject(&self, text: &str, target_window: Option<&str>) -> Result<(), String> {
        if let Some(target) = target_window {
            return xdotool_type(target, text);
        }
        self.portal_sender
            .as_ref()
            .ok_or_else(|| "no cross-application input backend is available".to_owned())?
            .send(text.to_owned())
            .map_err(|error| error.to_string())
    }
}

fn bounded_geometry(settings: &Settings) -> (i32, i32, Option<(i32, i32)>) {
    let requested_width = settings.quick_input_width.clamp(240, 1600) as i32;
    let requested_height = settings.quick_input_height.clamp(40, 240) as i32;
    let Some(display) = gdk::Display::default() else {
        return (requested_width, requested_height, None);
    };
    let Some(pointer) = display.default_seat().and_then(|seat| seat.pointer()) else {
        return (requested_width, requested_height, None);
    };
    let (_, pointer_x, pointer_y) = pointer.position();
    let Some(monitor) = display.monitor_at_point(pointer_x, pointer_y) else {
        return (requested_width, requested_height, None);
    };
    let workarea = monitor.workarea();
    let width = requested_width.min((workarea.width() - 24).max(240));
    let height = requested_height.min((workarea.height() - 24).max(40));
    if is_wayland() {
        return (width, height, None);
    }
    let x = pointer_x
        .min(workarea.x() + workarea.width() - width - 12)
        .max(workarea.x() + 12);
    let y = (pointer_y + 16)
        .min(workarea.y() + workarea.height() - height - 12)
        .max(workarea.y() + 12);
    (width, height, Some((x, y)))
}

fn capture_x11_target() -> Option<String> {
    if is_wayland() {
        return None;
    }
    let output = Command::new("xdotool")
        .arg("getactivewindow")
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn xdotool_type(target: &str, text: &str) -> Result<(), String> {
    let status = Command::new("xdotool")
        .args([
            "windowactivate",
            "--sync",
            target,
            "type",
            "--clearmodifiers",
            "--delay",
            "0",
            "--",
        ])
        .arg(text)
        .status()
        .map_err(|error| format!("failed to start xdotool: {error}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("xdotool exited with {status}"))
}

fn is_wayland() -> bool {
    env_var_present("WAYLAND_DISPLAY")
        && std::env::var("GDK_BACKEND").map_or(true, |value| value != "x11")
}

fn env_var_present(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

fn spawn_portal_typer() -> mpsc::Sender<String> {
    let (sender, receiver) = mpsc::channel::<String>();
    thread::spawn(move || {
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => {
                eprintln!("devtools-workerd: failed to start portal runtime: {error}");
                return;
            }
        };
        let mut session = None;
        while let Ok(text) = receiver.recv() {
            let result = runtime.block_on(async {
                if session.is_none() {
                    session = Some(create_portal_session().await?);
                }
                // Portal 首次授权也可能切走焦点；授权完成后再等待原窗口重新获得焦点。
                tokio::time::sleep(Duration::from_millis(300)).await;
                portal_type_text(session.as_ref().expect("session was initialized"), &text).await
            });
            if let Err(error) = result {
                session = None;
                eprintln!("devtools-workerd: Wayland quick input failed: {error}");
            }
        }
    });
    sender
}

struct PortalSession {
    proxy: RemoteDesktop,
    session: Session<RemoteDesktop>,
}

async fn create_portal_session() -> Result<PortalSession, String> {
    let proxy = RemoteDesktop::new()
        .await
        .map_err(|error| error.to_string())?;
    let session = proxy
        .create_session(Default::default())
        .await
        .map_err(|error| error.to_string())?;
    proxy
        .select_devices(
            &session,
            SelectDevicesOptions::default()
                .set_devices(Some(DeviceType::Keyboard.into()))
                .set_persist_mode(PersistMode::ExplicitlyRevoked),
        )
        .await
        .map_err(|error| error.to_string())?
        .response()
        .map_err(|error| error.to_string())?;
    proxy
        .start(&session, None, Default::default())
        .await
        .map_err(|error| error.to_string())?
        .response()
        .map_err(|error| error.to_string())?;
    Ok(PortalSession { proxy, session })
}

async fn portal_type_text(session: &PortalSession, text: &str) -> Result<(), String> {
    for character in text.chars() {
        let keysym = character_to_keysym(character);
        for state in [KeyState::Pressed, KeyState::Released] {
            session
                .proxy
                .notify_keyboard_keysym(&session.session, keysym, state, Default::default())
                .await
                .map_err(|error| error.to_string())?;
        }
        // Portal/合成器处理输入是异步的，轻微节流可避免字符丢失或到达顺序变化。
        tokio::time::sleep(Duration::from_millis(4)).await;
    }
    Ok(())
}

fn character_to_keysym(character: char) -> i32 {
    match character {
        '\n' | '\r' => 0xff0d,
        '\t' => 0xff09,
        '\u{8}' => 0xff08,
        value if value as u32 <= 0xff => value as i32,
        value => (0x0100_0000 | value as u32) as i32,
    }
}

/// KDE StatusNotifierItem 托盘实现，菜单动作只投递给应用主线程。
struct DevToolsTray {
    proxy: EventLoopProxy<UserEvent>,
    language: LanguageMode,
}

impl ksni::Tray for DevToolsTray {
    fn id(&self) -> String {
        "devtools-workerd".to_owned()
    }

    fn title(&self) -> String {
        "DevTools".to_owned()
    }

    fn icon_name(&self) -> String {
        "applications-development".to_owned()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.proxy.send_event(UserEvent::OpenLauncher);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        let labels = tray_labels(self.language);
        vec![
            StandardItem {
                label: labels.settings.to_owned(),
                icon_name: "configure".to_owned(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.proxy.send_event(UserEvent::OpenSettings);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: labels.restart.to_owned(),
                icon_name: "system-reboot".to_owned(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.proxy.send_event(UserEvent::Restart);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: labels.quit.to_owned(),
                icon_name: "application-exit".to_owned(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.proxy.send_event(UserEvent::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub struct TrayManager {
    proxy: EventLoopProxy<UserEvent>,
    handle: Option<Handle<DevToolsTray>>,
}

impl TrayManager {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        Self {
            proxy,
            handle: None,
        }
    }

    pub fn set_visible(&mut self, visible: bool, language: LanguageMode) -> Result<(), String> {
        let language = resolve_language(language);
        if visible && self.handle.is_none() {
            let tray = DevToolsTray {
                proxy: self.proxy.clone(),
                language,
            };
            self.handle = Some(tray.spawn().map_err(|error| error.to_string())?);
        } else if visible {
            if let Some(handle) = &self.handle {
                let _ = handle.update(|tray| tray.language = language);
            }
        } else {
            self.shutdown();
        }
        Ok(())
    }

    pub fn shutdown(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.shutdown();
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct TrayLabels {
    settings: &'static str,
    restart: &'static str,
    quit: &'static str,
}

fn resolve_language(language: LanguageMode) -> LanguageMode {
    if language != LanguageMode::System {
        return language;
    }
    system_language()
}

/// 返回当前 Linux 桌面会话对应的受支持界面语言。
pub fn system_language() -> LanguageMode {
    language_from_environment(|name| std::env::var(name).ok())
}

fn language_from_environment(mut value_of: impl FnMut(&str) -> Option<String>) -> LanguageMode {
    ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .find_map(|name| value_of(name).filter(|value| !value.is_empty()))
        .map_or(LanguageMode::English, |locale| {
            LanguageMode::from_locale(&locale)
        })
}

fn placeholder(language: LanguageMode) -> &'static str {
    match resolve_language(language) {
        LanguageMode::SimplifiedChinese => "输入内容，Enter 回填，↑↓ 历史",
        LanguageMode::TraditionalChinese => "輸入內容，Enter 回填，↑↓ 歷史",
        LanguageMode::System | LanguageMode::English => {
            "Type text, Enter to insert, ↑↓ for history"
        }
    }
}

fn tray_labels(language: LanguageMode) -> TrayLabels {
    match language {
        LanguageMode::SimplifiedChinese => TrayLabels {
            settings: "设置",
            restart: "重启",
            quit: "退出",
        },
        LanguageMode::TraditionalChinese => TrayLabels {
            settings: "設定",
            restart: "重新啟動",
            quit: "結束",
        },
        LanguageMode::System | LanguageMode::English => TrayLabels {
            settings: "Settings",
            restart: "Restart",
            quit: "Quit",
        },
    }
}

struct WorkerService {
    proxy: EventLoopProxy<UserEvent>,
    registry: Arc<ToolRegistry>,
    webview_ready: Arc<AtomicBool>,
}

#[zbus::interface(name = "org.loveyu.DevTools")]
impl WorkerService {
    fn OpenLauncher(&self) -> zbus::fdo::Result<()> {
        send_event(&self.proxy, UserEvent::OpenLauncher)
    }

    fn OpenQuickInput(&self) -> zbus::fdo::Result<()> {
        send_event(&self.proxy, UserEvent::OpenQuickInput)
    }

    fn OpenTool(&self, tool: &str, payload: &str) -> zbus::fdo::Result<()> {
        let event = tool_event(&self.registry, tool, payload).map_err(zbus::fdo::Error::Failed)?;
        send_event(&self.proxy, event)
    }

    fn OpenSettings(&self) -> zbus::fdo::Result<()> {
        send_event(&self.proxy, UserEvent::OpenSettings)
    }

    fn Restart(&self) -> zbus::fdo::Result<()> {
        send_event(&self.proxy, UserEvent::Restart)
    }

    fn Quit(&self) -> zbus::fdo::Result<()> {
        send_event(&self.proxy, UserEvent::Quit)
    }

    fn IsWebViewReady(&self) -> bool {
        self.webview_ready.load(Ordering::Acquire)
    }
}

fn send_event(proxy: &EventLoopProxy<UserEvent>, event: UserEvent) -> zbus::fdo::Result<()> {
    proxy
        .send_event(event)
        .map_err(|_| zbus::fdo::Error::Failed("worker event loop is closed".to_owned()))
}

pub struct IpcGuard {
    _connection: Connection,
}

pub fn start_ipc(
    proxy: EventLoopProxy<UserEvent>,
    registry: Arc<ToolRegistry>,
    webview_ready: Arc<AtomicBool>,
) -> Result<IpcGuard, Box<dyn std::error::Error>> {
    let service = WorkerService {
        proxy,
        registry,
        webview_ready,
    };
    let connection = ConnectionBuilder::session()?
        .name(WORKER_SERVICE_NAME)?
        .serve_at(WORKER_OBJECT_PATH, service)?
        .build()?;
    Ok(IpcGuard {
        _connection: connection,
    })
}

pub fn request_existing_worker(method: &str) -> bool {
    let request = || -> zbus::Result<()> {
        let connection = Connection::session()?;
        let proxy = Proxy::new(
            &connection,
            WORKER_SERVICE_NAME,
            WORKER_OBJECT_PATH,
            WORKER_INTERFACE,
        )?;
        proxy.call::<_, _, ()>(method, &())
    };
    request().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_supported_system_locales() {
        assert_eq!(
            LanguageMode::from_locale("zh_CN.UTF-8"),
            LanguageMode::SimplifiedChinese
        );
        assert_eq!(
            LanguageMode::from_locale("zh_Hant_HK.UTF-8"),
            LanguageMode::TraditionalChinese
        );
        assert_eq!(
            LanguageMode::from_locale("en_GB.UTF-8"),
            LanguageMode::English
        );
    }

    #[test]
    fn language_environment_prefers_language_over_c_locale() {
        let language = language_from_environment(|name| match name {
            "LANGUAGE" => Some("zh_CN:zh".to_owned()),
            "LC_ALL" => Some("C.UTF-8".to_owned()),
            _ => None,
        });

        assert_eq!(language, LanguageMode::SimplifiedChinese);
    }

    #[test]
    fn language_environment_falls_back_to_english() {
        assert_eq!(language_from_environment(|_| None), LanguageMode::English);
    }

    #[test]
    fn formats_default_shortcuts_for_the_wayland_portal() {
        assert_eq!(
            hotkey_to_wayland_trigger("Ctrl+Alt+Space".parse().unwrap()).as_deref(),
            Some("CTRL+ALT+space")
        );
        assert_eq!(
            hotkey_to_wayland_trigger("Ctrl+Alt+KeyI".parse().unwrap()).as_deref(),
            Some("CTRL+ALT+i")
        );
    }

    #[test]
    fn only_dispatches_events_from_the_active_portal_session() {
        assert_eq!(
            portal_shortcut_event_id(Some("/session/current"), "/session/current", "42"),
            Some(42)
        );
        assert_eq!(
            portal_shortcut_event_id(Some("/session/current"), "/session/old", "42"),
            None
        );
        assert_eq!(
            portal_shortcut_event_id(Some("/session/current"), "/session/current", "invalid"),
            None
        );
    }

    #[test]
    fn maps_text_characters_to_portal_keysyms() {
        assert_eq!(character_to_keysym('a'), 0x0061);
        assert_eq!(character_to_keysym('A'), 0x0041);
        assert_eq!(character_to_keysym('é'), 0x00e9);
        assert_eq!(character_to_keysym('\n'), 0xff0d);
        assert_eq!(character_to_keysym('\t'), 0xff09);
        assert_eq!(character_to_keysym('中'), 0x0100_0000 | '中' as i32);
    }

    #[test]
    fn supplies_all_tray_menu_translations() {
        assert_eq!(
            tray_labels(LanguageMode::SimplifiedChinese).settings,
            "设置"
        );
        assert_eq!(
            tray_labels(LanguageMode::TraditionalChinese).settings,
            "設定"
        );
        assert_eq!(tray_labels(LanguageMode::English).settings, "Settings");
    }

    #[test]
    fn desktop_exec_escapes_special_characters() {
        let executable = Path::new("/tmp/a\\b\"$`/worker");
        assert_eq!(
            quote_desktop_exec(executable),
            "\"/tmp/a\\\\b\\\"\\$\\`/worker\""
        );
    }

    #[test]
    fn normalizes_portal_color_channels() {
        assert_eq!(normalized_channel(-1.0), 0);
        assert_eq!(normalized_channel(0.5), 128);
        assert_eq!(normalized_channel(2.0), 255);
    }
}

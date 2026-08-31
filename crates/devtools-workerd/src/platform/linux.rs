use std::cell::{Cell, RefCell};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use ashpd::desktop::remote_desktop::{DeviceType, KeyState, RemoteDesktop, SelectDevicesOptions};
use ashpd::desktop::{PersistMode, Session};
use devtools_core::{
    LanguageMode, Settings, WORKER_INTERFACE, WORKER_OBJECT_PATH, WORKER_SERVICE_NAME,
};
use gtk::gdk;
use gtk::prelude::*;
use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::StandardItem;
use tao::event_loop::EventLoopProxy;
use tao::platform::unix::WindowExtUnix;
use tao::window::Window;
use wry::{WebView, WebViewBuilder, WebViewBuilderExtUnix};
use zbus::blocking::{Connection, ConnectionBuilder, Proxy};

use crate::ipc::tool_event;
use crate::registry::ToolRegistry;
use crate::UserEvent;

pub fn configure_webview<'a>(
    builder: WebViewBuilder<'a>,
    html: &'static str,
) -> WebViewBuilder<'a> {
    builder.with_html(html)
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

pub fn ensure_display_available() -> Result<(), &'static str> {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some() {
        Ok(())
    } else {
        Err("neither WAYLAND_DISPLAY nor DISPLAY is available")
    }
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
        window.set_resizable(false);
        window.set_position(gtk::WindowPosition::Mouse);

        let entry = gtk::Entry::new();
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
    portal_sender: Option<mpsc::Sender<()>>,
}

impl QuickInputInjector {
    pub fn new() -> Self {
        let portal_sender = is_wayland().then(spawn_portal_paster);
        Self { portal_sender }
    }

    pub fn inject(&self, text: &str, target_window: Option<&str>) -> Result<(), String> {
        copy_text(text);
        if let Some(target) = target_window {
            return xdotool_type(target, text);
        }
        self.portal_sender
            .as_ref()
            .ok_or_else(|| "no cross-application input backend is available".to_owned())?
            .send(())
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

fn spawn_portal_paster() -> mpsc::Sender<()> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => {
                eprintln!("devtools-workerd: failed to start portal runtime: {error}");
                return;
            }
        };
        let mut session = None;
        while receiver.recv().is_ok() {
            thread::sleep(Duration::from_millis(120));
            let result = runtime.block_on(async {
                if session.is_none() {
                    session = Some(create_portal_session().await?);
                }
                portal_paste(session.as_ref().expect("session was initialized")).await
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

async fn portal_paste(session: &PortalSession) -> Result<(), String> {
    const CONTROL_L: i32 = 0xffe3;
    const LOWER_V: i32 = 0x0076;
    for (keysym, state) in [
        (CONTROL_L, KeyState::Pressed),
        (LOWER_V, KeyState::Pressed),
        (LOWER_V, KeyState::Released),
        (CONTROL_L, KeyState::Released),
    ] {
        session
            .proxy
            .notify_keyboard_keysym(&session.session, keysym, state, Default::default())
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(())
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
    let locale = ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()));
    language_for_locale(locale.as_deref())
}

fn language_for_locale(locale: Option<&str>) -> LanguageMode {
    let locale = locale.unwrap_or_default().to_ascii_lowercase();
    if locale.starts_with("zh") {
        return if ["tw", "hk", "mo", "hant"]
            .into_iter()
            .any(|marker| locale.contains(marker))
        {
            LanguageMode::TraditionalChinese
        } else {
            LanguageMode::SimplifiedChinese
        };
    }
    LanguageMode::English
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
            language_for_locale(Some("zh_CN.UTF-8")),
            LanguageMode::SimplifiedChinese
        );
        assert_eq!(
            language_for_locale(Some("zh_Hant_HK.UTF-8")),
            LanguageMode::TraditionalChinese
        );
        assert_eq!(
            language_for_locale(Some("en_GB.UTF-8")),
            LanguageMode::English
        );
        assert_eq!(language_for_locale(None), LanguageMode::English);
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
}

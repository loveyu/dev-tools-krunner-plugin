use std::borrow::Cow;
use std::io;
use std::mem::{size_of, transmute};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::ptr::{null, null_mut};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use devtools_core::{LanguageMode, Settings};
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};
use tao::event_loop::{EventLoopBuilder, EventLoopProxy};
use tao::window::Window;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows_sys::Win32::Foundation::{GetLastError, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
use windows_sys::Win32::Graphics::Gdi::{
    GetDC, GetMonitorInfoW, GetPixel, MonitorFromPoint, ReleaseDC, MONITORINFO,
    MONITOR_DEFAULTTONEAREST,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SetFocus, VK_DOWN, VK_ESCAPE, VK_LBUTTON, VK_RETURN, VK_UP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, CreateWindowExW, DefWindowProcW, DestroyWindow, GetCursorPos, GetParent,
    GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, IsWindowVisible, MoveWindow,
    RegisterClassW, SendMessageW, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos,
    SetWindowTextW, ShowWindow, CS_HREDRAW, CS_VREDRAW, ES_AUTOHSCROLL, GWLP_USERDATA,
    GWLP_WNDPROC, HWND_TOPMOST, SWP_NOACTIVATE, SW_HIDE, SW_SHOW, WM_KEYDOWN, WM_NCDESTROY,
    WM_SIZE, WNDCLASSW, WNDPROC, WS_BORDER, WS_CHILD, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    WS_VISIBLE,
};
use wry::{WebView, WebViewBuilder};

use crate::registry::ToolRegistry;
use crate::ColorPickResult;
use crate::UserEvent;

pub fn configure_webview<'a>(
    builder: WebViewBuilder<'a>,
    html: &'static str,
    development_url: Option<&str>,
) -> WebViewBuilder<'a> {
    if let Some(url) = development_url {
        return builder.with_url(url);
    }
    builder
        .with_custom_protocol("devtools".to_owned(), move |_id, _request| {
            wry::http::Response::builder()
                .header("Content-Type", "text/html; charset=utf-8")
                .body(Cow::Borrowed(html.as_bytes()))
                .expect("static WebView response should be valid")
        })
        .with_url("devtools://localhost/index.html")
}

pub fn build_webview(
    builder: WebViewBuilder<'_>,
    window: &Window,
) -> Result<WebView, Box<dyn std::error::Error>> {
    Ok(builder.build(window)?)
}

pub fn configure_event_loop(_builder: &mut EventLoopBuilder<UserEvent>) {}

pub fn copy_text(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|error| error.to_string())?;
    clipboard.set_text(text).map_err(|error| error.to_string())
}

/// 用系统默认浏览器打开外部 http(s) 链接。
/// WebView 以自定义协议加载，无法自行打开新窗口，必须交给 Shell 打开。
pub fn open_external_url(url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|error| format!("invalid url: {error}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(format!("refused to open non-http url: {url}"));
    }
    Command::new("explorer")
        .arg(url)
        .spawn()
        .map(reap_in_background)
        .map_err(|error| format!("failed to open url with explorer: {error}"))
}

/// 后台等待子进程退出并回收：worker 是长驻进程，spawn 后不管会随使用累积僵尸。
fn reap_in_background(mut child: std::process::Child) {
    thread::spawn(move || {
        let _ = child.wait();
    });
}

pub fn pick_metadata_path() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Choose an image, video, or media file")
        .pick_file()
}

/// Windows 使用桌面 DC 读取全局光标像素，可自然跨越所有已连接显示器。
pub fn start_screen_color_picker(request_id: String, proxy: EventLoopProxy<UserEvent>) {
    thread::spawn(move || {
        let result = pick_windows_color(&request_id);
        let _ = proxy.send_event(UserEvent::ColorPickingFinished(result));
    });
}

fn pick_windows_color(request_id: &str) -> ColorPickResult {
    while unsafe { GetAsyncKeyState(VK_LBUTTON as i32) } < 0 {
        thread::sleep(Duration::from_millis(10));
    }
    loop {
        if unsafe { GetAsyncKeyState(VK_ESCAPE as i32) } < 0 {
            return ColorPickResult::cancelled(request_id.to_owned());
        }
        if unsafe { GetAsyncKeyState(VK_LBUTTON as i32) } < 0 {
            let mut point = POINT::default();
            if unsafe { GetCursorPos(&mut point) } == 0 {
                return ColorPickResult::error(request_id.to_owned(), "GetCursorPos failed");
            }
            let desktop = unsafe { GetDC(null_mut()) };
            if desktop.is_null() {
                return ColorPickResult::error(request_id.to_owned(), "GetDC failed");
            }
            let color = unsafe { GetPixel(desktop, point.x, point.y) };
            unsafe { ReleaseDC(null_mut(), desktop) };
            if color == u32::MAX {
                return ColorPickResult::error(request_id.to_owned(), "GetPixel failed");
            }
            return ColorPickResult::success(
                request_id.to_owned(),
                (color & 0xff) as u8,
                ((color >> 8) & 0xff) as u8,
                ((color >> 16) & 0xff) as u8,
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
}

pub fn ensure_display_available() -> Result<(), &'static str> {
    Ok(())
}

/// Windows 全局快捷键后端；注册操作仍由 Win32 消息循环所在主线程发起。
pub struct GlobalShortcutBackend {
    manager: GlobalHotKeyManager,
    registered: Vec<HotKey>,
}

impl GlobalShortcutBackend {
    pub fn new(
        handler: impl Fn(GlobalHotKeyEvent) + Send + Sync + 'static,
    ) -> Result<Self, String> {
        GlobalHotKeyEvent::set_event_handler(Some(handler));
        Ok(Self {
            manager: GlobalHotKeyManager::new().map_err(|error| error.to_string())?,
            registered: Vec::new(),
        })
    }

    pub fn replace(&mut self, next: &[HotKey]) -> Result<(), String> {
        if next == self.registered {
            return Ok(());
        }
        let previous = self.registered.clone();
        if !previous.is_empty() {
            self.manager
                .unregister_all(&previous)
                .map_err(|error| error.to_string())?;
        }
        if !next.is_empty() {
            if let Err(error) = self.manager.register_all(next) {
                if !previous.is_empty() {
                    let _ = self.manager.register_all(&previous);
                }
                return Err(error.to_string());
            }
        }
        self.registered = next.to_vec();
        Ok(())
    }
}

pub fn restart(executable: &Path) {
    match Command::new(executable).spawn() {
        Ok(_) => std::process::exit(0),
        Err(error) => eprintln!("devtools-workerd: failed to restart: {error}"),
    }
}

pub fn config_root_from_environment() -> io::Result<PathBuf> {
    std::env::var_os("APPDATA")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "APPDATA is not set"))
}

pub fn data_root_from_environment() -> io::Result<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA is not set"))
}

pub fn autostart_path(config_root: &Path) -> PathBuf {
    config_root
        .join("Microsoft/Windows/Start Menu/Programs/Startup")
        .join("DevTools Worker.cmd")
}

pub fn autostart_entry(executable: &Path) -> String {
    let executable = executable.to_string_lossy().replace('%', "%%");
    format!("@start \"\" \"{executable}\"\r\n")
}

const CLASS_NAME: &[u16] = &[
    b'D' as u16,
    b'e' as u16,
    b'v' as u16,
    b'T' as u16,
    b'o' as u16,
    b'o' as u16,
    b'l' as u16,
    b's' as u16,
    b'Q' as u16,
    b'u' as u16,
    b'i' as u16,
    b'c' as u16,
    b'k' as u16,
    0,
];
const EM_SETCUEBANNER: u32 = 0x1501;

struct WindowState {
    parent: HWND,
    edit: HWND,
    old_edit_proc: WNDPROC,
    proxy: EventLoopProxy<UserEvent>,
    history: Vec<String>,
    history_cursor: usize,
}

/// Windows 原生快速输入窗口使用 Win32 Edit，不创建 WebView2。
pub struct QuickInputWindow {
    state: *mut WindowState,
}

impl QuickInputWindow {
    pub fn new(
        proxy: EventLoopProxy<UserEvent>,
        initial_history: Vec<String>,
    ) -> Result<Self, String> {
        unsafe {
            let instance = GetModuleHandleW(null());
            let class = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(parent_window_proc),
                hInstance: instance,
                lpszClassName: CLASS_NAME.as_ptr(),
                ..Default::default()
            };
            RegisterClassW(&class);
            let parent = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                CLASS_NAME.as_ptr(),
                CLASS_NAME.as_ptr(),
                WS_POPUP | WS_BORDER,
                0,
                0,
                560,
                56,
                null_mut(),
                null_mut(),
                instance,
                null(),
            );
            if parent.is_null() {
                return Err(format!("CreateWindowExW failed: {}", GetLastError()));
            }
            let edit_class = wide("EDIT");
            let edit = CreateWindowExW(
                0,
                edit_class.as_ptr(),
                null(),
                WS_CHILD | WS_VISIBLE | ES_AUTOHSCROLL as u32 | WS_BORDER,
                8,
                8,
                544,
                40,
                parent,
                null_mut(),
                instance,
                null(),
            );
            if edit.is_null() {
                DestroyWindow(parent);
                return Err(format!(
                    "creating quick input edit failed: {}",
                    GetLastError()
                ));
            }
            let old_edit_proc = transmute::<isize, WNDPROC>(SetWindowLongPtrW(
                edit,
                GWLP_WNDPROC,
                edit_window_proc as *const () as isize,
            ));
            let state = Box::into_raw(Box::new(WindowState {
                parent,
                edit,
                old_edit_proc,
                proxy,
                history_cursor: initial_history.len(),
                history: initial_history,
            }));
            SetWindowLongPtrW(parent, GWLP_USERDATA, state as isize);
            SetWindowLongPtrW(edit, GWLP_USERDATA, state as isize);
            Ok(Self { state })
        }
    }

    pub fn show(&self, settings: &Settings) {
        unsafe {
            let state = &mut *self.state;
            if IsWindowVisible(state.parent) != 0 {
                SetForegroundWindow(state.parent);
                SetFocus(state.edit);
                return;
            }
            state.history_cursor = state.history.len();
            SetWindowTextW(state.edit, wide("").as_ptr());
            let cue = wide(placeholder(settings.language));
            SendMessageW(state.edit, EM_SETCUEBANNER, 1, cue.as_ptr() as LPARAM);
            let (x, y, width, height) = bounded_geometry(settings);
            SetWindowPos(
                state.parent,
                HWND_TOPMOST,
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE,
            );
            MoveWindow(state.edit, 8, 8, width - 16, height - 16, 1);
            ShowWindow(state.parent, SW_SHOW);
            SetForegroundWindow(state.parent);
            SetFocus(state.edit);
        }
    }
}

impl Drop for QuickInputWindow {
    fn drop(&mut self) {
        unsafe {
            if !self.state.is_null() {
                let state = Box::from_raw(self.state);
                DestroyWindow(state.parent);
                self.state = null_mut();
            }
        }
    }
}

unsafe extern "system" fn parent_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_SIZE {
        let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
        if !state.is_null() {
            let width = (lparam as u32 & 0xffff) as i32;
            let height = ((lparam as u32 >> 16) & 0xffff) as i32;
            MoveWindow((*state).edit, 8, 8, width - 16, height - 16, 1);
        }
    }
    DefWindowProcW(hwnd, message, wparam, lparam)
}

unsafe extern "system" fn edit_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let mut state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
    if state.is_null() {
        state = GetWindowLongPtrW(GetParent(hwnd), GWLP_USERDATA) as *mut WindowState;
    }
    if state.is_null() {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    let state = &mut *state;
    if message == WM_KEYDOWN {
        match wparam as u16 {
            VK_RETURN => {
                let text = window_text(hwnd);
                ShowWindow(state.parent, SW_HIDE);
                SetWindowTextW(hwnd, wide("").as_ptr());
                if !text.is_empty() {
                    state.history.push(text.clone());
                    state.history_cursor = state.history.len();
                    let result = state
                        .proxy
                        .send_event(UserEvent::QuickInputSubmitted { text });
                    if result.is_err() {
                        eprintln!("devtools-workerd: failed to queue native quick input");
                    }
                }
                return 0;
            }
            VK_ESCAPE => {
                ShowWindow(state.parent, SW_HIDE);
                return 0;
            }
            VK_UP => {
                if !state.history.is_empty() {
                    state.history_cursor = state.history_cursor.saturating_sub(1);
                    SetWindowTextW(hwnd, wide(&state.history[state.history_cursor]).as_ptr());
                }
                return 0;
            }
            VK_DOWN => {
                if !state.history.is_empty() {
                    state.history_cursor = (state.history_cursor + 1).min(state.history.len());
                    let text = state
                        .history
                        .get(state.history_cursor)
                        .map_or("", String::as_str);
                    SetWindowTextW(hwnd, wide(text).as_ptr());
                }
                return 0;
            }
            _ => {}
        }
    }
    if message == WM_NCDESTROY {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
    }
    CallWindowProcW(state.old_edit_proc, hwnd, message, wparam, lparam)
}

fn bounded_geometry(settings: &Settings) -> (i32, i32, i32, i32) {
    unsafe {
        let mut cursor = POINT::default();
        GetCursorPos(&mut cursor);
        let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        GetMonitorInfoW(monitor, &mut info);
        let work = info.rcWork;
        let width = (settings.quick_input_width.clamp(240, 1600) as i32)
            .min((work.right - work.left - 24).max(240));
        let height = (settings.quick_input_height.clamp(40, 240) as i32)
            .min((work.bottom - work.top - 24).max(40));
        let x = cursor.x.min(work.right - width - 12).max(work.left + 12);
        let y = (cursor.y + 16)
            .min(work.bottom - height - 12)
            .max(work.top + 12);
        (x, y, width, height)
    }
}

fn window_text(hwnd: HWND) -> String {
    unsafe {
        let length = GetWindowTextLengthW(hwnd);
        let mut value = vec![0u16; length as usize + 1];
        let copied = GetWindowTextW(hwnd, value.as_mut_ptr(), value.len() as i32);
        String::from_utf16_lossy(&value[..copied as usize])
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

pub struct TrayManager {
    tray: Option<TrayIcon>,
    settings: MenuItem,
    restart: MenuItem,
    quit: MenuItem,
    menu: Menu,
}

impl TrayManager {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        let settings = MenuItem::with_id("settings", "Settings", true, None);
        let restart = MenuItem::with_id("restart", "Restart", true, None);
        let quit = MenuItem::with_id("quit", "Quit", true, None);
        let menu = Menu::with_items(&[&settings, &restart, &quit])
            .expect("static tray menu should be valid");

        let click_proxy = proxy.clone();
        TrayIconEvent::set_event_handler(Some(move |event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                let _ = click_proxy.send_event(UserEvent::OpenLauncher);
            }
        }));

        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let action = match event.id.0.as_str() {
                "settings" => Some(UserEvent::OpenSettings),
                "restart" => Some(UserEvent::Restart),
                "quit" => Some(UserEvent::Quit),
                _ => None,
            };
            if let Some(action) = action {
                let _ = proxy.send_event(action);
            }
        }));

        Self {
            tray: None,
            settings,
            restart,
            quit,
            menu,
        }
    }

    pub fn set_visible(&mut self, visible: bool, language: LanguageMode) -> Result<(), String> {
        self.update_labels(language);
        if visible && self.tray.is_none() {
            self.tray = Some(
                TrayIconBuilder::new()
                    .with_tooltip("DevTools")
                    .with_icon(create_icon()?)
                    .with_menu(Box::new(self.menu.clone()))
                    .with_menu_on_left_click(false)
                    .build()
                    .map_err(|error| error.to_string())?,
            );
        } else if !visible {
            self.shutdown();
        }
        Ok(())
    }

    pub fn shutdown(&mut self) {
        self.tray = None;
    }

    fn update_labels(&self, language: LanguageMode) {
        let labels = match resolve_language(language) {
            LanguageMode::SimplifiedChinese => ("设置", "重启", "退出"),
            LanguageMode::TraditionalChinese => ("設定", "重新啟動", "結束"),
            LanguageMode::System | LanguageMode::English => ("Settings", "Restart", "Quit"),
        };
        self.settings.set_text(labels.0);
        self.restart.set_text(labels.1);
        self.quit.set_text(labels.2);
    }
}

fn resolve_language(language: LanguageMode) -> LanguageMode {
    if language != LanguageMode::System {
        return language;
    }
    system_language()
}

/// 返回当前 Windows 用户对应的受支持界面语言。
pub fn system_language() -> LanguageMode {
    let mut buffer = [0u16; 85];
    let length = unsafe { GetUserDefaultLocaleName(buffer.as_mut_ptr(), buffer.len() as i32) };
    if length <= 1 {
        return LanguageMode::English;
    }
    let locale = String::from_utf16_lossy(&buffer[..length as usize - 1]);
    LanguageMode::from_locale(&locale)
}

fn placeholder(language: LanguageMode) -> &'static str {
    match resolve_language(language) {
        LanguageMode::SimplifiedChinese => "输入内容，Enter 复制并关闭，↑↓ 历史",
        LanguageMode::TraditionalChinese => "輸入內容，Enter 複製並關閉，↑↓ 歷史",
        LanguageMode::System | LanguageMode::English => {
            "Type text, Enter to copy and close, ↑↓ for history"
        }
    }
}

fn create_icon() -> Result<Icon, String> {
    Icon::from_rgba(
        crate::app_icon::rgba(),
        crate::app_icon::ICON_SIZE,
        crate::app_icon::ICON_SIZE,
    )
    .map_err(|error| error.to_string())
}

pub struct IpcGuard;

pub fn start_ipc(
    _proxy: EventLoopProxy<UserEvent>,
    registry: Arc<ToolRegistry>,
    _webview_ready: Arc<AtomicBool>,
) -> Result<IpcGuard, Box<dyn std::error::Error>> {
    // 保留与 Linux 相同的工具路由契约，后续可直接接入 Windows 本地 IPC。
    let _tool_router = (registry, crate::ipc::tool_event);
    Ok(IpcGuard)
}

pub fn request_existing_worker(_method: &str) -> bool {
    false
}

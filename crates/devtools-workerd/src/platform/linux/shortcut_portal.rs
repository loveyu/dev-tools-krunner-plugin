//! Linux 全局快捷键后端：Wayland 走 XDG GlobalShortcuts portal，X11 复用 global-hotkey。

use std::str::FromStr;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use ashpd::desktop::global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut};
use ashpd::desktop::{CreateSessionOptions, Session};
use ashpd::AppID;
use futures::StreamExt;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use tokio::sync::mpsc as tokio_mpsc;

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

/// portal 线程启动（连接 D-Bus、建代理）的等待上限；挂死时避免启动流程永久卡住。
const PORTAL_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
/// 绑定快捷键的等待上限；需覆盖 KDE 首次绑定的系统授权对话框（用户手动确认）。
const PORTAL_REPLY_TIMEOUT: Duration = Duration::from_secs(60);

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
            .recv_timeout(PORTAL_STARTUP_TIMEOUT)
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
        // 必须带超时：KDE 首次绑定会弹系统授权对话框，BindShortcuts 直到用户响应
        // 才返回；若 xdg-desktop-portal 挂死，无限等待会冻结事件循环线程（整个 UI）。
        // 超时视为注册失败；portal 线程可能稍后才完成绑定，但迟到的结果会在下次
        // replace 重建会话时被收敛，不会累积。
        result
            .recv_timeout(PORTAL_REPLY_TIMEOUT)
            .map_err(|_| "timed out waiting for the global shortcuts portal response".to_owned())?
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

#[cfg(test)]
mod tests {
    use super::*;

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
}

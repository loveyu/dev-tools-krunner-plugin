use global_hotkey::hotkey::HotKey;
use global_hotkey::HotKeyState;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use tao::event_loop::EventLoopProxy;

use crate::{platform, UserEvent};
use devtools_core::Settings;

const NO_HOTKEY: u32 = u32::MAX;

/// 提交给快捷键 worker 线程的命令。
enum ShortcutCommand {
    Apply {
        settings: Settings,
        on_done: Box<dyn FnOnce(Result<(), String>) + Send>,
    },
    Shutdown,
}

/// 平台后端持有裸指针句柄（X11 Display / Win32 句柄），默认不跨线程。
/// 这里把后端整体移交给专职 worker 线程、此后只在该线程访问，move 一次
/// 即弃用原线程引用，符合独占所有权跨线程转移的安全前提。
struct SendShortcutBackend(platform::GlobalShortcutBackend);

unsafe impl Send for SendShortcutBackend {}

impl std::ops::Deref for SendShortcutBackend {
    type Target = platform::GlobalShortcutBackend;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for SendShortcutBackend {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// 跨平台全局快捷键管理器；Wayland 通过 XDG GlobalShortcuts portal 注册。
///
/// 注册在专职线程执行：portal 绑定可能弹出系统授权对话框并长时间等待用户
/// 响应（上限 60s），绝不能阻塞事件循环线程，否则整个 UI 冻结。
pub struct ShortcutManager {
    sender: mpsc::Sender<ShortcutCommand>,
}

impl ShortcutManager {
    /// 提前设置 global_hotkey 库依赖的环境变量。
    /// 必须在 Application 启动任何后台线程之前调用：多线程进程内调用
    /// `std::env::set_var` 与并发 getenv 构成未定义行为（Rust 2024 起为 unsafe）。
    pub fn prepare_environment() {
        std::env::set_var("GLOBAL_HOTKEY_APP_ID", "org.loveyu.DevTools");
    }

    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Result<Self, String> {
        // 事件 id 到动作的映射槽位；由后端事件回调与 worker 线程共享，
        // 生命周期跟随闭包与线程，无需保存在句柄上。
        let launcher_id = Arc::new(AtomicU32::new(NO_HOTKEY));
        let quick_input_id = Arc::new(AtomicU32::new(NO_HOTKEY));
        let event_launcher_id = Arc::clone(&launcher_id);
        let event_quick_input_id = Arc::clone(&quick_input_id);
        let manager = SendShortcutBackend(platform::GlobalShortcutBackend::new(move |event| {
            if event.state() == HotKeyState::Pressed {
                if event.id() == event_launcher_id.load(Ordering::Acquire) {
                    let _ = proxy.send_event(UserEvent::OpenLauncher);
                } else if event.id() == event_quick_input_id.load(Ordering::Acquire) {
                    let _ = proxy.send_event(UserEvent::OpenQuickInput);
                }
            }
        })?);
        let (sender, receiver) = mpsc::channel::<ShortcutCommand>();
        let worker_launcher_id = Arc::clone(&launcher_id);
        let worker_quick_input_id = Arc::clone(&quick_input_id);
        thread::spawn(move || {
            // 闭包按字段精确捕获：直接写 manager.0 会把未包装的平台后端当作
            // 捕获物、绕过 SendShortcutBackend 的 Send 声明，因此统一经
            // DerefMut 访问（&mut *manager 保证捕获整个包装）。
            // registered 缓存与后端一起归 worker 线程独占，命令天然串行。
            let mut manager = manager;
            let mut registered = Vec::new();
            while let Ok(command) = receiver.recv() {
                match command {
                    ShortcutCommand::Apply { settings, on_done } => {
                        let result = apply_registration(
                            &mut manager,
                            &mut registered,
                            &worker_launcher_id,
                            &worker_quick_input_id,
                            &settings,
                        );
                        on_done(result);
                    }
                    ShortcutCommand::Shutdown => break,
                }
            }
        });
        Ok(Self { sender })
    }

    /// 校验设置中的快捷键定义；不触碰注册后端，可在事件循环线程同步调用。
    pub fn validate(&self, settings: &Settings) -> Result<(), String> {
        validate_hotkeys(settings).map(|_| ())
    }

    /// 排队一次注册并立即返回；结果经 on_done 回调（通常转发为用户事件）。
    pub fn apply_async(
        &self,
        settings: Settings,
        on_done: impl FnOnce(Result<(), String>) + Send + 'static,
    ) -> Result<(), String> {
        self.sender
            .send(ShortcutCommand::Apply {
                settings,
                on_done: Box::new(on_done),
            })
            .map_err(|_| "global shortcut worker is unavailable".to_owned())
    }
}

impl Drop for ShortcutManager {
    fn drop(&mut self) {
        let _ = self.sender.send(ShortcutCommand::Shutdown);
    }
}

/// 校验快捷键定义并返回解析结果；与注册解耦以便事件循环线程提前反馈输入错误。
fn validate_hotkeys(settings: &Settings) -> Result<(Option<HotKey>, Option<HotKey>), String> {
    let launcher = settings
        .global_shortcut_enabled
        .then(|| parse_shortcut(&settings.global_shortcut))
        .transpose()?;
    let quick_input = settings
        .quick_input_enabled
        .then(|| parse_shortcut(&settings.quick_input_shortcut))
        .transpose()?;
    if launcher.is_some() && launcher == quick_input {
        return Err("launcher and quick input shortcuts must be different".to_owned());
    }
    Ok((launcher, quick_input))
}

/// 在 worker 线程内执行注册并提交缓存。
fn apply_registration(
    manager: &mut platform::GlobalShortcutBackend,
    registered: &mut Vec<HotKey>,
    launcher_id: &AtomicU32,
    quick_input_id: &AtomicU32,
    settings: &Settings,
) -> Result<(), String> {
    let (launcher, quick_input) = validate_hotkeys(settings)?;
    let next = [launcher, quick_input]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if next == *registered {
        return Ok(());
    }

    match manager.replace(&next) {
        Ok(()) => {
            launcher_id.store(
                launcher.map_or(NO_HOTKEY, |value| value.id()),
                Ordering::Release,
            );
            quick_input_id.store(
                quick_input.map_or(NO_HOTKEY, |value| value.id()),
                Ordering::Release,
            );
            *registered = next;
            Ok(())
        }
        Err(error) => {
            // replace 失败（如 portal 等待超时）后，portal 线程可能迟到完成
            // 绑定，留下仍占用系统快捷键的幽灵会话——期间 UI 显示禁用但
            // 快捷键实际可用。清空已注册缓存，确保下一次 apply（含回滚）
            // 不走同值短路、强制 replace 重建会话，把幽灵会话收敛掉。
            registered.clear();
            Err(error)
        }
    }
}

fn parse_shortcut(value: &str) -> Result<HotKey, String> {
    let hotkey = value
        .trim()
        .parse::<HotKey>()
        .map_err(|error| error.to_string())?;
    if hotkey.mods.is_empty() {
        return Err("global shortcut must contain at least one modifier".to_owned());
    }
    Ok(hotkey)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_portable_shortcut_syntax() {
        assert!(parse_shortcut("Ctrl+Alt+Space").is_ok());
        assert!(parse_shortcut("Shift+KeyQ").is_ok());
    }

    #[test]
    fn rejects_empty_invalid_and_unmodified_shortcuts() {
        assert!(parse_shortcut("").is_err());
        assert!(parse_shortcut("Ctrl+not-a-key").is_err());
        assert_eq!(
            parse_shortcut("Space").expect_err("无修饰键必须拒绝"),
            "global shortcut must contain at least one modifier"
        );
    }

    fn settings_with(launcher: &str, quick_input: &str) -> Settings {
        Settings {
            global_shortcut_enabled: true,
            global_shortcut: launcher.to_owned(),
            quick_input_enabled: true,
            quick_input_shortcut: quick_input.to_owned(),
            ..Settings::default()
        }
    }

    #[test]
    fn validates_distinct_shortcuts() {
        assert!(validate_hotkeys(&settings_with("Ctrl+Alt+Space", "Ctrl+Alt+KeyI")).is_ok());
        // 两个开关指向同一组合必须拒绝。
        assert!(validate_hotkeys(&settings_with("Ctrl+Alt+Space", "Ctrl+Alt+Space")).is_err());
        // 非法键名或裸键（无修饰键）拒绝。
        assert!(validate_hotkeys(&settings_with("not-a-key", "Ctrl+Alt+KeyI")).is_err());
        assert!(validate_hotkeys(&settings_with("Space", "Ctrl+Alt+KeyI")).is_err());
    }
}

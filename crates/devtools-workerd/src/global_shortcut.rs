use global_hotkey::hotkey::HotKey;
use global_hotkey::HotKeyState;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tao::event_loop::EventLoopProxy;

use crate::{platform, UserEvent};
use devtools_core::Settings;

const NO_HOTKEY: u32 = u32::MAX;

/// 跨平台全局快捷键管理器；Wayland 通过 XDG GlobalShortcuts portal 注册。
pub struct ShortcutManager {
    manager: platform::GlobalShortcutBackend,
    registered: Vec<HotKey>,
    launcher_id: Arc<AtomicU32>,
    quick_input_id: Arc<AtomicU32>,
}

impl ShortcutManager {
    /// 提前设置 global_hotkey 库依赖的环境变量。
    /// 必须在 Application 启动任何后台线程之前调用：多线程进程内调用
    /// `std::env::set_var` 与并发 getenv 构成未定义行为（Rust 2024 起为 unsafe）。
    pub fn prepare_environment() {
        std::env::set_var("GLOBAL_HOTKEY_APP_ID", "org.loveyu.DevTools");
    }

    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Result<Self, String> {
        let launcher_id = Arc::new(AtomicU32::new(NO_HOTKEY));
        let quick_input_id = Arc::new(AtomicU32::new(NO_HOTKEY));
        let event_launcher_id = Arc::clone(&launcher_id);
        let event_quick_input_id = Arc::clone(&quick_input_id);
        let manager = platform::GlobalShortcutBackend::new(move |event| {
            if event.state() == HotKeyState::Pressed {
                if event.id() == event_launcher_id.load(Ordering::Acquire) {
                    let _ = proxy.send_event(UserEvent::OpenLauncher);
                } else if event.id() == event_quick_input_id.load(Ordering::Acquire) {
                    let _ = proxy.send_event(UserEvent::OpenQuickInput);
                }
            }
        })?;
        Ok(Self {
            manager,
            registered: Vec::new(),
            launcher_id,
            quick_input_id,
        })
    }

    pub fn apply(&mut self, settings: &Settings) -> Result<(), String> {
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
        let next = [launcher, quick_input]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        if next == self.registered {
            return Ok(());
        }

        match self.manager.replace(&next) {
            Ok(()) => {
                self.commit_registration(launcher, quick_input, next);
                Ok(())
            }
            Err(error) => {
                // replace 失败（如 portal 等待超时）后，portal 线程可能迟到完成
                // 绑定，留下仍占用系统快捷键的幽灵会话——期间 UI 显示禁用但
                // 快捷键实际可用。清空已注册缓存，确保下一次 apply（含回滚）
                // 不走同值短路、强制 replace 重建会话，把幽灵会话收敛掉。
                self.registered.clear();
                Err(error)
            }
        }
    }

    fn commit_registration(
        &mut self,
        launcher: Option<HotKey>,
        quick_input: Option<HotKey>,
        registered: Vec<HotKey>,
    ) {
        self.launcher_id.store(
            launcher.map_or(NO_HOTKEY, |value| value.id()),
            Ordering::Release,
        );
        self.quick_input_id.store(
            quick_input.map_or(NO_HOTKEY, |value| value.id()),
            Ordering::Release,
        );
        self.registered = registered;
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
}

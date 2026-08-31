use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tao::event_loop::EventLoopProxy;

use crate::UserEvent;
use devtools_core::Settings;

const NO_HOTKEY: u32 = u32::MAX;

/// 跨平台全局快捷键管理器；Wayland 通过 XDG GlobalShortcuts portal 注册。
pub struct ShortcutManager {
    manager: GlobalHotKeyManager,
    registered: Vec<HotKey>,
    launcher_id: Arc<AtomicU32>,
    quick_input_id: Arc<AtomicU32>,
}

impl ShortcutManager {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Result<Self, String> {
        std::env::set_var("GLOBAL_HOTKEY_APP_ID", "org.loveyu.DevTools");
        let launcher_id = Arc::new(AtomicU32::new(NO_HOTKEY));
        let quick_input_id = Arc::new(AtomicU32::new(NO_HOTKEY));
        let event_launcher_id = Arc::clone(&launcher_id);
        let event_quick_input_id = Arc::clone(&quick_input_id);
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.state() == HotKeyState::Pressed {
                if event.id() == event_launcher_id.load(Ordering::Acquire) {
                    let _ = proxy.send_event(UserEvent::OpenLauncher);
                } else if event.id() == event_quick_input_id.load(Ordering::Acquire) {
                    let _ = proxy.send_event(UserEvent::OpenQuickInput);
                }
            }
        }));
        Ok(Self {
            manager: GlobalHotKeyManager::new().map_err(|error| error.to_string())?,
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

        let previous = self.registered.clone();
        if !previous.is_empty() {
            self.manager
                .unregister_all(&previous)
                .map_err(|error| error.to_string())?;
        }
        if !next.is_empty() {
            if let Err(error) = self.manager.register_all(&next) {
                if !previous.is_empty() {
                    let _ = self.manager.register_all(&previous);
                }
                return Err(error.to_string());
            }
        }
        self.launcher_id.store(
            launcher.map_or(NO_HOTKEY, |value| value.id()),
            Ordering::Release,
        );
        self.quick_input_id.store(
            quick_input.map_or(NO_HOTKEY, |value| value.id()),
            Ordering::Release,
        );
        self.registered = next;
        Ok(())
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

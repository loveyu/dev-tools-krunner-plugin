//! KDE StatusNotifierItem 托盘实现，菜单动作只投递给应用主线程。

use devtools_core::LanguageMode;
use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::StandardItem;
use tao::event_loop::EventLoopProxy;

use super::resolve_language;
use crate::UserEvent;

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
        "org.loveyu.DevTools".to_owned()
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

#[cfg(test)]
mod tests {
    use super::*;

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
}

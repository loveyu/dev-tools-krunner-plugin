use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::StandardItem;
use tao::event_loop::EventLoopProxy;

use crate::UserEvent;

/// KDE StatusNotifierItem 托盘实现，菜单动作只投递给 GUI 主线程。
pub struct DevToolsTray {
    proxy: EventLoopProxy<UserEvent>,
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
        let _ = self.proxy.send_event(UserEvent::OpenSettings);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            StandardItem {
                label: "设置".to_owned(),
                icon_name: "configure".to_owned(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.proxy.send_event(UserEvent::OpenSettings);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "重启".to_owned(),
                icon_name: "system-reboot".to_owned(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.proxy.send_event(UserEvent::Restart);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "退出".to_owned(),
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

/// 根据设置动态创建或移除托盘服务。
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

    pub fn set_visible(&mut self, visible: bool) -> Result<(), ksni::Error> {
        if visible && self.handle.is_none() {
            let tray = DevToolsTray {
                proxy: self.proxy.clone(),
            };
            self.handle = Some(tray.spawn()?);
        } else if !visible {
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

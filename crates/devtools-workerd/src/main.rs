//! DevTools 跨平台桌面 Worker 入口。

#![allow(non_snake_case)]

mod application;
mod global_shortcut;
mod ipc;
mod media_processor;
mod native_converter;
mod platform;
mod quick_input;
mod registry;
mod settings;
mod webview_manager;
mod window_manager;

pub(crate) use application::UserEvent;

fn main() {
    if let Err(error) = application::Application::run() {
        eprintln!("devtools-workerd: {error}");
        std::process::exit(1);
    }
}

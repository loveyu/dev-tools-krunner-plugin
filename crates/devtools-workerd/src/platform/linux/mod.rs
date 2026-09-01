//! Linux 平台实现：WebView 构建、剪贴板、外链、屏幕取色、XDG 目录与语言探测。
//! 快捷键 portal、快速输入、托盘、D-Bus 服务在同名子模块中。

mod ipc_service;
mod quick_input;
mod shortcut_portal;
mod tray;

pub use ipc_service::{request_existing_worker, start_ipc};
pub use quick_input::QuickInputWindow;
pub use shortcut_portal::GlobalShortcutBackend;
pub use tray::TrayManager;

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;

use ashpd::desktop::Color;
use devtools_core::LanguageMode;
use gtk::gdk;
use tao::event_loop::{EventLoopBuilder, EventLoopProxy};
use tao::platform::unix::{EventLoopBuilderExtUnix, WindowExtUnix};
use tao::window::Window;
use wry::{WebView, WebViewBuilder, WebViewBuilderExtUnix};

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

pub fn configure_event_loop(builder: &mut EventLoopBuilder<UserEvent>) {
    // 与 Worker 的 zbus 服务名分离，避免两个总线连接争抢同一个 well-known name。
    builder.with_app_id("org.loveyu.DevTools.Application");
}

pub fn copy_text(text: &str) -> Result<(), String> {
    let clipboard = gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD);
    clipboard.set_text(text);
    clipboard.store();
    Ok(())
}

/// 用系统默认应用打开外部链接：http(s) 交给默认浏览器，mailto 交给默认邮件客户端。
/// WebView 以内嵌 HTML 加载，无法自行打开新窗口，必须交给桌面处理。
pub fn open_external_url(url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|error| format!("invalid url: {error}"))?;
    if !matches!(parsed.scheme(), "http" | "https" | "mailto") {
        return Err(format!("refused to open external url: {url}"));
    }
    Command::new("xdg-open")
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(reap_in_background)
        .map_err(|error| format!("failed to run xdg-open: {error}"))
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
         Icon=org.loveyu.DevTools\n\
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

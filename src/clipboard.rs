use std::io;
use std::process::{Command, Output};

/// 读取 KDE 文本剪贴板。优先使用 Klipper D-Bus，失败时按显示后端回落。
pub fn read_text() -> io::Result<String> {
    let klipper = Command::new("qdbus6")
        .args([
            "org.kde.klipper",
            "/klipper",
            "org.kde.klipper.klipper.getClipboardContents",
        ])
        .output();
    if let Ok(output) = klipper {
        if output.status.success() {
            return output_text(output);
        }
    }

    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        let output = Command::new("wl-paste")
            .args(["--no-newline", "--type", "text"])
            .output()?;
        return successful_output_text("wl-paste", output);
    }

    let output = Command::new("xclip")
        .args(["-selection", "clipboard", "-out"])
        .output()?;
    successful_output_text("xclip", output)
}

fn successful_output_text(command: &str, output: Output) -> io::Result<String> {
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "{command} exited with {}",
            output.status
        )));
    }
    output_text(output)
}

fn output_text(output: Output) -> io::Result<String> {
    String::from_utf8(output.stdout)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

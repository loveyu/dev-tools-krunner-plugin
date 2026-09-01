use std::io;
use std::process::{Command, Output};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// 单个剪贴板子进程的等待上限。
///
/// zbus 的对象服务是串行分发的：这里若跟随 qdbus6 默认的 ~25s D-Bus 超时，
/// klipper 卡死时用户每敲一键都会队头阻塞整个 runner 的 Match/Run 分发，
/// 因此超时必须远小于该值；超时视为失败并回落下一级读取方式。
const READ_TIMEOUT: Duration = Duration::from_secs(2);

/// 读取 KDE 文本剪贴板。优先使用 Klipper D-Bus，失败时按显示后端回落。
pub fn read_text() -> io::Result<String> {
    let mut klipper = Command::new("qdbus6");
    klipper.args([
        "org.kde.klipper",
        "/klipper",
        "org.kde.klipper.klipper.getClipboardContents",
    ]);
    if let Ok(output) = run_with_timeout(klipper, READ_TIMEOUT) {
        if output.status.success() {
            return output_text(output);
        }
    }

    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        let mut paste = Command::new("wl-paste");
        paste.args(["--no-newline", "--type", "text"]);
        let output = run_with_timeout(paste, READ_TIMEOUT)?;
        return successful_output_text("wl-paste", output);
    }

    let mut xclip = Command::new("xclip");
    xclip.args(["-selection", "clipboard", "-out"]);
    let output = run_with_timeout(xclip, READ_TIMEOUT)?;
    successful_output_text("xclip", output)
}

/// 带超时地收集子进程输出。
///
/// 用独立线程跑 `Command::output`（其内部会持续排水管道，避免输出写满管道
/// 造成假超时），主线程 `recv_timeout` 到点即返回。超时后无法终止线程里的
/// 子进程：qdbus6 达到自身 D-Bus 超时会退出，wl-paste/xclip 正常情况立即
/// 返回，线程最终都会结束，不会无限累积。
fn run_with_timeout(mut command: Command, timeout: Duration) -> io::Result<Output> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || sender.send(command.output()));
    receiver.recv_timeout(timeout).map_err(|_| {
        io::Error::new(
            io::ErrorKind::TimedOut,
            format!("clipboard helper timed out after {timeout:?}"),
        )
    })?
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

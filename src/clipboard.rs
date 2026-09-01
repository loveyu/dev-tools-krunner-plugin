use std::io;
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// 单个剪贴板子进程的等待上限。
///
/// zbus 的对象服务是串行分发的：这里若跟随 qdbus6 默认的 ~25s D-Bus 超时，
/// klipper 卡死时用户每敲一键都会队头阻塞整个 runner 的 Match/Run 分发，
/// 因此超时必须远小于该值；超时视为失败并回落下一级读取方式。
const READ_TIMEOUT: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

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

/// 带超时地收集子进程输出；超时即 kill 子进程并报 TimedOut。
///
/// 输出经 reader 线程持续排水（剪贴板内容可达 2 MiB，远超管道缓冲，
/// 不排水会把子进程阻塞在写管道上造成假超时）。wl-paste/xclip 没有自身
/// 超时，compositor/X server 挂死时必须在这里 kill，否则每键累积一个
/// 永久挂起的进程与线程。
fn run_with_timeout(mut command: Command, timeout: Duration) -> io::Result<Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let stdout_reader = child.stdout.take().map(spawn_reader);
    let stderr_reader = child.stderr.take().map(spawn_reader);

    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("clipboard helper timed out after {timeout:?}"),
            ));
        }
        thread::sleep(POLL_INTERVAL);
    };

    let stdout = finish_reader(stdout_reader)?;
    let stderr = finish_reader(stderr_reader)?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

/// 排水一根子进程管道；进程退出或被 kill 关闭管道后线程自然结束。
fn spawn_reader(pipe: impl std::io::Read + Send + 'static) -> ReaderHandle {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut buffer = Vec::new();
        let mut pipe = pipe;
        let _ = std::io::Read::read_to_end(&mut pipe, &mut buffer);
        let _ = sender.send(buffer);
    });
    ReaderHandle { receiver }
}

struct ReaderHandle {
    receiver: mpsc::Receiver<Vec<u8>>,
}

fn finish_reader(reader: Option<ReaderHandle>) -> io::Result<Vec<u8>> {
    match reader {
        Some(handle) => handle
            .receiver
            .recv()
            .map_err(|_| io::Error::other("clipboard reader thread stopped")),
        None => Ok(Vec::new()),
    }
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

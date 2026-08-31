use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;

use devtools_core::MAX_TEXT_BYTES;
use serde::{Deserialize, Serialize};
use tao::event_loop::EventLoopProxy;
use wait_timeout::ChildExt;

use crate::UserEvent;

const PHP_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

const PHP_SCRIPT: &str = r#"
function reject_objects(mixed $value): void {
    if (is_object($value) || is_resource($value)) {
        throw new RuntimeException('PHP objects and resources are not supported');
    }
    if (is_array($value)) {
        foreach ($value as $item) {
            reject_objects($item);
        }
    }
}

function short_array(mixed $value, int $depth = 0): string {
    if (!is_array($value)) {
        return var_export($value, true);
    }
    if ($value === []) {
        return '[]';
    }
    $indent = str_repeat('    ', $depth);
    $child_indent = str_repeat('    ', $depth + 1);
    $is_list = array_is_list($value);
    $lines = [];
    foreach ($value as $key => $item) {
        $prefix = $is_list ? '' : var_export($key, true) . ' => ';
        $lines[] = $child_indent . $prefix . short_array($item, $depth + 1) . ',';
    }
    return "[\n" . implode("\n", $lines) . "\n" . $indent . ']';
}

$direction = $argv[1] ?? '';
$format = $argv[2] ?? '';
$input = stream_get_contents(STDIN);

if ($direction === 'parse' && $format === 'php-serialize') {
    set_error_handler(static function (int $severity, string $message): never {
        throw new ErrorException($message, 0, $severity);
    });
    try {
        $value = unserialize($input, ['allowed_classes' => false]);
    } finally {
        restore_error_handler();
    }
    reject_objects($value);
    echo json_encode($value, JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
    exit(0);
}

if ($direction !== 'stringify') {
    throw new InvalidArgumentException('unsupported native conversion direction');
}

$value = json_decode($input, true, 512, JSON_THROW_ON_ERROR);
reject_objects($value);

if ($format === 'php-serialize') {
    echo serialize($value);
} elseif ($format === 'php-var-export') {
    echo var_export($value, true);
} elseif ($format === 'php-array') {
    if (!is_array($value)) {
        throw new InvalidArgumentException('PHP array output requires a JSON array or object');
    }
    echo short_array($value);
} else {
    throw new InvalidArgumentException('unsupported native conversion format');
}
"#;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConverterCapabilities {
    native_formats: Vec<NativeFormat>,
    php_version: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub enum NativeFormat {
    #[serde(rename = "php-serialize")]
    Serialize,
    #[serde(rename = "php-var-export")]
    VarExport,
    #[serde(rename = "php-array")]
    Array,
}

impl NativeFormat {
    fn as_str(self) -> &'static str {
        match self {
            Self::Serialize => "php-serialize",
            Self::VarExport => "php-var-export",
            Self::Array => "php-array",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConversionDirection {
    Parse,
    Stringify,
}

impl ConversionDirection {
    fn as_str(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Stringify => "stringify",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeConversionResult {
    request_id: String,
    result: Option<String>,
    error: Option<String>,
}

impl NativeConversionResult {
    pub fn error(request_id: String, error: impl Into<String>) -> Self {
        Self {
            request_id,
            result: None,
            error: Some(error.into()),
        }
    }

    fn success(request_id: String, result: String) -> Self {
        Self {
            request_id,
            result: Some(result),
            error: None,
        }
    }
}

struct NativeConversionJob {
    request_id: String,
    format: NativeFormat,
    direction: ConversionDirection,
    payload: String,
}

/// 把可能阻塞的 CLI 转换放到独立线程，避免卡住 GTK/WebView 事件循环。
pub struct NativeConverter {
    capabilities: ConverterCapabilities,
    sender: Sender<NativeConversionJob>,
}

impl NativeConverter {
    pub fn start(proxy: EventLoopProxy<UserEvent>) -> Self {
        let php_version = detect_php_version();
        let capabilities = ConverterCapabilities {
            native_formats: php_version
                .as_ref()
                .map(|_| {
                    vec![
                        NativeFormat::Serialize,
                        NativeFormat::VarExport,
                        NativeFormat::Array,
                    ]
                })
                .unwrap_or_default(),
            php_version,
        };
        let php_available = capabilities.php_version.is_some();
        let (sender, receiver) = mpsc::channel::<NativeConversionJob>();
        thread::Builder::new()
            .name("native-converter".to_owned())
            .spawn(move || {
                while let Ok(job) = receiver.recv() {
                    let result = if php_available {
                        run_php(&job.payload, job.direction, job.format)
                            .map(|output| {
                                NativeConversionResult::success(job.request_id.clone(), output)
                            })
                            .unwrap_or_else(|error| {
                                NativeConversionResult::error(job.request_id.clone(), error)
                            })
                    } else {
                        NativeConversionResult::error(job.request_id, "PHP CLI is not available")
                    };
                    let _ = proxy.send_event(UserEvent::NativeConversionFinished(result));
                }
            })
            .expect("native converter thread should start");

        Self {
            capabilities,
            sender,
        }
    }

    pub fn capabilities(&self) -> &ConverterCapabilities {
        &self.capabilities
    }

    pub fn submit(
        &self,
        request_id: String,
        format: NativeFormat,
        direction: ConversionDirection,
        payload: String,
    ) -> Result<(), String> {
        if request_id.is_empty() {
            return Err("native conversion request id is empty".to_owned());
        }
        if payload.len() > MAX_TEXT_BYTES {
            return Err(format!(
                "input is too large: {} bytes (maximum {MAX_TEXT_BYTES})",
                payload.len()
            ));
        }
        self.sender
            .send(NativeConversionJob {
                request_id,
                format,
                direction,
                payload,
            })
            .map_err(|_| "native converter is unavailable".to_owned())
    }
}

fn detect_php_version() -> Option<String> {
    let output = Command::new("php")
        .args(["-n", "-r", "echo PHP_VERSION;"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let version = String::from_utf8(output.stdout).ok()?;
    let version = version.trim();
    (!version.is_empty()).then(|| version.to_owned())
}

fn run_php(
    input: &str,
    direction: ConversionDirection,
    format: NativeFormat,
) -> Result<String, String> {
    let mut child = Command::new("php")
        .args([
            "-n",
            "-d",
            "display_errors=stderr",
            "-r",
            PHP_SCRIPT,
            "--",
            direction.as_str(),
            format.as_str(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to start PHP CLI: {error}"))?;

    let mut stdin = child.stdin.take().ok_or("failed to open PHP stdin")?;
    let input = input.as_bytes().to_vec();
    let writer = thread::spawn(move || stdin.write_all(&input));

    let stdout = child.stdout.take().ok_or("failed to open PHP stdout")?;
    let stderr = child.stderr.take().ok_or("failed to open PHP stderr")?;
    let stdout_reader = thread::spawn(move || read_limited(stdout));
    let stderr_reader = thread::spawn(move || read_limited(stderr));

    let status = match child
        .wait_timeout(PHP_TIMEOUT)
        .map_err(|error| format!("failed to wait for PHP CLI: {error}"))?
    {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return Err("PHP conversion timed out after 5 seconds".to_owned());
        }
    };

    writer
        .join()
        .map_err(|_| "PHP stdin writer panicked".to_owned())?
        .map_err(|error| format!("failed to write PHP stdin: {error}"))?;
    let stdout = stdout_reader
        .join()
        .map_err(|_| "PHP stdout reader panicked".to_owned())??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "PHP stderr reader panicked".to_owned())??;

    if !status.success() {
        let detail = String::from_utf8_lossy(&stderr);
        return Err(format!("PHP conversion failed: {}", detail.trim()));
    }
    String::from_utf8(stdout).map_err(|error| format!("PHP returned invalid UTF-8: {error}"))
}

fn read_limited(mut reader: impl Read) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    reader
        .by_ref()
        .take((MAX_OUTPUT_BYTES + 1) as u64)
        .read_to_end(&mut output)
        .map_err(|error| format!("failed to read native converter output: {error}"))?;
    if output.len() > MAX_OUTPUT_BYTES {
        return Err(format!(
            "native converter output exceeds {MAX_OUTPUT_BYTES} bytes"
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn php_serialized_false_is_not_treated_as_an_error() {
        if detect_php_version().is_none() {
            return;
        }
        let output = run_php("b:0;", ConversionDirection::Parse, NativeFormat::Serialize)
            .expect("PHP false 应可反序列化");
        assert_eq!(output, "false");
    }

    #[test]
    fn php_unserialize_rejects_objects() {
        if detect_php_version().is_none() {
            return;
        }
        let result = run_php(
            r#"O:8:"stdClass":0:{}"#,
            ConversionDirection::Parse,
            NativeFormat::Serialize,
        );
        assert!(result.is_err());
    }

    #[test]
    fn php_array_uses_short_syntax() {
        if detect_php_version().is_none() {
            return;
        }
        let output = run_php(
            r#"{"a":[1,true]}"#,
            ConversionDirection::Stringify,
            NativeFormat::Array,
        )
        .expect("PHP 数组应可输出");
        assert!(output.starts_with("[\n"));
        assert!(output.contains("'a' => ["));
    }
}

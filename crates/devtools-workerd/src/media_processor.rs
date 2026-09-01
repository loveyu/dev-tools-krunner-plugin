use std::collections::HashSet;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use tao::event_loop::EventLoopProxy;
use tempfile::NamedTempFile;
use wait_timeout::ChildExt;

use crate::UserEvent;

const PROCESS_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;
const MAX_ENCODED_IMAGE_BYTES: usize = (MAX_IMAGE_BYTES * 4 / 3) + 4;
const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const ALLOWED_MIME_TYPES: [&str; 6] = [
    "image/png",
    "image/jpeg",
    "image/bmp",
    "image/tiff",
    "image/webp",
    "image/gif",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaCapabilities {
    pub ocr: OcrCapability,
    pub barcode: BarcodeCapability,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrCapability {
    pub available: bool,
    pub version: Option<String>,
    pub languages: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarcodeCapability {
    pub available: bool,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaOperation {
    Ocr,
    Barcode,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MediaOptions {
    pub language: Option<String>,
    pub page_segmentation_mode: Option<u8>,
    pub minimum_confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaProcessingResult {
    request_id: String,
    result: Option<serde_json::Value>,
    error: Option<String>,
}

impl MediaProcessingResult {
    pub fn error(request_id: String, error: impl Into<String>) -> Self {
        Self {
            request_id,
            result: None,
            error: Some(error.into()),
        }
    }

    fn success(request_id: String, result: impl Serialize) -> Result<Self, String> {
        Ok(Self {
            request_id,
            result: Some(
                serde_json::to_value(result)
                    .map_err(|error| format!("failed to serialize media result: {error}"))?,
            ),
            error: None,
        })
    }
}

struct MediaJob {
    request_id: String,
    operation: MediaOperation,
    image_base64: String,
    mime_type: String,
    options: MediaOptions,
}

/// 串行执行外部识别程序，避免多张大图同时抢占桌面资源。
pub struct MediaProcessor {
    capabilities: MediaCapabilities,
    sender: Sender<MediaJob>,
}

impl MediaProcessor {
    pub fn start(proxy: EventLoopProxy<UserEvent>) -> Self {
        let capabilities = detect_capabilities();
        let worker_capabilities = capabilities.clone();
        let (sender, receiver) = mpsc::channel::<MediaJob>();
        thread::Builder::new()
            .name("media-processor".to_owned())
            .spawn(move || {
                while let Ok(job) = receiver.recv() {
                    let request_id = job.request_id.clone();
                    let result = process_job(job, &worker_capabilities)
                        .and_then(|value| MediaProcessingResult::success(request_id.clone(), value))
                        .unwrap_or_else(|error| MediaProcessingResult::error(request_id, error));
                    let _ = proxy.send_event(UserEvent::MediaProcessingFinished(result));
                }
            })
            .expect("media processor thread should start");

        Self {
            capabilities,
            sender,
        }
    }

    pub fn capabilities(&self) -> &MediaCapabilities {
        &self.capabilities
    }

    pub fn submit(
        &self,
        request_id: String,
        operation: MediaOperation,
        image_base64: String,
        mime_type: String,
        options: MediaOptions,
    ) -> Result<(), String> {
        if request_id.trim().is_empty() {
            return Err("media processing request id is empty".to_owned());
        }
        if !ALLOWED_MIME_TYPES.contains(&mime_type.as_str()) {
            return Err(format!("unsupported image type: {mime_type}"));
        }
        if image_base64.len() > MAX_ENCODED_IMAGE_BYTES {
            return Err(format!(
                "encoded image is too large (maximum {MAX_IMAGE_BYTES} decoded bytes)"
            ));
        }
        self.sender
            .send(MediaJob {
                request_id,
                operation,
                image_base64,
                mime_type,
                options,
            })
            .map_err(|_| "media processor is unavailable".to_owned())
    }
}

fn process_job(
    job: MediaJob,
    capabilities: &MediaCapabilities,
) -> Result<serde_json::Value, String> {
    let bytes = decode_image(&job.image_base64)?;
    match job.operation {
        MediaOperation::Ocr => {
            if !capabilities.ocr.available {
                return Err("Tesseract OCR is not available".to_owned());
            }
            let result = run_ocr(&bytes, &job.options, &capabilities.ocr.languages)?;
            serde_json::to_value(result)
                .map_err(|error| format!("failed to serialize OCR result: {error}"))
        }
        MediaOperation::Barcode => {
            if !capabilities.barcode.available {
                return Err("ZBar barcode reader is not available".to_owned());
            }
            let result = run_barcode(&bytes, &job.mime_type)?;
            serde_json::to_value(result)
                .map_err(|error| format!("failed to serialize barcode result: {error}"))
        }
    }
}

fn decode_image(encoded: &str) -> Result<Vec<u8>, String> {
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|error| format!("invalid base64 image: {error}"))?;
    if bytes.is_empty() {
        return Err("image is empty".to_owned());
    }
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(format!(
            "image is too large: {} bytes (maximum {MAX_IMAGE_BYTES})",
            bytes.len()
        ));
    }
    Ok(bytes)
}

fn detect_capabilities() -> MediaCapabilities {
    let tesseract_version = command_version("tesseract", &["--version"]);
    let languages = if tesseract_version.is_some() {
        detect_tesseract_languages()
    } else {
        Vec::new()
    };
    let zbar_version = command_version("zbarimg", &["--version"]);
    MediaCapabilities {
        ocr: OcrCapability {
            available: tesseract_version.is_some() && !languages.is_empty(),
            version: tesseract_version,
            languages,
        },
        barcode: BarcodeCapability {
            available: zbar_version.is_some(),
            version: zbar_version,
        },
    }
}

fn command_version(program: &str, arguments: &[&str]) -> Option<String> {
    let output = Command::new(program).args(arguments).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = if output.stdout.is_empty() {
        String::from_utf8(output.stderr).ok()?
    } else {
        String::from_utf8(output.stdout).ok()?
    };
    text.lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
}

fn detect_tesseract_languages() -> Vec<String> {
    let output = Command::new("tesseract").arg("--list-langs").output().ok();
    let mut languages: Vec<String> = output
        .filter(|result| result.status.success())
        .and_then(|result| String::from_utf8(result.stdout).ok())
        .map(|text| {
            text.lines()
                .skip(1)
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default();
    languages.sort_unstable();
    languages
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OcrResult {
    full_text: String,
    average_confidence: f64,
    words: Vec<OcrWord>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OcrWord {
    text: String,
    confidence: f64,
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    #[serde(skip)]
    line_key: (u32, u32, u32, u32),
}

fn run_ocr(
    image: &[u8],
    options: &MediaOptions,
    available_languages: &[String],
) -> Result<OcrResult, String> {
    let language = options.language.as_deref().unwrap_or("eng");
    validate_languages(language, available_languages)?;
    let page_segmentation_mode = options.page_segmentation_mode.unwrap_or(3);
    if ![3, 4, 6, 11, 12, 13].contains(&page_segmentation_mode) {
        return Err(format!(
            "unsupported Tesseract page segmentation mode: {page_segmentation_mode}"
        ));
    }
    let minimum_confidence = options.minimum_confidence.unwrap_or(0.0);
    if !(0.0..=100.0).contains(&minimum_confidence) {
        return Err("minimum OCR confidence must be between 0 and 100".to_owned());
    }
    let psm = page_segmentation_mode.to_string();
    let output = run_with_stdin(
        "tesseract",
        &["stdin", "stdout", "-l", language, "--psm", &psm, "tsv"],
        image,
    )?;
    parse_tesseract_tsv(&output, minimum_confidence)
}

fn validate_languages(language: &str, available_languages: &[String]) -> Result<(), String> {
    let available: HashSet<&str> = available_languages.iter().map(String::as_str).collect();
    let requested: Vec<&str> = language.split('+').collect();
    if requested.is_empty()
        || requested
            .iter()
            .any(|item| item.is_empty() || !available.contains(item))
    {
        return Err(format!(
            "unsupported Tesseract language selection: {language}"
        ));
    }
    Ok(())
}

fn parse_tesseract_tsv(tsv: &[u8], minimum_confidence: f64) -> Result<OcrResult, String> {
    let text = std::str::from_utf8(tsv)
        .map_err(|error| format!("Tesseract returned invalid UTF-8: {error}"))?;
    let mut words = Vec::new();
    for line in text.lines().skip(1) {
        let columns: Vec<&str> = line.splitn(12, '\t').collect();
        if columns.len() != 12 || columns[0] != "5" {
            continue;
        }
        let word_text = columns[11].trim();
        if word_text.is_empty() {
            continue;
        }
        let confidence = parse_column::<f64>(&columns, 10, "confidence")?;
        if confidence < minimum_confidence || confidence < 0.0 {
            continue;
        }
        words.push(OcrWord {
            text: word_text.to_owned(),
            confidence,
            left: parse_column(&columns, 6, "left")?,
            top: parse_column(&columns, 7, "top")?,
            width: parse_column(&columns, 8, "width")?,
            height: parse_column(&columns, 9, "height")?,
            line_key: (
                parse_column(&columns, 1, "page")?,
                parse_column(&columns, 2, "block")?,
                parse_column(&columns, 3, "paragraph")?,
                parse_column(&columns, 4, "line")?,
            ),
        });
    }
    let average_confidence = if words.is_empty() {
        0.0
    } else {
        words.iter().map(|word| word.confidence).sum::<f64>() / words.len() as f64
    };
    let mut lines = Vec::<String>::new();
    for word in &words {
        let starts_new_line = words
            .iter()
            .position(|candidate| std::ptr::eq(candidate, word))
            .is_none_or(|index| index == 0 || words[index - 1].line_key != word.line_key);
        if starts_new_line {
            lines.push(word.text.clone());
        } else if let Some(line) = lines.last_mut() {
            line.push(' ');
            line.push_str(&word.text);
        }
    }
    Ok(OcrResult {
        full_text: lines.join("\n"),
        average_confidence,
        words,
    })
}

fn parse_column<T>(columns: &[&str], index: usize, name: &str) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    columns[index]
        .parse::<T>()
        .map_err(|error| format!("invalid Tesseract {name}: {error}"))
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct BarcodeResult {
    codes: Vec<DetectedCode>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct DetectedCode {
    code_type: String,
    data: String,
}

fn run_barcode(image: &[u8], _mime_type: &str) -> Result<BarcodeResult, String> {
    let mut file = NamedTempFile::new()
        .map_err(|error| format!("failed to create temporary image: {error}"))?;
    file.write_all(image)
        .map_err(|error| format!("failed to write temporary image: {error}"))?;
    let path = file.path().as_os_str();
    let mut child = Command::new("zbarimg")
        .arg("--quiet")
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to start ZBar: {error}"))?;
    let (status, stdout, stderr) = collect_child_output(&mut child, PROCESS_TIMEOUT)?;
    if !status.success() && status.code() != Some(4) {
        return Err(format!(
            "ZBar failed: {}",
            String::from_utf8_lossy(&stderr).trim()
        ));
    }
    parse_zbar_output(&stdout)
}

fn parse_zbar_output(output: &[u8]) -> Result<BarcodeResult, String> {
    let output = std::str::from_utf8(output)
        .map_err(|error| format!("ZBar returned invalid UTF-8: {error}"))?;
    let mut seen = HashSet::new();
    let mut codes = Vec::new();
    for line in output.lines() {
        let Some((code_type, data)) = line.split_once(':') else {
            continue;
        };
        let item = DetectedCode {
            code_type: code_type.trim().to_owned(),
            data: data.to_owned(),
        };
        if !item.code_type.is_empty()
            && !item.data.is_empty()
            && seen.insert((item.code_type.clone(), item.data.clone()))
        {
            codes.push(item);
        }
    }
    Ok(BarcodeResult { codes })
}

fn run_with_stdin(program: &str, arguments: &[&str], input: &[u8]) -> Result<Vec<u8>, String> {
    let mut child = Command::new(program)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to start {program}: {error}"))?;
    let mut stdin = child.stdin.take().ok_or("failed to open process stdin")?;
    let input = input.to_vec();
    let writer = thread::spawn(move || stdin.write_all(&input));
    let (status, stdout, stderr) = collect_child_output(&mut child, PROCESS_TIMEOUT)?;
    // 先报告子进程自身的失败原因：子进程因坏图/坏参数提前退出时，stdin 写入端
    // 必然收到 Broken pipe——那是症状不是病因，真实原因在 stderr 里。
    if !status.success() {
        return Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&stderr).trim()
        ));
    }
    writer
        .join()
        .map_err(|_| "media process stdin writer panicked".to_owned())?
        .map_err(|error| format!("failed to write media process stdin: {error}"))?;
    Ok(stdout)
}

fn collect_child_output(
    child: &mut std::process::Child,
    timeout: Duration,
) -> Result<(std::process::ExitStatus, Vec<u8>, Vec<u8>), String> {
    let stdout = child.stdout.take().ok_or("failed to open process stdout")?;
    let stderr = child.stderr.take().ok_or("failed to open process stderr")?;
    let stdout_reader = thread::spawn(move || read_limited(stdout));
    let stderr_reader = thread::spawn(move || read_limited(stderr));
    let status = match child
        .wait_timeout(timeout)
        .map_err(|error| format!("failed to wait for media process: {error}"))?
    {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            // kill 关闭管道后 reader 线程随之结束；若输出已读满上限，说明真实
            // 原因是输出超限（子进程写管道阻塞，并非真的超时），优先报超限。
            for reader in [stdout_reader, stderr_reader] {
                if let Ok(Err(error)) = reader.join() {
                    return Err(error);
                }
            }
            return Err("media processing timed out after 30 seconds".to_owned());
        }
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| "media process stdout reader panicked".to_owned())??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "media process stderr reader panicked".to_owned())??;
    Ok((status, stdout, stderr))
}

fn read_limited(mut reader: impl Read) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    reader
        .by_ref()
        .take((MAX_OUTPUT_BYTES + 1) as u64)
        .read_to_end(&mut output)
        .map_err(|error| format!("failed to read media process output: {error}"))?;
    if output.len() > MAX_OUTPUT_BYTES {
        return Err(format!(
            "media process output exceeds {MAX_OUTPUT_BYTES} bytes"
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tesseract_words_and_rebuilds_lines() {
        let tsv = b"level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
5\t1\t1\t1\t1\t1\t10\t20\t30\t12\t95.5\tHello\n\
5\t1\t1\t1\t1\t2\t45\t20\t35\t12\t88.5\tworld\n\
5\t1\t1\t1\t2\t1\t10\t40\t20\t12\t42.0\tnext\n";
        let result = parse_tesseract_tsv(tsv, 50.0).expect("TSV 应可解析");

        assert_eq!(result.full_text, "Hello world");
        assert_eq!(result.words.len(), 2);
        assert_eq!(result.average_confidence, 92.0);
    }

    #[test]
    fn parses_and_deduplicates_zbar_output() {
        let result = parse_zbar_output(
            b"QR-Code:https://example.com:a\nCODE-128:1234\nQR-Code:https://example.com:a\n",
        )
        .expect("ZBar 输出应可解析");

        assert_eq!(
            result.codes,
            vec![
                DetectedCode {
                    code_type: "QR-Code".to_owned(),
                    data: "https://example.com:a".to_owned(),
                },
                DetectedCode {
                    code_type: "CODE-128".to_owned(),
                    data: "1234".to_owned(),
                }
            ]
        );
    }

    #[test]
    fn rejects_unavailable_language_combinations() {
        let available = vec!["chi_sim".to_owned(), "eng".to_owned()];
        assert!(validate_languages("eng+chi_sim", &available).is_ok());
        assert!(validate_languages("eng+deu", &available).is_err());
        assert!(validate_languages("eng+", &available).is_err());
    }

    #[test]
    fn rejects_oversized_or_empty_images() {
        assert!(decode_image("").is_err());
        let oversized = vec![0_u8; MAX_IMAGE_BYTES + 1];
        assert!(decode_image(&STANDARD.encode(oversized)).is_err());
    }

    #[test]
    fn recognizes_generated_ocr_image_when_system_tools_are_available() {
        if command_version("tesseract", &["--version"]).is_none() {
            return;
        }
        let Some(image) = render_test_label() else {
            return;
        };
        let languages = detect_tesseract_languages();
        if !languages.iter().any(|language| language == "eng") {
            return;
        }
        let result = run_ocr(
            &image,
            &MediaOptions {
                language: Some("eng".to_owned()),
                page_segmentation_mode: Some(6),
                minimum_confidence: Some(0.0),
            },
            &languages,
        )
        .expect("生成的测试图片应可识别");

        assert!(result.full_text.contains("DEVTOOLS"));
        assert!(result.full_text.contains("2026"));
    }

    #[test]
    fn recognizes_generated_qr_code_when_system_tools_are_available() {
        if command_version("zbarimg", &["--version"]).is_none()
            || command_version("qrencode", &["--version"]).is_none()
        {
            return;
        }
        let output = Command::new("qrencode")
            .args(["-o", "-", "https://example.com/devtools"])
            .output()
            .expect("应可启动 qrencode");
        assert!(output.status.success());
        let result = run_barcode(&output.stdout, "image/png").expect("生成的二维码应可识别");

        assert_eq!(
            result.codes,
            vec![DetectedCode {
                code_type: "QR-Code".to_owned(),
                data: "https://example.com/devtools".to_owned(),
            }]
        );
    }

    fn render_test_label() -> Option<Vec<u8>> {
        let program = ["magick", "convert"]
            .into_iter()
            .find(|candidate| command_version(candidate, &["-version"]).is_some())?;
        let output = Command::new(program)
            .args([
                "-size",
                "900x160",
                "xc:white",
                "-font",
                "DejaVu-Sans",
                "-pointsize",
                "64",
                "-fill",
                "black",
                "-gravity",
                "center",
                "-annotate",
                "+0+0",
                "DEVTOOLS 2026",
                "png:-",
            ])
            .output()
            .ok()?;
        output.status.success().then_some(output.stdout)
    }
}

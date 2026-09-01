use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use devtools_core::MetadataBackend;
use serde::Serialize;
use tao::event_loop::EventLoopProxy;
use wait_timeout::ChildExt;

use crate::UserEvent;

const PROCESS_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;
const MAX_ENCODED_IMAGE_BYTES: usize = (MAX_IMAGE_BYTES * 4 / 3) + 4;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataCapabilities {
    pub builtin_version: &'static str,
    pub external_available: bool,
    pub external_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataProcessingResult {
    request_id: String,
    result: Option<MetadataDocument>,
    error: Option<String>,
}

impl MetadataProcessingResult {
    pub fn error(request_id: String, error: impl Into<String>) -> Self {
        Self {
            request_id,
            result: None,
            error: Some(error.into()),
        }
    }

    fn success(request_id: String, result: MetadataDocument) -> Self {
        Self {
            request_id,
            result: Some(result),
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataDocument {
    file_name: String,
    backend: MetadataBackend,
    fields: Vec<MetadataField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataField {
    group: String,
    name: String,
    value: String,
}

struct MetadataJob {
    request_id: String,
    source: MetadataSource,
    backend: MetadataBackend,
}

enum MetadataSource {
    Path(PathBuf),
    Image { bytes: Vec<u8>, mime_type: String },
}

/// 在专用串行线程读取文件元数据，避免大媒体文件阻塞桌面事件循环。
pub struct MetadataProcessor {
    capabilities: MetadataCapabilities,
    sender: Sender<MetadataJob>,
}

impl MetadataProcessor {
    pub fn start(proxy: EventLoopProxy<UserEvent>) -> Self {
        let capabilities = detect_capabilities();
        let worker_capabilities = capabilities.clone();
        let (sender, receiver) = mpsc::channel::<MetadataJob>();
        thread::Builder::new()
            .name("metadata-processor".to_owned())
            .spawn(move || {
                while let Ok(job) = receiver.recv() {
                    let request_id = job.request_id.clone();
                    let result = process_job(job, &worker_capabilities)
                        .map(|document| {
                            MetadataProcessingResult::success(request_id.clone(), document)
                        })
                        .unwrap_or_else(|error| MetadataProcessingResult::error(request_id, error));
                    let _ = proxy.send_event(UserEvent::MetadataProcessingFinished(result));
                }
            })
            .expect("metadata processor thread should start");
        Self {
            capabilities,
            sender,
        }
    }

    pub fn capabilities(&self) -> &MetadataCapabilities {
        &self.capabilities
    }

    pub fn submit(
        &self,
        request_id: String,
        path: PathBuf,
        backend: MetadataBackend,
    ) -> Result<(), String> {
        if request_id.trim().is_empty() {
            return Err("metadata request id is empty".to_owned());
        }
        if !path.is_file() {
            return Err("selected metadata path is not a regular file".to_owned());
        }
        self.sender
            .send(MetadataJob {
                request_id,
                source: MetadataSource::Path(path),
                backend,
            })
            .map_err(|_| "metadata processor is unavailable".to_owned())
    }

    pub fn submit_image(
        &self,
        request_id: String,
        image_base64: String,
        mime_type: String,
        backend: MetadataBackend,
    ) -> Result<(), String> {
        if request_id.trim().is_empty() {
            return Err("metadata request id is empty".to_owned());
        }
        if image_base64.len() > MAX_ENCODED_IMAGE_BYTES {
            return Err(format!(
                "encoded image exceeds {MAX_IMAGE_BYTES} decoded bytes"
            ));
        }
        let bytes = STANDARD
            .decode(image_base64)
            .map_err(|error| format!("invalid base64 image: {error}"))?;
        if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
            return Err(format!("image must contain 1 to {MAX_IMAGE_BYTES} bytes"));
        }
        let _ = extension_for_mime(&mime_type)?;
        self.sender
            .send(MetadataJob {
                request_id,
                source: MetadataSource::Image { bytes, mime_type },
                backend,
            })
            .map_err(|_| "metadata processor is unavailable".to_owned())
    }
}

fn detect_capabilities() -> MetadataCapabilities {
    let external_version = Command::new("exiftool")
        .arg("-ver")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|version| version.trim().to_owned())
        .filter(|version| !version.is_empty());
    MetadataCapabilities {
        builtin_version: "revelo 0.5.5",
        external_available: external_version.is_some(),
        external_version,
    }
}

fn process_job(
    job: MetadataJob,
    capabilities: &MetadataCapabilities,
) -> Result<MetadataDocument, String> {
    let (file_name, fields) = match (&job.source, job.backend) {
        (MetadataSource::Path(path), MetadataBackend::Builtin) => {
            (display_name(path), read_builtin(path)?)
        }
        (MetadataSource::Image { bytes, mime_type }, MetadataBackend::Builtin) => (
            format!("clipboard.{}", extension_for_mime(mime_type)?),
            read_builtin_bytes(bytes)?,
        ),
        (_, MetadataBackend::External) => {
            if !capabilities.external_available {
                return Err("ExifTool is not available; choose the built-in backend".to_owned());
            }
            match &job.source {
                MetadataSource::Path(path) => (display_name(path), read_external(path)?),
                MetadataSource::Image { bytes, mime_type } => {
                    let suffix = format!(".{}", extension_for_mime(mime_type)?);
                    let mut file = tempfile::Builder::new()
                        .suffix(&suffix)
                        .tempfile()
                        .map_err(|error| format!("failed to create temporary image: {error}"))?;
                    std::io::Write::write_all(&mut file, bytes)
                        .map_err(|error| format!("failed to write temporary image: {error}"))?;
                    (format!("clipboard{}", suffix), read_external(file.path())?)
                }
            }
        }
    };
    if fields.is_empty() {
        return Err("no readable metadata was found in this file".to_owned());
    }
    Ok(MetadataDocument {
        file_name,
        backend: job.backend,
        fields,
    })
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn read_builtin(path: &Path) -> Result<Vec<MetadataField>, String> {
    let path = path
        .to_str()
        .ok_or("the built-in metadata backend requires a UTF-8 path")?;
    let metadata = revelo::Metadata::from_file(path)
        .ok_or("the built-in metadata backend does not recognize this file")?;
    Ok(fields_from_builtin(&metadata))
}

fn read_builtin_bytes(bytes: &[u8]) -> Result<Vec<MetadataField>, String> {
    let metadata = revelo::Metadata::from_bytes(bytes)
        .ok_or("the built-in metadata backend does not recognize this image")?;
    Ok(fields_from_builtin(&metadata))
}

fn fields_from_builtin(metadata: &revelo::Metadata) -> Vec<MetadataField> {
    let mut fields = Vec::new();
    append_fields(&mut fields, "General", metadata.general());
    append_fields(&mut fields, "Video", metadata.video());
    append_fields(&mut fields, "Audio", metadata.audio());
    append_fields(&mut fields, "Text", metadata.text());
    append_fields(&mut fields, "Image", metadata.image());
    append_fields(&mut fields, "EXIF", metadata.exif());
    append_fields(&mut fields, "IPTC", metadata.iptc());
    append_fields(&mut fields, "XMP", metadata.xmp());
    fields
}

fn extension_for_mime(mime_type: &str) -> Result<&'static str, String> {
    match mime_type {
        "image/png" => Ok("png"),
        "image/jpeg" => Ok("jpg"),
        "image/bmp" => Ok("bmp"),
        "image/tiff" => Ok("tiff"),
        "image/webp" => Ok("webp"),
        "image/gif" => Ok("gif"),
        _ => Err(format!("unsupported clipboard image type: {mime_type}")),
    }
}

fn append_fields<'a>(
    fields: &mut Vec<MetadataField>,
    group: &str,
    values: impl Iterator<Item = (&'a str, &'a str)>,
) {
    fields.extend(values.map(|(name, value)| MetadataField {
        group: group.to_owned(),
        name: name.to_owned(),
        value: value.to_owned(),
    }));
}

fn read_external(path: &Path) -> Result<Vec<MetadataField>, String> {
    let mut child = Command::new("exiftool")
        .args(["-json", "-G1", "-s", "-api", "StructFormat=JSONQ"])
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to start ExifTool: {error}"))?;
    let (status, stdout, stderr) = collect_output(&mut child)?;
    if !status.success() {
        return Err(format!(
            "ExifTool failed: {}",
            String::from_utf8_lossy(&stderr).trim()
        ));
    }
    normalize_external_json(&stdout)
}

fn collect_output(
    child: &mut std::process::Child,
) -> Result<(std::process::ExitStatus, Vec<u8>, Vec<u8>), String> {
    let stdout = child
        .stdout
        .take()
        .ok_or("failed to open ExifTool stdout")?;
    let stderr = child
        .stderr
        .take()
        .ok_or("failed to open ExifTool stderr")?;
    let stdout_reader = thread::spawn(move || read_limited(stdout));
    let stderr_reader = thread::spawn(move || read_limited(stderr));
    let status = match child
        .wait_timeout(PROCESS_TIMEOUT)
        .map_err(|error| format!("failed to wait for ExifTool: {error}"))?
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
            return Err("ExifTool timed out after 30 seconds".to_owned());
        }
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| "ExifTool stdout reader panicked".to_owned())??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "ExifTool stderr reader panicked".to_owned())??;
    Ok((status, stdout, stderr))
}

fn read_limited(mut reader: impl Read) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    reader
        .by_ref()
        .take((MAX_OUTPUT_BYTES + 1) as u64)
        .read_to_end(&mut output)
        .map_err(|error| format!("failed to read ExifTool output: {error}"))?;
    if output.len() > MAX_OUTPUT_BYTES {
        return Err(format!("ExifTool output exceeds {MAX_OUTPUT_BYTES} bytes"));
    }
    Ok(output)
}

fn normalize_external_json(output: &[u8]) -> Result<Vec<MetadataField>, String> {
    let documents: Vec<serde_json::Map<String, serde_json::Value>> = serde_json::from_slice(output)
        .map_err(|error| format!("invalid ExifTool JSON: {error}"))?;
    let document = documents
        .into_iter()
        .next()
        .ok_or("ExifTool returned no result")?;
    let mut fields = document
        .into_iter()
        .map(|(key, value)| {
            let (group, name) = key.split_once(':').unwrap_or(("ExifTool", key.as_str()));
            MetadataField {
                group: group.to_owned(),
                name: name.to_owned(),
                value: json_value_to_string(value),
            }
        })
        .collect::<Vec<_>>();
    fields.sort_by(|left, right| {
        left.group
            .cmp(&right.group)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(fields)
}

fn json_value_to_string(value: serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value,
        serde_json::Value::Null => "null".to_owned(),
        other => serde_json::to_string(&other).expect("JSON value serialization cannot fail"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_sorts_exiftool_groups() {
        let fields = normalize_external_json(
            br#"[{"File:FileName":"movie.mp4","QuickTime:Duration":1.25,"Composite:GPS":[1,2]}]"#,
        )
        .expect("ExifTool JSON 应可规范化");

        assert_eq!(
            fields,
            vec![
                MetadataField {
                    group: "Composite".to_owned(),
                    name: "GPS".to_owned(),
                    value: "[1,2]".to_owned(),
                },
                MetadataField {
                    group: "File".to_owned(),
                    name: "FileName".to_owned(),
                    value: "movie.mp4".to_owned(),
                },
                MetadataField {
                    group: "QuickTime".to_owned(),
                    name: "Duration".to_owned(),
                    value: "1.25".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn reports_builtin_and_external_capabilities() {
        let capabilities = detect_capabilities();
        assert_eq!(capabilities.builtin_version, "revelo 0.5.5");
        assert_eq!(
            capabilities.external_available,
            capabilities.external_version.is_some()
        );
    }

    #[test]
    fn validates_clipboard_image_mime_types() {
        assert_eq!(extension_for_mime("image/jpeg"), Ok("jpg"));
        assert!(extension_for_mime("video/mp4").is_err());
    }

    #[test]
    fn reads_a_video_path_with_both_backends_when_tools_are_available() {
        if Command::new("ffmpeg").arg("-version").output().is_err()
            || Command::new("exiftool").arg("-ver").output().is_err()
        {
            return;
        }
        let directory = tempfile::tempdir().expect("应可创建临时目录");
        let video = directory.path().join("sample.mp4");
        let status = Command::new("ffmpeg")
            .args([
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=32x24:d=0.1",
                "-pix_fmt",
                "yuv420p",
                "-y",
            ])
            .arg(&video)
            .status()
            .expect("应可启动 ffmpeg");
        assert!(status.success());

        let builtin = read_builtin(&video).expect("内置后端应读取 MP4 路径");
        assert!(builtin.iter().any(|field| field.group == "Video"));
        let external = read_external(&video).expect("外部 ExifTool 应读取 MP4 路径");
        assert!(external.iter().any(|field| field.group == "QuickTime"));
    }
}

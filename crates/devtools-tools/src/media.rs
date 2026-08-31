use devtools_core::{Action, Context, Tool, ToolError, ToolRequest, ToolResult};

/// OCR 工具负责打开本地文字识别页面，图像处理由 Worker 执行。
#[derive(Debug, Default)]
pub struct OcrTool;

impl Tool for OcrTool {
    fn id(&self) -> &'static str {
        "ocr"
    }

    fn can_handle(&self, context: &Context, action: Action) -> bool {
        matches!(context, Context::Empty) && action == Action::RecognizeText
    }

    fn execute(&self, request: ToolRequest) -> Result<ToolResult, ToolError> {
        execute_empty_tool(self, request, "OCR")
    }
}

/// 条码工具同时承载条形码/二维码识别与生成页面。
#[derive(Debug, Default)]
pub struct BarcodeTool;

impl Tool for BarcodeTool {
    fn id(&self) -> &'static str {
        "barcode"
    }

    fn can_handle(&self, context: &Context, action: Action) -> bool {
        matches!(context, Context::Empty) && action == Action::ProcessBarcode
    }

    fn execute(&self, request: ToolRequest) -> Result<ToolResult, ToolError> {
        execute_empty_tool(self, request, "barcode")
    }
}

/// 图片压缩工具只负责打开页面；图像数据与编码始终留在 WebView 前端。
#[derive(Debug, Default)]
pub struct ImageCompressionTool;

impl Tool for ImageCompressionTool {
    fn id(&self) -> &'static str {
        "image-compress"
    }

    fn can_handle(&self, context: &Context, action: Action) -> bool {
        matches!(context, Context::Empty) && action == Action::CompressImage
    }

    fn execute(&self, request: ToolRequest) -> Result<ToolResult, ToolError> {
        execute_empty_tool(self, request, "image compression")
    }
}

/// 图片编辑工具只负责打开页面；实际编辑由 WebView 中的 TOAST UI 完成。
#[derive(Debug, Default)]
pub struct ImageEditorTool;

impl Tool for ImageEditorTool {
    fn id(&self) -> &'static str {
        "image-editor"
    }

    fn can_handle(&self, context: &Context, action: Action) -> bool {
        matches!(context, Context::Empty) && action == Action::EditImage
    }

    fn execute(&self, request: ToolRequest) -> Result<ToolResult, ToolError> {
        execute_empty_tool(self, request, "image editor")
    }
}

/// 图片水印工具只负责打开页面；渲染与编码完全在 WebView 前端完成。
#[derive(Debug, Default)]
pub struct WatermarkTool;

impl Tool for WatermarkTool {
    fn id(&self) -> &'static str {
        "watermark"
    }

    fn can_handle(&self, context: &Context, action: Action) -> bool {
        matches!(context, Context::Empty) && action == Action::WatermarkImage
    }

    fn execute(&self, request: ToolRequest) -> Result<ToolResult, ToolError> {
        execute_empty_tool(self, request, "image watermark")
    }
}

fn execute_empty_tool(
    tool: &impl Tool,
    request: ToolRequest,
    label: &str,
) -> Result<ToolResult, ToolError> {
    if !tool.can_handle(&request.context, request.action) {
        return Err(ToolError::new(format!(
            "{label} tool cannot handle this request"
        )));
    }
    Ok(ToolResult {
        payload: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ocr_accepts_empty_recognition_request() {
        let result = OcrTool
            .execute(ToolRequest {
                context: Context::Empty,
                action: Action::RecognizeText,
            })
            .expect("OCR 工具应接受空上下文");

        assert!(result.payload.is_empty());
    }

    #[test]
    fn barcode_rejects_wrong_action() {
        let result = BarcodeTool.execute(ToolRequest {
            context: Context::Empty,
            action: Action::Inspect,
        });

        assert!(result.is_err());
    }

    #[test]
    fn image_compression_accepts_empty_request() {
        let result = ImageCompressionTool
            .execute(ToolRequest {
                context: Context::Empty,
                action: Action::CompressImage,
            })
            .expect("图片压缩工具应接受空上下文");

        assert!(result.payload.is_empty());
    }

    #[test]
    fn image_editor_accepts_empty_request() {
        let result = ImageEditorTool
            .execute(ToolRequest {
                context: Context::Empty,
                action: Action::EditImage,
            })
            .expect("图片编辑工具应接受空上下文");

        assert!(result.payload.is_empty());
    }

    #[test]
    fn watermark_accepts_empty_request() {
        let result = WatermarkTool
            .execute(ToolRequest {
                context: Context::Empty,
                action: Action::WatermarkImage,
            })
            .expect("图片水印工具应接受空上下文");

        assert!(result.payload.is_empty());
    }
}

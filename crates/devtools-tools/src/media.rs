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
}

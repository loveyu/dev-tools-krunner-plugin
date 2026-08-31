use std::collections::HashMap;

use devtools_core::{Action, Context, Tool, ToolError, ToolRequest, ToolResult};
use devtools_tools::{
    BarcodeTool, ConvertTool, ImageCompressionTool, ImageEditorTool, JsonTool, OcrTool,
    WatermarkTool,
};

/// Worker 启动时注册的业务工具集合。
pub struct ToolRegistry {
    tools: HashMap<&'static str, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn standard() -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
        };
        registry.register(JsonTool);
        registry.register(ConvertTool);
        registry.register(OcrTool);
        registry.register(BarcodeTool);
        registry.register(ImageCompressionTool);
        registry.register(ImageEditorTool);
        registry.register(WatermarkTool);
        registry
    }

    pub fn execute(&self, tool_id: &str, payload: &str) -> Result<ToolResult, ToolError> {
        let tool = self
            .tools
            .get(tool_id)
            .ok_or_else(|| ToolError::new(format!("unknown tool: {tool_id}")))?;
        let context = match tool_id {
            "json" => Context::from_json_text(payload)
                .map_err(|error| ToolError::new(error.to_string()))?,
            "convert" => {
                Context::from_text(payload).map_err(|error| ToolError::new(error.to_string()))?
            }
            "ocr" | "barcode" | "image-compress" | "image-editor" | "watermark" => Context::Empty,
            _ => return Err(ToolError::new(format!("unsupported tool: {tool_id}"))),
        };
        let action = match tool_id {
            "json" => Action::Inspect,
            "convert" => Action::Convert,
            "ocr" => Action::RecognizeText,
            "barcode" => Action::ProcessBarcode,
            "image-compress" => Action::CompressImage,
            "image-editor" => Action::EditImage,
            "watermark" => Action::WatermarkImage,
            _ => return Err(ToolError::new(format!("unsupported tool: {tool_id}"))),
        };
        let request = ToolRequest { context, action };

        if !tool.can_handle(&request.context, request.action) {
            return Err(ToolError::new(format!(
                "tool {tool_id} cannot handle this request"
            )));
        }
        tool.execute(request)
    }

    fn register(&mut self, tool: impl Tool + 'static) {
        self.tools.insert(tool.id(), Box::new(tool));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_executes_json_tool() {
        let result = ToolRegistry::standard()
            .execute("json", "{\"a\":1}")
            .expect("JSON 请求应执行成功");

        assert_eq!(result.payload, "{\"a\":1}");
    }

    #[test]
    fn registry_rejects_unknown_tool() {
        assert!(ToolRegistry::standard().execute("unknown", "").is_err());
    }

    #[test]
    fn registry_executes_convert_tool() {
        let result = ToolRegistry::standard()
            .execute("convert", "a=1&b=2")
            .expect("转换请求应执行成功");

        assert_eq!(result.payload, "a=1&b=2");
    }

    #[test]
    fn registry_executes_media_tools_without_payload() {
        assert_eq!(
            ToolRegistry::standard()
                .execute("ocr", "ignored")
                .expect("OCR 请求应执行成功")
                .payload,
            ""
        );
        assert_eq!(
            ToolRegistry::standard()
                .execute("watermark", "ignored")
                .expect("图片水印请求应执行成功")
                .payload,
            ""
        );
        assert_eq!(
            ToolRegistry::standard()
                .execute("image-editor", "ignored")
                .expect("图片编辑请求应执行成功")
                .payload,
            ""
        );
        assert_eq!(
            ToolRegistry::standard()
                .execute("barcode", "ignored")
                .expect("条码请求应执行成功")
                .payload,
            ""
        );
        assert_eq!(
            ToolRegistry::standard()
                .execute("image-compress", "ignored")
                .expect("图片压缩请求应执行成功")
                .payload,
            ""
        );
    }
}

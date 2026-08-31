use devtools_core::{Action, Context, Tool, ToolError, ToolRequest, ToolResult};

/// JSON 工具只负责验证与准备业务数据，展示和交互由 Web 前端完成。
#[derive(Debug, Default)]
pub struct JsonTool;

impl Tool for JsonTool {
    fn id(&self) -> &'static str {
        "json"
    }

    fn can_handle(&self, context: &Context, action: Action) -> bool {
        matches!(context, Context::Json { .. }) && action == Action::Inspect
    }

    fn execute(&self, request: ToolRequest) -> Result<ToolResult, ToolError> {
        if !self.can_handle(&request.context, request.action) {
            return Err(ToolError::new("JSON tool cannot handle this request"));
        }

        Ok(ToolResult {
            payload: request.context.raw_text().to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use devtools_core::Context;

    use super::*;

    #[test]
    fn prepares_json_payload_without_changing_formatting() {
        let raw = "{\n  \"name\": \"loveyu\"\n}";
        let result = JsonTool
            .execute(ToolRequest {
                context: Context::from_json_text(raw).expect("JSON 应可解析"),
                action: Action::Inspect,
            })
            .expect("JSON 工具应接受检查请求");

        assert_eq!(result.payload, raw);
    }
}

use devtools_core::{Action, Context, Tool, ToolError, ToolRequest, ToolResult};

/// 转换工具只负责把普通文本上下文交给 Web 前端；具体转换由 TS 或 native IPC 完成。
#[derive(Debug, Default)]
pub struct ConvertTool;

impl Tool for ConvertTool {
    fn id(&self) -> &'static str {
        "convert"
    }

    fn can_handle(&self, context: &Context, action: Action) -> bool {
        matches!(context, Context::Text { .. }) && action == Action::Convert
    }

    fn execute(&self, request: ToolRequest) -> Result<ToolResult, ToolError> {
        if !self.can_handle(&request.context, request.action) {
            return Err(ToolError::new("convert tool cannot handle this request"));
        }

        Ok(ToolResult {
            payload: request.context.raw_text().to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepares_text_payload_without_changing_it() {
        let result = ConvertTool
            .execute(ToolRequest {
                context: Context::from_text("a=1&b=2").expect("文本应有效"),
                action: Action::Convert,
            })
            .expect("转换工具应接受普通文本");

        assert_eq!(result.payload, "a=1&b=2");
    }
}

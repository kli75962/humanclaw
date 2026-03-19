mod dispatch;
mod memory;
mod phone;

pub use memory::{build_core_prompt, read_core};
pub use dispatch::{execute_tool_with_context, ToolExecutionContext, ToolResult};

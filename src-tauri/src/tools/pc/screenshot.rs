use serde_json::Value;

use crate::tools::types::ToolResult;

pub fn execute(tool_name: &str, args: &Value) -> ToolResult {
    let display = args.get("display").and_then(Value::as_u64).unwrap_or(0) as usize;
    match capture_png(display) {
        Ok(png) => {
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
            ToolResult {
                tool_name: tool_name.to_string(),
                success: true,
                output: format!("data:image/png;base64,{b64}"),
            }
        }
        Err(e) => ToolResult { tool_name: tool_name.to_string(), success: false, output: e },
    }
}

fn capture_png(display: usize) -> Result<Vec<u8>, String> {
    use screenshots::image::{DynamicImage, ImageFormat};
    use std::io::Cursor;

    let screens = screenshots::Screen::all().map_err(|e| e.to_string())?;
    let screen  = screens.get(display).ok_or_else(|| format!("Display {display} not found"))?;
    let image   = screen.capture().map_err(|e| e.to_string())?;

    let dynamic = DynamicImage::ImageRgba8(image);
    let mut png = Vec::new();
    dynamic
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(png)
}

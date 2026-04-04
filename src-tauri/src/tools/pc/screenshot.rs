use serde_json::Value;

use crate::tools::types::ToolResult;

pub fn execute(tool_name: &str, args: &Value) -> ToolResult {
    let display = args.get("display").and_then(Value::as_u64).unwrap_or(0) as usize;
    match capture_jpg(display) {
        Ok(jpg) => {
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&jpg);
            ToolResult::ok(tool_name, format!("data:image/jpeg;base64,{b64}"))
        }
        Err(e) => ToolResult::err(tool_name, "EXECUTION_FAILED", e),
    }
}

fn capture_jpg(display: usize) -> Result<Vec<u8>, String> {
    use screenshots::image::{DynamicImage, codecs::jpeg::JpegEncoder, imageops::FilterType};

    let screens = screenshots::Screen::all().map_err(|e| e.to_string())?;
    let screen  = screens.get(display).ok_or_else(|| format!("Display {display} not found"))?;
    let image   = screen.capture().map_err(|e| e.to_string())?;

    let dynamic = DynamicImage::ImageRgba8(image);

    // Scale to max 512px wide. Smaller image = fewer tokens, avoids context overflow.
    let scaled = if dynamic.width() > 512 {
        dynamic.resize(512, u32::MAX, FilterType::Triangle)
    } else {
        dynamic
    };

    let rgb = scaled.into_rgb8();
    let mut jpg = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut jpg, 70);
    encoder.encode_image(&rgb).map_err(|e| e.to_string())?;
    Ok(jpg)
}

use screenshots::image::{DynamicImage, ImageFormat};
use std::io::Cursor;

fn main() {
    let screens = screenshots::Screen::all().expect("failed to list screens");
    println!("Found {} screen(s)", screens.len());

    for (i, screen) in screens.iter().enumerate() {
        let info = screen.display_info;
        println!(
            "Screen {i}: {}x{} @ ({}, {}) scale={:.1}",
            info.width, info.height, info.x, info.y, info.scale_factor
        );

        let image = screen.capture().expect("capture failed");
        let dynamic = DynamicImage::ImageRgba8(image);
        let mut png = Vec::new();
        dynamic
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .expect("encode failed");

        let path = format!("/tmp/screen_{i}.png");
        std::fs::write(&path, &png).expect("write failed");
        println!("  → saved to {path} ({} bytes)", png.len());
    }

    println!("\nOpen with:  feh /tmp/screen_0.png");
    println!("       or:  xdg-open /tmp/screen_0.png");
}

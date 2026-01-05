//! Test the image analyzer with a generated UI screenshot.
//!
//! Run with: cargo run --example test_analyze

use image::{ImageBuffer, Rgba, RgbaImage};
use std::io::Cursor;

fn main() {
    println!("Generating test UI screenshot...\n");

    // Create a 400x300 image with a card-like UI
    let mut img: RgbaImage = ImageBuffer::new(400, 300);

    // Fill background (light gray)
    for pixel in img.pixels_mut() {
        *pixel = Rgba([240, 240, 245, 255]);
    }

    // Draw a card (white rectangle with shadow effect)
    draw_rect(&mut img, 40, 30, 320, 240, [255, 255, 255, 255]);

    // Draw card header area (blue gradient approximation)
    draw_rect(&mut img, 40, 30, 320, 60, [66, 133, 244, 255]);

    // Draw "text" lines (dark gray rectangles simulating text)
    draw_rect(&mut img, 60, 110, 200, 12, [50, 50, 50, 255]);
    draw_rect(&mut img, 60, 135, 280, 10, [100, 100, 100, 255]);
    draw_rect(&mut img, 60, 155, 250, 10, [100, 100, 100, 255]);
    draw_rect(&mut img, 60, 175, 270, 10, [100, 100, 100, 255]);

    // Draw a button (rounded would be nice, but solid works)
    draw_rect(&mut img, 240, 220, 100, 36, [66, 133, 244, 255]);

    // Draw a circular icon (approximated with a filled square for now)
    draw_circle(&mut img, 80, 50, 15, [255, 255, 255, 255]);

    // Draw a divider line
    draw_rect(&mut img, 60, 195, 280, 2, [220, 220, 220, 255]);

    // Encode to PNG bytes
    let mut png_bytes = Vec::new();
    {
        let mut cursor = Cursor::new(&mut png_bytes);
        img.write_to(&mut cursor, image::ImageFormat::Png)
            .expect("Failed to encode PNG");
    }

    println!("Generated {}x{} PNG image ({} bytes)\n",
             img.width(), img.height(), png_bytes.len());

    // Save to file for inspection
    img.save("/tmp/test_ui_screenshot.png").expect("Failed to save test image");
    println!("Saved test image to /tmp/test_ui_screenshot.png\n");

    // Run the analyzer
    println!("Running analyzer...\n");

    match seed_analyze::analyze_image(&png_bytes) {
        Ok(seed_code) => {
            println!("=== Generated Seed Code ===\n");
            println!("{}", seed_code);
            println!("\n=== Analysis Complete ===");
        }
        Err(e) => {
            eprintln!("Analysis failed: {}", e);
        }
    }
}

fn draw_rect(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, color: [u8; 4]) {
    for dy in 0..h {
        for dx in 0..w {
            let px = x + dx;
            let py = y + dy;
            if px < img.width() && py < img.height() {
                img.put_pixel(px, py, Rgba(color));
            }
        }
    }
}

fn draw_circle(img: &mut RgbaImage, cx: u32, cy: u32, r: u32, color: [u8; 4]) {
    let r_sq = (r * r) as i32;
    for dy in 0..=r * 2 {
        for dx in 0..=r * 2 {
            let px = cx + dx - r;
            let py = cy + dy - r;
            let dist_x = dx as i32 - r as i32;
            let dist_y = dy as i32 - r as i32;
            let dist_sq = dist_x * dist_x + dist_y * dist_y;

            if dist_sq <= r_sq && px < img.width() && py < img.height() {
                img.put_pixel(px, py, Rgba(color));
            }
        }
    }
}

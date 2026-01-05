//! Test with a more complex UI including gradients and rounded corners.
//!
//! Run with: cargo run --example test_complex

use image::{ImageBuffer, Rgba, RgbaImage};
use std::io::Cursor;

fn main() {
    println!("Generating complex UI screenshot...\n");

    // Create a 500x400 image
    let mut img: RgbaImage = ImageBuffer::new(500, 400);

    // Fill background with gradient (top to bottom, light blue to white)
    for y in 0..400 {
        let t = y as f32 / 400.0;
        let r = lerp(230, 255, t) as u8;
        let g = lerp(240, 255, t) as u8;
        let b = lerp(250, 255, t) as u8;
        for x in 0..500 {
            img.put_pixel(x, y, Rgba([r, g, b, 255]));
        }
    }

    // Draw card with shadow effect (darker region below/right)
    // Shadow
    draw_rounded_rect(&mut img, 55, 55, 390, 290, 12, [180, 180, 190, 255]);
    // Card
    draw_rounded_rect(&mut img, 50, 50, 390, 290, 12, [255, 255, 255, 255]);

    // Draw header with gradient (purple to blue)
    for y in 60..120 {
        let t = (y - 60) as f32 / 60.0;
        let r = lerp(102, 66, t) as u8;
        let g = lerp(126, 133, t) as u8;
        let b = lerp(234, 244, t) as u8;
        for x in 60..430 {
            // Skip corners
            let dx = if x < 72 { 72 - x } else if x > 418 { x - 418 } else { 0 };
            let dy = if y < 62 { 62 - y } else { 0 };
            if dx * dx + dy * dy <= 144 || dy == 0 {
                img.put_pixel(x, y, Rgba([r, g, b, 255]));
            }
        }
    }

    // Draw circular avatar
    draw_circle(&mut img, 100, 90, 20, [255, 255, 255, 255]);
    draw_circle(&mut img, 100, 90, 18, [200, 200, 220, 255]);

    // Draw title text (simulated with rectangles)
    draw_rect(&mut img, 140, 80, 180, 16, [255, 255, 255, 255]);

    // Draw subtitle
    draw_rect(&mut img, 140, 102, 120, 10, [220, 220, 240, 255]);

    // Draw content area - multiple lines simulating text
    draw_rect(&mut img, 70, 140, 340, 14, [60, 60, 70, 255]);
    draw_rect(&mut img, 70, 165, 350, 12, [100, 100, 110, 255]);
    draw_rect(&mut img, 70, 185, 320, 12, [100, 100, 110, 255]);
    draw_rect(&mut img, 70, 205, 330, 12, [100, 100, 110, 255]);

    // Draw horizontal divider
    draw_rect(&mut img, 70, 235, 350, 1, [220, 220, 230, 255]);

    // Draw action buttons row
    // Primary button (rounded, blue)
    draw_rounded_rect(&mut img, 280, 260, 130, 44, 8, [66, 133, 244, 255]);
    // Secondary button (outline style - just border)
    draw_rounded_rect_outline(&mut img, 140, 260, 120, 44, 8, 2, [66, 133, 244, 255]);

    // Draw small icons (circles)
    draw_circle(&mut img, 85, 280, 12, [150, 150, 160, 255]);
    draw_circle(&mut img, 115, 280, 12, [150, 150, 160, 255]);

    // Encode to PNG
    let mut png_bytes = Vec::new();
    {
        let mut cursor = Cursor::new(&mut png_bytes);
        img.write_to(&mut cursor, image::ImageFormat::Png)
            .expect("Failed to encode PNG");
    }

    println!("Generated {}x{} PNG image ({} bytes)\n",
             img.width(), img.height(), png_bytes.len());

    // Save to file
    img.save("/tmp/test_complex_ui.png").expect("Failed to save");
    println!("Saved to /tmp/test_complex_ui.png\n");

    // Run analyzer
    println!("Running analyzer...\n");

    match seed_analyze::analyze_image(&png_bytes) {
        Ok(seed_code) => {
            println!("=== Generated Seed Code ===\n");
            println!("{}", seed_code);

            // Count elements
            let frame_count = seed_code.matches("Frame:").count();
            let text_count = seed_code.matches("Text:").count();
            println!("\n=== Statistics ===");
            println!("Frames detected: {}", frame_count);
            println!("Text regions: {}", text_count);
            println!("Total elements: {}", frame_count + text_count);
        }
        Err(e) => {
            eprintln!("Analysis failed: {}", e);
        }
    }
}

fn lerp(a: u32, b: u32, t: f32) -> u32 {
    (a as f32 * (1.0 - t) + b as f32 * t) as u32
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

fn draw_rounded_rect(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, r: u32, color: [u8; 4]) {
    for dy in 0..h {
        for dx in 0..w {
            let px = x + dx;
            let py = y + dy;
            if px >= img.width() || py >= img.height() {
                continue;
            }

            // Check if in corner region
            let in_corner = |cx: u32, cy: u32| -> bool {
                let dist_x = if dx < r { r - dx } else if dx >= w - r { dx - (w - r - 1) } else { 0 };
                let dist_y = if dy < r { r - dy } else if dy >= h - r { dy - (h - r - 1) } else { 0 };
                dist_x > 0 && dist_y > 0 && dist_x * dist_x + dist_y * dist_y > r * r
            };

            if !in_corner(dx, dy) {
                img.put_pixel(px, py, Rgba(color));
            }
        }
    }
}

fn draw_rounded_rect_outline(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, r: u32, thickness: u32, color: [u8; 4]) {
    // Draw outer rounded rect then "cut out" inner
    for dy in 0..h {
        for dx in 0..w {
            let px = x + dx;
            let py = y + dy;
            if px >= img.width() || py >= img.height() {
                continue;
            }

            // Check if on border
            let on_border = dx < thickness || dx >= w - thickness || dy < thickness || dy >= h - thickness;

            // Check corners
            let in_corner = || -> bool {
                let dist_x = if dx < r { r - dx } else if dx >= w - r { dx - (w - r - 1) } else { 0 };
                let dist_y = if dy < r { r - dy } else if dy >= h - r { dy - (h - r - 1) } else { 0 };
                dist_x > 0 && dist_y > 0 && dist_x * dist_x + dist_y * dist_y > r * r
            };

            if on_border && !in_corner() {
                img.put_pixel(px, py, Rgba(color));
            }
        }
    }
}

fn draw_circle(img: &mut RgbaImage, cx: u32, cy: u32, r: u32, color: [u8; 4]) {
    let r_sq = (r * r) as i32;
    for dy in 0..=r * 2 {
        for dx in 0..=r * 2 {
            let px = cx.saturating_add(dx).saturating_sub(r);
            let py = cy.saturating_add(dy).saturating_sub(r);
            let dist_x = dx as i32 - r as i32;
            let dist_y = dy as i32 - r as i32;
            let dist_sq = dist_x * dist_x + dist_y * dist_y;

            if dist_sq <= r_sq && px < img.width() && py < img.height() {
                img.put_pixel(px, py, Rgba(color));
            }
        }
    }
}

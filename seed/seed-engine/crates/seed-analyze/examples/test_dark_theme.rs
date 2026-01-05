//! Test the image analyzer with a dark-themed UI screenshot.
//!
//! This tests the adaptive pipeline including:
//! - Theme detection
//! - CLAHE preprocessing
//! - Edge-constrained flood fill
//!
//! Run with: cargo run --example test_dark_theme

use image::{ImageBuffer, Rgba, RgbaImage};
use std::io::Cursor;

fn main() {
    println!("Generating dark theme UI screenshot...\n");

    // Create a 400x300 image with a dark-themed UI
    let mut img: RgbaImage = ImageBuffer::new(400, 300);

    // Fill background with very dark color (simulating app background)
    for pixel in img.pixels_mut() {
        *pixel = Rgba([18, 18, 22, 255]); // Very dark gray
    }

    // Draw a dark card (slightly lighter than background)
    draw_rounded_rect(&mut img, 30, 30, 340, 240, 12, [28, 28, 35, 255]);

    // Draw card header area (dark purple/blue gradient approximation)
    for y in 40..100 {
        let t = (y - 40) as f32 / 60.0;
        let r = lerp(45, 35, t) as u8;
        let g = lerp(45, 40, t) as u8;
        let b = lerp(70, 60, t) as u8;
        for x in 40..360 {
            // Skip corners
            let in_corner = (x < 52 || x > 348) && y < 42;
            if !in_corner {
                img.put_pixel(x, y, Rgba([r, g, b, 255]));
            }
        }
    }

    // Draw avatar circle (purple accent)
    draw_circle(&mut img, 80, 70, 20, [100, 80, 180, 255]);
    draw_circle(&mut img, 80, 70, 17, [120, 100, 200, 255]);

    // Draw "text" lines (light gray on dark - simulating content)
    draw_rect(&mut img, 120, 55, 160, 12, [200, 200, 210, 255]); // Title
    draw_rect(&mut img, 120, 75, 120, 8, [140, 140, 150, 255]); // Subtitle

    // Draw content text lines (medium gray)
    draw_rect(&mut img, 50, 120, 300, 10, [160, 160, 170, 255]);
    draw_rect(&mut img, 50, 145, 280, 10, [130, 130, 140, 255]);
    draw_rect(&mut img, 50, 165, 290, 10, [130, 130, 140, 255]);

    // Draw horizontal divider
    draw_rect(&mut img, 50, 195, 300, 1, [50, 50, 60, 255]);

    // Draw action buttons
    // Primary button (purple accent)
    draw_rounded_rect(&mut img, 250, 215, 110, 40, 8, [100, 80, 180, 255]);
    // Secondary button (outline on dark - just border)
    draw_rounded_rect_outline(&mut img, 130, 215, 100, 40, 8, 2, [100, 80, 180, 255]);

    // Draw small icons (circles with dark theme colors)
    draw_circle(&mut img, 60, 235, 10, [70, 70, 80, 255]);
    draw_circle(&mut img, 90, 235, 10, [70, 70, 80, 255]);

    // Encode to PNG
    let mut png_bytes = Vec::new();
    {
        let mut cursor = Cursor::new(&mut png_bytes);
        img.write_to(&mut cursor, image::ImageFormat::Png)
            .expect("Failed to encode PNG");
    }

    println!(
        "Generated {}x{} dark theme PNG ({} bytes)\n",
        img.width(),
        img.height(),
        png_bytes.len()
    );

    // Save to file for inspection
    img.save("/tmp/test_dark_theme_ui.png")
        .expect("Failed to save");
    println!("Saved to /tmp/test_dark_theme_ui.png\n");

    // Run analyzer
    println!("Running analyzer (adaptive pipeline)...\n");

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

            // Check for dark theme detection
            if frame_count > 3 {
                println!("\n✓ Success: Multiple regions detected in dark theme!");
            } else {
                println!("\n✗ Warning: Few regions detected - may need tuning");
            }
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

fn draw_rounded_rect(
    img: &mut RgbaImage,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    r: u32,
    color: [u8; 4],
) {
    for dy in 0..h {
        for dx in 0..w {
            let px = x + dx;
            let py = y + dy;
            if px >= img.width() || py >= img.height() {
                continue;
            }

            // Check if in corner region
            let in_corner = || -> bool {
                let dist_x = if dx < r {
                    r - dx
                } else if dx >= w - r {
                    dx - (w - r - 1)
                } else {
                    0
                };
                let dist_y = if dy < r {
                    r - dy
                } else if dy >= h - r {
                    dy - (h - r - 1)
                } else {
                    0
                };
                dist_x > 0 && dist_y > 0 && dist_x * dist_x + dist_y * dist_y > r * r
            };

            if !in_corner() {
                img.put_pixel(px, py, Rgba(color));
            }
        }
    }
}

fn draw_rounded_rect_outline(
    img: &mut RgbaImage,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    r: u32,
    thickness: u32,
    color: [u8; 4],
) {
    for dy in 0..h {
        for dx in 0..w {
            let px = x + dx;
            let py = y + dy;
            if px >= img.width() || py >= img.height() {
                continue;
            }

            // Check if on border
            let on_border =
                dx < thickness || dx >= w - thickness || dy < thickness || dy >= h - thickness;

            // Check corners
            let in_corner = || -> bool {
                let dist_x = if dx < r {
                    r - dx
                } else if dx >= w - r {
                    dx - (w - r - 1)
                } else {
                    0
                };
                let dist_y = if dy < r {
                    r - dy
                } else if dy >= h - r {
                    dy - (h - r - 1)
                } else {
                    0
                };
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

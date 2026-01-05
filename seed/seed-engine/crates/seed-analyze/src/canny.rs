//! Canny edge detection for precise edge extraction.
//!
//! The Canny algorithm produces thin, clean edges ideal for shape detection:
//! 1. Gaussian blur to reduce noise
//! 2. Sobel gradients for edge direction
//! 3. Non-maximum suppression to thin edges
//! 4. Hysteresis thresholding to connect edges
//!
//! Enhanced features for dark theme support:
//! - Theme detection (light vs dark based on average luminance)
//! - CLAHE preprocessing for dark images
//! - Adaptive threshold scaling

use std::f32::consts::PI;

/// Detected theme of an image.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImageTheme {
    Light,
    Dark,
}

/// Result of theme detection.
#[derive(Debug, Clone)]
pub struct ThemeInfo {
    pub theme: ImageTheme,
    pub avg_luminance: f32,
    /// Recommended threshold multiplier (lower for dark themes).
    pub threshold_multiplier: f32,
}

/// Detect whether an image is light or dark themed.
///
/// Uses average luminance to determine theme. Dark UIs typically have
/// average luminance below 50 (on 0-255 scale).
pub fn detect_theme(pixels: &[[u8; 4]]) -> ThemeInfo {
    if pixels.is_empty() {
        return ThemeInfo {
            theme: ImageTheme::Light,
            avg_luminance: 128.0,
            threshold_multiplier: 1.0,
        };
    }

    let avg_lum = pixels
        .iter()
        .map(|p| 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32)
        .sum::<f32>()
        / pixels.len() as f32;

    let (theme, multiplier) = if avg_lum < 50.0 {
        // Very dark - needs significant adjustment
        (ImageTheme::Dark, 0.5)
    } else if avg_lum < 80.0 {
        // Moderately dark
        (ImageTheme::Dark, 0.7)
    } else {
        // Light theme - standard parameters
        (ImageTheme::Light, 1.0)
    };

    ThemeInfo {
        theme,
        avg_luminance: avg_lum,
        threshold_multiplier: multiplier,
    }
}

/// Apply CLAHE (Contrast Limited Adaptive Histogram Equalization).
///
/// CLAHE enhances local contrast without over-amplifying noise, which is
/// critical for detecting subtle edges in dark UI elements.
///
/// # Arguments
/// * `grayscale` - Grayscale pixel values (0.0-255.0)
/// * `width` - Image width
/// * `height` - Image height
/// * `tile_size` - Size of tiles for local histogram (default: 8)
/// * `clip_limit` - Contrast limit to prevent over-amplification (default: 2.0)
pub fn apply_clahe(
    grayscale: &[f32],
    width: u32,
    height: u32,
    tile_size: u32,
    clip_limit: f32,
) -> Vec<f32> {
    let w = width as usize;
    let h = height as usize;
    let ts = tile_size.max(4) as usize;

    // Calculate number of tiles
    let tiles_x = (w + ts - 1) / ts;
    let tiles_y = (h + ts - 1) / ts;

    // Compute histogram and CDF for each tile
    let mut tile_cdfs: Vec<Vec<f32>> = Vec::with_capacity(tiles_x * tiles_y);

    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let x_start = tx * ts;
            let y_start = ty * ts;
            let x_end = (x_start + ts).min(w);
            let y_end = (y_start + ts).min(h);

            // Build histogram for this tile
            let mut histogram = [0u32; 256];
            let mut pixel_count = 0u32;

            for y in y_start..y_end {
                for x in x_start..x_end {
                    let val = grayscale[y * w + x].clamp(0.0, 255.0) as usize;
                    histogram[val] += 1;
                    pixel_count += 1;
                }
            }

            if pixel_count == 0 {
                tile_cdfs.push(vec![0.0; 256]);
                continue;
            }

            // Clip histogram (redistribute excess)
            let clip_threshold = (clip_limit * pixel_count as f32 / 256.0) as u32;
            let mut excess = 0u32;

            for bin in histogram.iter_mut() {
                if *bin > clip_threshold {
                    excess += *bin - clip_threshold;
                    *bin = clip_threshold;
                }
            }

            // Redistribute excess uniformly
            let redistrib = excess / 256;
            let remainder = excess % 256;
            for (i, bin) in histogram.iter_mut().enumerate() {
                *bin += redistrib;
                if (i as u32) < remainder {
                    *bin += 1;
                }
            }

            // Compute CDF
            let mut cdf = vec![0.0f32; 256];
            let mut cumsum = 0u32;
            for i in 0..256 {
                cumsum += histogram[i];
                cdf[i] = (cumsum as f32 * 255.0) / pixel_count as f32;
            }

            tile_cdfs.push(cdf);
        }
    }

    // Apply CLAHE with bilinear interpolation between tiles
    let mut output = vec![0.0f32; grayscale.len()];

    for y in 0..h {
        for x in 0..w {
            let val = grayscale[y * w + x].clamp(0.0, 255.0) as usize;

            // Find tile position and interpolation weights
            let tx_f = (x as f32 / ts as f32) - 0.5;
            let ty_f = (y as f32 / ts as f32) - 0.5;

            let tx0 = (tx_f.floor() as isize).max(0) as usize;
            let ty0 = (ty_f.floor() as isize).max(0) as usize;
            let tx1 = (tx0 + 1).min(tiles_x - 1);
            let ty1 = (ty0 + 1).min(tiles_y - 1);

            let wx = (tx_f - tx0 as f32).clamp(0.0, 1.0);
            let wy = (ty_f - ty0 as f32).clamp(0.0, 1.0);

            // Bilinear interpolation of CDF values
            let cdf_00 = tile_cdfs[ty0 * tiles_x + tx0][val];
            let cdf_10 = tile_cdfs[ty0 * tiles_x + tx1][val];
            let cdf_01 = tile_cdfs[ty1 * tiles_x + tx0][val];
            let cdf_11 = tile_cdfs[ty1 * tiles_x + tx1][val];

            let top = cdf_00 * (1.0 - wx) + cdf_10 * wx;
            let bottom = cdf_01 * (1.0 - wx) + cdf_11 * wx;
            output[y * w + x] = top * (1.0 - wy) + bottom * wy;
        }
    }

    output
}

/// Perform adaptive Canny edge detection that adjusts for dark themes.
///
/// This version detects the image theme and applies CLAHE preprocessing
/// for dark images to enhance subtle edges before edge detection.
pub fn adaptive_canny_edge_detection(
    pixels: &[[u8; 4]],
    width: u32,
    height: u32,
    base_low_threshold: f32,
    base_high_threshold: f32,
    use_clahe: bool,
) -> CannyResult {
    // Detect theme
    let theme_info = detect_theme(pixels);

    // Convert to grayscale
    let grayscale = to_grayscale(pixels);

    // Apply CLAHE for dark themes
    let preprocessed = if use_clahe && theme_info.theme == ImageTheme::Dark {
        apply_clahe(&grayscale, width, height, 8, 2.5)
    } else {
        grayscale
    };

    // Gaussian blur
    let blurred = gaussian_blur(&preprocessed, width, height);

    // Sobel gradients
    let (grad_x, grad_y) = sobel_gradients(&blurred, width, height);

    // Gradient magnitude and direction
    let (magnitude, direction) = compute_gradient_magnitude_direction(&grad_x, &grad_y);

    // Adjust thresholds for dark themes
    let low_threshold = base_low_threshold * theme_info.threshold_multiplier;
    let high_threshold = base_high_threshold * theme_info.threshold_multiplier;

    // Non-maximum suppression
    let suppressed = non_maximum_suppression(&magnitude, &direction, width, height);

    // Hysteresis thresholding
    let edges = hysteresis_threshold(&suppressed, width, height, low_threshold, high_threshold);

    CannyResult {
        edges,
        magnitude,
        direction,
        grad_x,
        grad_y,
        width,
        height,
    }
}

/// Result of Canny edge detection including gradient information.
pub struct CannyResult {
    /// Binary edge map (true = edge pixel)
    pub edges: Vec<bool>,
    /// Gradient magnitude at each pixel
    pub magnitude: Vec<f32>,
    /// Gradient direction at each pixel (radians, 0 = horizontal right)
    pub direction: Vec<f32>,
    /// Gradient X component (for SWT)
    pub grad_x: Vec<f32>,
    /// Gradient Y component (for SWT)
    pub grad_y: Vec<f32>,
    pub width: u32,
    pub height: u32,
}

/// Perform Canny edge detection on an image.
///
/// # Arguments
/// * `pixels` - RGBA pixel data
/// * `width` - Image width
/// * `height` - Image height
/// * `low_threshold` - Lower threshold for hysteresis (e.g., 20-50)
/// * `high_threshold` - Upper threshold for hysteresis (e.g., 50-150)
pub fn canny_edge_detection(
    pixels: &[[u8; 4]],
    width: u32,
    height: u32,
    low_threshold: f32,
    high_threshold: f32,
) -> CannyResult {
    // Step 1: Convert to grayscale
    let grayscale = to_grayscale(pixels);

    // Step 2: Gaussian blur (5x5 kernel)
    let blurred = gaussian_blur(&grayscale, width, height);

    // Step 3: Sobel gradients
    let (grad_x, grad_y) = sobel_gradients(&blurred, width, height);

    // Step 4: Compute magnitude and direction
    let (magnitude, direction) = compute_gradient_magnitude_direction(&grad_x, &grad_y);

    // Step 5: Non-maximum suppression
    let suppressed = non_maximum_suppression(&magnitude, &direction, width, height);

    // Step 6: Hysteresis thresholding
    let edges = hysteresis_threshold(&suppressed, width, height, low_threshold, high_threshold);

    CannyResult {
        edges,
        magnitude,
        direction,
        grad_x,
        grad_y,
        width,
        height,
    }
}

/// Convert RGBA pixels to grayscale (0.0-255.0).
fn to_grayscale(pixels: &[[u8; 4]]) -> Vec<f32> {
    pixels
        .iter()
        .map(|p| 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32)
        .collect()
}

/// Apply 5x5 Gaussian blur.
fn gaussian_blur(input: &[f32], width: u32, height: u32) -> Vec<f32> {
    // 5x5 Gaussian kernel (sigma ≈ 1.4)
    #[rustfmt::skip]
    const KERNEL: [[f32; 5]; 5] = [
        [2.0/159.0,  4.0/159.0,  5.0/159.0,  4.0/159.0, 2.0/159.0],
        [4.0/159.0,  9.0/159.0, 12.0/159.0,  9.0/159.0, 4.0/159.0],
        [5.0/159.0, 12.0/159.0, 15.0/159.0, 12.0/159.0, 5.0/159.0],
        [4.0/159.0,  9.0/159.0, 12.0/159.0,  9.0/159.0, 4.0/159.0],
        [2.0/159.0,  4.0/159.0,  5.0/159.0,  4.0/159.0, 2.0/159.0],
    ];

    let mut output = vec![0.0f32; input.len()];

    for y in 2..height.saturating_sub(2) {
        for x in 2..width.saturating_sub(2) {
            let mut sum = 0.0;
            for ky in 0..5 {
                for kx in 0..5 {
                    let px = (x as i32 + kx as i32 - 2) as usize;
                    let py = (y as i32 + ky as i32 - 2) as usize;
                    sum += input[py * width as usize + px] * KERNEL[ky][kx];
                }
            }
            output[(y * width + x) as usize] = sum;
        }
    }

    // Copy edges
    for y in 0..height {
        for x in 0..width {
            if y < 2 || y >= height - 2 || x < 2 || x >= width - 2 {
                output[(y * width + x) as usize] = input[(y * width + x) as usize];
            }
        }
    }

    output
}

/// Compute Sobel gradients (Gx, Gy).
fn sobel_gradients(input: &[f32], width: u32, height: u32) -> (Vec<f32>, Vec<f32>) {
    let mut grad_x = vec![0.0f32; input.len()];
    let mut grad_y = vec![0.0f32; input.len()];

    // Sobel kernels
    // Gx: [-1, 0, 1; -2, 0, 2; -1, 0, 1]
    // Gy: [-1, -2, -1; 0, 0, 0; 1, 2, 1]

    for y in 1..height.saturating_sub(1) {
        for x in 1..width.saturating_sub(1) {
            let idx = |dx: i32, dy: i32| -> f32 {
                let nx = (x as i32 + dx) as usize;
                let ny = (y as i32 + dy) as usize;
                input[ny * width as usize + nx]
            };

            // Horizontal gradient (detects vertical edges)
            let gx = -idx(-1, -1) + idx(1, -1) - 2.0 * idx(-1, 0) + 2.0 * idx(1, 0) - idx(-1, 1)
                + idx(1, 1);

            // Vertical gradient (detects horizontal edges)
            let gy = -idx(-1, -1) - 2.0 * idx(0, -1) - idx(1, -1) + idx(-1, 1)
                + 2.0 * idx(0, 1)
                + idx(1, 1);

            let i = (y * width + x) as usize;
            grad_x[i] = gx;
            grad_y[i] = gy;
        }
    }

    (grad_x, grad_y)
}

/// Compute gradient magnitude and direction.
fn compute_gradient_magnitude_direction(grad_x: &[f32], grad_y: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let magnitude: Vec<f32> = grad_x
        .iter()
        .zip(grad_y.iter())
        .map(|(&gx, &gy)| (gx * gx + gy * gy).sqrt())
        .collect();

    let direction: Vec<f32> = grad_x
        .iter()
        .zip(grad_y.iter())
        .map(|(&gx, &gy)| gy.atan2(gx))
        .collect();

    (magnitude, direction)
}

/// Non-maximum suppression to thin edges to 1 pixel width.
fn non_maximum_suppression(
    magnitude: &[f32],
    direction: &[f32],
    width: u32,
    height: u32,
) -> Vec<f32> {
    let mut output = vec![0.0f32; magnitude.len()];

    for y in 1..height.saturating_sub(1) {
        for x in 1..width.saturating_sub(1) {
            let idx = (y * width + x) as usize;
            let mag = magnitude[idx];
            let dir = direction[idx];

            // Quantize direction to 4 angles: 0°, 45°, 90°, 135°
            let angle = ((dir + PI) * 4.0 / PI).round() as i32 % 4;

            // Get neighboring magnitudes along gradient direction
            let (n1, n2) = match angle {
                0 => {
                    // Horizontal (0° or 180°) - compare left/right
                    (
                        magnitude[(y * width + x - 1) as usize],
                        magnitude[(y * width + x + 1) as usize],
                    )
                }
                1 => {
                    // Diagonal (45° or 225°) - compare top-right/bottom-left
                    (
                        magnitude[((y - 1) * width + x + 1) as usize],
                        magnitude[((y + 1) * width + x - 1) as usize],
                    )
                }
                2 => {
                    // Vertical (90° or 270°) - compare top/bottom
                    (
                        magnitude[((y - 1) * width + x) as usize],
                        magnitude[((y + 1) * width + x) as usize],
                    )
                }
                _ => {
                    // Diagonal (135° or 315°) - compare top-left/bottom-right
                    (
                        magnitude[((y - 1) * width + x - 1) as usize],
                        magnitude[((y + 1) * width + x + 1) as usize],
                    )
                }
            };

            // Keep only if local maximum
            if mag >= n1 && mag >= n2 {
                output[idx] = mag;
            }
        }
    }

    output
}

/// Hysteresis thresholding to connect edges.
fn hysteresis_threshold(
    suppressed: &[f32],
    width: u32,
    height: u32,
    low: f32,
    high: f32,
) -> Vec<bool> {
    let len = (width * height) as usize;
    let mut output = vec![false; len];
    let mut visited = vec![false; len];

    // First pass: mark strong edges
    let mut strong_edges = Vec::new();
    for i in 0..len {
        if suppressed[i] >= high {
            output[i] = true;
            strong_edges.push(i);
        }
    }

    // Second pass: trace from strong edges to connect weak edges
    while let Some(idx) = strong_edges.pop() {
        if visited[idx] {
            continue;
        }
        visited[idx] = true;

        let x = (idx % width as usize) as i32;
        let y = (idx / width as usize) as i32;

        // Check 8-connected neighbors
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }

                let nx = x + dx;
                let ny = y + dy;

                if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                    let nidx = (ny * width as i32 + nx) as usize;

                    // If neighbor is a weak edge and not yet visited
                    if !visited[nidx] && suppressed[nidx] >= low && suppressed[nidx] < high {
                        output[nidx] = true;
                        strong_edges.push(nidx);
                    }
                }
            }
        }
    }

    output
}

/// Get edge pixels as coordinates.
pub fn get_edge_coordinates(result: &CannyResult) -> Vec<(u32, u32)> {
    result
        .edges
        .iter()
        .enumerate()
        .filter_map(|(i, &is_edge)| {
            if is_edge {
                let x = (i % result.width as usize) as u32;
                let y = (i / result.width as usize) as u32;
                Some((x, y))
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canny_uniform_image() {
        // Uniform image should have no edges
        let pixels: Vec<[u8; 4]> = vec![[128, 128, 128, 255]; 100];
        let result = canny_edge_detection(&pixels, 10, 10, 20.0, 50.0);

        let edge_count = result.edges.iter().filter(|&&e| e).count();
        assert_eq!(edge_count, 0, "Uniform image should have no edges");
    }

    #[test]
    fn test_canny_vertical_edge() {
        // Create image with vertical edge (left half black, right half white)
        let mut pixels = Vec::new();
        for _y in 0..20 {
            for x in 0..20 {
                if x < 10 {
                    pixels.push([0, 0, 0, 255]);
                } else {
                    pixels.push([255, 255, 255, 255]);
                }
            }
        }

        let result = canny_edge_detection(&pixels, 20, 20, 20.0, 50.0);

        // Should detect vertical edge around x=10
        let edge_count = result.edges.iter().filter(|&&e| e).count();
        assert!(edge_count > 0, "Should detect vertical edge");

        // Edges should be near x=10 (Gaussian blur spreads them slightly)
        let edge_coords = get_edge_coordinates(&result);
        for (x, _y) in &edge_coords {
            assert!(
                *x >= 6 && *x <= 14,
                "Edge should be near center: x={}",
                x
            );
        }
    }

    #[test]
    fn test_canny_horizontal_edge() {
        // Create image with horizontal edge (top half black, bottom half white)
        let mut pixels = Vec::new();
        for y in 0..20 {
            for _x in 0..20 {
                if y < 10 {
                    pixels.push([0, 0, 0, 255]);
                } else {
                    pixels.push([255, 255, 255, 255]);
                }
            }
        }

        let result = canny_edge_detection(&pixels, 20, 20, 20.0, 50.0);

        let edge_count = result.edges.iter().filter(|&&e| e).count();
        assert!(edge_count > 0, "Should detect horizontal edge");
    }

    #[test]
    fn test_canny_gradient_available() {
        let pixels: Vec<[u8; 4]> = vec![[128, 128, 128, 255]; 100];
        let result = canny_edge_detection(&pixels, 10, 10, 20.0, 50.0);

        assert_eq!(result.grad_x.len(), 100);
        assert_eq!(result.grad_y.len(), 100);
        assert_eq!(result.magnitude.len(), 100);
        assert_eq!(result.direction.len(), 100);
    }

    #[test]
    fn test_detect_theme_dark() {
        // Dark image (avg luminance ~20)
        let pixels: Vec<[u8; 4]> = vec![[20, 20, 25, 255]; 100];
        let theme_info = detect_theme(&pixels);

        assert_eq!(theme_info.theme, ImageTheme::Dark);
        assert!(theme_info.avg_luminance < 50.0);
        assert!(theme_info.threshold_multiplier < 1.0);
    }

    #[test]
    fn test_detect_theme_light() {
        // Light image (avg luminance ~200)
        let pixels: Vec<[u8; 4]> = vec![[200, 200, 200, 255]; 100];
        let theme_info = detect_theme(&pixels);

        assert_eq!(theme_info.theme, ImageTheme::Light);
        assert!(theme_info.avg_luminance > 100.0);
        assert_eq!(theme_info.threshold_multiplier, 1.0);
    }

    #[test]
    fn test_detect_theme_empty() {
        let pixels: Vec<[u8; 4]> = vec![];
        let theme_info = detect_theme(&pixels);

        // Should default to light theme
        assert_eq!(theme_info.theme, ImageTheme::Light);
    }

    #[test]
    fn test_clahe_preserves_dimensions() {
        let grayscale: Vec<f32> = vec![50.0; 400];
        let result = apply_clahe(&grayscale, 20, 20, 8, 2.0);

        assert_eq!(result.len(), 400);
    }

    #[test]
    fn test_clahe_enhances_contrast() {
        // Create low-contrast dark image
        let mut grayscale = vec![30.0f32; 400];
        // Add slightly brighter region
        for i in 0..100 {
            grayscale[i] = 40.0;
        }

        let result = apply_clahe(&grayscale, 20, 20, 8, 2.0);

        // CLAHE should spread the histogram - result should have more range
        let min_out = result.iter().cloned().fold(f32::MAX, f32::min);
        let max_out = result.iter().cloned().fold(f32::MIN, f32::max);
        let range = max_out - min_out;

        // Should expand contrast (original range was 10, should be larger)
        assert!(range > 10.0, "CLAHE should enhance contrast, range was {}", range);
    }

    #[test]
    fn test_adaptive_canny_dark_theme() {
        // Create dark image with edge
        let mut pixels = Vec::new();
        for _y in 0..20 {
            for x in 0..20 {
                if x < 10 {
                    pixels.push([20, 20, 20, 255]); // Very dark
                } else {
                    pixels.push([50, 50, 50, 255]); // Slightly less dark
                }
            }
        }

        // Adaptive should detect edges in dark image
        let result = adaptive_canny_edge_detection(&pixels, 20, 20, 30.0, 100.0, true);

        let edge_count = result.edges.iter().filter(|&&e| e).count();
        assert!(edge_count > 0, "Should detect edge in dark image with CLAHE");
    }
}

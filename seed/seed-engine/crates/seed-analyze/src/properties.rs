//! Property extraction for regions (corner radius, stroke, shadow, gradients).
//!
//! Enhanced detection includes:
//! - Per-corner radius detection
//! - Multi-stop gradient support
//! - Multi-directional shadow detection
//! - Diagonal gradient detection

use crate::color::Color;
use crate::region::{Bounds, Fill, Region, Shadow, Stroke};

/// Corner radii for all four corners.
/// Order: [top_left, top_right, bottom_right, bottom_left]
#[derive(Debug, Clone, Copy, Default)]
pub struct CornerRadii(pub [f32; 4]);

impl CornerRadii {
    /// Check if all corners have the same radius.
    pub fn is_uniform(&self) -> bool {
        let base = self.0[0];
        self.0.iter().all(|&r| (r - base).abs() < 1.0)
    }

    /// Get single radius if uniform, or average.
    pub fn uniform_or_average(&self) -> f32 {
        if self.is_uniform() {
            self.0[0]
        } else {
            let sum: f32 = self.0.iter().sum();
            sum / 4.0
        }
    }
}

/// Detect corner radii for all four corners.
pub fn detect_corner_radii(
    pixels: &[[u8; 4]],
    width: u32,
    bounds: &Bounds,
    interior_color: &Color,
    threshold: f32,
) -> CornerRadii {
    CornerRadii([
        detect_single_corner_radius(pixels, width, bounds, interior_color, threshold, Corner::TopLeft),
        detect_single_corner_radius(pixels, width, bounds, interior_color, threshold, Corner::TopRight),
        detect_single_corner_radius(pixels, width, bounds, interior_color, threshold, Corner::BottomRight),
        detect_single_corner_radius(pixels, width, bounds, interior_color, threshold, Corner::BottomLeft),
    ])
}

/// Detect corner radius by sampling diagonals from corners (legacy single-value).
pub fn detect_corner_radius(
    pixels: &[[u8; 4]],
    width: u32,
    bounds: &Bounds,
    interior_color: &Color,
    threshold: f32,
) -> f32 {
    let radii = detect_corner_radii(pixels, width, bounds, interior_color, threshold);
    radii.uniform_or_average()
}

enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

fn detect_single_corner_radius(
    pixels: &[[u8; 4]],
    width: u32,
    bounds: &Bounds,
    interior_color: &Color,
    threshold: f32,
    corner: Corner,
) -> f32 {
    // Start from corner, walk diagonally inward until we hit interior color
    let (start_x, start_y, dx, dy) = match corner {
        Corner::TopLeft => (bounds.x, bounds.y, 1i32, 1i32),
        Corner::TopRight => (bounds.x + bounds.width - 1, bounds.y, -1, 1),
        Corner::BottomLeft => (bounds.x, bounds.y + bounds.height - 1, 1, -1),
        Corner::BottomRight => (bounds.x + bounds.width - 1, bounds.y + bounds.height - 1, -1, -1),
    };

    let max_radius = bounds.width.min(bounds.height) / 4;
    let mut radius = 0.0f32;

    for step in 0..max_radius {
        let x = (start_x as i32 + dx * step as i32) as u32;
        let y = (start_y as i32 + dy * step as i32) as u32;
        let idx = (y * width + x) as usize;

        if idx >= pixels.len() {
            break;
        }

        let color = Color::from_pixel(pixels[idx]);
        if color.distance(interior_color) < threshold {
            // Found interior - this is the approximate radius
            radius = step as f32 * 1.414; // Diagonal step is sqrt(2)
            break;
        }
    }

    radius
}

/// Detect stroke by comparing border pixels to interior.
pub fn detect_stroke(
    pixels: &[[u8; 4]],
    width: u32,
    bounds: &Bounds,
    interior_color: &Color,
    threshold: f32,
) -> Option<Stroke> {
    if bounds.width < 4 || bounds.height < 4 {
        return None;
    }

    // Sample border pixels
    let mut border_colors = Vec::new();

    // Top edge
    for x in bounds.x..bounds.x + bounds.width {
        let idx = (bounds.y * width + x) as usize;
        if idx < pixels.len() {
            border_colors.push(Color::from_pixel(pixels[idx]));
        }
    }

    // Bottom edge
    let bottom_y = bounds.y + bounds.height - 1;
    for x in bounds.x..bounds.x + bounds.width {
        let idx = (bottom_y * width + x) as usize;
        if idx < pixels.len() {
            border_colors.push(Color::from_pixel(pixels[idx]));
        }
    }

    if border_colors.is_empty() {
        return None;
    }

    // Average border color
    let avg_r = border_colors.iter().map(|c| c.r as u32).sum::<u32>() / border_colors.len() as u32;
    let avg_g = border_colors.iter().map(|c| c.g as u32).sum::<u32>() / border_colors.len() as u32;
    let avg_b = border_colors.iter().map(|c| c.b as u32).sum::<u32>() / border_colors.len() as u32;
    let border_color = Color::rgb(avg_r as u8, avg_g as u8, avg_b as u8);

    // If border is different from interior, it's a stroke
    if border_color.distance(interior_color) > threshold {
        // Estimate stroke width by sampling inward
        let stroke_width = detect_stroke_width(pixels, width, bounds, &border_color, threshold);
        Some(Stroke {
            color: border_color,
            width: stroke_width,
        })
    } else {
        None
    }
}

fn detect_stroke_width(
    pixels: &[[u8; 4]],
    width: u32,
    bounds: &Bounds,
    stroke_color: &Color,
    threshold: f32,
) -> f32 {
    // Sample inward from top edge
    let center_x = bounds.x + bounds.width / 2;
    let mut stroke_width = 0;

    for dy in 0..bounds.height.min(10) {
        let y = bounds.y + dy;
        let idx = (y * width + center_x) as usize;
        if idx >= pixels.len() {
            break;
        }

        let color = Color::from_pixel(pixels[idx]);
        if color.distance(stroke_color) < threshold {
            stroke_width = dy + 1;
        } else {
            break;
        }
    }

    stroke_width as f32
}

/// Shadow detection result with direction.
#[derive(Debug, Clone, Copy)]
pub struct ShadowDirection {
    /// Horizontal offset (positive = right)
    pub offset_x: f32,
    /// Vertical offset (positive = down)
    pub offset_y: f32,
    /// Shadow strength (luminance difference)
    pub strength: f32,
}

/// Detect shadow by looking for darker pixels in all directions.
pub fn detect_shadow(
    pixels: &[[u8; 4]],
    width: u32,
    height: u32,
    bounds: &Bounds,
    background_color: &Color,
) -> Option<Shadow> {
    let check_offset = 5u32;
    let blur_sample_dist = 10u32;

    let luminance_bg = 0.299 * background_color.r as f32
        + 0.587 * background_color.g as f32
        + 0.114 * background_color.b as f32;

    // Check all 4 directions and diagonals
    let directions = [
        (0.0, 1.0, "below"),   // Below
        (0.0, -1.0, "above"),  // Above
        (1.0, 0.0, "right"),   // Right
        (-1.0, 0.0, "left"),   // Left
        (1.0, 1.0, "br"),      // Bottom-right
        (-1.0, 1.0, "bl"),     // Bottom-left
        (1.0, -1.0, "tr"),     // Top-right
        (-1.0, -1.0, "tl"),    // Top-left
    ];

    let mut best_shadow: Option<ShadowDirection> = None;

    for (dx, dy, _name) in directions {
        let sample_x = if dx > 0.0 {
            bounds.x + bounds.width + check_offset
        } else if dx < 0.0 {
            bounds.x.saturating_sub(check_offset)
        } else {
            bounds.x + bounds.width / 2
        };

        let sample_y = if dy > 0.0 {
            bounds.y + bounds.height + check_offset
        } else if dy < 0.0 {
            bounds.y.saturating_sub(check_offset)
        } else {
            bounds.y + bounds.height / 2
        };

        if sample_x >= width || sample_y >= height {
            continue;
        }

        let idx = (sample_y * width + sample_x) as usize;
        if idx >= pixels.len() {
            continue;
        }

        let color = Color::from_pixel(pixels[idx]);
        let luminance = 0.299 * color.r as f32 + 0.587 * color.g as f32 + 0.114 * color.b as f32;
        let diff = luminance_bg - luminance;

        if diff > 20.0 {
            let current = ShadowDirection {
                offset_x: dx * check_offset as f32,
                offset_y: dy * check_offset as f32,
                strength: diff,
            };

            if let Some(ref best) = best_shadow {
                if current.strength > best.strength {
                    best_shadow = Some(current);
                }
            } else {
                best_shadow = Some(current);
            }
        }
    }

    best_shadow.map(|shadow| {
        let blur = estimate_shadow_blur_directional(
            pixels, width, height, bounds, background_color,
            blur_sample_dist, shadow.offset_x.signum() as i32, shadow.offset_y.signum() as i32,
        );

        // Estimate shadow color from the darkest sampled pixel
        let alpha = ((shadow.strength / 100.0) * 200.0).min(150.0) as u8;

        Shadow {
            offset_x: shadow.offset_x.abs().max(2.0) * shadow.offset_x.signum(),
            offset_y: shadow.offset_y.abs().max(2.0) * shadow.offset_y.signum(),
            blur,
            color: Color::rgba(0, 0, 0, alpha),
        }
    })
}

fn estimate_shadow_blur_directional(
    pixels: &[[u8; 4]],
    width: u32,
    height: u32,
    bounds: &Bounds,
    background_color: &Color,
    max_dist: u32,
    dx: i32,
    dy: i32,
) -> f32 {
    let mut blur_extent = 0u32;

    let start_x = if dx > 0 {
        bounds.x + bounds.width
    } else if dx < 0 {
        bounds.x
    } else {
        bounds.x + bounds.width / 2
    };

    let start_y = if dy > 0 {
        bounds.y + bounds.height
    } else if dy < 0 {
        bounds.y
    } else {
        bounds.y + bounds.height / 2
    };

    for dist in 1..=max_dist {
        let x = (start_x as i32 + dx * dist as i32) as u32;
        let y = (start_y as i32 + dy * dist as i32) as u32;

        if x >= width || y >= height {
            break;
        }

        let idx = (y * width + x) as usize;
        if idx >= pixels.len() {
            break;
        }

        let color = Color::from_pixel(pixels[idx]);
        let color_dist = color.distance(background_color);

        if color_dist > 5.0 {
            blur_extent = dist;
        } else {
            break;
        }
    }

    (blur_extent as f32 * 2.0).max(4.0)
}

/// Detect if region has a gradient fill (enhanced with multi-stop and diagonal support).
pub fn detect_gradient(
    pixels: &[[u8; 4]],
    width: u32,
    bounds: &Bounds,
) -> Fill {
    if bounds.width < 4 || bounds.height < 4 {
        // Too small, just sample center
        let cx = bounds.x + bounds.width / 2;
        let cy = bounds.y + bounds.height / 2;
        let idx = (cy * width + cx) as usize;
        return Fill::Solid(Color::from_pixel(pixels.get(idx).copied().unwrap_or([128, 128, 128, 255])));
    }

    // Try horizontal gradient (angle = 90)
    if let Some(fill) = detect_gradient_along_axis(pixels, width, bounds, GradientAxis::Horizontal) {
        return fill;
    }

    // Try vertical gradient (angle = 180)
    if let Some(fill) = detect_gradient_along_axis(pixels, width, bounds, GradientAxis::Vertical) {
        return fill;
    }

    // Try diagonal gradients
    if let Some(fill) = detect_gradient_along_axis(pixels, width, bounds, GradientAxis::DiagonalTLBR) {
        return fill;
    }

    if let Some(fill) = detect_gradient_along_axis(pixels, width, bounds, GradientAxis::DiagonalTRBL) {
        return fill;
    }

    // No gradient detected, use center color
    let cx = bounds.x + bounds.width / 2;
    let cy = bounds.y + bounds.height / 2;
    let idx = (cy * width + cx) as usize;
    Fill::Solid(Color::from_pixel(pixels.get(idx).copied().unwrap_or([128, 128, 128, 255])))
}

/// Gradient axis direction.
enum GradientAxis {
    Horizontal,   // Left to right (angle 90)
    Vertical,     // Top to bottom (angle 180)
    DiagonalTLBR, // Top-left to bottom-right (angle 135)
    DiagonalTRBL, // Top-right to bottom-left (angle 45)
}

/// Detect gradient along a specific axis with multi-stop support.
fn detect_gradient_along_axis(
    pixels: &[[u8; 4]],
    width: u32,
    bounds: &Bounds,
    axis: GradientAxis,
) -> Option<Fill> {
    // Sample more points for multi-stop detection
    let num_samples = 5;
    let mut colors = Vec::with_capacity(num_samples);

    for i in 0..num_samples {
        let t = i as f32 / (num_samples - 1) as f32;
        let color = sample_along_axis(pixels, width, bounds, &axis, t);
        colors.push(color);
    }

    // Check if there's a gradient (first and last colors differ)
    let first_last_dist = colors[0].distance(&colors[num_samples - 1]);
    if first_last_dist < 15.0 {
        return None;
    }

    // Check monotonic progression (gradient should transition smoothly)
    let mut cumulative_dist = 0.0;
    for i in 1..num_samples {
        cumulative_dist += colors[i - 1].distance(&colors[i]);
    }

    // If cumulative distance is much larger than first-last, it's not a smooth gradient
    if cumulative_dist > first_last_dist * 2.0 {
        return None;
    }

    // Build gradient stops
    let stops = build_gradient_stops(&colors);
    let angle = match axis {
        GradientAxis::Horizontal => 90.0,
        GradientAxis::Vertical => 180.0,
        GradientAxis::DiagonalTLBR => 135.0,
        GradientAxis::DiagonalTRBL => 45.0,
    };

    Some(Fill::LinearGradient { angle, stops })
}

/// Sample color at position t (0.0-1.0) along the specified axis.
fn sample_along_axis(
    pixels: &[[u8; 4]],
    width: u32,
    bounds: &Bounds,
    axis: &GradientAxis,
    t: f32,
) -> Color {
    let (x, y) = match axis {
        GradientAxis::Horizontal => {
            let x = bounds.x as f32 + 2.0 + (bounds.width as f32 - 4.0) * t;
            let y = bounds.y as f32 + bounds.height as f32 / 2.0;
            (x as u32, y as u32)
        }
        GradientAxis::Vertical => {
            let x = bounds.x as f32 + bounds.width as f32 / 2.0;
            let y = bounds.y as f32 + 2.0 + (bounds.height as f32 - 4.0) * t;
            (x as u32, y as u32)
        }
        GradientAxis::DiagonalTLBR => {
            let x = bounds.x as f32 + 2.0 + (bounds.width as f32 - 4.0) * t;
            let y = bounds.y as f32 + 2.0 + (bounds.height as f32 - 4.0) * t;
            (x as u32, y as u32)
        }
        GradientAxis::DiagonalTRBL => {
            let x = bounds.x as f32 + bounds.width as f32 - 3.0 - (bounds.width as f32 - 4.0) * t;
            let y = bounds.y as f32 + 2.0 + (bounds.height as f32 - 4.0) * t;
            (x as u32, y as u32)
        }
    };

    let idx = (y * width + x) as usize;
    Color::from_pixel(pixels.get(idx).copied().unwrap_or([128, 128, 128, 255]))
}

/// Build gradient stops from sampled colors, removing redundant stops.
fn build_gradient_stops(colors: &[Color]) -> Vec<(f32, Color)> {
    if colors.is_empty() {
        return vec![];
    }

    if colors.len() == 1 {
        return vec![(0.0, colors[0])];
    }

    let mut stops = Vec::new();
    let n = colors.len();

    // Always include first stop
    stops.push((0.0, colors[0]));

    // Check intermediate stops - only include if they significantly differ from linear interpolation
    for i in 1..n - 1 {
        let t = i as f32 / (n - 1) as f32;

        // Calculate expected color from linear interpolation between first and last
        let expected = colors[0].blend(&colors[n - 1], t);
        let actual = colors[i];

        // Include this stop if it differs significantly from expected
        if actual.distance(&expected) > 10.0 {
            stops.push((t, actual));
        }
    }

    // Always include last stop
    stops.push((1.0, colors[n - 1]));

    stops
}

/// Apply property detection to all regions recursively.
pub fn detect_all_properties(
    regions: &mut [Region],
    pixels: &[[u8; 4]],
    width: u32,
    height: u32,
    background_color: &Color,
) {
    for region in regions {
        detect_region_properties(region, pixels, width, height, background_color);
        detect_all_properties(&mut region.children, pixels, width, height, background_color);
    }
}

fn detect_region_properties(
    region: &mut Region,
    pixels: &[[u8; 4]],
    width: u32,
    height: u32,
    background_color: &Color,
) {
    // Get interior color for comparisons
    let interior_color = match &region.fill {
        Fill::Solid(c) => *c,
        Fill::LinearGradient { stops, .. } | Fill::RadialGradient { stops } => {
            stops.first().map(|(_, c)| *c).unwrap_or(Color::rgb(128, 128, 128))
        }
    };

    // Detect gradient (updates fill)
    region.fill = detect_gradient(pixels, width, &region.bounds);

    // Get updated interior color
    let interior_color = match &region.fill {
        Fill::Solid(c) => *c,
        Fill::LinearGradient { stops, .. } | Fill::RadialGradient { stops } => {
            stops.first().map(|(_, c)| *c).unwrap_or(interior_color)
        }
    };

    // Detect corner radius
    region.corner_radius = detect_corner_radius(pixels, width, &region.bounds, &interior_color, 15.0);

    // Detect stroke
    region.stroke = detect_stroke(pixels, width, &region.bounds, &interior_color, 15.0);

    // Detect shadow (only for non-child regions or large regions)
    if region.bounds.area() > 1000 {
        region.shadow = detect_shadow(pixels, width, height, &region.bounds, background_color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_gradient_solid() {
        // Uniform color
        let pixels: Vec<[u8; 4]> = vec![[100, 100, 100, 255]; 100];
        let bounds = Bounds::new(0, 0, 10, 10);
        let fill = detect_gradient(&pixels, 10, &bounds);

        match fill {
            Fill::Solid(c) => {
                assert_eq!(c.r, 100);
            }
            _ => panic!("Expected solid fill"),
        }
    }

    #[test]
    fn test_detect_gradient_horizontal() {
        // Horizontal gradient from red to blue
        let mut pixels = Vec::new();
        for _y in 0..10 {
            for x in 0..10 {
                let t = x as f32 / 9.0;
                let r = ((1.0 - t) * 255.0) as u8;
                let b = (t * 255.0) as u8;
                pixels.push([r, 0, b, 255]);
            }
        }

        let bounds = Bounds::new(0, 0, 10, 10);
        let fill = detect_gradient(&pixels, 10, &bounds);

        match fill {
            Fill::LinearGradient { angle, stops } => {
                assert!((angle - 90.0).abs() < 1.0);
                // Multi-stop detection may add intermediate stops
                assert!(stops.len() >= 2, "Should have at least 2 stops");
            }
            _ => panic!("Expected linear gradient"),
        }
    }

    #[test]
    fn test_corner_radii() {
        let radii = CornerRadii([5.0, 5.0, 5.0, 5.0]);
        assert!(radii.is_uniform());
        assert!((radii.uniform_or_average() - 5.0).abs() < 0.1);

        let radii = CornerRadii([10.0, 5.0, 5.0, 5.0]);
        assert!(!radii.is_uniform());
        assert!((radii.uniform_or_average() - 6.25).abs() < 0.1);
    }
}

//! Region detection via flood fill and rectangle finding.

use crate::color::Color;

/// Bounding box for a region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Bounds {
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    /// Check if this bounds contains a point.
    pub fn contains(&self, px: u32, py: u32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }

    /// Check if this bounds fully contains another.
    pub fn contains_bounds(&self, other: &Bounds) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.x + other.width <= self.x + self.width
            && other.y + other.height <= self.y + self.height
    }

    /// Calculate area.
    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    /// Expand bounds to include a point.
    pub fn expand_to(&mut self, px: u32, py: u32) {
        let x2 = self.x + self.width;
        let y2 = self.y + self.height;

        self.x = self.x.min(px);
        self.y = self.y.min(py);
        let new_x2 = x2.max(px + 1);
        let new_y2 = y2.max(py + 1);
        self.width = new_x2 - self.x;
        self.height = new_y2 - self.y;
    }

    /// Check if two bounds overlap.
    pub fn overlaps(&self, other: &Bounds) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }
}

/// Type of detected region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RegionType {
    Frame,
    Text,
    Unknown,
}

/// Fill style for a region.
#[derive(Debug, Clone)]
pub enum Fill {
    Solid(Color),
    LinearGradient {
        angle: f32,
        stops: Vec<(f32, Color)>,
    },
    RadialGradient {
        stops: Vec<(f32, Color)>,
    },
}

/// Stroke style.
#[derive(Debug, Clone)]
pub struct Stroke {
    pub color: Color,
    pub width: f32,
}

/// Shadow style.
#[derive(Debug, Clone)]
pub struct Shadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
    pub color: Color,
}

/// A detected region in the image.
#[derive(Debug, Clone)]
pub struct Region {
    pub bounds: Bounds,
    pub fill: Fill,
    pub corner_radius: f32,
    pub stroke: Option<Stroke>,
    pub shadow: Option<Shadow>,
    pub region_type: RegionType,
    pub children: Vec<Region>,
    /// Layout information for children (if detected).
    pub layout: Option<crate::hierarchy::LayoutInfo>,
}

impl Region {
    pub fn new(bounds: Bounds, fill: Fill) -> Self {
        Self {
            bounds,
            fill,
            corner_radius: 0.0,
            stroke: None,
            shadow: None,
            region_type: RegionType::Frame,
            children: Vec::new(),
            layout: None,
        }
    }
}

/// Result of flood fill: bounds and dominant color.
#[derive(Debug)]
pub struct FloodFillResult {
    pub bounds: Bounds,
    pub color: Color,
    pub pixel_count: usize,
}

/// Detect regions using flood fill on similar-colored areas.
pub fn detect_regions(
    pixels: &[[u8; 4]],
    width: u32,
    height: u32,
    color_threshold: f32,
    min_area: u64,
) -> Vec<FloodFillResult> {
    let mut visited = vec![false; (width * height) as usize];
    let mut regions = Vec::new();

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            if visited[idx] {
                continue;
            }

            let seed_color = Color::from_pixel(pixels[idx]);

            // Skip transparent pixels
            if seed_color.a < 128 {
                visited[idx] = true;
                continue;
            }

            // Flood fill from this pixel
            let result = flood_fill(pixels, width, height, x, y, seed_color, color_threshold, &mut visited);

            if result.pixel_count > 0 && result.bounds.area() >= min_area {
                regions.push(result);
            }
        }
    }

    // Sort by area (largest first)
    regions.sort_by(|a, b| b.bounds.area().cmp(&a.bounds.area()));

    regions
}

/// Flood fill from a seed point, returning bounds and average color.
fn flood_fill(
    pixels: &[[u8; 4]],
    width: u32,
    height: u32,
    start_x: u32,
    start_y: u32,
    seed_color: Color,
    threshold: f32,
    visited: &mut [bool],
) -> FloodFillResult {
    let mut stack = vec![(start_x, start_y)];
    let mut bounds = Bounds::new(start_x, start_y, 1, 1);
    let mut pixel_count = 0usize;
    let mut r_sum = 0u64;
    let mut g_sum = 0u64;
    let mut b_sum = 0u64;

    while let Some((x, y)) = stack.pop() {
        let idx = (y * width + x) as usize;

        if visited[idx] {
            continue;
        }

        let color = Color::from_pixel(pixels[idx]);
        if color.distance(&seed_color) > threshold {
            continue;
        }

        visited[idx] = true;
        bounds.expand_to(x, y);
        pixel_count += 1;
        r_sum += color.r as u64;
        g_sum += color.g as u64;
        b_sum += color.b as u64;

        // Add neighbors (4-connected)
        if x > 0 {
            stack.push((x - 1, y));
        }
        if x < width - 1 {
            stack.push((x + 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        if y < height - 1 {
            stack.push((x, y + 1));
        }
    }

    let avg_color = if pixel_count > 0 {
        Color::rgb(
            (r_sum / pixel_count as u64) as u8,
            (g_sum / pixel_count as u64) as u8,
            (b_sum / pixel_count as u64) as u8,
        )
    } else {
        seed_color
    };

    FloodFillResult {
        bounds,
        color: avg_color,
        pixel_count,
    }
}

/// Detect regions using edge-constrained flood fill.
///
/// This version uses detected edges as barriers that flood fill cannot cross,
/// which is critical for accurately separating dark-themed UI elements where
/// colors are similar but edges are distinct.
///
/// # Arguments
/// * `pixels` - RGBA pixel data
/// * `width` - Image width
/// * `height` - Image height
/// * `edges` - Edge map from Canny detection (true = edge pixel = barrier)
/// * `color_threshold` - Maximum color distance to include in region
/// * `min_area` - Minimum region area to keep
pub fn detect_regions_edge_constrained(
    pixels: &[[u8; 4]],
    width: u32,
    height: u32,
    edges: &[bool],
    color_threshold: f32,
    min_area: u64,
) -> Vec<FloodFillResult> {
    let mut visited = vec![false; (width * height) as usize];
    let mut regions = Vec::new();

    // Pre-mark edge pixels as visited so flood fill doesn't start from them
    for (i, &is_edge) in edges.iter().enumerate() {
        if is_edge {
            visited[i] = true;
        }
    }

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            if visited[idx] {
                continue;
            }

            let seed_color = Color::from_pixel(pixels[idx]);

            // Skip transparent pixels
            if seed_color.a < 128 {
                visited[idx] = true;
                continue;
            }

            // Flood fill with edge barriers
            let result = flood_fill_edge_constrained(
                pixels,
                width,
                height,
                x,
                y,
                seed_color,
                edges,
                color_threshold,
                &mut visited,
            );

            if result.pixel_count > 0 && result.bounds.area() >= min_area {
                regions.push(result);
            }
        }
    }

    // Sort by area (largest first)
    regions.sort_by(|a, b| b.bounds.area().cmp(&a.bounds.area()));

    regions
}

/// Flood fill with edge constraints.
///
/// The flood fill will NOT cross pixels marked as edges, effectively using
/// the edge map as physical barriers between UI elements.
fn flood_fill_edge_constrained(
    pixels: &[[u8; 4]],
    width: u32,
    height: u32,
    start_x: u32,
    start_y: u32,
    seed_color: Color,
    edges: &[bool],
    threshold: f32,
    visited: &mut [bool],
) -> FloodFillResult {
    let mut stack = vec![(start_x, start_y)];
    let mut bounds = Bounds::new(start_x, start_y, 1, 1);
    let mut pixel_count = 0usize;
    let mut r_sum = 0u64;
    let mut g_sum = 0u64;
    let mut b_sum = 0u64;

    while let Some((x, y)) = stack.pop() {
        let idx = (y * width + x) as usize;

        if visited[idx] {
            continue;
        }

        // Don't cross edge boundaries
        if edges[idx] {
            visited[idx] = true;
            continue;
        }

        let color = Color::from_pixel(pixels[idx]);
        if color.distance(&seed_color) > threshold {
            continue;
        }

        visited[idx] = true;
        bounds.expand_to(x, y);
        pixel_count += 1;
        r_sum += color.r as u64;
        g_sum += color.g as u64;
        b_sum += color.b as u64;

        // Add neighbors (4-connected), but NOT if neighbor is an edge
        if x > 0 {
            let n_idx = (y * width + x - 1) as usize;
            if !edges[n_idx] {
                stack.push((x - 1, y));
            }
        }
        if x < width - 1 {
            let n_idx = (y * width + x + 1) as usize;
            if !edges[n_idx] {
                stack.push((x + 1, y));
            }
        }
        if y > 0 {
            let n_idx = ((y - 1) * width + x) as usize;
            if !edges[n_idx] {
                stack.push((x, y - 1));
            }
        }
        if y < height - 1 {
            let n_idx = ((y + 1) * width + x) as usize;
            if !edges[n_idx] {
                stack.push((x, y + 1));
            }
        }
    }

    let avg_color = if pixel_count > 0 {
        Color::rgb(
            (r_sum / pixel_count as u64) as u8,
            (g_sum / pixel_count as u64) as u8,
            (b_sum / pixel_count as u64) as u8,
        )
    } else {
        seed_color
    };

    FloodFillResult {
        bounds,
        color: avg_color,
        pixel_count,
    }
}

/// Merge overlapping regions.
pub fn merge_overlapping(regions: Vec<FloodFillResult>, overlap_threshold: f32) -> Vec<FloodFillResult> {
    if regions.is_empty() {
        return regions;
    }

    let mut merged: Vec<FloodFillResult> = Vec::new();

    for region in regions {
        let mut was_merged = false;

        for existing in &mut merged {
            // Check if regions overlap significantly
            if regions_overlap(&existing.bounds, &region.bounds, overlap_threshold) {
                // Merge into existing
                existing.bounds = merge_bounds(&existing.bounds, &region.bounds);
                existing.pixel_count += region.pixel_count;
                was_merged = true;
                break;
            }
        }

        if !was_merged {
            merged.push(region);
        }
    }

    merged
}

/// Check if two regions overlap by more than threshold percentage.
fn regions_overlap(a: &Bounds, b: &Bounds, threshold: f32) -> bool {
    if !a.overlaps(b) {
        return false;
    }

    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    let x2 = (a.x + a.width).min(b.x + b.width);
    let y2 = (a.y + a.height).min(b.y + b.height);

    let overlap_area = (x2 - x1) as f32 * (y2 - y1) as f32;
    let smaller_area = a.area().min(b.area()) as f32;

    overlap_area / smaller_area >= threshold
}

/// Merge two bounds into one containing both.
fn merge_bounds(a: &Bounds, b: &Bounds) -> Bounds {
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    let x2 = (a.x + a.width).max(b.x + b.width);
    let y2 = (a.y + a.height).max(b.y + b.height);

    Bounds::new(x, y, x2 - x, y2 - y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounds_contains() {
        let b = Bounds::new(10, 10, 20, 20);
        assert!(b.contains(10, 10));
        assert!(b.contains(25, 25));
        assert!(!b.contains(30, 30));
        assert!(!b.contains(5, 15));
    }

    #[test]
    fn test_bounds_contains_bounds() {
        let outer = Bounds::new(0, 0, 100, 100);
        let inner = Bounds::new(10, 10, 20, 20);
        let partial = Bounds::new(90, 90, 20, 20);

        assert!(outer.contains_bounds(&inner));
        assert!(!outer.contains_bounds(&partial));
    }

    #[test]
    fn test_detect_regions_uniform() {
        // All same color
        let pixels: Vec<[u8; 4]> = vec![[100, 100, 100, 255]; 100];
        let regions = detect_regions(&pixels, 10, 10, 10.0, 1);

        // Should find one region
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].bounds.width, 10);
        assert_eq!(regions[0].bounds.height, 10);
    }

    #[test]
    fn test_detect_regions_two_colors() {
        // Half red, half blue
        let mut pixels = Vec::new();
        for _y in 0..10 {
            for x in 0..10 {
                if x < 5 {
                    pixels.push([255, 0, 0, 255]);
                } else {
                    pixels.push([0, 0, 255, 255]);
                }
            }
        }

        let regions = detect_regions(&pixels, 10, 10, 10.0, 1);

        // Should find two regions
        assert_eq!(regions.len(), 2);
    }

    #[test]
    fn test_edge_constrained_separates_similar_colors() {
        // Create uniform dark gray image
        let pixels: Vec<[u8; 4]> = vec![[30, 30, 30, 255]; 100];

        // Create vertical edge barrier at x=5
        let mut edges = vec![false; 100];
        for y in 0..10 {
            edges[y * 10 + 5] = true; // Vertical edge at x=5
        }

        // Without edges, flood fill would find 1 region
        let regions_no_edge = detect_regions(&pixels, 10, 10, 50.0, 1);
        assert_eq!(regions_no_edge.len(), 1, "Without edges, should find 1 region");

        // With edges, should find 2 regions separated by edge
        let regions_with_edge = detect_regions_edge_constrained(&pixels, 10, 10, &edges, 50.0, 1);
        assert_eq!(regions_with_edge.len(), 2, "With edge barrier, should find 2 regions");
    }

    #[test]
    fn test_edge_constrained_dark_theme() {
        // Simulate dark theme: two similar dark colors separated by edge
        let mut pixels = Vec::new();
        for _y in 0..20 {
            for x in 0..20 {
                if x < 10 {
                    pixels.push([25, 25, 30, 255]); // Dark blue-ish
                } else {
                    pixels.push([30, 25, 25, 255]); // Dark red-ish
                }
            }
        }

        // Standard flood fill with low threshold might merge them
        let regions_standard = detect_regions(&pixels, 20, 20, 15.0, 1);

        // Edge barrier at x=10
        let mut edges = vec![false; 400];
        for y in 0..20 {
            edges[y * 20 + 10] = true;
        }

        let regions_constrained = detect_regions_edge_constrained(&pixels, 20, 20, &edges, 15.0, 1);

        // With edge constraint, should definitely find 2 regions
        assert_eq!(
            regions_constrained.len(),
            2,
            "Edge-constrained should separate dark regions"
        );

        // Standard might or might not find 2 depending on threshold
        // but edge-constrained guarantees separation
        assert!(
            regions_constrained.len() >= regions_standard.len(),
            "Edge-constrained should find at least as many regions"
        );
    }

    #[test]
    fn test_edge_constrained_box() {
        // Create a box drawn with edges
        let pixels: Vec<[u8; 4]> = vec![[50, 50, 50, 255]; 100];

        // Draw a box from (2,2) to (7,7)
        let mut edges = vec![false; 100];
        for i in 2..=7 {
            edges[2 * 10 + i] = true; // Top edge
            edges[7 * 10 + i] = true; // Bottom edge
            edges[i * 10 + 2] = true; // Left edge
            edges[i * 10 + 7] = true; // Right edge
        }

        let regions = detect_regions_edge_constrained(&pixels, 10, 10, &edges, 50.0, 1);

        // Should find at least 2 regions: inside box and outside box
        assert!(
            regions.len() >= 2,
            "Should separate inside from outside box"
        );
    }
}

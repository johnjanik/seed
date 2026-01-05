//! Text detection using Stroke Width Transform (SWT).
//!
//! The SWT algorithm detects text regions without OCR by identifying
//! areas with consistent stroke widths (characteristic of text).
//!
//! Key steps:
//! 1. Compute SWT: For each edge pixel, ray march in gradient direction
//! 2. Find connected components with consistent stroke widths
//! 3. Filter by geometric properties (aspect ratio, variance)
//! 4. Group into words/lines based on proximity
//! 5. Estimate font properties from stroke width and height

use crate::canny::CannyResult;
use crate::color::Color;
use crate::region::Bounds;
use std::collections::HashMap;

/// A detected text region.
#[derive(Debug, Clone)]
pub struct TextRegion {
    /// Bounding box of the text region.
    pub bounds: Bounds,
    /// Estimated font size in pixels.
    pub font_size: f32,
    /// Estimated font weight.
    pub weight: FontWeight,
    /// Primary text color.
    pub text_color: Color,
    /// Background color behind text.
    pub background_color: Color,
    /// Estimated number of lines.
    pub line_count: u32,
    /// Confidence score (0.0-1.0).
    pub confidence: f32,
    /// Average stroke width.
    pub stroke_width: f32,
}

/// Font weight categories.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FontWeight {
    Light,
    Regular,
    Medium,
    Bold,
}

impl FontWeight {
    /// Estimate weight from stroke width relative to font size.
    pub fn from_stroke_ratio(stroke_width: f32, font_size: f32) -> Self {
        let ratio = stroke_width / font_size;

        if ratio < 0.08 {
            FontWeight::Light
        } else if ratio < 0.12 {
            FontWeight::Regular
        } else if ratio < 0.16 {
            FontWeight::Medium
        } else {
            FontWeight::Bold
        }
    }

    /// Get CSS font-weight value.
    pub fn to_css_value(&self) -> u32 {
        match self {
            FontWeight::Light => 300,
            FontWeight::Regular => 400,
            FontWeight::Medium => 500,
            FontWeight::Bold => 700,
        }
    }
}

/// Configuration for text detection.
#[derive(Debug, Clone)]
pub struct TextDetectConfig {
    /// Maximum ray march distance for SWT.
    pub max_stroke_width: f32,
    /// Maximum stroke width variance within a component.
    pub max_stroke_variance: f32,
    /// Minimum component area to consider.
    pub min_component_area: u32,
    /// Maximum component area to consider.
    pub max_component_area: u32,
    /// Maximum aspect ratio for character candidates.
    pub max_aspect_ratio: f32,
    /// Minimum aspect ratio for character candidates.
    pub min_aspect_ratio: f32,
    /// Maximum spacing between characters (as multiple of height).
    pub max_char_spacing: f32,
    /// Edge density threshold for text regions.
    pub edge_density_threshold: f32,
}

impl Default for TextDetectConfig {
    fn default() -> Self {
        Self {
            max_stroke_width: 50.0,
            max_stroke_variance: 0.5,
            min_component_area: 10,
            max_component_area: 50000,
            max_aspect_ratio: 10.0,
            min_aspect_ratio: 0.1,
            max_char_spacing: 2.0,
            edge_density_threshold: 0.15,
        }
    }
}

/// Compute Stroke Width Transform on edge map.
///
/// Returns stroke width at each pixel (f32::MAX for non-text pixels).
pub fn stroke_width_transform(
    canny: &CannyResult,
    direction: f32, // 1.0 for dark text on light, -1.0 for light text on dark
) -> Vec<f32> {
    let width = canny.width;
    let height = canny.height;
    let len = (width * height) as usize;

    let mut swt = vec![f32::MAX; len];
    let mut rays: Vec<Vec<usize>> = Vec::new();

    // For each edge pixel, cast ray in gradient direction
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let idx = (y * width + x) as usize;

            if !canny.edges[idx] {
                continue;
            }

            let gx = canny.grad_x[idx] * direction;
            let gy = canny.grad_y[idx] * direction;
            let mag = (gx * gx + gy * gy).sqrt();

            if mag < 1e-6 {
                continue;
            }

            // Normalize gradient
            let dx = gx / mag;
            let dy = gy / mag;

            // Ray march
            let mut ray = vec![idx];
            let mut cx = x as f32 + 0.5;
            let mut cy = y as f32 + 0.5;
            let mut found_opposite = false;

            for step in 1..50 {
                cx += dx;
                cy += dy;

                let nx = cx as i32;
                let ny = cy as i32;

                if nx < 0 || nx >= width as i32 || ny < 0 || ny >= height as i32 {
                    break;
                }

                let nidx = (ny as u32 * width + nx as u32) as usize;
                ray.push(nidx);

                // Check if we hit opposite edge
                if canny.edges[nidx] {
                    // Verify gradient is opposite
                    let gx2 = canny.grad_x[nidx];
                    let gy2 = canny.grad_y[nidx];
                    let dot = gx * gx2 + gy * gy2;

                    if dot < -mag * 0.5 {
                        // Found opposite edge - this is the stroke width
                        let stroke = step as f32;

                        // Set stroke width for all pixels along ray
                        for &pidx in &ray {
                            swt[pidx] = swt[pidx].min(stroke);
                        }

                        found_opposite = true;
                        rays.push(ray.clone());
                    }
                    break;
                }
            }

            if !found_opposite {
                // Reset any partial stroke widths
                for &pidx in &ray {
                    if swt[pidx] != f32::MAX && swt[pidx] > 30.0 {
                        swt[pidx] = f32::MAX;
                    }
                }
            }
        }
    }

    // Second pass: smooth stroke widths along rays
    for ray in &rays {
        let valid_strokes: Vec<f32> = ray
            .iter()
            .map(|&idx| swt[idx])
            .filter(|&s| s != f32::MAX)
            .collect();

        if valid_strokes.is_empty() {
            continue;
        }

        let median = median_f32(&valid_strokes);

        for &idx in ray {
            if swt[idx] != f32::MAX {
                swt[idx] = median;
            }
        }
    }

    swt
}

/// Calculate median of f32 slice.
fn median_f32(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }

    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

/// Connected component from SWT.
#[derive(Debug)]
struct SwtComponent {
    pixels: Vec<(u32, u32)>,
    stroke_widths: Vec<f32>,
    bounds: Bounds,
}

impl SwtComponent {
    fn area(&self) -> u32 {
        self.pixels.len() as u32
    }

    fn mean_stroke_width(&self) -> f32 {
        if self.stroke_widths.is_empty() {
            return 0.0;
        }
        self.stroke_widths.iter().sum::<f32>() / self.stroke_widths.len() as f32
    }

    fn stroke_variance(&self) -> f32 {
        if self.stroke_widths.len() < 2 {
            return 0.0;
        }

        let mean = self.mean_stroke_width();
        let variance: f32 = self.stroke_widths.iter().map(|s| (s - mean).powi(2)).sum::<f32>()
            / self.stroke_widths.len() as f32;
        variance.sqrt() / mean // Coefficient of variation
    }

    fn aspect_ratio(&self) -> f32 {
        self.bounds.width as f32 / self.bounds.height.max(1) as f32
    }
}

/// Find connected components in SWT image.
fn find_swt_components(swt: &[f32], width: u32, height: u32, config: &TextDetectConfig) -> Vec<SwtComponent> {
    let len = (width * height) as usize;
    let mut labels = vec![0u32; len];
    let mut next_label = 1u32;
    let mut components: HashMap<u32, SwtComponent> = HashMap::new();

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;

            if swt[idx] == f32::MAX || labels[idx] != 0 {
                continue;
            }

            // BFS to find connected component
            let mut stack = vec![(x, y)];
            let mut component = SwtComponent {
                pixels: Vec::new(),
                stroke_widths: Vec::new(),
                bounds: Bounds {
                    x: x,
                    y: y,
                    width: 1,
                    height: 1,
                },
            };

            let base_stroke = swt[idx];

            while let Some((cx, cy)) = stack.pop() {
                let cidx = (cy * width + cx) as usize;

                if labels[cidx] != 0 {
                    continue;
                }

                let stroke = swt[cidx];
                if stroke == f32::MAX {
                    continue;
                }

                // Check stroke width consistency
                let ratio = if stroke > base_stroke {
                    stroke / base_stroke
                } else {
                    base_stroke / stroke
                };

                if ratio > 3.0 {
                    continue;
                }

                labels[cidx] = next_label;
                component.pixels.push((cx, cy));
                component.stroke_widths.push(stroke);

                // Update bounds
                component.bounds.x = component.bounds.x.min(cx);
                component.bounds.y = component.bounds.y.min(cy);
                let max_x = component.bounds.x + component.bounds.width;
                let max_y = component.bounds.y + component.bounds.height;
                component.bounds.width = (cx + 1).max(max_x) - component.bounds.x;
                component.bounds.height = (cy + 1).max(max_y) - component.bounds.y;

                // Add neighbors
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }

                        let nx = cx as i32 + dx;
                        let ny = cy as i32 + dy;

                        if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                            let nidx = (ny as u32 * width + nx as u32) as usize;
                            if labels[nidx] == 0 && swt[nidx] != f32::MAX {
                                stack.push((nx as u32, ny as u32));
                            }
                        }
                    }
                }
            }

            if !component.pixels.is_empty() {
                components.insert(next_label, component);
                next_label += 1;
            }
        }
    }

    // Filter components by properties
    components
        .into_values()
        .filter(|c| {
            let area = c.area();
            let ar = c.aspect_ratio();
            let variance = c.stroke_variance();

            area >= config.min_component_area
                && area <= config.max_component_area
                && ar >= config.min_aspect_ratio
                && ar <= config.max_aspect_ratio
                && variance <= config.max_stroke_variance
        })
        .collect()
}

/// Group character candidates into text lines.
fn group_into_lines(components: &[SwtComponent], config: &TextDetectConfig) -> Vec<Vec<usize>> {
    if components.is_empty() {
        return Vec::new();
    }

    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut used = vec![false; components.len()];

    for i in 0..components.len() {
        if used[i] {
            continue;
        }

        used[i] = true;
        let mut group = vec![i];
        let comp = &components[i];
        let height = comp.bounds.height as f32;
        let stroke = comp.mean_stroke_width();

        // Find nearby similar components on same line
        for (j, other) in components.iter().enumerate().skip(i + 1) {
            if used[j] {
                continue;
            }

            // Check vertical alignment (same line)
            let y_overlap = (comp.bounds.y as f32 + comp.bounds.height as f32)
                .min(other.bounds.y as f32 + other.bounds.height as f32)
                - comp.bounds.y.max(other.bounds.y) as f32;

            if y_overlap < height * 0.3 {
                continue;
            }

            // Check horizontal spacing
            let x_gap = if other.bounds.x >= comp.bounds.x + comp.bounds.width {
                other.bounds.x - (comp.bounds.x + comp.bounds.width)
            } else if comp.bounds.x >= other.bounds.x + other.bounds.width {
                comp.bounds.x - (other.bounds.x + other.bounds.width)
            } else {
                0 // Overlapping
            };

            if x_gap as f32 > height * config.max_char_spacing {
                continue;
            }

            // Check similar stroke width
            let other_stroke = other.mean_stroke_width();
            let stroke_ratio = if stroke > other_stroke {
                stroke / other_stroke
            } else {
                other_stroke / stroke
            };

            if stroke_ratio > 2.0 {
                continue;
            }

            // Check similar height
            let height_ratio = height / other.bounds.height as f32;
            if height_ratio < 0.5 || height_ratio > 2.0 {
                continue;
            }

            used[j] = true;
            group.push(j);
        }

        if group.len() >= 2 {
            // At least 2 characters to be considered text
            groups.push(group);
        }
    }

    groups
}

/// Detect text regions in an image.
pub fn detect_text_regions(
    pixels: &[[u8; 4]],
    width: u32,
    height: u32,
    canny: &CannyResult,
    config: &TextDetectConfig,
) -> Vec<TextRegion> {
    let mut regions = Vec::new();

    // Try both dark-on-light and light-on-dark text
    for direction in [1.0f32, -1.0] {
        let swt = stroke_width_transform(canny, direction);
        let components = find_swt_components(&swt, width, height, config);
        let lines = group_into_lines(&components, config);

        for line in lines {
            // Calculate bounding box of line
            let mut min_x = u32::MAX;
            let mut min_y = u32::MAX;
            let mut max_x = 0u32;
            let mut max_y = 0u32;
            let mut total_stroke = 0.0f32;
            let mut stroke_count = 0;

            for &idx in &line {
                let comp = &components[idx];
                min_x = min_x.min(comp.bounds.x);
                min_y = min_y.min(comp.bounds.y);
                max_x = max_x.max(comp.bounds.x + comp.bounds.width);
                max_y = max_y.max(comp.bounds.y + comp.bounds.height);
                total_stroke += comp.mean_stroke_width() * comp.stroke_widths.len() as f32;
                stroke_count += comp.stroke_widths.len();
            }

            if stroke_count == 0 {
                continue;
            }

            let bounds = Bounds {
                x: min_x,
                y: min_y,
                width: max_x - min_x,
                height: max_y - min_y,
            };

            let avg_stroke = total_stroke / stroke_count as f32;
            let font_size = bounds.height as f32;

            // Estimate colors
            let (text_color, bg_color) = estimate_text_colors(pixels, width, &bounds);

            let weight = FontWeight::from_stroke_ratio(avg_stroke, font_size);

            // Calculate confidence based on number of characters and consistency
            let confidence = (line.len() as f32 / 10.0).min(1.0);

            regions.push(TextRegion {
                bounds,
                font_size,
                weight,
                text_color,
                background_color: bg_color,
                line_count: 1,
                confidence,
                stroke_width: avg_stroke,
            });
        }
    }

    // Merge overlapping text regions
    merge_overlapping_text_regions(regions)
}

/// Estimate text and background colors in a region.
fn estimate_text_colors(pixels: &[[u8; 4]], width: u32, bounds: &Bounds) -> (Color, Color) {
    let mut color_counts: HashMap<u32, u32> = HashMap::new();

    // Sample pixels in region
    for dy in 0..bounds.height {
        for dx in 0..bounds.width {
            let x = bounds.x + dx;
            let y = bounds.y + dy;
            let idx = (y * width + x) as usize;

            if idx < pixels.len() {
                let p = pixels[idx];
                // Quantize to reduce colors
                let key = ((p[0] as u32 / 32) << 10) | ((p[1] as u32 / 32) << 5) | (p[2] as u32 / 32);
                *color_counts.entry(key).or_insert(0) += 1;
            }
        }
    }

    if color_counts.is_empty() {
        return (Color::rgb(0, 0, 0), Color::rgb(255, 255, 255));
    }

    // Get two most common colors
    let mut sorted: Vec<_> = color_counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    let decode_color = |key: u32| -> Color {
        let r = ((key >> 10) & 0x1F) * 8 + 4;
        let g = ((key >> 5) & 0x1F) * 8 + 4;
        let b = (key & 0x1F) * 8 + 4;
        Color::rgb(r as u8, g as u8, b as u8)
    };

    let color1 = decode_color(sorted[0].0);
    let color2 = if sorted.len() > 1 {
        decode_color(sorted[1].0)
    } else {
        Color::rgb(255, 255, 255)
    };

    // Darker color is likely text
    let lum1 = color1.r as u32 + color1.g as u32 + color1.b as u32;
    let lum2 = color2.r as u32 + color2.g as u32 + color2.b as u32;

    if lum1 < lum2 {
        (color1, color2)
    } else {
        (color2, color1)
    }
}

/// Merge overlapping text regions.
fn merge_overlapping_text_regions(mut regions: Vec<TextRegion>) -> Vec<TextRegion> {
    if regions.len() < 2 {
        return regions;
    }

    let mut result = Vec::new();
    let mut used = vec![false; regions.len()];

    // Sort by position
    regions.sort_by_key(|r| (r.bounds.y, r.bounds.x));

    for i in 0..regions.len() {
        if used[i] {
            continue;
        }

        used[i] = true;
        let mut merged = regions[i].clone();

        // Find overlapping regions
        for j in (i + 1)..regions.len() {
            if used[j] {
                continue;
            }

            let other = &regions[j];

            // Check overlap
            let overlaps = merged.bounds.x < other.bounds.x + other.bounds.width
                && merged.bounds.x + merged.bounds.width > other.bounds.x
                && merged.bounds.y < other.bounds.y + other.bounds.height
                && merged.bounds.y + merged.bounds.height > other.bounds.y;

            if overlaps {
                used[j] = true;

                // Expand bounds
                let new_x = merged.bounds.x.min(other.bounds.x);
                let new_y = merged.bounds.y.min(other.bounds.y);
                let new_x2 = (merged.bounds.x + merged.bounds.width)
                    .max(other.bounds.x + other.bounds.width);
                let new_y2 = (merged.bounds.y + merged.bounds.height)
                    .max(other.bounds.y + other.bounds.height);

                merged.bounds.x = new_x;
                merged.bounds.y = new_y;
                merged.bounds.width = new_x2 - new_x;
                merged.bounds.height = new_y2 - new_y;

                // Combine confidence
                merged.confidence = merged.confidence.max(other.confidence);
                merged.line_count += 1;
            }
        }

        result.push(merged);
    }

    result
}

/// Calculate edge density in a region (useful for text detection).
pub fn edge_density(edges: &[bool], width: u32, bounds: &Bounds) -> f32 {
    let mut edge_count = 0;
    let mut total = 0;

    for dy in 0..bounds.height {
        for dx in 0..bounds.width {
            let x = bounds.x + dx;
            let y = bounds.y + dy;
            let idx = (y * width + x) as usize;

            if idx < edges.len() {
                if edges[idx] {
                    edge_count += 1;
                }
                total += 1;
            }
        }
    }

    if total > 0 {
        edge_count as f32 / total as f32
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_weight_from_ratio() {
        assert_eq!(FontWeight::from_stroke_ratio(1.0, 20.0), FontWeight::Light);
        assert_eq!(FontWeight::from_stroke_ratio(2.0, 20.0), FontWeight::Regular);
        assert_eq!(FontWeight::from_stroke_ratio(3.0, 20.0), FontWeight::Medium);
        assert_eq!(FontWeight::from_stroke_ratio(4.0, 20.0), FontWeight::Bold);
    }

    #[test]
    fn test_font_weight_css() {
        assert_eq!(FontWeight::Light.to_css_value(), 300);
        assert_eq!(FontWeight::Regular.to_css_value(), 400);
        assert_eq!(FontWeight::Medium.to_css_value(), 500);
        assert_eq!(FontWeight::Bold.to_css_value(), 700);
    }

    #[test]
    fn test_median_f32() {
        assert!((median_f32(&[1.0, 2.0, 3.0]) - 2.0).abs() < 0.01);
        assert!((median_f32(&[1.0, 2.0, 3.0, 4.0]) - 2.5).abs() < 0.01);
        assert!((median_f32(&[5.0]) - 5.0).abs() < 0.01);
        assert!((median_f32(&[]) - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_text_detect_config_default() {
        let config = TextDetectConfig::default();
        assert_eq!(config.max_stroke_width, 50.0);
        assert_eq!(config.max_stroke_variance, 0.5);
        assert!(config.min_component_area < config.max_component_area);
    }

    #[test]
    fn test_edge_density() {
        let width = 10u32;
        let mut edges = vec![false; 100];

        // Set 25% of pixels as edges
        for i in 0..25 {
            edges[i] = true;
        }

        let bounds = Bounds {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        };

        let density = edge_density(&edges, width, &bounds);
        assert!((density - 0.25).abs() < 0.01);
    }

    #[test]
    fn test_estimate_colors() {
        // Create a simple black text on white background
        let mut pixels = vec![[255u8, 255, 255, 255]; 100];

        // Draw some "text" (black pixels)
        for i in 0..25 {
            pixels[i] = [0, 0, 0, 255];
        }

        let bounds = Bounds {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        };

        let (text_color, bg_color) = estimate_text_colors(&pixels, 10, &bounds);

        // Text should be dark, background should be light
        let text_lum = text_color.r as u32 + text_color.g as u32 + text_color.b as u32;
        let bg_lum = bg_color.r as u32 + bg_color.g as u32 + bg_color.b as u32;

        assert!(text_lum < bg_lum, "Text should be darker than background");
    }
}

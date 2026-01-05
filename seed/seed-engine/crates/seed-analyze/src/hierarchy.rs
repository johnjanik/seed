//! Build parent-child hierarchy from detected regions.
//!
//! Enhanced with layout pattern detection:
//! - Row/column detection
//! - Spacing analysis
//! - Alignment detection
//! - List pattern recognition

use crate::region::{Bounds, FloodFillResult, Region, Fill, RegionType};

/// Layout information for a container region.
#[derive(Debug, Clone, Default)]
pub struct LayoutInfo {
    /// Layout direction.
    pub direction: LayoutDirection,
    /// Horizontal alignment of children.
    pub h_align: Alignment,
    /// Vertical alignment of children.
    pub v_align: Alignment,
    /// Consistent spacing between children (if detected).
    pub spacing: Option<f32>,
    /// Whether children appear to be a list (uniform sizes).
    pub is_list: bool,
    /// Gap between children (CSS gap property).
    pub gap: Option<f32>,
}

/// Layout direction.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum LayoutDirection {
    #[default]
    Column,  // Vertical stack
    Row,     // Horizontal row
    Grid,    // Grid layout
    Absolute, // Positioned absolutely
}

/// Alignment within container.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Alignment {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
}

/// Build a tree of regions based on containment.
/// Larger regions that contain smaller ones become parents.
pub fn build_hierarchy(mut regions: Vec<FloodFillResult>) -> Vec<Region> {
    if regions.is_empty() {
        return Vec::new();
    }

    // Sort by area descending (largest first = potential parents)
    regions.sort_by(|a, b| b.bounds.area().cmp(&a.bounds.area()));

    // Convert to Region objects
    let mut all_regions: Vec<Region> = regions
        .into_iter()
        .map(|r| Region::new(r.bounds, Fill::Solid(r.color)))
        .collect();

    // Build tree: each region's parent is the smallest region that fully contains it
    let mut parent_indices: Vec<Option<usize>> = vec![None; all_regions.len()];

    for i in 1..all_regions.len() {
        let child_bounds = &all_regions[i].bounds;

        // Find smallest containing parent (iterate from largest to smallest)
        for j in 0..i {
            let parent_bounds = &all_regions[j].bounds;
            if parent_bounds.contains_bounds(child_bounds) {
                parent_indices[i] = Some(j);
                // Don't break - continue looking for a smaller parent
            }
        }
    }

    // Build the tree structure
    // Work backwards to attach children to parents
    for i in (1..all_regions.len()).rev() {
        if let Some(parent_idx) = parent_indices[i] {
            let child = all_regions[i].clone();
            all_regions[parent_idx].children.push(child);
        }
    }

    // Return only root-level regions (those without parents)
    all_regions
        .into_iter()
        .enumerate()
        .filter(|(i, _)| parent_indices[*i].is_none())
        .map(|(_, r)| r)
        .collect()
}

/// Adjust child positions to be relative to parent.
pub fn make_positions_relative(regions: &mut [Region]) {
    for region in regions {
        make_children_relative(region);
    }
}

fn make_children_relative(region: &mut Region) {
    let parent_x = region.bounds.x;
    let parent_y = region.bounds.y;

    for child in &mut region.children {
        // Make position relative to parent
        child.bounds.x = child.bounds.x.saturating_sub(parent_x);
        child.bounds.y = child.bounds.y.saturating_sub(parent_y);

        // Recurse
        make_children_relative(child);
    }
}

/// Detect if a region is likely text based on characteristics.
pub fn classify_regions(regions: &mut [Region], pixels: &[[u8; 4]], width: u32) {
    for region in regions {
        classify_region_recursive(region, None, pixels, width);
    }
}

fn classify_region_recursive(
    region: &mut Region,
    parent: Option<&Region>,
    pixels: &[[u8; 4]],
    width: u32,
) {
    region.region_type = classify_region(region, parent, pixels, width);

    // Clone parent bounds for children
    let parent_for_children = Some(region.clone());
    for child in &mut region.children {
        classify_region_recursive(child, parent_for_children.as_ref(), pixels, width);
    }
}

fn classify_region(
    region: &Region,
    parent: Option<&Region>,
    pixels: &[[u8; 4]],
    width: u32,
) -> RegionType {
    let bounds = &region.bounds;
    let area = bounds.area();

    // Very small regions - likely noise or anti-aliasing artifacts
    if area < 30 {
        return RegionType::Frame;
    }

    // Large regions are almost always frames/containers, not text
    // Text is rarely larger than 400x60 pixels
    if bounds.width > 200 && bounds.height > 60 {
        return RegionType::Frame;
    }

    let aspect_ratio = bounds.width as f32 / bounds.height.max(1) as f32;
    let variation = sample_color_variation(pixels, width, bounds);

    // Check if region has gradient (indicates Frame, not Text)
    // Gradients have moderate-high variation but uniform structure
    let has_gradient = variation > 0.08 && bounds.width > 50 && bounds.height > 30;
    if has_gradient {
        return RegionType::Frame;
    }

    // Check if this region is inside a parent container
    if let Some(parent_region) = parent {
        let parent_area = parent_region.bounds.area();
        let area_ratio = area as f32 / parent_area as f32;

        // Region inside a container (< 40% of parent area)
        if area_ratio < 0.4 {
            let edge_density = calculate_edge_density(pixels, width, bounds);

            // Check contrast with parent
            let region_color = get_dominant_color(pixels, width, bounds);
            let parent_color = get_dominant_color(pixels, width, &parent_region.bounds);
            let contrast = color_contrast(&region_color, &parent_color);

            // Icon detection: nearly square, small size, with internal details
            let is_icon_like = aspect_ratio > 0.5
                && aspect_ratio < 2.0
                && bounds.width < 60
                && bounds.height < 60
                && (edge_density > 0.03 || contrast > 50.0);

            // Text detection: must be reasonably small and have text-like properties
            let max_text_width = 350;  // Text lines rarely exceed this
            let max_text_height = 50;  // Single line text height limit

            let is_text_like = bounds.width <= max_text_width
                && bounds.height <= max_text_height
                && (
                    // Wide text (labels, titles) - high aspect ratio, thin
                    (aspect_ratio > 3.0 && bounds.height < 30)
                    // Small region with good contrast (buttons labels, small text)
                    || (area_ratio < 0.08 && contrast > 60.0 && bounds.height < 40)
                    // Narrow text elements
                    || (bounds.width < 50 && bounds.height < 30 && contrast > 40.0)
                )
                && (variation > 0.02 || edge_density > 0.02);

            if is_icon_like || is_text_like {
                return RegionType::Text;
            }
        }
    }

    // Standalone text detection (not inside container)
    // Headers, titles outside of cards - must be thin and wide
    if aspect_ratio > 4.0 && bounds.height < 40 && bounds.height > 8 {
        return RegionType::Text;
    }

    RegionType::Frame
}

/// Calculate edge density (how many pixels have neighbors with different colors).
fn calculate_edge_density(pixels: &[[u8; 4]], width: u32, bounds: &Bounds) -> f32 {
    if bounds.width < 3 || bounds.height < 3 {
        return 0.0;
    }

    let mut edge_count = 0u32;
    let mut total = 0u32;
    let threshold = 30.0; // Color difference threshold

    for dy in 1..bounds.height.saturating_sub(1) {
        for dx in 1..bounds.width.saturating_sub(1) {
            let x = bounds.x + dx;
            let y = bounds.y + dy;
            let idx = (y * width + x) as usize;

            if idx >= pixels.len() {
                continue;
            }

            let center = crate::color::Color::from_pixel(pixels[idx]);

            // Check neighbors
            let neighbors = [
                ((y.saturating_sub(1)) * width + x) as usize,
                ((y + 1) * width + x) as usize,
                (y * width + x.saturating_sub(1)) as usize,
                (y * width + x + 1) as usize,
            ];

            let mut is_edge = false;
            for &nidx in &neighbors {
                if nidx < pixels.len() {
                    let neighbor = crate::color::Color::from_pixel(pixels[nidx]);
                    if center.distance(&neighbor) > threshold {
                        is_edge = true;
                        break;
                    }
                }
            }

            if is_edge {
                edge_count += 1;
            }
            total += 1;
        }
    }

    if total > 0 {
        edge_count as f32 / total as f32
    } else {
        0.0
    }
}

/// Get dominant color in a region.
fn get_dominant_color(pixels: &[[u8; 4]], width: u32, bounds: &Bounds) -> crate::color::Color {
    let mut r_sum = 0u64;
    let mut g_sum = 0u64;
    let mut b_sum = 0u64;
    let mut count = 0u64;

    let step = 2u32.max(bounds.width / 10).max(bounds.height / 10);

    for dy in (0..bounds.height).step_by(step as usize) {
        for dx in (0..bounds.width).step_by(step as usize) {
            let x = bounds.x + dx;
            let y = bounds.y + dy;
            let idx = (y * width + x) as usize;

            if idx < pixels.len() {
                r_sum += pixels[idx][0] as u64;
                g_sum += pixels[idx][1] as u64;
                b_sum += pixels[idx][2] as u64;
                count += 1;
            }
        }
    }

    if count > 0 {
        crate::color::Color::rgb(
            (r_sum / count) as u8,
            (g_sum / count) as u8,
            (b_sum / count) as u8,
        )
    } else {
        crate::color::Color::rgb(128, 128, 128)
    }
}

/// Calculate color contrast between two colors.
fn color_contrast(c1: &crate::color::Color, c2: &crate::color::Color) -> f32 {
    let dr = c1.r as f32 - c2.r as f32;
    let dg = c1.g as f32 - c2.g as f32;
    let db = c1.b as f32 - c2.b as f32;
    (dr * dr + dg * dg + db * db).sqrt()
}

fn sample_color_variation(pixels: &[[u8; 4]], width: u32, bounds: &Bounds) -> f32 {
    if bounds.width < 3 || bounds.height < 3 {
        return 0.0;
    }

    let mut colors = Vec::new();
    let step = 3u32;

    for dy in (0..bounds.height).step_by(step as usize) {
        for dx in (0..bounds.width).step_by(step as usize) {
            let x = bounds.x + dx;
            let y = bounds.y + dy;
            let idx = (y * width + x) as usize;
            if idx < pixels.len() {
                colors.push(crate::color::Color::from_pixel(pixels[idx]));
            }
        }
    }

    if colors.len() < 2 {
        return 0.0;
    }

    // Calculate variance of colors
    let mean_r: f32 = colors.iter().map(|c| c.r as f32).sum::<f32>() / colors.len() as f32;
    let mean_g: f32 = colors.iter().map(|c| c.g as f32).sum::<f32>() / colors.len() as f32;
    let mean_b: f32 = colors.iter().map(|c| c.b as f32).sum::<f32>() / colors.len() as f32;

    let variance: f32 = colors
        .iter()
        .map(|c| {
            let dr = c.r as f32 - mean_r;
            let dg = c.g as f32 - mean_g;
            let db = c.b as f32 - mean_b;
            dr * dr + dg * dg + db * db
        })
        .sum::<f32>()
        / colors.len() as f32;

    variance.sqrt() / 255.0
}

/// Analyze layout of children within all regions.
pub fn analyze_layouts(regions: &mut [Region]) {
    for region in regions {
        if !region.children.is_empty() {
            region.layout = analyze_children_layout(&region.children);
        }
        analyze_layouts(&mut region.children);
    }
}

/// Analyze the layout pattern of children.
pub fn analyze_children_layout(children: &[Region]) -> Option<LayoutInfo> {
    if children.len() < 2 {
        return None;
    }

    // Collect child bounds
    let bounds: Vec<&Bounds> = children.iter().map(|c| &c.bounds).collect();

    // Detect direction by checking alignment
    let direction = detect_layout_direction(&bounds);
    let h_align = detect_horizontal_alignment(&bounds);
    let v_align = detect_vertical_alignment(&bounds);
    let (spacing, gap) = detect_spacing(&bounds, direction);
    let is_list = detect_list_pattern(&bounds);

    Some(LayoutInfo {
        direction,
        h_align,
        v_align,
        spacing,
        is_list,
        gap,
    })
}

/// Detect if children are arranged in a row, column, or grid.
fn detect_layout_direction(bounds: &[&Bounds]) -> LayoutDirection {
    if bounds.len() < 2 {
        return LayoutDirection::Absolute;
    }

    // Check vertical arrangement (column)
    let vertical_overlap_count = count_vertical_overlaps(bounds);
    let horizontal_overlap_count = count_horizontal_overlaps(bounds);

    // If children mostly stack vertically (little horizontal overlap)
    if horizontal_overlap_count > bounds.len() / 2 {
        return LayoutDirection::Row;
    }

    // If children mostly stack horizontally (little vertical overlap)
    if vertical_overlap_count > bounds.len() / 2 {
        return LayoutDirection::Column;
    }

    // Check for grid pattern
    if is_grid_layout(bounds) {
        return LayoutDirection::Grid;
    }

    LayoutDirection::Absolute
}

/// Count how many adjacent pairs have horizontal overlap.
fn count_horizontal_overlaps(bounds: &[&Bounds]) -> usize {
    let mut sorted: Vec<_> = bounds.iter().collect();
    sorted.sort_by_key(|b| b.y);

    sorted
        .windows(2)
        .filter(|pair| {
            let b1 = pair[0];
            let b2 = pair[1];
            // Horizontal overlap exists
            b1.x < b2.x + b2.width && b1.x + b1.width > b2.x
        })
        .count()
}

/// Count how many adjacent pairs have vertical overlap.
fn count_vertical_overlaps(bounds: &[&Bounds]) -> usize {
    let mut sorted: Vec<_> = bounds.iter().collect();
    sorted.sort_by_key(|b| b.x);

    sorted
        .windows(2)
        .filter(|pair| {
            let b1 = pair[0];
            let b2 = pair[1];
            // Vertical overlap exists
            b1.y < b2.y + b2.height && b1.y + b1.height > b2.y
        })
        .count()
}

/// Check if bounds form a grid pattern.
fn is_grid_layout(bounds: &[&Bounds]) -> bool {
    if bounds.len() < 4 {
        return false;
    }

    // Extract unique x positions (left edges)
    let mut x_positions: Vec<u32> = bounds.iter().map(|b| b.x).collect();
    x_positions.sort();
    x_positions.dedup();

    // Extract unique y positions (top edges)
    let mut y_positions: Vec<u32> = bounds.iter().map(|b| b.y).collect();
    y_positions.sort();
    y_positions.dedup();

    // Grid has at least 2 rows and 2 columns
    x_positions.len() >= 2 && y_positions.len() >= 2 && bounds.len() >= 4
}

/// Detect horizontal alignment of children.
fn detect_horizontal_alignment(bounds: &[&Bounds]) -> Alignment {
    if bounds.is_empty() {
        return Alignment::Start;
    }

    // Find the leftmost and rightmost edges
    let min_left = bounds.iter().map(|b| b.x).min().unwrap_or(0);
    let max_right = bounds.iter().map(|b| b.x + b.width).max().unwrap_or(0);
    let container_width = max_right - min_left;

    if container_width == 0 {
        return Alignment::Start;
    }

    // Check if left edges are aligned
    let left_variance = calculate_position_variance(bounds.iter().map(|b| b.x));

    // Check if centers are aligned
    let center_variance = calculate_position_variance(bounds.iter().map(|b| b.x + b.width / 2));

    // Check if right edges are aligned
    let right_variance = calculate_position_variance(bounds.iter().map(|b| b.x + b.width));

    let min_variance = left_variance.min(center_variance).min(right_variance);
    let tolerance = 5.0;

    if left_variance <= tolerance && left_variance == min_variance {
        Alignment::Start
    } else if center_variance <= tolerance && center_variance == min_variance {
        Alignment::Center
    } else if right_variance <= tolerance && right_variance == min_variance {
        Alignment::End
    } else {
        Alignment::Start // Default
    }
}

/// Detect vertical alignment of children.
fn detect_vertical_alignment(bounds: &[&Bounds]) -> Alignment {
    if bounds.is_empty() {
        return Alignment::Start;
    }

    // Check if top edges are aligned
    let top_variance = calculate_position_variance(bounds.iter().map(|b| b.y));

    // Check if centers are aligned
    let center_variance = calculate_position_variance(bounds.iter().map(|b| b.y + b.height / 2));

    // Check if bottom edges are aligned
    let bottom_variance = calculate_position_variance(bounds.iter().map(|b| b.y + b.height));

    let min_variance = top_variance.min(center_variance).min(bottom_variance);
    let tolerance = 5.0;

    if top_variance <= tolerance && top_variance == min_variance {
        Alignment::Start
    } else if center_variance <= tolerance && center_variance == min_variance {
        Alignment::Center
    } else if bottom_variance <= tolerance && bottom_variance == min_variance {
        Alignment::End
    } else {
        Alignment::Start
    }
}

/// Calculate variance of positions.
fn calculate_position_variance(positions: impl Iterator<Item = u32>) -> f32 {
    let values: Vec<f32> = positions.map(|p| p as f32).collect();

    if values.len() < 2 {
        return 0.0;
    }

    let mean: f32 = values.iter().sum::<f32>() / values.len() as f32;
    let variance: f32 = values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / values.len() as f32;
    variance.sqrt()
}

/// Detect consistent spacing between children.
fn detect_spacing(bounds: &[&Bounds], direction: LayoutDirection) -> (Option<f32>, Option<f32>) {
    if bounds.len() < 2 {
        return (None, None);
    }

    let gaps: Vec<f32> = match direction {
        LayoutDirection::Column => {
            // Sort by y position
            let mut sorted: Vec<_> = bounds.iter().collect();
            sorted.sort_by_key(|b| b.y);

            sorted
                .windows(2)
                .map(|pair| {
                    let top = pair[0];
                    let bottom = pair[1];
                    (bottom.y as i32 - (top.y + top.height) as i32) as f32
                })
                .filter(|&gap| gap > 0.0)
                .collect()
        }
        LayoutDirection::Row => {
            // Sort by x position
            let mut sorted: Vec<_> = bounds.iter().collect();
            sorted.sort_by_key(|b| b.x);

            sorted
                .windows(2)
                .map(|pair| {
                    let left = pair[0];
                    let right = pair[1];
                    (right.x as i32 - (left.x + left.width) as i32) as f32
                })
                .filter(|&gap| gap > 0.0)
                .collect()
        }
        _ => return (None, None),
    };

    if gaps.is_empty() {
        return (None, None);
    }

    // Check if gaps are consistent
    let avg_gap: f32 = gaps.iter().sum::<f32>() / gaps.len() as f32;
    let variance: f32 = gaps.iter().map(|g| (g - avg_gap).powi(2)).sum::<f32>() / gaps.len() as f32;

    if variance.sqrt() < avg_gap * 0.2 {
        // Gaps are consistent (within 20% variance)
        (Some(avg_gap), Some(avg_gap))
    } else {
        (None, Some(avg_gap))
    }
}

/// Detect if children form a list (uniform sizes).
fn detect_list_pattern(bounds: &[&Bounds]) -> bool {
    if bounds.len() < 3 {
        return false;
    }

    // Check if sizes are uniform
    let widths: Vec<u32> = bounds.iter().map(|b| b.width).collect();
    let heights: Vec<u32> = bounds.iter().map(|b| b.height).collect();

    let width_variance = calculate_size_variance(&widths);
    let height_variance = calculate_size_variance(&heights);

    // List items have similar sizes
    let avg_width = widths.iter().sum::<u32>() as f32 / widths.len() as f32;
    let avg_height = heights.iter().sum::<u32>() as f32 / heights.len() as f32;

    width_variance < avg_width * 0.2 && height_variance < avg_height * 0.2
}

/// Calculate variance of sizes.
fn calculate_size_variance(sizes: &[u32]) -> f32 {
    if sizes.len() < 2 {
        return 0.0;
    }

    let mean: f32 = sizes.iter().sum::<u32>() as f32 / sizes.len() as f32;
    let variance: f32 = sizes.iter().map(|s| (*s as f32 - mean).powi(2)).sum::<f32>() / sizes.len() as f32;
    variance.sqrt()
}

/// Group adjacent text regions into multi-line text blocks.
///
/// This merges text regions that are:
/// - Vertically adjacent (small gap between them)
/// - Horizontally aligned (similar x position or overlapping)
/// - Similar in character (both are Text type)
pub fn group_text_regions(regions: &mut [Region]) {
    for region in regions.iter_mut() {
        group_text_children(region);
        group_text_regions(&mut region.children);
    }
}

/// Group text children within a single parent region.
fn group_text_children(parent: &mut Region) {
    if parent.children.len() < 2 {
        return;
    }

    // Collect text regions and their indices
    let text_indices: Vec<usize> = parent
        .children
        .iter()
        .enumerate()
        .filter(|(_, c)| c.region_type == RegionType::Text)
        .map(|(i, _)| i)
        .collect();

    if text_indices.len() < 2 {
        return;
    }

    // Find groups of text regions to merge
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut used = vec![false; parent.children.len()];

    for &i in &text_indices {
        if used[i] {
            continue;
        }

        let mut group = vec![i];
        used[i] = true;

        let base = &parent.children[i].bounds;
        let base_right = base.x + base.width;

        // Find other text regions that should be grouped with this one
        for &j in &text_indices {
            if used[j] || i == j {
                continue;
            }

            let other = &parent.children[j].bounds;
            let other_right = other.x + other.width;

            // Check vertical adjacency (gap less than 1.5x the height)
            let vertical_gap = if other.y > base.y + base.height {
                other.y - (base.y + base.height)
            } else if base.y > other.y + other.height {
                base.y - (other.y + other.height)
            } else {
                0 // Overlapping vertically
            };

            let max_gap = (base.height.max(other.height) as f32 * 1.5) as u32;

            // Check horizontal alignment (overlapping x ranges or similar left edge)
            let h_overlap = base.x < other_right && base_right > other.x;
            let similar_left = (base.x as i32 - other.x as i32).abs() < 20;

            if vertical_gap < max_gap && (h_overlap || similar_left) {
                group.push(j);
                used[j] = true;
            }
        }

        if group.len() > 1 {
            groups.push(group);
        }
    }

    // Merge groups into single text regions
    for group in groups.iter().rev() {
        if group.len() < 2 {
            continue;
        }

        // Calculate merged bounds
        let mut min_x = u32::MAX;
        let mut min_y = u32::MAX;
        let mut max_x = 0u32;
        let mut max_y = 0u32;

        for &idx in group {
            let b = &parent.children[idx].bounds;
            min_x = min_x.min(b.x);
            min_y = min_y.min(b.y);
            max_x = max_x.max(b.x + b.width);
            max_y = max_y.max(b.y + b.height);
        }

        // Get the first region's properties as base
        let first_idx = group[0];
        let _line_count = group.len() as u32;

        // Update the first region to be the merged region
        parent.children[first_idx].bounds = Bounds::new(
            min_x,
            min_y,
            max_x - min_x,
            max_y - min_y,
        );

        // Update layout info to indicate multi-line
        if parent.children[first_idx].layout.is_none() {
            parent.children[first_idx].layout = Some(LayoutInfo {
                direction: LayoutDirection::Column,
                ..Default::default()
            });
        }

        // Store line count in a comment or we could add a field
        // For now, the height increase indicates multi-line

        // Remove the other regions (in reverse order to maintain indices)
        let mut to_remove: Vec<usize> = group[1..].to_vec();
        to_remove.sort_by(|a, b| b.cmp(a)); // Sort descending

        for idx in to_remove {
            parent.children.remove(idx);
        }
    }
}

/// Estimate line count from region height and typical line height.
pub fn estimate_line_count(height: u32, base_font_size: f32) -> u32 {
    let line_height = base_font_size * 1.4; // Typical line height
    ((height as f32 / line_height).round() as u32).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;

    #[test]
    fn test_build_hierarchy_single() {
        let regions = vec![FloodFillResult {
            bounds: Bounds::new(0, 0, 100, 100),
            color: Color::rgb(255, 255, 255),
            pixel_count: 10000,
        }];

        let tree = build_hierarchy(regions);
        assert_eq!(tree.len(), 1);
        assert!(tree[0].children.is_empty());
    }

    #[test]
    fn test_build_hierarchy_nested() {
        let regions = vec![
            FloodFillResult {
                bounds: Bounds::new(0, 0, 100, 100),
                color: Color::rgb(255, 255, 255),
                pixel_count: 10000,
            },
            FloodFillResult {
                bounds: Bounds::new(10, 10, 50, 50),
                color: Color::rgb(200, 200, 200),
                pixel_count: 2500,
            },
        ];

        let tree = build_hierarchy(regions);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].children.len(), 1);
    }

    #[test]
    fn test_build_hierarchy_siblings() {
        let regions = vec![
            FloodFillResult {
                bounds: Bounds::new(0, 0, 100, 100),
                color: Color::rgb(255, 255, 255),
                pixel_count: 10000,
            },
            FloodFillResult {
                bounds: Bounds::new(10, 10, 30, 30),
                color: Color::rgb(200, 200, 200),
                pixel_count: 900,
            },
            FloodFillResult {
                bounds: Bounds::new(50, 50, 30, 30),
                color: Color::rgb(150, 150, 150),
                pixel_count: 900,
            },
        ];

        let tree = build_hierarchy(regions);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].children.len(), 2);
    }
}

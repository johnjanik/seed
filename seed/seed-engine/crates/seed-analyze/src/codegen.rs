//! Generate Seed markup from analyzed regions.
//!
//! Enhanced code generation includes:
//! - Layout properties (direction, gap, alignment)
//! - Circle detection (elements with 50% corner radius)
//! - Line/divider elements
//! - Text with font properties

use crate::color::Color;
use crate::hierarchy::{Alignment, LayoutDirection, LayoutInfo};
use crate::region::{Fill, Region, RegionType};

/// Generate Seed source code from a tree of regions.
pub fn generate_seed(regions: &[Region]) -> String {
    let mut output = String::new();

    for region in regions {
        generate_region(&mut output, region, 0);
    }

    output
}

fn generate_region(output: &mut String, region: &Region, depth: usize) {
    let indent = "  ".repeat(depth);

    // Detect special element types
    let is_circle = is_circle_element(region);
    let is_line = is_line_element(region);

    // Element type
    let element_type = if is_line {
        "Frame" // Lines are thin frames
    } else {
        match region.region_type {
            RegionType::Frame => "Frame",
            RegionType::Text => "Text",
            RegionType::Unknown => "Frame",
        }
    };

    output.push_str(&format!("{}{}:\n", indent, element_type));

    let prop_indent = "  ".repeat(depth + 1);

    // Dimensions
    output.push_str(&format!("{}width: {}px\n", prop_indent, region.bounds.width));
    output.push_str(&format!("{}height: {}px\n", prop_indent, region.bounds.height));

    // Position (only if not at origin)
    if region.bounds.x > 0 {
        output.push_str(&format!("{}x: {}px\n", prop_indent, region.bounds.x));
    }
    if region.bounds.y > 0 {
        output.push_str(&format!("{}y: {}px\n", prop_indent, region.bounds.y));
    }

    // Fill
    output.push_str(&format_fill(&prop_indent, &region.fill));

    // Corner radius - special handling for circles
    if is_circle {
        output.push_str(&format!("{}corner-radius: 50%\n", prop_indent));
    } else if region.corner_radius > 0.5 {
        output.push_str(&format!(
            "{}corner-radius: {}px\n",
            prop_indent,
            region.corner_radius.round() as u32
        ));
    }

    // Stroke
    if let Some(stroke) = &region.stroke {
        output.push_str(&format!(
            "{}stroke: {}px {}\n",
            prop_indent,
            stroke.width.round() as u32,
            stroke.color.to_hex()
        ));
    }

    // Shadow
    if let Some(shadow) = &region.shadow {
        output.push_str(&format!(
            "{}shadow: {}px {}px {}px {}\n",
            prop_indent,
            shadow.offset_x.round() as i32,
            shadow.offset_y.round() as i32,
            shadow.blur.round() as u32,
            shadow.color.to_hex()
        ));
    }

    // Layout properties (if detected)
    if let Some(layout) = &region.layout {
        output.push_str(&format_layout(&prop_indent, layout));
    }

    // Text content and font properties
    if region.region_type == RegionType::Text {
        output.push_str(&format!("{}content: \"[detected]\"\n", prop_indent));
        // Font size estimation from height
        let font_size = (region.bounds.height as f32 * 0.8).round() as u32;
        if font_size > 0 {
            output.push_str(&format!("{}font-size: {}px\n", prop_indent, font_size));
        }
    }

    // Empty line before children
    if !region.children.is_empty() {
        output.push('\n');
    }

    // Children
    for child in &region.children {
        generate_region(output, child, depth + 1);
    }
}

/// Check if a region appears to be a circle (width ≈ height and large corner radius).
fn is_circle_element(region: &Region) -> bool {
    let w = region.bounds.width as f32;
    let h = region.bounds.height as f32;
    let aspect_ratio = w / h.max(1.0);

    // Circle if roughly square and corner radius is at least half of size
    aspect_ratio > 0.9 && aspect_ratio < 1.1 && region.corner_radius >= w.min(h) * 0.4
}

/// Check if a region appears to be a line/divider.
fn is_line_element(region: &Region) -> bool {
    let w = region.bounds.width;
    let h = region.bounds.height;

    // Horizontal line: very wide and thin
    if w > h * 10 && h <= 5 {
        return true;
    }

    // Vertical line: very tall and thin
    if h > w * 10 && w <= 5 {
        return true;
    }

    false
}

/// Format layout properties for output.
fn format_layout(indent: &str, layout: &LayoutInfo) -> String {
    let mut output = String::new();

    // Layout direction
    match layout.direction {
        LayoutDirection::Row => {
            output.push_str(&format!("{}layout: row\n", indent));
        }
        LayoutDirection::Column => {
            output.push_str(&format!("{}layout: column\n", indent));
        }
        LayoutDirection::Grid => {
            output.push_str(&format!("{}layout: grid\n", indent));
        }
        LayoutDirection::Absolute => {
            // Don't output layout for absolute positioning
        }
    }

    // Gap (spacing between children)
    if let Some(gap) = layout.gap {
        if gap > 0.0 {
            output.push_str(&format!("{}gap: {}px\n", indent, gap.round() as u32));
        }
    }

    // Horizontal alignment
    match layout.h_align {
        Alignment::Center => {
            output.push_str(&format!("{}align-items: center\n", indent));
        }
        Alignment::End => {
            output.push_str(&format!("{}align-items: end\n", indent));
        }
        Alignment::SpaceBetween => {
            output.push_str(&format!("{}justify-content: space-between\n", indent));
        }
        _ => {}
    }

    // Vertical alignment
    match layout.v_align {
        Alignment::Center => {
            output.push_str(&format!("{}justify-content: center\n", indent));
        }
        Alignment::End => {
            output.push_str(&format!("{}justify-content: end\n", indent));
        }
        _ => {}
    }

    output
}

fn format_fill(indent: &str, fill: &Fill) -> String {
    match fill {
        Fill::Solid(color) => {
            format!("{}fill: {}\n", indent, color.to_hex())
        }
        Fill::LinearGradient { angle, stops } => {
            let stops_str = stops
                .iter()
                .map(|(offset, color)| format!("{} {}%", color.to_hex(), (offset * 100.0).round() as u32))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{}fill: linear-gradient({}deg, {})\n",
                indent,
                angle.round() as i32,
                stops_str
            )
        }
        Fill::RadialGradient { stops } => {
            let stops_str = stops
                .iter()
                .map(|(offset, color)| format!("{} {}%", color.to_hex(), (offset * 100.0).round() as u32))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}fill: radial-gradient({})\n", indent, stops_str)
        }
    }
}

/// Format a color to hex string.
impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::Bounds;

    #[test]
    fn test_generate_simple_frame() {
        let region = Region {
            bounds: Bounds::new(0, 0, 100, 100),
            fill: Fill::Solid(Color::rgb(255, 255, 255)),
            corner_radius: 0.0,
            stroke: None,
            shadow: None,
            region_type: RegionType::Frame,
            children: Vec::new(),
            layout: None,
        };

        let code = generate_seed(&[region]);
        assert!(code.contains("Frame:"));
        assert!(code.contains("width: 100px"));
        assert!(code.contains("height: 100px"));
        assert!(code.contains("fill: #ffffff"));
    }

    #[test]
    fn test_generate_with_corner_radius() {
        let region = Region {
            bounds: Bounds::new(0, 0, 100, 100),
            fill: Fill::Solid(Color::rgb(255, 255, 255)),
            corner_radius: 12.0,
            stroke: None,
            shadow: None,
            region_type: RegionType::Frame,
            children: Vec::new(),
            layout: None,
        };

        let code = generate_seed(&[region]);
        assert!(code.contains("corner-radius: 12px"));
    }

    #[test]
    fn test_generate_gradient() {
        let region = Region {
            bounds: Bounds::new(0, 0, 100, 100),
            fill: Fill::LinearGradient {
                angle: 90.0,
                stops: vec![
                    (0.0, Color::rgb(102, 126, 234)),
                    (1.0, Color::rgb(118, 75, 162)),
                ],
            },
            corner_radius: 0.0,
            stroke: None,
            shadow: None,
            region_type: RegionType::Frame,
            children: Vec::new(),
            layout: None,
        };

        let code = generate_seed(&[region]);
        assert!(code.contains("linear-gradient(90deg"));
    }

    #[test]
    fn test_generate_nested() {
        let child = Region {
            bounds: Bounds::new(10, 10, 50, 50),
            fill: Fill::Solid(Color::rgb(200, 200, 200)),
            corner_radius: 0.0,
            stroke: None,
            shadow: None,
            region_type: RegionType::Frame,
            children: Vec::new(),
            layout: None,
        };

        let parent = Region {
            bounds: Bounds::new(0, 0, 100, 100),
            fill: Fill::Solid(Color::rgb(255, 255, 255)),
            corner_radius: 0.0,
            stroke: None,
            shadow: None,
            region_type: RegionType::Frame,
            children: vec![child],
            layout: None,
        };

        let code = generate_seed(&[parent]);

        // Check proper nesting (child should be indented)
        let lines: Vec<&str> = code.lines().collect();
        let frame_indices: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.contains("Frame:"))
            .map(|(i, _)| i)
            .collect();

        assert_eq!(frame_indices.len(), 2);

        // Second Frame should be more indented than first
        let first_indent = lines[frame_indices[0]].len() - lines[frame_indices[0]].trim_start().len();
        let second_indent = lines[frame_indices[1]].len() - lines[frame_indices[1]].trim_start().len();
        assert!(second_indent > first_indent);
    }
}
